//! Serializable data structures: the batch anchor, the changed-file report, and
//! the repository inventory. All tool-owned formats carry a `schema_version` so
//! later changes stay detectable.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Render `path` for a record or for printed output, relative to `root` when it is inside it.
///
/// **Always forward-slashed.** These strings are a wire format: agents parse them, compare
/// them, and pass them back as `--anchor` arguments, and the constants they sit beside
/// (`allowed_script_locations`) are forward-slashed literals. A record that carried
/// `.aikit\outputs\...` next to `.aikit/temp/` would be two conventions in one document.
/// Forward slashes are accepted by Windows filesystem APIs, so normalising costs nothing.
///
/// The `\\?\` verbatim prefix is stripped: `std::fs::canonicalize` produces it on Windows,
/// and it is an implementation detail of path resolution rather than something a caller
/// should ever see or have to strip itself.
///
/// This lives here, beside the record definitions, because four modules previously carried
/// identical private copies and half of them forgot to normalise — which is precisely the
/// defect this replaces.
pub fn relative_display(root: &Path, path: &Path) -> String {
    let rendered = match path.strip_prefix(root) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => path.to_string_lossy().into_owned(),
    };
    normalize_path_string(&rendered)
}

/// Forward-slash a path string and drop any `\\?\` verbatim prefix.
pub fn normalize_path_string(s: &str) -> String {
    s.strip_prefix(r"\\?\").unwrap_or(s).replace('\\', "/")
}

/// `std::fs::canonicalize` with Windows' verbatim prefix removed.
///
/// On Windows the standard call returns an extended-length path: `\\?\C:\...`, or
/// `\\?\UNC\server\share\...` for a network location such as a mapped drive. Those forms do
/// not compare equal to the ordinary paths aikit holds elsewhere, so a `strip_prefix` of a
/// canonicalized child against a non-canonicalized root silently fails — which turns a
/// containment check into a fall-through, and a repo-relative record path into an absolute
/// one. Both are failures that look like something else.
///
/// Every canonicalization goes through here so the two sides of a comparison are always the
/// same shape. On Unix it is exactly `fs::canonicalize`.
pub fn canonicalize(path: impl AsRef<Path>) -> std::io::Result<std::path::PathBuf> {
    let resolved = std::fs::canonicalize(path)?;
    if !cfg!(windows) {
        return Ok(resolved);
    }
    let s = resolved.to_string_lossy();
    // `\\?\UNC\server\share` denotes `\\server\share`; dropping only the `\\?\` would
    // leave the bogus `UNC\server\share`.
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return Ok(std::path::PathBuf::from(format!(r"\\{rest}")));
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        return Ok(std::path::PathBuf::from(rest));
    }
    Ok(resolved.clone())
}

/// Write `body` plus a trailing newline to `path` **atomically**: fully to a sibling
/// temporary, synced, then renamed over the destination.
///
/// Every file aikit writes here is a record something else will later read and trust — an
/// anchor, a manifest, an inventory, a run record. A reader cannot distinguish a truncated
/// record from one that genuinely contained less, so a crash, a full disk, or a scanner
/// reading mid-write would produce a document that parses as corrupt rather than as
/// "this did not finish". Rename within a directory is atomic on both platforms, so a reader
/// sees the complete record or no record.
///
/// This lives here beside the record definitions because three modules previously carried
/// near-identical private copies and only one of them was atomic — the same drift that
/// `relative_display` was consolidated to end.
pub fn write_with_newline(path: &Path, body: &str) -> std::io::Result<()> {
    use std::io::Write;

    let tmp = path.with_extension("tmp");
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(body.as_bytes())?;
        file.write_all(b"\n")?;
        // Flush to the OS before renaming: the rename can otherwise become visible while the
        // contents are still buffered, reintroducing the torn read this exists to prevent.
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        // Leave nothing behind on failure; a stray `.tmp` beside a missing record looks like
        // an artifact of the operation rather than a failure to complete it.
        let _ = std::fs::remove_file(&tmp);
    })
}

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
pub const KIND_BATCH_LIST: &str = "aikit.batch_list";
pub const KIND_BATCH_SHOW: &str = "aikit.batch_show";
pub const KIND_DIFF_ANCHOR: &str = "aikit.diff_anchor";
pub const KIND_ENV_SNAPSHOT: &str = "aikit.env_snapshot";
pub const KIND_SCAN_SECRETS: &str = "aikit.scan_secrets";
pub const KIND_VERSION: &str = "aikit.version";
/// Emitted on stdout when a command fails and `--json` was requested, so a machine caller
/// gets structured output on the failure path rather than only prose on stderr.
pub const KIND_ERROR: &str = "aikit.error";
pub const KIND_CONFIG_SHOW: &str = "aikit.config_show";

