//! `aikit batch start` and `aikit batch changed` implementations.
//!
//! Anchor-based changed-file discovery is **timestamp-based**: it reports existing files
//! whose filesystem modification time is newer than the anchor file. It does not consult
//! `git status` — tracked/untracked/staged/unstaged status is not the deciding factor, so
//! a file that is dirty relative to `HEAD` but was last modified before the anchor is
//! excluded. `.gitignore`/`.git/info/exclude` and aikit's own areas are skipped. Deleted
//! files are out of scope (no content exists to bundle). mtime is a best-effort heuristic.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use globset::GlobSet;
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};
use time::format_description::FormatItem;
use time::macros::format_description;
use time::{OffsetDateTime, PrimitiveDateTime};

use crate::cli::{BatchListArgs, BatchShowArgs, ChangedArgs, StartArgs};
use crate::config::ResolvedConfig;
use crate::errors::{blocked, AikitError};
use crate::formats::{
    AnchorRef, AnchorView, BatchAnchor, BatchList, BatchListCounts, BatchShow, ChangedFile,
    ChangedOutput, Counts, SkippedAnchor, KIND_BATCH_ANCHOR, KIND_BATCH_CHANGED, KIND_BATCH_LIST,
    KIND_BATCH_SHOW, SCHEMA_VERSION,
};
use crate::{output, repo};

