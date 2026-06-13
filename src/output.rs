//! Output-root selection and local output directory helpers.
//!
//! Policy: the default output root is always `<repo>/.aikit/outputs/`. aikit never
//! auto-selects or auto-creates anything under `.scratch/`; `.scratch` is used only
//! when the caller explicitly passes `--output`. A `--output <dir>` override
//! replaces the root entirely.

use std::path::{Path, PathBuf};

/// Select the aikit output root for a repository.
///
/// - `--output <dir>` override → that directory, verbatim.
/// - otherwise → `<repo>/.aikit/outputs` (always; `.scratch` is never auto-selected).
pub fn select_output_root(repo_root: &Path, override_dir: Option<&str>) -> PathBuf {
    if let Some(dir) = override_dir {
        return PathBuf::from(dir);
    }
    repo_root.join(".aikit").join("outputs")
}

/// The `batches/` subdirectory under an output root.
pub fn batches_dir(root: &Path) -> PathBuf {
    root.join("batches")
}

/// The `inventory/` subdirectory under an output root.
pub fn inventory_dir(root: &Path) -> PathBuf {
    root.join("inventory")
}

/// The `reviews/` subdirectory under an output root.
pub fn reviews_dir(root: &Path) -> PathBuf {
    root.join("reviews")
}

/// The `runs/` subdirectory under an output root.
pub fn runs_dir(root: &Path) -> PathBuf {
    root.join("runs")
}
