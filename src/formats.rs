//! Serializable data structures: the batch anchor, the changed-file report, and
//! the repository inventory. All tool-owned formats carry a `schema_version` so
//! later changes stay detectable.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
pub const KIND_BATCH_ANCHOR: &str = "aikit.batch_anchor";
pub const KIND_BATCH_CHANGED: &str = "aikit.batch_changed";
pub const KIND_REPO_INVENTORY: &str = "aikit.repo_inventory";

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
