//! Output-root selection and local output directory helpers.
//!
//! Deterministic rule (per the plan): aikit never creates the
//! `.scratch/work/outputs/` parent itself. If that parent already exists, aikit
//! uses its own `aikit/` subtree under it; otherwise it falls back to
//! `.aikit/outputs/`. A `--output <dir>` override replaces the root entirely.

use std::path::{Path, PathBuf};

/// Select the aikit output root for a repository.
///
/// - `--output <dir>` override → that directory, verbatim.
/// - `<repo>/.scratch/work/outputs/` exists → `<repo>/.scratch/work/outputs/aikit`.
/// - otherwise → `<repo>/.aikit/outputs`.
pub fn select_output_root(repo_root: &Path, override_dir: Option<&str>) -> PathBuf {
    if let Some(dir) = override_dir {
        return PathBuf::from(dir);
    }

    let scratch_parent = repo_root.join(".scratch").join("work").join("outputs");
    if scratch_parent.is_dir() {
        scratch_parent.join("aikit")
    } else {
        repo_root.join(".aikit").join("outputs")
    }
}

/// The `batches/` subdirectory under an output root.
pub fn batches_dir(root: &Path) -> PathBuf {
    root.join("batches")
}

/// The `inventory/` subdirectory under an output root.
pub fn inventory_dir(root: &Path) -> PathBuf {
    root.join("inventory")
}