/// Timestamp format used for `created_at` / `filesystem_anchor_time` / `generated_at`.
const TS_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
/// Compact format used inside anchor ids.
const ID_FORMAT: &[FormatItem<'static>] =
    format_description!("[year][month][day]-[hour][minute][second]");

const MTIME_NOTE: &str =
    "Anchor-based discovery is timestamp-based: it reports existing files whose filesystem \
modification time is newer than the anchor file. mtime is a best-effort heuristic and may \
be imprecise.";

pub fn start(args: StartArgs) -> Result<(), AikitError> {
    let root = repo::detect_root()?;
    let head = repo::git_head(&root);
    let branch = repo::git_branch(&root);

    let now = OffsetDateTime::now_utc();
    let created_at = format_ts(now, TS_FORMAT);
    let short = short_head(&head);
    let anchor_id = format!("{}-{}", format_ts(now, ID_FORMAT), short);

    // The initial file snapshot is optional (no full repo scan unless requested): only
    // `--snapshot` records the tracked-file list at anchor time.
    let initial_snapshot = if args.snapshot {
        Some(repo::git_tracked_files(&root))
    } else {
        None
    };

    let anchor = BatchAnchor {
        schema_version: SCHEMA_VERSION,
        kind: KIND_BATCH_ANCHOR.to_string(),
        anchor_id: anchor_id.clone(),
        created_at: created_at.clone(),
        repo_root: root.display().to_string(),
        git_head: head,
        git_branch: branch,
        filesystem_anchor_time: created_at,
        aikit_version: env!("CARGO_PKG_VERSION").to_string(),
        initial_snapshot,
    };

    // A relative --output is resolved against the repo root (not the cwd), matching
    // inventory/review, so the anchor lands under <repo>/<output>/batches regardless
    // of the directory the command is run from.
    let selected = output::select_output_root(&root, args.output.as_deref());
    let out_root = if selected.is_absolute() {
        selected
    } else {
        root.join(selected)
    };
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
    let reference = anchor_reference_time(&args.anchor, &anchor);
    let cfg = crate::config::load(&root)?;

    // Timestamp-based discovery: existing files whose filesystem mtime is newer than the
    // anchor. Git status is NOT consulted — tracked/untracked/staged/unstaged is not the
    // deciding factor; a pre-existing file that is dirty vs HEAD but was last modified
    // before the anchor is excluded. Deleted files are out of scope (no content on disk).
    let paths = walk_changed_since_anchor(&root, reference, &cfg)?;
    let files: Vec<ChangedFile> = paths
        .iter()
        .map(|p| make_file(&root, p, "modified", "anchor_mtime", args.hash))
        .collect();

    let counts = count(&files);
    let notes = if files.is_empty() {
        None
    } else {
        Some(vec![MTIME_NOTE.to_string()])
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

/// The reference time for anchor-based changed-file discovery: the anchor **file's**
/// filesystem mtime (full precision — "the anchor file timestamp is the reference
/// point"), falling back to the recorded `filesystem_anchor_time` only when the file's
/// mtime cannot be read.
fn anchor_reference_time(anchor_path: &str, anchor: &BatchAnchor) -> OffsetDateTime {
    file_mtime(Path::new(anchor_path)).unwrap_or_else(|| {
        parse_ts(&anchor.filesystem_anchor_time).expect("anchor timestamp validated in load_anchor")
    })
}

/// Top-level repo areas always excluded from anchor discovery (git metadata and aikit's
/// own local working/output areas), regardless of gitignore state. This is a safety net
/// independent of `.gitignore` so anchor/output/run artifacts never enter the changed set.
fn is_hard_excluded(rel: &str) -> bool {
    let top = rel.split('/').next().unwrap_or(rel);
    matches!(top, ".git" | ".aikit" | ".scratch" | ".claude")
}

/// Repo-relative paths of existing regular files whose filesystem mtime is strictly
/// newer than the anchor reference time. This is the timestamp-based "changed since
/// anchor" set shared by `batch changed --anchor` and `review generate --anchor`.
///
/// It does **not** consult `git status`: tracked/untracked/staged/unstaged status is not
/// the deciding factor — only mtime is. A pre-existing file that is dirty relative to
/// `HEAD` but was last modified before the anchor is excluded. `.gitignore` /
/// `.git/info/exclude` are honored (so aikit outputs, build trees, and other ignored
/// areas are skipped); aikit's own areas and configured build/dependency directories are
/// also excluded as a safety net. Symlinks are not followed and only regular files are
/// returned, so nothing outside the repo is reported. Deleted files are out of scope
/// (no content exists on disk to bundle).
fn walk_changed_since_anchor(
    repo_root: &Path,
    reference_time: OffsetDateTime,
    cfg: &ResolvedConfig,
) -> Result<Vec<String>, AikitError> {
    let root_canon = fs::canonicalize(repo_root)
        .map_err(|e| AikitError::other(format!("failed to resolve repo root: {e}")))?;
    let exclude = cfg.exclude_globset()?;
    let rules = cfg.exclude_dir_rules();
    let prune_root = root_canon.clone();
    let prune_rules = rules.clone();

    let mut walker = WalkBuilder::new(&root_canon);
    walker
        .hidden(false) // visit tracked dotfiles (.gitignore, .github/, …)
        .git_global(false) // ignore the user's global excludes for determinism
        .follow_links(false);
    walker.filter_entry(move |entry| {
        let rel = match entry.path().strip_prefix(&prune_root) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => return true,
        };
        if rel.is_empty() {
            return true;
        }
        if is_hard_excluded(&rel) {
            return false;
        }
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            return !prune_rules.prunes(&rel);
        }
        true
    });

    let mut out: Vec<String> = Vec::new();
    for result in walker.build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let rel = match entry.path().strip_prefix(&root_canon) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        if exclude.is_match(&rel) {
            continue;
        }
        match file_mtime(entry.path()) {
            Some(mt) if mt > reference_time => out.push(rel),
            _ => {}
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// A file discovered by enhanced anchor discovery, with the signal that detected it.
#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: String,
    /// `anchor_mtime` (timestamp-based) | `explicit` (configured `include_files`).
    pub source: String,
}

/// The result of enhanced anchor discovery: bundle-able files (created/modified or
/// allowlisted), plus tracked deletions/renames recorded for the manifest only.
#[derive(Debug, Default)]
pub struct BundleDiscovery {
    pub anchor_id: String,
    pub files: Vec<DiscoveredFile>,
    pub deletions: Vec<DiscoveredFile>,
    pub notes: Vec<String>,
}

/// Timestamp-based anchor discovery for `review generate --anchor`.
///
/// The base set is [`walk_changed_since_anchor`] — existing files whose filesystem mtime
/// is newer than the anchor (no `git status`; status/dirtiness is not the deciding
/// factor). Every file records the `anchor_mtime` detection source. When enhanced
/// discovery is enabled (config `include_ignored_batch_files` or
/// `--include-ignored-batch-files`) plus `include_globs`, allowlisted *ignored* files
/// modified after the anchor are also pulled in. Configured `include_files` are always
/// included when present. Deleted files are out of scope (no content to bundle).
pub fn discover_for_bundle(
    repo_root: &Path,
    anchor_path: &str,
    cfg: &ResolvedConfig,
) -> Result<BundleDiscovery, AikitError> {
    let anchor = load_anchor(anchor_path, repo_root)?;
    let reference = anchor_reference_time(anchor_path, &anchor);

    let mut files: Vec<DiscoveredFile> = walk_changed_since_anchor(repo_root, reference, cfg)?
        .into_iter()
        .map(|p| discovered(&p, "anchor_mtime"))
        .collect();

    // Enhanced: allowlisted ignored files modified after the anchor (these are skipped by
    // the gitignore-honoring base walk, so they are pulled in explicitly here).
    if cfg.include_ignored_batch_files && !cfg.include_globs.is_empty() {
        let include = cfg.include_globset()?;
        let exclude = cfg.exclude_globset()?;
        let rules = cfg.exclude_dir_rules();
        for rel in scan_ignored_files(repo_root, &include, &exclude, &rules, reference)? {
            files.push(discovered(&rel, "anchor_mtime"));
        }
    }

    // Explicit always-include files, when they currently exist and are not excluded.
    let exclude = cfg.exclude_globset()?;
    for inc in &cfg.include_files {
        let normalized = inc.replace('\\', "/");
        if !exclude.is_match(&normalized) && exists_in_worktree(repo_root, &normalized) {
            files.push(discovered(&normalized, "explicit"));
        }
    }

    // Deterministic order; first-seen source wins on duplicates.
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.dedup_by(|a, b| a.path == b.path);

    let notes = if files.is_empty() {
        Vec::new()
    } else {
        vec![MTIME_NOTE.to_string()]
    };

    Ok(BundleDiscovery {
        anchor_id: anchor.anchor_id,
        files,
        deletions: Vec::new(),
        notes,
    })
}

fn discovered(path: &str, source: &str) -> DiscoveredFile {
    DiscoveredFile {
        path: path.to_string(),
        source: source.to_string(),
    }
}

/// Walk the worktree (gitignore filtering disabled, so ignored files are visited) and
/// return repo-relative paths that match the include allowlist, do not match the
/// exclude globs, and were modified after the anchor. Excluded directory prefixes are
/// pruned so large/sensitive trees are never descended into. Symlinks are not followed
/// and only regular files are returned, so nothing outside the repo is ever reported.
fn scan_ignored_files(
    repo_root: &Path,
    include: &GlobSet,
    exclude: &GlobSet,
    dir_rules: &crate::config::ExcludeDirRules,
    anchor_time: OffsetDateTime,
) -> Result<Vec<String>, AikitError> {
    let root_canon = fs::canonicalize(repo_root)
        .map_err(|e| AikitError::other(format!("failed to resolve repo root: {e}")))?;
    let rules = dir_rules.clone();
    let prune_root = root_canon.clone();

    let mut walker = WalkBuilder::new(&root_canon);
    walker
        .standard_filters(false) // visit ignored files too
        .hidden(false) // visit dotfiles such as .aikit/
        .parents(false)
        .follow_links(false);
    walker.filter_entry(move |entry| {
        // Hard-exclude git metadata and aikit's own areas; prune excluded directories.
        let rel = match entry.path().strip_prefix(&prune_root) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => return true,
        };
        if rel.is_empty() {
            return true;
        }
        if is_hard_excluded(&rel) {
            return false;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            return !rules.prunes(&rel);
        }
        true
    });

    let mut out: Vec<String> = Vec::new();
    for result in walker.build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let rel = match entry.path().strip_prefix(&root_canon) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        if !include.is_match(&rel) || exclude.is_match(&rel) {
            continue;
        }
        match file_mtime(entry.path()) {
            Some(mt) if mt > anchor_time => out.push(rel),
            _ => {}
        }
    }
    Ok(out)
}

