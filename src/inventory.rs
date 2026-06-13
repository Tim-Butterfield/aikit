//! `aikit inventory repo` — a mechanical inventory of repository files.
//!
//! Walks the repository with gitignore-aware traversal, always excluding `.git/`
//! and a fixed set of build/dependency/output directories (matched by directory
//! component, never by bare substring), hashes each included file, and writes a
//! deterministic JSON + text inventory under the local output directory.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use sha2::{Digest, Sha256};
use time::format_description::FormatItem;
use time::macros::format_description;
use time::OffsetDateTime;

use crate::cli::InventoryRepoArgs;
use crate::errors::AikitError;
use crate::formats::{
    InventoryCounts, InventoryFile, RepoInventory, KIND_REPO_INVENTORY, SCHEMA_VERSION,
};
use crate::{output, repo};

const TS_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
const ID_FORMAT: &[FormatItem<'static>] =
    format_description!("[year][month][day]-[hour][minute][second]");

/// Directories excluded by directory-component name (matches any ancestor
/// directory of that name; never a bare substring and never a file of that name).
const DIR_NAME_EXCLUDES: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    ".venv",
    "venv",
];

/// Multi-component directory subtrees excluded by path prefix (aikit's own output
/// areas). Matched directory-only: an exact match is excluded only when it is a
/// directory, but descendants are always excluded.
const PREFIX_EXCLUDES: &[&str] = &[".scratch/work/outputs/aikit", ".aikit/outputs"];

pub fn repo(args: InventoryRepoArgs) -> Result<(), AikitError> {
    let root = repo::detect_root()?;
    let head = repo::git_head(&root);

    // Resolve the output root up front so we can exclude it from the walk when it
    // lives inside the repository (e.g. a `--output` directory under the repo). A
    // relative `--output` is resolved against the repo root, so the path used for
    // writing matches the repo-relative paths the walker produces.
    let selected = output::select_output_root(&root, args.output.as_deref());
    let out_root = if selected.is_absolute() {
        selected
    } else {
        root.join(selected)
    };
    // Exclude the exact subdirectory this command writes into (`<out-root>/inventory`).
    // Using the write subdir keeps the exclusion non-empty even when the output root
    // is the repo root itself (e.g. `--output .`).
    let out_rel = repo_relative(&root, &output::inventory_dir(&out_root));

    let (mut rel_paths, walk_errors) =
        collect_files(&root, args.include_ignored, out_rel.as_deref());
    rel_paths.sort();

    let discovered = rel_paths.len();
    let truncated = matches!(args.max_files, Some(n) if discovered > n);
    if let Some(n) = args.max_files {
        rel_paths.truncate(n);
    }

    let mut files: Vec<InventoryFile> = Vec::with_capacity(rel_paths.len());
    let mut total_bytes: u64 = 0;
    let mut read_errors = 0usize;
    for rel in &rel_paths {
        match build_entry(&root, rel) {
            Some(entry) => {
                total_bytes += entry.size_bytes;
                files.push(entry);
            }
            // Could not stat/hash the file (permissions, race). Skip it and note it
            // rather than emitting an entry with an empty digest.
            None => read_errors += 1,
        }
    }

    let mut notes: Vec<String> = Vec::new();
    if let Some(n) = args.max_files {
        if truncated {
            notes.push(format!(
                "Listing limited to the first {n} of {discovered} discovered files by --max-files."
            ));
        }
    }
    let omitted = walk_errors + read_errors;
    if omitted > 0 {
        notes.push(format!(
            "{omitted} path(s) omitted due to traversal or read errors."
        ));
    }

    let counts = InventoryCounts {
        files: files.len(),
        bytes: total_bytes,
        truncated,
        max_files: args.max_files,
        total_discovered: if truncated { Some(discovered) } else { None },
    };

    let now = OffsetDateTime::now_utc();
    let inventory_id = format!(
        "{}-{}",
        now.format(ID_FORMAT).unwrap_or_default(),
        short_head(&head)
    );

    let inventory = RepoInventory {
        schema_version: SCHEMA_VERSION,
        kind: KIND_REPO_INVENTORY.to_string(),
        inventory_id: inventory_id.clone(),
        repo_root: root.display().to_string(),
        git_head: head,
        generated_at: now.format(TS_FORMAT).unwrap_or_default(),
        files,
        counts,
        notes: if notes.is_empty() { None } else { Some(notes) },
    };

    // Write JSON + text inventory under <output-root>/inventory/<inventory-id>/.
    let dir = output::inventory_dir(&out_root).join(&inventory_id);
    fs::create_dir_all(&dir).map_err(|e| {
        AikitError::other(format!(
            "failed to create output dir {}: {e}",
            dir.display()
        ))
    })?;

    let json = serde_json::to_string_pretty(&inventory)
        .map_err(|e| AikitError::other(format!("failed to serialize inventory: {e}")))?;
    let json_path = dir.join("inventory.json");
    write_with_newline(&json_path, &json)?;

    let txt_path = dir.join("inventory.txt");
    fs::write(&txt_path, render_text(&inventory))
        .map_err(|e| AikitError::other(format!("failed to write {}: {e}", txt_path.display())))?;

    if args.json {
        println!("{json}");
    } else {
        println!("Repository inventory written:");
        println!("  {}", display_relative(&root, &json_path));
        println!("  {}", display_relative(&root, &txt_path));
        println!(
            "  {} file(s), {} byte(s){}",
            inventory.counts.files,
            inventory.counts.bytes,
            if inventory.counts.truncated {
                " (truncated)"
            } else {
                ""
            }
        );
        if let Some(notes) = &inventory.notes {
            for note in notes {
                println!("note: {note}");
            }
        }
    }
    Ok(())
}

