//! `aikit batch start` and `aikit batch changed` implementations.
//!
//! Batch 1 keeps change detection deliberately simple (per the plan): Git status
//! is the primary signal for tracked files; an optional mtime heuristic covers
//! untracked files. Perfect historical reconstruction is a non-goal.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};
use time::format_description::FormatItem;
use time::macros::format_description;
use time::{OffsetDateTime, PrimitiveDateTime};

use crate::cli::{ChangedArgs, StartArgs};
use crate::errors::{blocked, AikitError};
use crate::formats::{
    AnchorRef, BatchAnchor, ChangedFile, ChangedOutput, Counts, KIND_BATCH_ANCHOR,
    KIND_BATCH_CHANGED, SCHEMA_VERSION,
};
use crate::{output, repo};

/// Timestamp format used for `created_at` / `filesystem_anchor_time` / `generated_at`.
const TS_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
/// Compact format used inside anchor ids.
const ID_FORMAT: &[FormatItem<'static>] =
    format_description!("[year][month][day]-[hour][minute][second]");

/// aikit's own output directories, excluded from changed-file results by default.
/// Because change detection uses `--untracked-files=all`, untracked files are
/// listed individually and can be matched against these precise prefixes.
const DEFAULT_EXCLUDES: &[&str] = &[".git/", ".aikit/outputs/", ".scratch/work/outputs/aikit/"];

const MTIME_NOTE: &str =
    "Untracked results use a best-effort filesystem mtime heuristic and may be imprecise.";

pub fn start(args: StartArgs) -> Result<(), AikitError> {
    let root = repo::detect_root()?;
    let head = repo::git_head(&root);
    let branch = repo::git_branch(&root);
    let status = repo::git_status_porcelain(&root);

    let now = OffsetDateTime::now_utc();
    let created_at = format_ts(now, TS_FORMAT);
    let short = short_head(&head);
    let anchor_id = format!("{}-{}", format_ts(now, ID_FORMAT), short);

    let anchor = BatchAnchor {
        schema_version: SCHEMA_VERSION,
        kind: KIND_BATCH_ANCHOR.to_string(),
        anchor_id: anchor_id.clone(),
        created_at: created_at.clone(),
        repo_root: root.display().to_string(),
        git_head: head,
        git_branch: branch,
        git_status_porcelain: status,
        filesystem_anchor_time: created_at,
    };

    let out_root = output::select_output_root(&root, args.output.as_deref());
    let batches = output::batches_dir(&out_root);
    fs::create_dir_all(&batches).map_err(|e| {
        AikitError::other(format!(
            "failed to create output dir {}: {e}",
            batches.display()
        ))
    })?;

    let file_path = batches.join(format!("{anchor_id}.json"));
    let body = serde_json::to_string_pretty(&anchor)
        .map_err(|e| AikitError::other(format!("failed to serialize anchor: {e}")))?;
    fs::write(&file_path, format!("{body}\n")).map_err(|e| {
        AikitError::other(format!(
            "failed to write anchor {}: {e}",
            file_path.display()
        ))
    })?;

    let rel = display_relative(&root, &file_path);
    if args.json {
        let anchor_value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| AikitError::other(format!("failed to re-read anchor json: {e}")))?;
        let out = serde_json::json!({
            "anchor_path": rel,
            "anchor": anchor_value,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&out)
                .map_err(|e| AikitError::other(format!("failed to serialize output: {e}")))?
        );
    } else {
        println!("Batch anchor created:");
        println!("  {rel}");
    }
    Ok(())
}

