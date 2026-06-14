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
pub const KIND_SCRIPT_CHECK: &str = "aikit.script_check";
pub const KIND_REPO_INIT: &str = "aikit.repo_init";
pub const KIND_REPO_DOCTOR: &str = "aikit.repo_doctor";
pub const KIND_OUTPUT_LIST: &str = "aikit.output_list";
pub const KIND_OUTPUT_SHOW: &str = "aikit.output_show";
pub const KIND_OUTPUT_CLEAN: &str = "aikit.output_clean";

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

/// The audit record written by `aikit script run` (run.json).
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

/// The report written to stdout by `aikit script check`. The command executes
/// nothing and creates no run output, so `executed` and `output_created` are always
/// false; `accepted` and `blocked_state` carry the policy verdict.
#[derive(Debug, Serialize)]
pub struct ScriptCheck {
    pub schema_version: u32,
    pub kind: String,
    /// Repo root, or `null` when the repo could not be detected.
    pub repo_root: Option<String>,
    /// Repo-relative path when resolved, else the path as supplied on the command line.
    pub script_path: String,
    /// Real (canonicalized) path when resolution succeeded; `null` otherwise.
    pub resolved_script_path: Option<String>,
    /// Interpreter that would be used; `null` when extension/location was not accepted.
    pub interpreter: Option<String>,
    pub require_clean: bool,
    pub allow_dirty: bool,
    /// Always false — `script check` never executes the script.
    pub executed: bool,
    /// Always false — `script check` creates no run output.
    pub output_created: bool,
    pub accepted: bool,
    /// Set when the policy blocked the script; `null` when accepted.
    pub blocked_state: Option<String>,
    /// Human-readable detail for a block; `null` when accepted.
    pub detail: Option<String>,
}

/// A repo-relative path and whether it currently exists. Used by `repo doctor` for
/// allowed script locations and interpreter availability.
#[derive(Debug, Serialize)]
pub struct PathStatus {
    pub path: String,
    pub exists: bool,
}

/// A known aikit output artifact (a `batches/*.json` file, or an `inventory/`,
/// `reviews/`, or `runs/` subdirectory). Used by the `output` command family.
#[derive(Debug, Serialize)]
pub struct OutputArtifact {
    /// `batches` | `inventory` | `reviews` | `runs`.
    pub family: String,
    /// Artifact id (filename without `.json` for batches; directory name otherwise).
    pub artifact_id: String,
    /// Repo-relative path of the artifact.
    pub path: String,
    /// `file` (a batch anchor) or `dir` (inventory/review/run directory).
    pub artifact_type: String,
    pub size_bytes: u64,
    pub modified_at: String,
}

/// Per-family artifact counts for `output list`.
#[derive(Debug, Serialize)]
pub struct OutputCounts {
    pub total: usize,
    pub batches: usize,
    pub inventory: usize,
    pub reviews: usize,
    pub runs: usize,
}

/// The report written by `aikit output list`.
#[derive(Debug, Serialize)]
pub struct OutputList {
    pub schema_version: u32,
    pub kind: String,
    pub repo_root: String,
    /// Repo-relative selected output root.
    pub output_root: String,
    pub generated_at: String,
    pub artifacts: Vec<OutputArtifact>,
    pub counts: OutputCounts,
    pub blocked_state: Option<String>,
}

/// A file within an artifact (for `output show`).
#[derive(Debug, Serialize)]
pub struct OutputFile {
    pub path: String,
    pub size_bytes: u64,
}

/// The report written by `aikit output show`.
#[derive(Debug, Serialize)]
pub struct OutputShow {
    pub schema_version: u32,
    pub kind: String,
    pub repo_root: String,
    pub output_root: String,
    pub artifact: OutputArtifact,
    /// Files contained in the artifact (the file itself for a batch anchor).
    pub files: Vec<OutputFile>,
    /// Parsed summary of the artifact's main JSON (kind/schema_version, ids), or null.
    pub metadata: serde_json::Value,
    pub blocked_state: Option<String>,
}

/// Filters applied by `aikit output clean`.
#[derive(Debug, Serialize)]
pub struct OutputCleanFilters {
    pub family: Option<String>,
    pub older_than: Option<String>,
    pub all: bool,
}

/// Candidate/deleted counts for `output clean`.
#[derive(Debug, Serialize)]
pub struct OutputCleanCounts {
    pub candidates: usize,
    pub deleted: usize,
}

/// The report written by `aikit output clean`. In dry-run mode `deleted` is empty.
#[derive(Debug, Serialize)]
pub struct OutputClean {
    pub schema_version: u32,
    pub kind: String,
    pub repo_root: String,
    pub output_root: String,
    pub dry_run: bool,
    pub execute: bool,
    pub filters: OutputCleanFilters,
    pub candidates: Vec<OutputArtifact>,
    /// Repo-relative paths actually deleted (only in execute mode).
    pub deleted: Vec<String>,
    pub counts: OutputCleanCounts,
    pub blocked_state: Option<String>,
}

/// The report written by `aikit repo init`. `repo init` only blocks on
/// `blocked_repo_not_found` (handled as an error), so `blocked_state` is `null` here.
#[derive(Debug, Serialize)]
pub struct RepoInit {
    pub schema_version: u32,
    pub kind: String,
    pub repo_root: String,
    /// Repo-relative aikit directory (`.aikit`).
    pub aikit_dir: String,
    /// Repo-relative temp directory (`.aikit/temp`).
    pub temp_dir: String,
    /// Repo-relative directories created during this run (empty when idempotent).
    pub created_dirs: Vec<String>,
    /// Whether `.aikit/` is ignored after this run.
    pub aikit_ignored: bool,
    /// The ignore source covering `.aikit/` (e.g. `.gitignore`, `.git/info/exclude`).
    pub ignore_source: Option<String>,
    /// Whether `.git/info/exclude` was updated this run.
    pub info_exclude_updated: bool,
    /// Human-readable action log (created / already-present / ignore actions).
    pub actions: Vec<String>,
    pub blocked_state: Option<String>,
}

/// The read-only readiness report written by `aikit repo doctor`. `repo doctor` only
/// blocks on `blocked_repo_not_found`, so `blocked_state` is `null` here.
#[derive(Debug, Serialize)]
pub struct RepoDoctor {
    pub schema_version: u32,
    pub kind: String,
    pub repo_root: String,
    pub git_branch: String,
    pub git_head: String,
    pub tracked_tree_clean: bool,
    pub aikit_dir_exists: bool,
    pub temp_dir_exists: bool,
    pub outputs_dir_exists: bool,
    pub aikit_ignored: bool,
    pub ignore_source: Option<String>,
    /// Repo-relative default output root (`.aikit/outputs`).
    pub default_output_root: String,
    pub allowed_script_locations: Vec<PathStatus>,
    pub interpreters: Vec<PathStatus>,
    pub current_exe: Option<String>,
    pub version: String,
    pub warnings: Vec<String>,
    pub ready: bool,
    pub blocked_state: Option<String>,
}
