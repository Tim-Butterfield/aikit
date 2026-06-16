//! `aikit diff anchor <anchor-path-or-id>` — a mechanical Git diff report from a batch
//! anchor's recorded head to the current working-tree state.
//!
//! This is inspection only: it never advances workflow state, never creates a review
//! bundle or output artifact, and never touches remotes. Git is the source of truth for
//! the diff. Untracked file *contents* are not part of a `git diff` and are therefore not
//! included here (callers wanting a timestamp-based file list use `batch changed --anchor`).

use std::fs;

use time::format_description::FormatItem;
use time::macros::format_description;
use time::OffsetDateTime;

use crate::batch;
use crate::cli::DiffAnchorArgs;
use crate::errors::{blocked, AikitError};
use crate::formats::{
    AnchorRef, DiffAnchor, DiffCounts, DiffFile, KIND_DIFF_ANCHOR, SCHEMA_VERSION,
};
use crate::{output, repo};

const TS_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");

const UNTRACKED_NOTE: &str = "Untracked file contents are not part of the Git diff. Use \
`aikit batch changed --anchor <anchor>` for a timestamp-based list of changed files.";

/// `aikit diff anchor` — diff the anchor's recorded `git_head` against the current
/// working tree.
pub fn anchor(args: DiffAnchorArgs) -> Result<(), AikitError> {
    let root = repo::detect_root()?;
    let repo_canon = fs::canonicalize(&root)
        .map_err(|e| AikitError::other(format!("failed to resolve repo root: {e}")))?;
    // Anchor ids are looked up under the default output root's batches/ folder.
    let default_root = output::resolve_output_root(&repo_canon, None)?;
    let path = batch::resolve_anchor_path(&repo_canon, &default_root, &args.anchor)?;
    let anchor = batch::load_anchor(path.to_string_lossy().as_ref(), &repo_canon)?;

    let base = anchor.git_head.clone();
    if !repo::commit_exists(&repo_canon, &base) {
        return Err(AikitError::blocked(
            blocked::MISSING_BASE_COMMIT,
            format!("anchor base commit {base:?} is not present in this repository"),
        ));
    }

    let current = repo::git_head(&repo_canon);
    let tracked_tree_clean = !repo::is_tracked_tree_dirty(&repo_canon);

    // `git diff <base>` compares the base commit to the current working tree, so it
    // captures both committed changes since the anchor and current tracked worktree/index
    // changes. Untracked files are not part of `git diff` output.
    let name_status =
        repo::git_diff(&repo_canon, &["--name-status", "-z", &base]).unwrap_or_default();
    let mut files = parse_name_status_z(&name_status);
    files.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| a.raw_status.cmp(&b.raw_status))
    });
    let counts = count_files(&files);
    let stat = repo::git_diff(&repo_canon, &["--stat", &base]).unwrap_or_default();
    let patch = if args.patch {
        repo::git_diff(&repo_canon, &[base.as_str()])
    } else {
        None
    };

    let rel = path
        .strip_prefix(&repo_canon)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.display().to_string());

    let record = DiffAnchor {
        schema_version: SCHEMA_VERSION,
        kind: KIND_DIFF_ANCHOR.to_string(),
        repo_root: repo_canon.display().to_string(),
        generated_at: OffsetDateTime::now_utc()
            .format(TS_FORMAT)
            .unwrap_or_default(),
        anchor: AnchorRef {
            anchor_id: anchor.anchor_id.clone(),
            path: rel,
        },
        base_git_head: base,
        current_git_head: current,
        tracked_tree_clean,
        files,
        counts,
        stat,
        notes: vec![UNTRACKED_NOTE.to_string()],
        patch,
        blocked_state: None,
    };

    if args.json {
        let json = serde_json::to_string_pretty(&record)
            .map_err(|e| AikitError::other(format!("failed to serialize record: {e}")))?;
        println!("{json}");
    } else {
        println!("aikit diff anchor:");
        println!("  repo root: {}", record.repo_root);
        println!(
            "  anchor: {} ({})",
            record.anchor.anchor_id, record.anchor.path
        );
        println!("  base head: {}", record.base_git_head);
        println!("  current head: {}", record.current_git_head);
        println!(
            "  tracked tree: {}",
            if record.tracked_tree_clean {
                "clean"
            } else {
                "dirty"
            }
        );
        println!("  files: {}", record.counts.total);
        for f in &record.files {
            match &f.old_path {
                Some(old) => println!("    {:<6} {} <- {}", f.raw_status, f.path, old),
                None => println!("    {:<6} {}", f.raw_status, f.path),
            }
        }
        // diff stat is included by default; --stat is the explicit form of the same.
        if !record.stat.trim().is_empty() {
            println!("  stat:");
            for line in record.stat.lines() {
                println!("    {line}");
            }
        }
        for note in &record.notes {
            println!("note: {note}");
        }
        if let Some(patch) = &record.patch {
            if !patch.is_empty() {
                println!("--- patch ---");
                print!("{patch}");
            }
        }
    }
    Ok(())
}

/// Parse `git diff --name-status -z` output into file entries. Rename/copy records carry
/// two path fields (old, new); other records carry one.
fn parse_name_status_z(raw: &str) -> Vec<DiffFile> {
    let fields: Vec<&str> = raw.split('\0').filter(|s| !s.is_empty()).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < fields.len() {
        let raw_status = fields[i];
        i += 1;
        let first = raw_status.chars().next().unwrap_or(' ');
        if first == 'R' || first == 'C' {
            let old = fields.get(i).copied().unwrap_or("");
            i += 1;
            let new = fields.get(i).copied().unwrap_or("");
            i += 1;
            out.push(DiffFile {
                path: new.to_string(),
                status: status_word(first).to_string(),
                old_path: Some(old.to_string()),
                raw_status: raw_status.to_string(),
            });
        } else {
            let p = fields.get(i).copied().unwrap_or("");
            i += 1;
            out.push(DiffFile {
                path: p.to_string(),
                status: status_word(first).to_string(),
                old_path: None,
                raw_status: raw_status.to_string(),
            });
        }
    }
    out
}

fn status_word(c: char) -> &'static str {
    match c {
        'A' => "added",
        'M' => "modified",
        'D' => "deleted",
        'R' => "renamed",
        'C' => "copied",
        'T' => "type_changed",
        'U' => "unmerged",
        _ => "other",
    }
}

fn count_files(files: &[DiffFile]) -> DiffCounts {
    let mut c = DiffCounts {
        total: files.len(),
        added: 0,
        modified: 0,
        deleted: 0,
        renamed: 0,
        copied: 0,
        other: 0,
    };
    for f in files {
        match f.status.as_str() {
            "added" => c.added += 1,
            "modified" => c.modified += 1,
            "deleted" => c.deleted += 1,
            "renamed" => c.renamed += 1,
            "copied" => c.copied += 1,
            _ => c.other += 1,
        }
    }
    c
}
