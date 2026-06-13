//! Serializable data structures: the batch anchor, the changed-file report, and
//! the repository inventory. All tool-owned formats carry a `schema_version` so
//! later changes stay detectable.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
pub const KIND_BATCH_ANCHOR: &str = "aikit.batch_anchor";
pub const KIND_BATCH_CHANGED: &str = "aikit.batch_changed";
pub const KIND_REPO_INVENTORY: &str = "aikit.repo_inventory";
pub const KIND_REVIEW_BUNDLE: &str = "aikit.review_bundle";
pub const KIND_SCRIPT_RUN: &str = "aikit.script_run";

/// A point-in-time anchor written by `aikit batch start`.
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchAnchor {
    pub schema_version: u32,
    pub kind: String,
    pub anchor_id: String,
    pub created_at: String,
    pub repo_root: String,
    pub git_head: String,
    pub git_branch: String,
    pub git_status_porcelain: String,
    pub filesystem_anchor_time: String,
}

/// The output of `aikit batch changed`.
#[derive(Debug, Serialize)]
pub struct ChangedOutput {
    pub schema_version: u32,
    pub kind: String,
    pub anchor: AnchorRef,
    pub repo_root: String,
    pub generated_at: String,
    pub files: Vec<ChangedFile>,
    pub counts: Counts,
    /// Limitation notes (e.g., that untracked results use a best-effort mtime
    /// heuristic). Present only when relevant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct AnchorRef {
    pub anchor_id: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct ChangedFile {
    pub path: String,
    pub status: String,
    pub source: String,
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct Counts {
    pub total: usize,
    pub created: usize,
    pub modified: usize,
    pub deleted: usize,
}

/// The output of `aikit inventory repo`.
#[derive(Debug, Serialize)]
pub struct RepoInventory {
    pub schema_version: u32,
    pub kind: String,
    pub inventory_id: String,
    pub repo_root: String,
    pub git_head: String,
    pub generated_at: String,
    pub files: Vec<InventoryFile>,
    pub counts: InventoryCounts,
    /// Limitation notes (e.g., that `--max-files` truncated the listing). Present
    /// only when relevant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct InventoryFile {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub kind_hint: String,
}

#[derive(Debug, Serialize, Default)]
pub struct InventoryCounts {
    /// Number of files included in this inventory (after any `--max-files` limit).
    pub files: usize,
    /// Total bytes across the included files.
    pub bytes: u64,
    /// Whether a `--max-files` limit truncated the listing.
    pub truncated: bool,
    /// The applied `--max-files` limit, when one was given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_files: Option<usize>,
    /// Total files discovered before truncation, present only when truncated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_discovered: Option<usize>,
}

/// The manifest written by `aikit review generate` alongside the text bundle.
#[derive(Debug, Serialize)]
pub struct ReviewManifest {
    pub schema_version: u32,
    pub kind: String,
    pub review_id: String,
    pub repo_root: String,
    pub git_head: String,
    pub generated_at: String,
    pub inputs: ReviewInputs,
    pub limits: ReviewLimits,
    pub files: Vec<ReviewFile>,
    pub bundle_path: String,
    pub totals: ReviewTotals,
}

#[derive(Debug, Serialize)]
pub struct ReviewInputs {
    /// Input mode: `"explicit_files"` (from `--files`) or `"changed_since_anchor"`
    /// (from `--anchor`).
    pub mode: String,
    /// For anchor mode: the anchor file path as supplied on the command line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor_path: Option<String>,
    /// For anchor mode: the anchor's id, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor_id: Option<String>,
    /// The repo-relative input files in scope, in deterministic order.
    pub files: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ReviewLimits {
    pub max_file_bytes: Option<u64>,
    pub max_total_bytes: Option<u64>,
    pub max_file_lines: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ReviewFile {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub included: bool,
    pub truncated: bool,
    pub lines_included: usize,
    pub bytes_included: u64,
    /// Reason the file was omitted from the bundle, or `null` when included.
    pub omitted_reason: Option<String>,
    /// Which cap bound this file (`file_bytes` | `file_lines` | `total_bytes`), or `null`.
    pub cap_hit: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct ReviewTotals {
    pub files_total: usize,
    pub files_included: usize,
    pub files_omitted: usize,
    pub bytes_included: u64,
}

/// The audit record written by `aikit run script` (run.json).
#[derive(Debug, Serialize)]
pub struct ScriptRun {
    pub schema_version: u32,
    pub kind: String,
    pub run_id: String,
    pub repo_root: String,
    /// Repo-relative path of the script as resolved (canonicalized).
    pub script_path: String,
    pub script_sha256: String,
    /// Filename of the script copy inside the run directory (retains the extension).
    pub script_copy_path: Option<String>,
    pub interpreter: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub require_clean: bool,
    pub allow_dirty: bool,
    /// False when `--print` was used (the script was not executed).
    pub executed: bool,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub git_head_before: String,
    pub git_head_after: Option<String>,
    /// Exit code of the executed script, or `null` when not executed.
    pub exit_code: Option<i32>,
    /// Set when the run was blocked before execution; `null` otherwise.
    pub blocked_state: Option<String>,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,
}
