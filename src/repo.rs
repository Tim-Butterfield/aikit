//! Repo-root detection and small Git helpers.
//!
//! Git is the source of truth for the repository root and working-tree state.
//! Batch 1 shells out to the `git` CLI rather than linking a Git library — that
//! keeps the dependency surface minimal and matches how the architect's workflows
//! already observe repo state.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::errors::{blocked, AikitError};

/// Detect the repository root via `git rev-parse --show-toplevel`, run from the
/// current working directory. Returns `blocked_repo_not_found` when there is none.
pub fn detect_root() -> Result<PathBuf, AikitError> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| AikitError::other(format!("failed to run git: {e}")))?;

    if !output.status.success() {
        return Err(AikitError::blocked(
            blocked::REPO_NOT_FOUND,
            "no Git repository root detected from the current directory",
        ));
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        return Err(AikitError::blocked(
            blocked::REPO_NOT_FOUND,
            "no Git repository root detected from the current directory",
        ));
    }
    Ok(PathBuf::from(root))
}

/// Current HEAD commit hash, or an empty string on an unborn branch (no commits).
pub fn git_head(root: &Path) -> String {
    run_git(root, &["rev-parse", "HEAD"]).unwrap_or_default()
}

/// Current branch name, or an empty string when unavailable.
pub fn git_branch(root: &Path) -> String {
    run_git(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default()
}

/// `git status --porcelain=v1` output (working-tree state vs HEAD), trailing
/// newline trimmed but internal lines preserved verbatim. Used for the anchor
/// snapshot recorded by `batch start`.
pub fn git_status_porcelain(root: &Path) -> String {
    run_git_raw(root, &["status", "--porcelain=v1"]).unwrap_or_default()
}

/// `git status --porcelain=v1 -z --untracked-files=all` — used for change detection.
///
/// - `-z` makes records NUL-delimited and paths **verbatim and unquoted** (no
///   C-style quoting/escaping), so paths with spaces, arrows, or special bytes are
///   safe to use directly; rename/copy records carry the original path as a
///   separate NUL field rather than an ambiguous `orig -> new` string.
/// - `--untracked-files=all` lists files inside otherwise-untracked directories
///   individually (no directory collapsing), so each path can be matched precisely
///   against the output-directory excludes.
pub fn git_status_changed(root: &Path) -> String {
    run_git_raw(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )
    .unwrap_or_default()
}

/// Run a git command and return trimmed stdout, or `None` on failure.
fn run_git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Like `run_git` but only trims the trailing newline, preserving internal
/// whitespace (needed for porcelain status lines whose first columns are spaces).
fn run_git_raw(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(text.trim_end_matches('\n').to_string())
}