/// Collect repo-relative paths (regular files and symlinks) via gitignore-aware
/// traversal. `.git/`, the configured build/dependency/output directories, and the
/// run's own output directory (when inside the repo) are always pruned. Returns the
/// paths plus a count of traversal errors that were skipped.
fn collect_files(
    root: &Path,
    include_ignored: bool,
    extra_exclude: Option<&str>,
) -> (Vec<PathBuf>, usize) {
    let root_owned = root.to_path_buf();
    let extra = extra_exclude.map(|s| s.to_string());
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false) // include dotfiles (e.g. .gitignore); .git/ is pruned below
        .git_ignore(!include_ignored)
        .git_global(!include_ignored)
        .git_exclude(!include_ignored)
        .ignore(!include_ignored)
        .parents(!include_ignored);
    let extra_for_filter = extra.clone();
    builder.filter_entry(move |entry| {
        let rel = match entry.path().strip_prefix(&root_owned) {
            Ok(p) => p.to_string_lossy().replace('\\', "/"),
            Err(_) => return true,
        };
        if rel.is_empty() {
            return true;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        !is_excluded(&rel, is_dir, extra_for_filter.as_deref())
    });

    let mut out = Vec::new();
    let mut errors = 0usize;
    for result in builder.build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        // Include regular files and symlinks (symlinks are recorded, not followed).
        let include = entry
            .file_type()
            .map(|t| t.is_file() || t.is_symlink())
            .unwrap_or(false);
        if include {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                out.push(rel.to_path_buf());
            }
        }
    }
    (out, errors)
}

/// Build one inventory entry from a repo-relative path, hashing the real file
/// bytes (or, for a symlink, the link target). Returns `None` if the path cannot
/// be stat'd or read, so the caller can skip-and-note rather than emit a bad entry.
fn build_entry(root: &Path, rel: &Path) -> Option<InventoryFile> {
    let abs = root.join(rel);
    let meta = fs::symlink_metadata(&abs).ok()?;
    let (sha256, kind_hint) = if meta.file_type().is_symlink() {
        let target = fs::read_link(&abs).ok()?;
        // Hash the link target's native bytes (not followed; no lossy conversion).
        (sha256_bytes(&symlink_target_bytes(&target)), "symlink")
    } else {
        (compute_sha256(&abs)?, kind_hint(rel))
    };
    Some(InventoryFile {
        path: rel.to_string_lossy().replace('\\', "/"),
        size_bytes: meta.len(),
        sha256,
        kind_hint: kind_hint.to_string(),
    })
}

/// Directory-only exclusion. A path is excluded when it lies under one of the
/// prefix subtrees (or the run's output dir), or when any of its *directory*
/// components matches a configured directory name. The final filename component is
/// not treated as a directory, so a regular file literally named `target` is never
/// excluded, and an exact-path prefix match is excluded only for a directory.
fn is_excluded(rel: &str, is_dir: bool, extra: Option<&str>) -> bool {
    for p in PREFIX_EXCLUDES.iter().copied().chain(extra) {
        if rel.starts_with(&format!("{p}/")) || (is_dir && rel == p) {
            return true;
        }
    }
    let comps: Vec<&str> = rel.split('/').collect();
    let dir_count = if is_dir {
        comps.len()
    } else {
        comps.len().saturating_sub(1)
    };
    comps[..dir_count]
        .iter()
        .any(|c| DIR_NAME_EXCLUDES.contains(c))
}

/// The repo-relative, forward-slash form of `path` when it is inside `root`.
fn repo_relative(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .filter(|s| !s.is_empty())
}

fn kind_hint(rel: &Path) -> &'static str {
    let ext = rel
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("md" | "markdown") => "markdown",
        Some("rs") => "rust",
        Some("toml") => "toml",
        Some("json") => "json",
        Some("txt") => "text",
        _ => "unknown",
    }
}

fn render_text(inv: &RepoInventory) -> String {
    let mut s = String::new();
    s.push_str("# aikit Repository Inventory\n\n");
    s.push_str(&format!("Repo: {}\n", inv.repo_root));
    s.push_str(&format!("HEAD: {}\n", inv.git_head));
    s.push_str(&format!("Generated: {}\n", inv.generated_at));
    s.push_str(&format!(
        "Files: {}  Bytes: {}{}\n",
        inv.counts.files,
        inv.counts.bytes,
        if inv.counts.truncated {
            " (truncated)"
        } else {
            ""
        }
    ));
    if let Some(notes) = &inv.notes {
        for note in notes {
            s.push_str(&format!("Note: {note}\n"));
        }
    }
    s.push_str("\nsha256  size  kind  path\n");
    for f in &inv.files {
        s.push_str(&format!(
            "{}  {}  {}  {}\n",
            f.sha256, f.size_bytes, f.kind_hint, f.path
        ));
    }
    s
}

/// Stream a file through SHA-256 in fixed-size chunks (bounded memory).
fn compute_sha256(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// The native byte representation of a symlink target (no lossy conversion).
#[cfg(unix)]
fn symlink_target_bytes(target: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    target.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn symlink_target_bytes(target: &Path) -> Vec<u8> {
    target.to_string_lossy().as_bytes().to_vec()
}

/// Write `body` plus a trailing newline without allocating the body twice.
fn write_with_newline(path: &Path, body: &str) -> Result<(), AikitError> {
    let mut file = File::create(path)
        .map_err(|e| AikitError::other(format!("failed to write {}: {e}", path.display())))?;
    file.write_all(body.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .map_err(|e| AikitError::other(format!("failed to write {}: {e}", path.display())))
}

fn short_head(head: &str) -> String {
    if head.is_empty() {
        "nohead".to_string()
    } else if head.len() >= 7 {
        head[..7].to_string()
    } else {
        head.to_string()
    }
}

fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}
