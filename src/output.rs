//! Output-root selection and local output directory helpers.
//!
//! Policy: the default output root is always `<repo>/.aikit/outputs/`. aikit never
//! auto-selects or auto-creates anything under `.scratch/`; `.scratch` is used only
//! when the caller explicitly passes `--output`. A `--output <dir>` override
//! replaces the root entirely.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::errors::{blocked, AikitError};

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

/// Resolve a *management* output root for the `output` / `batch list|show` commands and
/// reject escapes. `repo_canon` must be the canonicalized repo root.
///
/// With no `--root`, returns the default `<repo>/.aikit/outputs`. An explicit `--root`
/// must resolve inside the repo (no `..`, no symlink escape) AND be a known aikit output
/// area — `.aikit/outputs` or `.scratch/work/outputs` (or under them) — so management can
/// never be redirected at protected/non-output directories like `.git`, `target`,
/// `.scratch`, `.claude`, `.aikit`, or the repo root.
pub fn resolve_output_root(
    repo_canon: &Path,
    root_arg: Option<&str>,
) -> Result<PathBuf, AikitError> {
    let candidate = match root_arg {
        None => return Ok(repo_canon.join(".aikit").join("outputs")),
        Some(p) => {
            let pb = PathBuf::from(p);
            if pb.is_absolute() {
                pb
            } else {
                repo_canon.join(pb)
            }
        }
    };
    if candidate
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(AikitError::blocked(
            blocked::PATH_ESCAPE,
            format!("--root must not contain `..`: {}", candidate.display()),
        ));
    }
    let resolved = if candidate.exists() {
        let c = fs::canonicalize(&candidate)
            .map_err(|e| AikitError::other(format!("failed to resolve --root: {e}")))?;
        if !c.starts_with(repo_canon) {
            return Err(AikitError::blocked(
                blocked::PATH_ESCAPE,
                "--root resolves outside the repository".to_string(),
            ));
        }
        c
    } else {
        if !candidate.starts_with(repo_canon) {
            return Err(AikitError::blocked(
                blocked::PATH_ESCAPE,
                "--root resolves outside the repository".to_string(),
            ));
        }
        candidate
    };
    let rel = resolved
        .strip_prefix(repo_canon)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let allowed = [".aikit/outputs", ".scratch/work/outputs"];
    let ok = allowed
        .iter()
        .any(|a| rel == *a || rel.starts_with(&format!("{a}/")));
    if !ok {
        return Err(AikitError::blocked(
            blocked::PATH_ESCAPE,
            format!("--root must be under .aikit/outputs/ or .scratch/work/outputs/ (got {rel:?})"),
        ));
    }
    Ok(resolved)
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