/// `aikit batch list` — list valid batch anchors under the selected output root
/// (read-only). Invalid files are reported as skipped, not guessed. Does NOT auto-select
/// any "latest" anchor — anchor-consuming commands always require an explicit anchor.
pub fn list(args: BatchListArgs) -> Result<(), AikitError> {
    let root = repo::detect_root()?;
    let repo_canon = fs::canonicalize(&root)
        .map_err(|e| AikitError::other(format!("failed to resolve repo root: {e}")))?;
    let output_root = output::resolve_output_root(&repo_canon, args.root.as_deref())?;

    let mut anchors: Vec<AnchorView> = Vec::new();
    let mut skipped: Vec<SkippedAnchor> = Vec::new();
    // Only read a real (non-symlink) batches directory, so a symlinked `batches/` can
    // never redirect listing outside the intended output root.
    if let Some(batches) = safe_batches_dir(&output_root) {
        if let Ok(entries) = fs::read_dir(&batches) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.ends_with(".json") {
                    continue;
                }
                let lmeta = match fs::symlink_metadata(&path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if lmeta.file_type().is_symlink() || !lmeta.is_file() {
                    continue;
                }
                let rel = display_relative(&repo_canon, &path);
                match parse_list_anchor(&path) {
                    Ok(anchor) => anchors.push(anchor_view(&anchor, rel)),
                    Err(reason) => skipped.push(SkippedAnchor { path: rel, reason }),
                }
            }
        }
    }
    anchors.sort_by(|a, b| a.anchor_id.cmp(&b.anchor_id));
    skipped.sort_by(|a, b| a.path.cmp(&b.path));

    let record = BatchList {
        schema_version: SCHEMA_VERSION,
        kind: KIND_BATCH_LIST.to_string(),
        repo_root: repo_canon.display().to_string(),
        output_root: display_relative(&repo_canon, &output_root),
        generated_at: format_ts(OffsetDateTime::now_utc(), TS_FORMAT),
        counts: BatchListCounts {
            total: anchors.len(),
            skipped: skipped.len(),
        },
        anchors,
        skipped,
        blocked_state: None,
    };

    if args.json {
        print_json(&record)?;
    } else {
        println!("aikit batch list:");
        println!("  repo root: {}", record.repo_root);
        println!("  batch anchor root: {}/batches", record.output_root);
        println!("  anchors: {}", record.counts.total);
        for a in &record.anchors {
            println!(
                "    {}  {}  {}  {}  {}",
                a.anchor_id,
                a.created_at,
                a.git_branch,
                short_head(&a.git_head),
                a.path
            );
        }
        if !record.skipped.is_empty() {
            println!("  skipped (invalid): {}", record.counts.skipped);
            for s in &record.skipped {
                println!("    {} ({})", s.path, s.reason);
            }
        }
    }
    Ok(())
}