/// A point-in-time anchor written by `aikit batch start`.
///
/// The anchor is a **minimal timestamp reference**: anchor-based changed-file discovery
/// is timestamp-based against the anchor file's mtime, so the anchor deliberately does
/// NOT capture Git status. It records only identifying/timestamp metadata.
///
/// `aikit_version` and `initial_snapshot` were added after the initial schema; both
/// carry `#[serde(default)]` so anchors written by older versions still deserialize
/// (an absent `aikit_version` reads as an empty string, an absent snapshot as `None`).
/// Older anchors may still contain a `git_status_porcelain` field; it is simply ignored
/// on read (serde drops unknown fields).
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchAnchor {
    pub schema_version: u32,
    pub kind: String,
    pub anchor_id: String,
    pub created_at: String,
    pub repo_root: String,
    pub git_head: String,
    pub git_branch: String,
    pub filesystem_anchor_time: String,
    /// The aikit version that created the anchor (empty for anchors from older versions).
    #[serde(default)]
    pub aikit_version: String,
    /// Optional initial file snapshot (repo-relative tracked paths at anchor time),
    /// recorded only when `batch start --snapshot` is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_snapshot: Option<Vec<String>>,
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
    /// Limitation notes (e.g., that changed-file discovery uses a best-effort filesystem
    /// mtime heuristic relative to the anchor). Present only when relevant.
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

/// Build/version metadata reported by `aikit version`. Package version is the Cargo
/// package version; this is distinct from any schema version (`schema_version`) used by
/// other records. `git_commit`, `build_profile`, and `target` are best-effort build-time
/// values and may be `null`.
#[derive(Debug, Serialize)]
pub struct VersionInfo {
    pub schema_version: u32,
    pub kind: String,
    pub name: String,
    pub version: String,
    pub git_commit: Option<String>,
    pub build_profile: Option<String>,
    pub os: String,
    pub arch: String,
    pub target: Option<String>,
    /// Reserved for a future Rust/toolchain profile string; currently always `null`.
    pub rust_profile: Option<String>,
}

/// The manifest written by `aikit review generate` alongside the text bundle.
#[derive(Debug, Serialize)]
pub struct ReviewManifest {
    pub schema_version: u32,
    pub kind: String,
    pub review_id: String,
    pub repo_root: String,
    pub git_head: String,
    /// The aikit version that generated this manifest (package version).
    pub aikit_version: String,
    pub generated_at: String,
    pub inputs: ReviewInputs,
    pub limits: ReviewLimits,
    pub files: Vec<ReviewFile>,
    /// Repo-relative bundle path: `review_bundle.txt` (directory mode) or the single
    /// output file path (single-file mode).
    pub bundle_path: String,
    /// Whether the manifest is also embedded inside the bundle text.
    pub embedded_manifest: bool,
    /// Whether a sidecar `manifest.json` was written next to the bundle.
    pub sidecar_manifest: bool,
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
    /// For anchor mode: whether enhanced discovery (untracked/ignored/deletions) was used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enhanced_discovery: Option<bool>,
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
    /// Reason the file was omitted from the bundle, or `null` when included
    /// (`max_total_bytes` for a cap).
    pub omitted_reason: Option<String>,
    /// Which cap bound this file (`file_bytes` | `file_lines` | `total_bytes`), or `null`.
    pub cap_hit: Option<String>,
    /// How the file was detected during anchor discovery (`anchor_mtime`, or `explicit`
    /// for configured include_files), or `null` for explicit-files (`--files`) mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
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
    /// Forbidden patterns the caller explicitly accepted via `--acknowledge-forbidden`.
    ///
    /// Empty for the overwhelming majority of runs. It is recorded because the guard is
    /// overridable: an override that left no trace would let the scan look like a boundary
    /// that was never crossed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acknowledged_forbidden: Vec<String>,
    pub repo_root: String,
    /// VCS managing the run root: `"git"`, `"mercurial"`, or `"none"` (non-repo
    /// `.aikit/` folder). `git_head_*` are populated only for `"git"`.
    pub vcs: String,
    /// Repo-relative path of the script as resolved (canonicalized).
    pub script_path: String,
    pub script_sha256: String,
    /// Filename of the script copy inside the run directory (retains the extension).
    pub script_copy_path: Option<String>,
    /// The resolved interpreter/runner program path (e.g. `/bin/sh`, `node`, `pwsh`).
    pub interpreter: String,
    /// Symbolic runner name chosen (`sh`, `zsh`, `bash`, `pwsh`, `powershell`, `cmd`,
    /// `python`, `python3`, `node`).
    pub detected_runner: String,
    /// How the runner was chosen: `explicit_runner` | `config` | `shebang` |
    /// `extension_map` | `default_fallback`.
    pub detection_source: String,
    /// Whether a `#!` shebang line selected the runner.
    pub used_shebang: bool,
    /// Whether the extension map (config or built-in) selected the runner.
    pub used_extension_map: bool,
    /// The full argv used to execute the script (program, flags, then the script path).
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
    /// How far `exit_code` can be trusted: `interpreter` | `epilogue` | `unpropagated`.
    ///
    /// `unpropagated` means the runner (PowerShell, cmd) can exit 0 despite a failed
    /// command and aikit did not append a propagation, because `script run` executes the
    /// caller's own file rather than one it generated. A caller gating on `exit_code == 0`
    /// needs this to know whether the 0 is load-bearing. See `policy::script::exit_code_source`.
    pub exit_code_source: String,
    /// Set when the run was blocked before execution; `null` otherwise.
    pub blocked_state: Option<String>,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,
    /// Older run directories deleted by retention after this run, if any.
    ///
    /// Recorded because retention deletes audit trails: a caller that finds an earlier run
    /// missing should be able to see that aikit removed it deliberately, rather than
    /// concluding the record was never written. Omitted when nothing was pruned, which is
    /// the overwhelming majority of runs.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pruned_runs: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// The report written to stdout by `aikit script check`. The command executes
/// nothing and creates no run output, so `executed` and `output_created` are always
/// false; `accepted` and `blocked_state` carry the policy verdict.
#[derive(Debug, Serialize)]
pub struct ScriptCheck {
    pub schema_version: u32,
    pub kind: String,
    /// Forbidden patterns the caller explicitly accepted via `--acknowledge-forbidden`.
    ///
    /// `script check` accepts the flag and its help promises that every acknowledgement is
    /// recorded, so the check record has to carry it too — otherwise the promise holds only
    /// for `script run`, and a caller auditing what was waived during validation sees
    /// nothing. Empty for the overwhelming majority of checks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acknowledged_forbidden: Vec<String>,
    /// Repo root, or `null` when the repo could not be detected.
    pub repo_root: Option<String>,
    /// Repo-relative path when resolved, else the path as supplied on the command line.
    pub script_path: String,
    /// Real (canonicalized) path when resolution succeeded; `null` otherwise.
    pub resolved_script_path: Option<String>,
    /// Interpreter that would be used; `null` when extension/location was not accepted.
    pub interpreter: Option<String>,
    /// Symbolic runner name that would be used; `null` when no runner was resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_runner: Option<String>,
    /// How the runner was chosen, or `null` when none was resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_source: Option<String>,
    /// Whether a shebang selected the runner (false when no runner resolved).
    pub used_shebang: bool,
    /// Whether the extension map selected the runner (false when no runner resolved).
    pub used_extension_map: bool,
    /// The argv that would be used, or `null` when no runner was resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argv: Option<Vec<String>>,
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

/// Availability of a supported script runner for `repo doctor`. `applicable` is whether
/// the runner can apply to the current OS (e.g. `cmd`/`powershell` are Windows-only);
/// `available` is whether its program was found on this system.
#[derive(Debug, Serialize)]
pub struct RunnerStatus {
    pub name: String,
    pub available: bool,
    pub applicable: bool,
    /// Absolute path of the resolved interpreter, or null when none was found. Two hosts
    /// both reporting `powershell: available` may be running 5.1 and 7.
    pub path: Option<String>,
    /// Best-effort version string, or null when the interpreter does not report one.
    /// Null means "not determined", never "old" or "broken".
    pub version: Option<String>,
    /// Stable token explaining `available`: `available` | `available_via_alias` |
    /// `not_found_on_path` | `not_applicable_on_this_os`. Lets a caller distinguish "install
    /// it" from "this OS never has it" without parsing prose. `available_via_alias` means
    /// only a Windows App Execution Alias was found — which may be a working Store install
    /// or a stub that opens the Store and exits 9009.
    pub reason: String,
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

/// An anchor view (the anchor's fields plus its repo-relative path) for `batch list`
/// and `batch show`.
#[derive(Debug, Serialize)]
pub struct AnchorView {
    pub schema_version: u32,
    pub kind: String,
    pub anchor_id: String,
    /// Repo-relative path of the anchor file.
    pub path: String,
    pub created_at: String,
    pub repo_root: String,
    pub git_branch: String,
    pub git_head: String,
    pub filesystem_anchor_time: String,
    /// The aikit version that created the anchor (empty for anchors from older versions).
    pub aikit_version: String,
    /// Number of paths in the recorded initial snapshot, or `null` when none was recorded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_snapshot_count: Option<usize>,
}

/// A file under the batch folder that could not be parsed as a valid anchor.
#[derive(Debug, Serialize)]
pub struct SkippedAnchor {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct BatchListCounts {
    pub total: usize,
    pub skipped: usize,
}

/// The report written by `aikit batch list`.
#[derive(Debug, Serialize)]
pub struct BatchList {
    pub schema_version: u32,
    pub kind: String,
    pub repo_root: String,
    pub output_root: String,
    pub generated_at: String,
    pub anchors: Vec<AnchorView>,
    pub skipped: Vec<SkippedAnchor>,
    pub counts: BatchListCounts,
    pub blocked_state: Option<String>,
}

/// The report written by `aikit batch show`.
#[derive(Debug, Serialize)]
pub struct BatchShow {
    pub schema_version: u32,
    pub kind: String,
    pub repo_root: String,
    pub anchor: AnchorView,
    /// True once the anchor has been validated as belonging to the current repo.
    pub belongs_to_repo: bool,
    pub blocked_state: Option<String>,
}

/// One file in a `diff anchor` report (from `git diff --name-status`).
#[derive(Debug, Serialize)]
pub struct DiffFile {
    pub path: String,
    /// Word form: added / modified / deleted / renamed / copied / type_changed / …
    pub status: String,
    /// Source path for renames/copies; `null` otherwise.
    pub old_path: Option<String>,
    /// The raw git status code (e.g. `M`, `A`, `R100`).
    pub raw_status: String,
}

#[derive(Debug, Serialize)]
pub struct DiffCounts {
    pub total: usize,
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub renamed: usize,
    pub copied: usize,
    pub other: usize,
}

/// The report written by `aikit batch diff`.
#[derive(Debug, Serialize)]
pub struct DiffAnchor {
    pub schema_version: u32,
    pub kind: String,
    pub repo_root: String,
    pub generated_at: String,
    pub anchor: AnchorRef,
    pub base_git_head: String,
    pub current_git_head: String,
    pub tracked_tree_clean: bool,
    pub files: Vec<DiffFile>,
    pub counts: DiffCounts,
    pub stat: String,
    pub notes: Vec<String>,
    /// Full patch text; present only when `--patch` was given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    pub blocked_state: Option<String>,
}

/// The repo-scoped facts in an `aikit env snapshot` (present only inside a Git repo).
#[derive(Debug, Serialize)]
pub struct EnvRepo {
    pub root: String,
    pub branch: String,
    pub head: String,
    pub tracked_tree_clean: bool,
    /// Repo-relative default output root (`.aikit/outputs`).
    pub default_output_root: String,
    pub aikit_dir_exists: bool,
    pub temp_dir_exists: bool,
    pub outputs_dir_exists: bool,
    pub aikit_ignored: bool,
}

/// A safe PATH summary for `env snapshot`. The raw PATH value is never emitted.
#[derive(Debug, Serialize)]
pub struct EnvPaths {
    /// Whether the current executable's directory appears on PATH (`null` when the
    /// current executable or PATH could not be determined).
    pub current_exe_dir_on_path: Option<bool>,
    /// Number of entries in PATH (a count only — never the entries themselves).
    pub path_entry_count: usize,
}

/// Local tool versions for `env snapshot` (each `null` when the tool is unavailable).
/// These are obtained from local `--version` invocations; no network calls are made.
#[derive(Debug, Serialize)]
pub struct EnvTools {
    pub git_version: Option<String>,
    pub rustc_version: Option<String>,
    pub cargo_version: Option<String>,
}

/// The mechanical local environment report written by `aikit env snapshot`.
///
/// Deliberately bounded: it reports a fixed set of debugging facts and never dumps the
/// full environment, the raw PATH, tokens, credentials, keys, or any network-derived or
/// provider-specific information.
#[derive(Debug, Serialize)]
pub struct EnvSnapshot {
    pub schema_version: u32,
    pub kind: String,
    pub generated_at: String,
    pub version: String,
    pub current_exe: Option<String>,
    /// OS family (e.g. `macos`, `linux`, `windows`) — `std::env::consts::OS`.
    pub os: String,
    /// CPU architecture (e.g. `aarch64`, `x86_64`) — `std::env::consts::ARCH`.
    pub arch: String,
    pub current_dir: String,
    /// Repo-scoped facts, or `null` when not inside a Git repository.
    pub repo: Option<EnvRepo>,
    pub paths: EnvPaths,
    pub interpreters: Vec<PathStatus>,
    pub tools: EnvTools,
    /// The shell from `$SHELL`, when set (a single value — never the whole environment).
    pub shell: Option<String>,
    pub warnings: Vec<String>,
    pub blocked_state: Option<String>,
}

/// One likely-secret finding from `aikit scan secrets`. The raw matched value is NEVER
/// included — `redacted` is always true.
#[derive(Debug, Serialize)]
pub struct ScanFinding {
    /// Repo-relative path of the file containing the match.
    pub path: String,
    /// 1-based line number of the match.
    pub line: usize,
    pub rule_id: String,
    pub description: String,
    /// `high` | `medium` | `low`.
    pub severity: String,
    /// Always true — raw secret values are never emitted.
    pub redacted: bool,
}

/// A file skipped by `aikit scan secrets`, with the reason (`binary`, `too_large`,
/// `unreadable`).
#[derive(Debug, Serialize)]
pub struct ScanSkipped {
    pub path: String,
    pub reason: String,
}

/// Finding/file counts for `aikit scan secrets`.
#[derive(Debug, Serialize)]
pub struct ScanCounts {
    pub findings: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub files_scanned: usize,
    pub files_skipped: usize,
}

/// The report written by `aikit scan secrets`. Best-effort and heuristic: it can
/// false-positive and false-negative, never proves a file is safe to share, and never
/// includes raw secret values.
#[derive(Debug, Serialize)]
pub struct ScanSecrets {
    pub schema_version: u32,
    pub kind: String,
    pub repo_root: String,
    pub generated_at: String,
    /// The explicit input paths as supplied on the command line.
    pub inputs: Vec<String>,
    pub include_ignored: bool,
    pub max_file_bytes: u64,
    pub files_scanned: usize,
    pub files_skipped: Vec<ScanSkipped>,
    pub findings: Vec<ScanFinding>,
    pub counts: ScanCounts,
    /// The `--fail-on` threshold in effect (`high` | `medium` | `low`), or `null` when the
    /// scan was reporting only — which is the default.
    pub fail_on: Option<String>,
    pub blocked_state: Option<String>,
}

/// The report written by `aikit init --require-repo`. `repo init` only blocks on
/// `blocked_repo_not_found` (handled as an error), so `blocked_state` is `null` here.
#[derive(Debug, Serialize)]
pub struct RepoInit {
    pub schema_version: u32,
    pub kind: String,
    pub repo_root: String,
    /// Version-control system managing the repo: `"git"` or `"mercurial"`.
    pub vcs: String,
    /// Repo-relative aikit directory (`.aikit`).
    pub aikit_dir: String,
    /// Repo-relative temp directory (`.aikit/temp`).
    pub temp_dir: String,
    /// Repo-relative directories created during this run (empty when idempotent).
    pub created_dirs: Vec<String>,
    /// Whether `.aikit/` is ignored after this run.
    pub aikit_ignored: bool,
    /// The ignore source covering `.aikit/` (e.g. `.gitignore`, `.git/info/exclude`,
    /// `.hgignore`, `.hg/hgignore.aikit`).
    pub ignore_source: Option<String>,
    /// Whether the local-only VCS exclude was updated this run (git:
    /// `.git/info/exclude`; mercurial: `.hg/hgignore.aikit` + `.hg/hgrc`).
    pub info_exclude_updated: bool,
    /// Human-readable action log (created / already-present / ignore actions).
    pub actions: Vec<String>,
    pub blocked_state: Option<String>,
}

/// The read-only readiness report written by `aikit doctor --require-root`. `repo doctor` only
/// blocks on `blocked_repo_not_found`, so `blocked_state` is `null` here.
#[derive(Debug, Serialize)]
pub struct RepoDoctor {
    pub schema_version: u32,
    pub kind: String,
    pub repo_root: String,
    /// How `repo_root` was arrived at: `"marker"` when a `.git`, `.hg`, or `.aikit` marker
    /// was found, or `"cwd_no_marker"` when there was none and the report is against the
    /// current directory.
    ///
    /// Without this a caller cannot tell an anchored root from an unanchored one, since
    /// both are just a path. Only `aikit doctor` can produce `"cwd_no_marker"`;
    /// `aikit doctor --require-root` still blocks in that case.
    pub root_source: String,
    /// VCS managing the root: `"git"`, `"mercurial"`, or `"none"` (non-repo folder).
    pub vcs: String,
    /// Current branch/bookmark label (VCS-specific; empty for a non-repo folder or when
    /// the CLI is unavailable). For Mercurial this is the named branch with the active
    /// bookmark appended. Field name kept for schema stability.
    pub git_branch: String,
    /// Current HEAD/commit id (Git HEAD, or the Mercurial working-dir parent node; empty
    /// for a non-repo folder or an unborn repo). Field name kept for schema stability.
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
    /// Shell interpreter probe (`/bin/sh`, `/bin/zsh`), retained for compatibility and
    /// informational only. Readiness no longer depends on these; see `runners`.
    pub interpreters: Vec<PathStatus>,
    /// Availability of every supported script runner on the current OS.
    pub runners: Vec<RunnerStatus>,
    /// Whether at least one supported runner is available for the current OS.
    pub any_runner_available: bool,
    pub current_exe: Option<String>,
    pub version: String,
    pub warnings: Vec<String>,
    pub ready: bool,
    pub blocked_state: Option<String>,
}