pub fn changed(args: ChangedArgs) -> Result<(), AikitError> {
    let root = repo::detect_root()?;
    let anchor = load_anchor(&args.anchor, &root)?;
    // Validated above, so the timestamp parses.
    let anchor_time = parse_ts(&anchor.filesystem_anchor_time)
        .expect("anchor timestamp validated in load_anchor");

    let porcelain = repo::git_status_changed(&root);
    let mut files: Vec<ChangedFile> = Vec::new();

    // `-z` output is NUL-delimited; rename/copy records are followed by their
    // original path as the next NUL field. Paths are verbatim (never quoted).
    let records: Vec<&str> = porcelain.split('\0').collect();
    let mut i = 0;
    while i < records.len() {
        let entry = records[i];
        i += 1;
        // Skip empty entries (including the trailing field after the final NUL).
        // The first 3 bytes of a real record are "XY " (ASCII), so byte-slicing is safe.
        if entry.len() < 4 {
            continue;
        }
        let xy = &entry[..2];
        let path = &entry[3..];

        if xy.contains('R') || xy.contains('C') {
            // Rename/copy: `path` is the new name; the next NUL field is the
            // original path, which must be consumed either way to stay in sync.
            let orig = records.get(i).copied().filter(|s| !s.is_empty());
            if orig.is_some() {
                i += 1;
            }
            push_if_included(&mut files, &root, path, "created", "git_status", args.hash);
            // A rename removes the original; a copy leaves it in place.
            if xy.contains('R') {
                if let Some(orig) = orig {
                    push_if_included(&mut files, &root, orig, "deleted", "git_status", args.hash);
                }
            }
        } else if xy == "??" {
            // Untracked: included only when requested, and only when its mtime is
            // strictly newer than the anchor. A file we cannot stat is skipped
            // rather than falsely reported as created.
            if args.tracked_only || !args.include_untracked {
                continue;
            }
            if is_excluded(path) {
                continue;
            }
            match file_mtime(&root.join(path)) {
                Some(mt) if mt > anchor_time => {
                    files.push(make_file(&root, path, "created", "mtime", args.hash));
                }
                _ => continue,
            }
        } else {
            let status = status_from_xy(xy);
            push_if_included(&mut files, &root, path, status, "git_status", args.hash);
        }
    }

    files.sort_by(|a, b| a.path.cmp(&b.path).then(a.status.cmp(&b.status)));
    files.dedup_by(|a, b| a.path == b.path && a.status == b.status);
    let counts = count(&files);
    let notes = if files.iter().any(|f| f.source == "mtime") {
        Some(vec![MTIME_NOTE.to_string()])
    } else {
        None
    };

    let out = ChangedOutput {
        schema_version: SCHEMA_VERSION,
        kind: KIND_BATCH_CHANGED.to_string(),
        anchor: AnchorRef {
            anchor_id: anchor.anchor_id.clone(),
            path: args.anchor.clone(),
        },
        repo_root: root.display().to_string(),
        generated_at: format_ts(OffsetDateTime::now_utc(), TS_FORMAT),
        files,
        counts,
        notes,
    };

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&out)
                .map_err(|e| AikitError::other(format!("failed to serialize output: {e}")))?
        );
    } else if out.files.is_empty() {
        println!("No changes since anchor {}.", out.anchor.anchor_id);
    } else {
        println!(
            "Changed since anchor {} ({} file(s)):",
            out.anchor.anchor_id, out.counts.total
        );
        for f in &out.files {
            println!("  {:<8} {}", f.status, f.path);
        }
        if let Some(notes) = &out.notes {
            for note in notes {
                println!("note: {note}");
            }
        }
    }
    Ok(())
}