/// `aikit batch show <anchor-path-or-id>` — show one explicit anchor (read-only).
/// Validates the anchor and that it belongs to the current repo. Never auto-selects.
pub fn show(args: BatchShowArgs) -> Result<(), AikitError> {
    let root = repo::detect_root()?;
    let repo_canon = fs::canonicalize(&root)
        .map_err(|e| AikitError::other(format!("failed to resolve repo root: {e}")))?;
    let output_root = output::resolve_output_root(&repo_canon, args.root.as_deref())?;
    let path = resolve_anchor_path(&repo_canon, &output_root, &args.anchor)?;
    let anchor = load_anchor(path.to_string_lossy().as_ref(), &repo_canon)?;
    let rel = display_relative(&repo_canon, &path);

    let record = BatchShow {
        schema_version: SCHEMA_VERSION,
        kind: KIND_BATCH_SHOW.to_string(),
        repo_root: repo_canon.display().to_string(),
        anchor: anchor_view(&anchor, rel),
        belongs_to_repo: true,
        blocked_state: None,
    };

    if args.json {
        print_json(&record)?;
    } else {
        let a = &record.anchor;
        println!("aikit batch show:");
        println!("  repo root: {}", record.repo_root);
        println!("  anchor path: {}", a.path);
        println!("  anchor id: {}", a.anchor_id);
        println!("  created_at: {}", a.created_at);
        println!("  filesystem_anchor_time: {}", a.filesystem_anchor_time);
        println!("  git branch: {}", a.git_branch);
        println!("  git head: {}", a.git_head);
        println!("  belongs to current repo: {}", record.belongs_to_repo);
    }
    Ok(())
}

/// The canonical `batches/` directory under `output_root`, but only when it is a real
/// (non-symlink) directory — so a symlinked `batches/` can never redirect id lookup or
/// listing outside the intended output root. Returns `None` when it is absent or a symlink.
fn safe_batches_dir(output_root: &Path) -> Option<PathBuf> {
    let batches = output_root.join("batches");
    match fs::symlink_metadata(&batches) {
        Ok(m) if m.is_dir() && !m.file_type().is_symlink() => fs::canonicalize(&batches).ok(),
        _ => None,
    }
}

/// Resolve an `<anchor-path-or-id>` argument to a real anchor file path inside the repo.
/// An existing file path that resolves outside the repo is rejected as `blocked_path_escape`;
/// otherwise an id is looked up as `<output_root>/batches/<id>.json`.
pub fn resolve_anchor_path(
    repo_canon: &Path,
    output_root: &Path,
    arg: &str,
) -> Result<PathBuf, AikitError> {
    // A *bare id* (no path separators, not absolute, no `..`) is looked up under the
    // output root's batches/ folder; anything path-shaped is treated as an explicit path.
    // Resolving ids first means a stray repo file that happens to share an id's name can
    // never shadow the real anchor.
    let looks_like_path = arg.contains('/')
        || arg.contains('\\')
        || arg.ends_with(".json")
        || Path::new(arg).is_absolute();

    if !looks_like_path {
        // Resolve under the *canonical* batches directory (a symlinked batches/ is not
        // accepted), and require the anchor file to be a real (non-symlink) file that
        // stays under that canonical batches directory.
        let batches = match safe_batches_dir(output_root) {
            Some(b) => b,
            None => {
                return Err(AikitError::blocked(
                    blocked::MISSING_ANCHOR,
                    format!("no anchor found for id: {arg}"),
                ))
            }
        };
        let cand = batches.join(format!("{arg}.json"));
        if let Ok(m) = fs::symlink_metadata(&cand) {
            if m.is_file() && !m.file_type().is_symlink() {
                if let Ok(real) = fs::canonicalize(&cand) {
                    if real.starts_with(&batches) {
                        return Ok(real);
                    }
                }
                return Err(AikitError::blocked(
                    blocked::PATH_ESCAPE,
                    format!("anchor for id {arg:?} resolves outside the batch folder"),
                ));
            }
        }
        return Err(AikitError::blocked(
            blocked::MISSING_ANCHOR,
            format!("no anchor found for id: {arg}"),
        ));
    }

    // Explicit path form.
    let raw = if Path::new(arg).is_absolute() {
        PathBuf::from(arg)
    } else {
        repo_canon.join(arg)
    };
    if raw.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(AikitError::blocked(
            blocked::PATH_ESCAPE,
            format!("anchor path must not contain `..`: {arg}"),
        ));
    }
    let real = fs::canonicalize(&raw).map_err(|_| {
        AikitError::blocked(
            blocked::MISSING_ANCHOR,
            format!("anchor file not found or unreadable: {arg}"),
        )
    })?;
    if !real.starts_with(repo_canon) {
        return Err(AikitError::blocked(
            blocked::PATH_ESCAPE,
            format!("anchor resolves outside the repository: {arg}"),
        ));
    }
    if !real.is_file() {
        return Err(AikitError::blocked(
            blocked::MISSING_ANCHOR,
            format!("anchor path is not a regular file: {arg}"),
        ));
    }
    Ok(real)
}