/// Read and validate an anchor file, returning `blocked_*` errors on failure.
fn load_anchor(anchor_path: &str, repo_root: &Path) -> Result<BatchAnchor, AikitError> {
    let content = fs::read_to_string(anchor_path).map_err(|_| {
        AikitError::blocked(
            blocked::MISSING_ANCHOR,
            format!("anchor file not found or unreadable: {anchor_path}"),
        )
    })?;
    let anchor: BatchAnchor = serde_json::from_str(&content).map_err(|e| {
        AikitError::blocked(
            blocked::INVALID_ANCHOR,
            format!("anchor file could not be parsed: {e}"),
        )
    })?;

    if anchor.kind != KIND_BATCH_ANCHOR {
        return Err(AikitError::blocked(
            blocked::INVALID_ANCHOR,
            format!("anchor file has unexpected kind: {}", anchor.kind),
        ));
    }
    if anchor.schema_version != SCHEMA_VERSION {
        return Err(AikitError::blocked(
            blocked::INVALID_ANCHOR,
            format!(
                "anchor schema_version {} is not supported (expected {SCHEMA_VERSION})",
                anchor.schema_version
            ),
        ));
    }
    if parse_ts(&anchor.filesystem_anchor_time).is_none() {
        return Err(AikitError::blocked(
            blocked::INVALID_ANCHOR,
            format!(
                "anchor filesystem_anchor_time is not a valid timestamp: {}",
                anchor.filesystem_anchor_time
            ),
        ));
    }
    if !same_repo(repo_root, &anchor.repo_root) {
        return Err(AikitError::blocked(
            blocked::INVALID_ANCHOR,
            format!(
                "anchor was created in a different repository ({}); refusing to compare against {}",
                anchor.repo_root,
                repo_root.display()
            ),
        ));
    }
    Ok(anchor)
}

/// Whether the anchor's recorded repo root refers to the same repository as
/// `repo_root`, comparing canonicalized paths when possible.
fn same_repo(repo_root: &Path, anchor_root: &str) -> bool {
    match (
        fs::canonicalize(repo_root).ok(),
        fs::canonicalize(anchor_root).ok(),
    ) {
        (Some(a), Some(b)) => a == b,
        _ => repo_root.to_string_lossy() == anchor_root,
    }
}

fn push_if_included(
    files: &mut Vec<ChangedFile>,
    root: &Path,
    rel: &str,
    status: &str,
    source: &str,
    hash: bool,
) {
    if is_excluded(rel) {
        return;
    }
    files.push(make_file(root, rel, status, source, hash));
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

fn status_from_xy(xy: &str) -> &'static str {
    if xy.contains('D') {
        "deleted"
    } else if xy.contains('A') {
        "created"
    } else {
        "modified"
    }
}

fn make_file(root: &Path, rel: &str, status: &str, source: &str, hash: bool) -> ChangedFile {
    let (size_bytes, sha256) = if status == "deleted" {
        (None, None)
    } else {
        let abs = root.join(rel);
        // symlink_metadata: report the link itself, never follow it.
        let size = fs::symlink_metadata(&abs).map(|m| m.len()).ok();
        let sha = if hash { compute_sha256(&abs) } else { None };
        (size, sha)
    };
    ChangedFile {
        path: rel.to_string(),
        status: status.to_string(),
        source: source.to_string(),
        size_bytes,
        sha256,
    }
}

fn count(files: &[ChangedFile]) -> Counts {
    let mut c = Counts {
        total: files.len(),
        ..Counts::default()
    };
    for f in files {
        match f.status.as_str() {
            "created" => c.created += 1,
            "modified" => c.modified += 1,
            "deleted" => c.deleted += 1,
            _ => {}
        }
    }
    c
}

fn is_excluded(path: &str) -> bool {
    DEFAULT_EXCLUDES.iter().any(|p| path.starts_with(p))
}

/// Stream a file through SHA-256 in fixed-size chunks (bounded memory, even for
/// multi-gigabyte files). Returns `None` if the file cannot be read.
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

fn file_mtime(path: &Path) -> Option<OffsetDateTime> {
    let modified = fs::symlink_metadata(path).ok()?.modified().ok()?;
    Some(OffsetDateTime::from(modified))
}

fn format_ts(dt: OffsetDateTime, fmt: &[FormatItem<'static>]) -> String {
    dt.format(fmt).unwrap_or_default()
}

fn parse_ts(s: &str) -> Option<OffsetDateTime> {
    PrimitiveDateTime::parse(s, TS_FORMAT)
        .ok()
        .map(|pdt| pdt.assume_utc())
}

fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}