/// Lenient parse for `batch list`: returns the anchor or a human-readable skip reason.
/// Unlike `load_anchor`, it does NOT enforce same-repo (the batch folder is repo-local).
fn parse_list_anchor(path: &Path) -> Result<BatchAnchor, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("unreadable: {e}"))?;
    let anchor: BatchAnchor =
        serde_json::from_str(&content).map_err(|e| format!("invalid json: {e}"))?;
    if anchor.kind != KIND_BATCH_ANCHOR {
        return Err(format!("unexpected kind: {}", anchor.kind));
    }
    if anchor.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "unsupported schema_version: {}",
            anchor.schema_version
        ));
    }
    // Validate the same timestamps anchor-consuming commands require, so `batch list`
    // reports a malformed anchor as skipped rather than valid.
    if parse_ts(&anchor.created_at).is_none() {
        return Err(format!("invalid created_at: {}", anchor.created_at));
    }
    if parse_ts(&anchor.filesystem_anchor_time).is_none() {
        return Err(format!(
            "invalid filesystem_anchor_time: {}",
            anchor.filesystem_anchor_time
        ));
    }
    Ok(anchor)
}

/// Build a serializable anchor view from an anchor and its repo-relative path.
pub fn anchor_view(anchor: &BatchAnchor, path: String) -> AnchorView {
    AnchorView {
        schema_version: anchor.schema_version,
        kind: anchor.kind.clone(),
        anchor_id: anchor.anchor_id.clone(),
        path,
        created_at: anchor.created_at.clone(),
        repo_root: anchor.repo_root.clone(),
        git_branch: anchor.git_branch.clone(),
        git_head: anchor.git_head.clone(),
        filesystem_anchor_time: anchor.filesystem_anchor_time.clone(),
        aikit_version: anchor.aikit_version.clone(),
        initial_snapshot_count: anchor.initial_snapshot.as_ref().map(|s| s.len()),
    }
}

fn print_json<T: serde::Serialize>(record: &T) -> Result<(), AikitError> {
    let json = serde_json::to_string_pretty(record)
        .map_err(|e| AikitError::other(format!("failed to serialize record: {e}")))?;
    println!("{json}");
    Ok(())
}

/// Read and validate an anchor file, returning `blocked_*` errors on failure.
pub fn load_anchor(anchor_path: &str, repo_root: &Path) -> Result<BatchAnchor, AikitError> {
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
    if parse_ts(&anchor.created_at).is_none() {
        return Err(AikitError::blocked(
            blocked::INVALID_ANCHOR,
            format!(
                "anchor created_at is not a valid timestamp: {}",
                anchor.created_at
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

fn short_head(head: &str) -> String {
    if head.is_empty() {
        "nohead".to_string()
    } else if head.len() >= 7 {
        head[..7].to_string()
    } else {
        head.to_string()
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

/// Whether a repo-relative path is currently present in the worktree (a regular
/// file or a symlink; `symlink_metadata` does not follow the link). Used to keep
/// non-existent configured `include_files` out of the anchor-driven review input set.
fn exists_in_worktree(repo_root: &Path, rel: &str) -> bool {
    fs::symlink_metadata(repo_root.join(rel)).is_ok()
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
