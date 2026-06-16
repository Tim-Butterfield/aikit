//! Command-line interface definition and help text.
//!
//! Help text is part of the product: it must let a human or an AI agent decide
//! when and how to call each command. Each command documents its purpose, when to
//! use it, key flags, default output behavior, JSON behavior, and an example.

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "aikit",
    version,
    about = "Deterministic, local, mechanical operations that support AI-agent and human workflows.",
    long_about = "aikit performs deterministic, local, mechanical operations that support \
AI-agent and human-in-the-loop workflows. It does not call AI providers, performs no \
model/provider logic, and has no knowledge of any specific AI agent, CLI, slash command, \
or model. Commands operate on the current Git repository and write machine-readable output \
where useful.",
    after_help = "Examples:\n  \
aikit batch start\n  \
aikit batch changed --anchor .aikit/outputs/batches/<anchor-id>.json\n\n\
Exit codes: 0 success, 1 command failure, 2 invalid usage, 3 blocked state."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create batch anchors and report what changed since one.
    #[command(
        long_about = "Batch anchors mark a point in time before AI-agent work begins, and \
report what was created or modified since. Use `batch start` to create an anchor, then \
`batch changed --anchor <file>` to list changes.\n\n\
`batch start` writes a JSON anchor under the local output directory \
(.aikit/outputs/batches/ by default; override with --output <dir>); output is local-only \
and never needs committing. Both subcommands support --json for machine-readable output.",
        after_help = "Examples:\n  \
aikit batch start\n  \
aikit batch changed --anchor .aikit/outputs/batches/<anchor-id>.json --json"
    )]
    Batch(BatchCli),

    /// Mechanical Git diff reports (e.g. from a batch anchor).
    #[command(
        long_about = "Mechanical Git diff reports. `diff anchor <anchor>` diffs a batch \
anchor's recorded Git head against the current working tree. Inspection only: it creates \
no review bundle or output artifact, advances no workflow state, and never touches \
remotes.",
        after_help = "Examples:\n  \
aikit diff anchor <anchor-id>\n  \
aikit diff anchor <anchor-id> --json"
    )]
    Diff(DiffCli),

    /// Report mechanical local environment facts (for debugging aikit usage).
    #[command(
        long_about = "Report mechanical local environment facts useful for debugging aikit \
usage. `env snapshot` prints a bounded, fixed set of facts — aikit version, current \
executable, OS family, CPU architecture, working directory, repo facts when inside a Git \
repository, legacy/informational shell-interpreter probes (`/bin/sh`, `/bin/zsh`), and \
local git/Rust/Cargo versions. These shell probes are informational only and are NOT the \
cross-OS runner-availability/readiness model used by `repo doctor`.\n\n\
It is read-only: it creates no files or directories, modifies no repo files, touches no \
remotes, and runs no network commands. It deliberately does NOT dump all environment \
variables, the raw PATH, tokens, credentials, keys, or any provider/model-specific or \
network-derived information; PATH is summarized only (entry count and whether the current \
executable's directory is on it). Supports human output and `--json`.",
        after_help = "Examples:\n  \
aikit env snapshot\n  \
aikit env snapshot --json"
    )]
    Env(EnvCli),

    /// Best-effort local heuristic scan for likely secrets in explicit paths.
    #[command(
        long_about = "Run a local, best-effort heuristic scan for likely secrets in \
explicit repo-local paths. You must pass at least one path; the whole repository is never \
scanned implicitly unless you pass the repo root or `.`.\n\n\
Scanning is heuristic: it can false-positive and false-negative, does not prove a file or \
repo is safe to share, does not judge whether a finding is a live credential, and does not \
replace dedicated secret-scanning tools. It NEVER prints raw secret values in human or \
JSON output — only the file path, line number, rule id, description, and severity. It \
creates no output artifacts by default and never touches remotes. By default findings are reported \
and the command still exits 0; `--fail-on-findings` exits 3 (blocked_secret_findings) when \
findings are present.",
        after_help = "Examples:\n  \
aikit scan secrets README.md docs\n  \
aikit scan secrets . --fail-on-findings\n  \
aikit scan secrets src --json --include-ignored"
    )]
    Scan(ScanCli),

    /// Generate a mechanical inventory of repository files.
    #[command(
        long_about = "Generate a mechanical inventory of repository files: a deterministic, \
hashed listing of every included file. Use it to capture a reproducible snapshot of repo \
contents for review or comparison. Traversal is gitignore-aware and always excludes `.git/` \
and common build/dependency/output directories (matched by directory name, not substring).\n\n\
The `repo` subcommand writes inventory.json and inventory.txt under the local output \
directory (.aikit/outputs/inventory/<id>/ by default; override with --output <dir>); output \
is local-only. Key flags (on `inventory repo`): --json (also print JSON to stdout, including \
the created file paths), --include-ignored (include .gitignore'd files; always-excluded dirs \
still apply), --max-files <n> (limit deterministically after sorting), and --output <dir> \
(override the output root).",
        after_help = "Example:\n  \
aikit inventory repo\n  \
aikit inventory repo --json --include-ignored --max-files 500"
    )]
    Inventory(InventoryCli),

    /// List, show, and clean local aikit output artifacts.
    #[command(
        long_about = "List, show, and clean local aikit output artifacts under an output \
root (default `.aikit/outputs/`). Only known artifacts are recognized: `batches/*.json` \
files and `inventory/`, `reviews/`, and `runs/` subdirectories.\n\n\
`output list` and `output show` are read-only. `output clean` is dry-run by default and \
deletes only with `--execute` plus a selector (`--older-than` or `--all`); it never \
deletes outside the output root, never follows symlink escapes, and never touches \
`.aikit/temp/`, `.scratch/`, `.claude/`, `target/`, or `.git/`.",
        after_help = "Examples:\n  \
aikit output list\n  \
aikit output show <artifact-path-or-id>\n  \
aikit output clean --dry-run\n  \
aikit output clean --all --execute"
    )]
    Output(OutputCli),

    /// Generate a bounded review bundle from explicit files or a batch anchor.
    #[command(
        long_about = "Generate a bounded, hashed review bundle for AI/human review. Package a \
set of files into a single reviewable text bundle plus a manifest, with deterministic \
ordering and size caps so the surface stays bounded.\n\n\
`review generate` accepts exactly one input mode: `--files <file>...` (explicit files) or \
`--anchor <anchor.json>` (the files changed since a batch anchor). Supplying both, or \
neither, is invalid usage. The precomputed `--changed <changed.json>` mode is not \
implemented. Key flags: --max-file-bytes / --max-file-lines (truncate a file and record \
it), --max-total-bytes (omit later files once the running total is exceeded), --output \
<dir> (override the output root), and --json (also print the manifest JSON to stdout).\n\n\
`review generate` writes review_bundle.txt and manifest.json under the default output \
directory .aikit/outputs/reviews/<id>/; override with --output <dir>. `.scratch` is never \
used by default and is available only via an explicit `--output .scratch/...`. Created \
artifact paths are printed; output is local-only.",
        after_help = "Example:\n  \
aikit review generate --files src/main.rs README.md\n  \
aikit review generate --anchor .aikit/outputs/batches/<anchor-id>.json --json"
    )]
    Review(ReviewCli),

    /// Prepare and inspect repo-local aikit setup.
    #[command(
        long_about = "Prepare and inspect the current repository's local aikit setup. \
`repo init` creates the local working area (`.aikit/temp/`) and ensures `.aikit/` is \
locally ignored; `repo doctor` reports readiness without changing anything.\n\n\
Neither command touches remote Git state, runs build/test/review commands, or modifies \
`.gitignore`. `repo init` uses `.git/info/exclude` (local Git metadata) for ignore \
coverage so it does not dirty tracked project files.",
        after_help = "Examples:\n  \
aikit repo doctor\n  \
aikit repo init\n  \
aikit repo doctor --json"
    )]
    Repo(RepoCli),

    /// Validate and run local scripts under mechanical safety controls.
    #[command(
        long_about = "Validate and run local scripts under mechanical safety controls. This \
is NOT a security sandbox: it reduces accidental unsafe execution but does not make an \
arbitrary script safe.\n\n\
`script run <script-path>` runs the script through its detected runner and records an \
audit trail; `script check <script-path>` applies the same policy but does not execute \
and writes nothing. The script must live under an allowed local work area (.aikit/temp/, \
.scratch/work/temp/, or .scratch/work/outputs/) — those are input locations, not output \
locations.\n\n\
Cross-OS runner detection is deterministic and OS-aware. Supported extensions: .sh, .zsh, \
.ps1, .cmd, .bat, .py, .js. Runner names: sh, zsh, bash, pwsh, powershell, cmd, python, \
python3, node. Selection order: (1) explicit `--runner <name>`; (2) config \
`script_runner.extension_map` for the extension; (3) a recognized `#!` shebang (unless \
`--no-shebang` or `detect_from_shebang=false`); (4) the built-in extension map; (5) an \
OS-aware default fallback; else a clear blocked failure. `script_runner.preferred_runners` \
reorders candidates, and unknown configured runner names fail clearly. On Windows, .ps1 \
uses pwsh/powershell and .cmd/.bat use cmd with no Git Bash required; .sh/.zsh run only \
when a discoverable interpreter exists. Blocked states: blocked_unknown_script_type, \
blocked_runner_not_found, blocked_runner_not_allowed. For `script run`, the run record \
(copied script, stdout.txt, stderr.txt, run.json with detected_runner / detection_source \
/ used_shebang / used_extension_map / argv) is written under .aikit/outputs/runs/<id>/; \
override with --output <dir> (`.scratch` output only when requested explicitly).",
        after_help = "Examples:\n  \
aikit script check .aikit/temp/build.sh\n  \
aikit script run .aikit/temp/build.sh\n  \
aikit script run .aikit/temp/task.py --runner python3\n  \
aikit script run .scratch/work/temp/task.zsh --print"
    )]
    Script(ScriptCli),

    /// Report aikit's version and build metadata.
    #[command(
        long_about = "Report aikit's version and build metadata. The package/binary \
version is the Cargo package version (the same string as `aikit --version`); it is \
distinct from the per-record `schema_version` used by other artifacts.\n\n\
Human output is compact. With --json, emits a machine-readable record \
(`aikit.version`) with name, version, git_commit, build_profile, os, arch, target, and \
rust_profile. git_commit/build_profile/target are best-effort build-time values and may \
be null. Read-only; creates nothing and works outside a Git repository.",
        after_help = "Examples:\n  \
aikit version\n  \
aikit version --json\n  \
aikit --version"
    )]
    Version(VersionArgs),
}

#[derive(Debug, Args)]
pub struct VersionArgs {
    /// Print the machine-readable version record to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct InventoryCli {
    #[command(subcommand)]
    pub command: InventoryCommand,
}

#[derive(Debug, Subcommand)]
pub enum InventoryCommand {
    /// Inventory the files in the current repository.
    #[command(
        long_about = "Inventory the files in the current Git repository. Walks the repo with \
gitignore-aware traversal, always excluding `.git/` and common build/dependency/output \
directories (matched by directory name, not by substring). For each included file it records \
the repo-relative path, size, SHA-256, and a simple extension-based kind hint.\n\n\
When to use: to capture a deterministic, hashed snapshot of repo contents for review or to \
compare repo state over time.\n\n\
By default, files ignored by .gitignore are excluded; pass --include-ignored to include them \
(the always-excluded directories above are still excluded). Output (inventory.json + \
inventory.txt) is written under the local output directory \
.aikit/outputs/inventory/<id>/ by default; override the root with --output <dir>. With \
--json the inventory is also printed to stdout as machine-readable JSON, including a \
`written` array of the created file paths. --max-files <n> limits the listing \
deterministically (after sorting) and records the limitation.",
        after_help = "Examples:\n  \
aikit inventory repo\n  \
aikit inventory repo --json\n  \
aikit inventory repo --include-ignored --max-files 500"
    )]
    Repo(InventoryRepoArgs),
}

#[derive(Debug, Args)]
pub struct InventoryRepoArgs {
    /// Override the output directory root (default: .aikit/outputs; pass .scratch/... to use scratch).
    #[arg(long, value_name = "DIR")]
    pub output: Option<String>,

    /// Print machine-readable JSON to stdout in addition to writing the files.
    #[arg(long)]
    pub json: bool,

    /// Include files ignored by .gitignore (always-excluded directories still excluded).
    #[arg(long)]
    pub include_ignored: bool,

    /// Limit the inventory to the first N files after deterministic sorting.
    #[arg(long, value_name = "N")]
    pub max_files: Option<usize>,
}

/// A known aikit output family.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFamily {
    Batches,
    Inventory,
    Reviews,
    Runs,
}

#[derive(Debug, Args)]
pub struct OutputCli {
    #[command(subcommand)]
    pub command: OutputCommand,
}

#[derive(Debug, Subcommand)]
pub enum OutputCommand {
    /// List local aikit output artifacts (read-only).
    #[command(
        long_about = "List local aikit output artifacts under the selected output root \
(default `.aikit/outputs/`). Read-only: creates and deletes nothing. Only known \
artifacts are listed — `batches/*.json` files and `inventory/`, `reviews/`, and `runs/` \
subdirectories — sorted by family then artifact id. If the output root does not exist, \
the list is empty (success). Each row reports family, id, path, size, and modified time. \
Supports `--family <batches|inventory|reviews|runs>`, `--root <path>`, and `--json`.",
        after_help = "Examples:\n  \
aikit output list\n  \
aikit output list --family runs --json"
    )]
    List(OutputListArgs),

    /// Show details for one local aikit output artifact (read-only).
    #[command(
        long_about = "Show details for one explicit local aikit output artifact \
(read-only; creates and deletes nothing). The argument is an artifact path under the \
output root or an artifact id; an id is matched against the known family folders \
(batches/inventory/reviews/runs). Ambiguous ids and paths that resolve outside the \
output root are rejected; a missing artifact is reported as a clear blocked state. \
Reports the artifact family/id/path, the files it contains, and a compact summary of its \
main JSON (run.json / manifest.json / inventory.json / the batch anchor). This command \
makes no judgment about correctness. Supports `--root <path>` and `--json`.",
        after_help = "Examples:\n  \
aikit output show <artifact-path-or-id>\n  \
aikit output show .aikit/outputs/runs/<id> --json"
    )]
    Show(OutputShowArgs),

    /// Clean local aikit output artifacts (dry-run by default; --execute to delete).
    #[command(
        long_about = "Clean local aikit output artifacts under the selected output root. \
SAFE BY DEFAULT: dry-run unless `--execute` is given, and `--execute` requires a selector \
(`--older-than <duration>` or `--all`). With neither selector, all candidates are listed \
in dry-run and nothing is deleted. Deletion removes only known artifacts \
(`batches/*.json` files and `inventory/`/`reviews/`/`runs/` subdirectories) inside the \
output root; it never deletes outside the root, never follows symlink escapes, and never \
touches `.aikit/temp/`, `.scratch/`, `.claude/`, `target/`, or `.git/`.\n\n\
`--older-than` takes a simple duration: `<n>h` (hours) or `<n>d` (days), e.g. `24h` or \
`7d`. `--older-than` and `--all` cannot be combined. Supports `--family`, `--root`, and \
`--json`.",
        after_help = "Examples:\n  \
aikit output clean --dry-run\n  \
aikit output clean --older-than 7d --dry-run\n  \
aikit output clean --older-than 7d --execute\n  \
aikit output clean --all --execute"
    )]
    Clean(OutputCleanArgs),
}

#[derive(Debug, Args)]
pub struct OutputListArgs {
    /// Only list this output family.
    #[arg(long, value_enum)]
    pub family: Option<OutputFamily>,

    /// Output root to inspect (default: .aikit/outputs; an explicit root must be under
    /// .aikit/outputs/ or .scratch/work/outputs/).
    #[arg(long, value_name = "PATH")]
    pub root: Option<String>,

    /// Print the machine-readable list to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct OutputShowArgs {
    /// Artifact to show: a path under the output root, or an artifact id.
    #[arg(value_name = "ARTIFACT")]
    pub artifact: String,

    /// Output root to inspect (default: .aikit/outputs; an explicit root must be under
    /// .aikit/outputs/ or .scratch/work/outputs/).
    #[arg(long, value_name = "PATH")]
    pub root: Option<String>,

    /// Print the machine-readable details to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
#[command(group(ArgGroup::new("selector").args(["older_than", "all"])))]
pub struct OutputCleanArgs {
    /// Only clean this output family.
    #[arg(long, value_enum)]
    pub family: Option<OutputFamily>,

    /// Output root to clean (default: .aikit/outputs; an explicit root must be under
    /// .aikit/outputs/ or .scratch/work/outputs/).
    #[arg(long, value_name = "PATH")]
    pub root: Option<String>,

    /// Show what would be deleted without deleting (this is the default).
    #[arg(long, conflicts_with = "execute")]
    pub dry_run: bool,

    /// Actually delete the selected artifacts (requires --older-than or --all).
    #[arg(long, requires = "selector")]
    pub execute: bool,

    /// Only clean artifacts older than this duration: <n>h (hours) or <n>d (days).
    #[arg(long, value_name = "DURATION")]
    pub older_than: Option<String>,

    /// Select all known output artifacts (mutually exclusive with --older-than).
    #[arg(long)]
    pub all: bool,

    /// Print the machine-readable clean report to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ReviewCli {
    #[command(subcommand)]
    pub command: ReviewCommand,
}

#[derive(Debug, Subcommand)]
pub enum ReviewCommand {
    /// Generate a review bundle from explicit files or from a batch anchor.
    #[command(
        long_about = "Generate a review bundle from one input mode: explicit files \
(`--files <file>...`) or the files changed since a batch anchor (`--anchor <anchor.json>`). \
Exactly one mode must be used; supplying both, or neither, is invalid usage. The \
precomputed `--changed <changed.json>` mode is not implemented.\n\n\
With `--files`, each path is resolved relative to the repository root and must resolve \
(after symlink resolution) to a real path inside the repo; paths that escape the repo are \
rejected. With `--anchor`, the changed files since the anchor are computed with the same \
logic as `batch changed` (the anchor must exist, be a valid batch anchor, and belong to \
this repo); changed files are bundled and unchanged files are excluded.\n\n\
In both modes files are sorted by repo-relative path, hashed (SHA-256), and packaged into \
review_bundle.txt plus a manifest.json. Caps keep the bundle bounded: --max-file-bytes and \
--max-file-lines truncate individual files (recording truncation and the bound), and \
--max-total-bytes omits later files once the running total would be exceeded (recording \
omitted_reason/cap_hit). Every scoped file appears exactly once in the manifest whether \
included, truncated, or omitted.\n\n\
Output (review_bundle.txt + manifest.json) is written under the default local output \
directory .aikit/outputs/reviews/<id>/; override the root with --output <dir> (pass a \
.scratch/... path to use scratch, which is never used by default). Created artifact paths \
are printed in human output; with --json the manifest is printed to stdout including a \
`written` array of the created file paths.\n\n\
Output shape (defaults preserve legacy behavior): --single-file writes exactly one bundle \
file with the manifest embedded and no review directory or sidecar manifest.json (default \
file path tmp/review_bundle.txt; override with --output <file>). --embed-manifest embeds \
the manifest in the bundle text without changing the directory layout; \
--no-sidecar-manifest suppresses the sidecar manifest.json. Enhanced anchor discovery \
(--include-ignored-batch-files) additionally bundles untracked non-ignored files and \
allowlisted ignored files modified after the anchor (per include/exclude globs) and \
records tracked deletions in the manifest. Defaults for any of these can be set in \
aikit.config.json or .aikit/config.json (CLI flags take precedence); see \
aikit.config.example.json.",
        after_help = "Examples:\n  \
aikit review generate --files src/main.rs README.md\n  \
aikit review generate --anchor .aikit/outputs/batches/<anchor-id>.json --json\n  \
aikit review generate --anchor <anchor.json> --single-file --include-ignored-batch-files\n  \
aikit review generate --anchor <anchor.json> --single-file --output tmp/review_bundle.txt"
    )]
    Generate(ReviewGenerateArgs),
}

#[derive(Debug, Args)]
#[command(group(ArgGroup::new("input").required(true).args(["files", "anchor"])))]
pub struct ReviewGenerateArgs {
    /// Explicit files to include, resolved relative to the repo root (one or more).
    #[arg(long, value_name = "FILE", num_args = 1..)]
    pub files: Vec<String>,

    /// Bundle the files changed since this batch anchor (mutually exclusive with --files).
    #[arg(long, value_name = "ANCHOR_JSON")]
    pub anchor: Option<String>,

    /// Override the output location. Directory mode: the output root (default
    /// .aikit/outputs; pass .scratch/... to use scratch). Single-file mode: the bundle
    /// file path (default tmp/review_bundle.txt).
    #[arg(long, value_name = "PATH")]
    pub output: Option<String>,

    /// Write exactly one bundle file (embedded manifest, no review directory, no sidecar manifest.json).
    #[arg(long)]
    pub single_file: bool,

    /// Embed the manifest inside the bundle text (implied by --single-file).
    #[arg(long)]
    pub embed_manifest: bool,

    /// Do not write a sidecar manifest.json (directory mode only; single-file never writes one).
    #[arg(long)]
    pub no_sidecar_manifest: bool,

    /// Enhanced anchor discovery: also bundle untracked non-ignored files and
    /// allowlisted ignored files modified after the anchor, and record tracked deletions.
    #[arg(long)]
    pub include_ignored_batch_files: bool,

    /// Truncate each file's embedded content to at most N bytes.
    #[arg(long, value_name = "N")]
    pub max_file_bytes: Option<u64>,

    /// Omit later files once the running included-bytes total would exceed N.
    #[arg(long, value_name = "N")]
    pub max_total_bytes: Option<u64>,

    /// Truncate each file's embedded content to at most N lines.
    #[arg(long, value_name = "N")]
    pub max_file_lines: Option<usize>,

    /// Print the machine-readable manifest JSON to stdout in addition to writing files.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct RepoCli {
    #[command(subcommand)]
    pub command: RepoCommand,
}

#[derive(Debug, Subcommand)]
pub enum RepoCommand {
    /// Prepare the current repository for local aikit usage (idempotent).
    #[command(
        long_about = "Prepare the current Git repository for local aikit usage. Creates \
`.aikit/` and `.aikit/temp/` if missing, and ensures `.aikit/` is locally ignored.\n\n\
Ignore coverage is added to `.git/info/exclude` (local Git metadata that is never \
staged), NOT to `.gitignore`, so the command does not dirty tracked project files. If \
`.aikit/` is already ignored by any Git ignore source, no duplicate entry is added.\n\n\
The command is idempotent: it creates `.aikit/temp/` only if missing and adds the ignore \
entry only if needed. It creates no output artifacts, does not create `.scratch/` or \
`.claude/`, runs no build/test/review commands, and never touches remote Git state. It \
reports what was already present and what was created (and `--json` for machine output).",
        after_help = "Examples:\n  \
aikit repo init\n  \
aikit repo init --json"
    )]
    Init(RepoInitArgs),

    /// Report repo-local aikit readiness (read-only; mutates nothing).
    #[command(
        long_about = "Report repo-local aikit readiness without changing anything. This \
command is read-only: it creates no files or directories (no `.aikit/`, `.scratch/`, \
`.claude/`, or `.aikit/outputs/`) and does not modify `.gitignore` or \
`.git/info/exclude`.\n\n\
It reports the repo root, branch, HEAD, tracked-tree clean/dirty state, whether \
`.aikit/`, `.aikit/temp/`, and `.aikit/outputs/` exist, whether `.aikit/` is ignored \
(and the ignore source), the default output root, allowed script input locations (and \
whether each exists), the aikit version, any warnings, and an overall readiness \
summary.\n\n\
Runner availability: it reports `runners` (each supported script runner — sh, bash, zsh, \
pwsh, powershell, cmd, python3, python, node — with `available` and OS `applicable` \
flags) and `any_runner_available`. Readiness means sane local aikit state (`.aikit/temp/` \
present and `.aikit/` ignored) AND at least one supported runner available for the \
current OS; it does NOT require any specific Unix shell (Windows is ready with \
pwsh/cmd, and a host without zsh is still ready). The legacy `/bin/sh` and `/bin/zsh` \
probes are reported as informational shell interpreters only and do not gate \
readiness.\n\n\
Exit 0 when a repository is found, even with warnings (missing `.aikit/temp/`, ignore \
coverage, or no available runner are warnings, not failures); only being outside a Git \
repository is an error.",
        after_help = "Examples:\n  \
aikit repo doctor\n  \
aikit repo doctor --json"
    )]
    Doctor(RepoDoctorArgs),
}

#[derive(Debug, Args)]
pub struct RepoInitArgs {
    /// Print the machine-readable init record to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct RepoDoctorArgs {
    /// Print the machine-readable readiness record to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ScriptCli {
    #[command(subcommand)]
    pub command: ScriptCommand,
}

#[derive(Debug, Subcommand)]
pub enum ScriptCommand {
    /// Run a local script under mechanical safety controls (not a security sandbox).
    #[command(
        long_about = "Run a local script through its detected runner and write an audit \
record. NOTE: this is NOT a security sandbox. The allowed-location policy is the primary \
control; the forbidden-operation scan is best-effort (naive substring matching, easily \
bypassed, can false-positive), and running a script here does not make it safe.\n\n\
The <script-path> must resolve (after symlink resolution) to a real file under an allowed \
local work area: .aikit/temp/, .scratch/work/temp/, or .scratch/work/outputs/. These are \
input locations only.\n\n\
Cross-OS runner detection (deterministic, OS-aware) selects the interpreter in this \
order: (1) an explicit `--runner <name>`; (2) the config `script_runner.extension_map`; \
(3) a recognized `#!` shebang unless `--no-shebang`; (4) the built-in extension map; (5) \
an OS-aware default fallback; else a clear blocked failure. Supported extensions: .sh, \
.zsh, .ps1, .cmd, .bat, .py, .js. Runner values: sh, zsh, bash, pwsh, powershell, cmd, \
python, python3, node. On Windows, .ps1 uses pwsh/powershell and .cmd/.bat use cmd \
(no Git Bash required); .sh/.zsh run only if a discoverable interpreter exists. Unknown \
types block with blocked_unknown_script_type; a selected-but-unavailable runner blocks \
with blocked_runner_not_found; an unrecognized --runner blocks with \
blocked_runner_not_allowed. run.json records detected_runner, detection_source, \
used_shebang, used_extension_map, and the full argv.\n\n\
Clean-tree policy: the default is allow-dirty (these scripts operate on working content). \
`--require-clean` blocks when the tracked tree is dirty; `--allow-dirty` is the explicit \
default; the two cannot be combined. With `--print`, policy is validated and the planned \
command is shown but the script is not executed (recorded as executed: false). To validate \
policy without running anything and without writing a run record, use `script check`.\n\n\
On execution the script is copied into the run directory (retaining its extension), stdout \
and stderr are captured to stdout.txt / stderr.txt, and run.json records the audit metadata. \
Output is written under .aikit/outputs/runs/<id>/ by default; override with --output <dir> \
(`.scratch` output only when requested explicitly). Created artifact paths are printed (and \
included in --json). The executed script's exit code is propagated.",
        after_help = "Examples:\n  \
aikit script run .aikit/temp/build.sh\n  \
aikit script run .scratch/work/temp/task.zsh --print\n  \
aikit script run .aikit/temp/check.sh --require-clean --json"
    )]
    Run(ScriptRunArgs),

    /// Validate a local script against the run policy without executing it.
    #[command(
        long_about = "Validate a local script against the same policy `script run` uses, \
without executing it and without writing any run output. NOTE: this is NOT a security \
sandbox; it reports whether the mechanical policy accepts the script, not whether the \
script is safe.\n\n\
The <script-path> must resolve (after symlink resolution) to a real file under an allowed \
local work area: .aikit/temp/, .scratch/work/temp/, or .scratch/work/outputs/. The check \
validates the allowed location, the path/symlink boundary, cross-OS runner detection (same \
order as `script run`: --runner, config extension_map, shebang unless --no-shebang, \
built-in extension map, OS-aware fallback), the best-effort forbidden-operation scan, and \
the clean-tree policy. The JSON report includes detected_runner, detection_source, \
used_shebang, used_extension_map, and argv.\n\n\
The script is never executed and never copied; no run directory, stdout.txt, stderr.txt, \
or run.json is created. Exit 0 when the policy accepts the script, exit 3 with the named \
blocked state when it does not, and exit 2 for invalid usage (e.g. --require-clean and \
--allow-dirty together).",
        after_help = "Examples:\n  \
aikit script check .aikit/temp/build.sh\n  \
aikit script check .aikit/temp/build.sh --require-clean --json"
    )]
    Check(ScriptCheckArgs),
}

#[derive(Debug, Args)]
pub struct ScriptRunArgs {
    /// Path to the script to run; must be under an allowed local work area.
    #[arg(value_name = "SCRIPT_PATH")]
    pub script: String,

    /// Validate and print the planned command without executing the script.
    #[arg(long)]
    pub print: bool,

    /// Force a specific runner (sh, zsh, bash, pwsh, powershell, cmd, python, python3, node).
    #[arg(long, value_name = "RUNNER")]
    pub runner: Option<String>,

    /// Disable `#!` shebang detection (use config/extension mapping only).
    #[arg(long)]
    pub no_shebang: bool,

    /// Block when the tracked working tree is dirty (mutually exclusive with --allow-dirty).
    #[arg(long, conflicts_with = "allow_dirty")]
    pub require_clean: bool,

    /// Permit a dirty tracked working tree (this is the default when neither flag is given).
    #[arg(long)]
    pub allow_dirty: bool,

    /// Override the output directory root (default: .aikit/outputs; pass .scratch/... to use scratch).
    #[arg(long, value_name = "DIR")]
    pub output: Option<String>,

    /// Print the machine-readable run record (run.json) to stdout in addition to writing it.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ScriptCheckArgs {
    /// Path to the script to validate; must be under an allowed local work area.
    #[arg(value_name = "SCRIPT_PATH")]
    pub script: String,

    /// Force a specific runner (sh, zsh, bash, pwsh, powershell, cmd, python, python3, node).
    #[arg(long, value_name = "RUNNER")]
    pub runner: Option<String>,

    /// Disable `#!` shebang detection (use config/extension mapping only).
    #[arg(long)]
    pub no_shebang: bool,

    /// Block when the tracked working tree is dirty (mutually exclusive with --allow-dirty).
    #[arg(long, conflicts_with = "allow_dirty")]
    pub require_clean: bool,

    /// Permit a dirty tracked working tree (this is the default when neither flag is given).
    #[arg(long)]
    pub allow_dirty: bool,

    /// Print the machine-readable check record to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct BatchCli {
    #[command(subcommand)]
    pub command: BatchCommand,
}

#[derive(Debug, Subcommand)]
pub enum BatchCommand {
    /// Create a batch anchor before AI-agent work begins.
    #[command(
        long_about = "Create a batch anchor: a minimal timestamp reference recording the \
current Git HEAD, branch, and timestamp. Use this immediately before starting a unit of \
AI-agent or manual work, so `batch changed` can later report what that work touched. The \
anchor is a timestamp reference only — it does NOT capture Git status; anchor-based \
changed-file discovery is timestamp-based relative to the anchor file's mtime.\n\n\
The anchor is written as JSON under the local output directory \
.aikit/outputs/batches/ by default; override with --output <dir>. Output is local-only and \
never needs committing. The anchor also records the aikit version that created it.\n\n\
--snapshot additionally records an initial snapshot of tracked files (repo-relative \
paths) in the anchor; this is optional and is never a full repo content scan.\n\n\
With --json, prints the anchor path and the anchor object as machine-readable JSON.",
        after_help = "Example:\n  aikit batch start\n  aikit batch start --snapshot --json"
    )]
    Start(StartArgs),

    /// List files modified since a batch anchor (timestamp-based; not git status).
    #[command(
        long_about = "List existing files whose filesystem modification time is newer than \
the anchor. This is TIMESTAMP-BASED discovery relative to the anchor file: it does NOT use \
`git status`, and tracked/untracked/staged/unstaged status is not the deciding factor. A \
pre-existing file that is dirty relative to HEAD but was last modified before the anchor is \
NOT reported; a file modified after the anchor IS reported whether or not it is tracked. \
Deleted files are out of scope (no content exists on disk to bundle).\n\n\
`.gitignore` / `.git/info/exclude` are honored, and aikit's own areas (.git/, .aikit/, \
.scratch/, .claude/) and configured build/dependency directories (target/, node_modules/, \
dist/, build/) are excluded; configured include/exclude globs apply. Symlinks are not \
followed.\n\n\
Results are deterministic, repo-relative, and sorted lexicographically (status `modified`, \
source `anchor_mtime`). With --json, prints the full report (files, sources, sizes, counts); \
--hash adds a SHA-256 for each file.\n\n\
Limitation: mtime is a best-effort heuristic (and can miss changed-then-reverted files).",
        after_help = "Example:\n  \
aikit batch changed --anchor .aikit/outputs/batches/<anchor-id>.json\n  \
aikit batch changed --anchor <anchor.json> --hash --json"
    )]
    Changed(ChangedArgs),

    /// List batch anchors (read-only; does not auto-select an anchor).
    #[command(
        long_about = "List valid batch anchors under the selected output root's batches/ \
folder (default .aikit/outputs/batches/). Read-only: creates and deletes nothing. \
Invalid files in the folder are reported as skipped rather than guessed. Anchors are \
sorted deterministically by anchor id.\n\n\
This command does NOT auto-select a \"latest\" anchor for work — anchor-consuming \
commands (`batch changed`, `review generate --anchor`, `diff anchor`) always require an \
explicit anchor. Supports `--root <path>` (a known output root) and `--json`.",
        after_help = "Examples:\n  \
aikit batch list\n  \
aikit batch list --json"
    )]
    List(BatchListArgs),

    /// Show one explicit batch anchor (read-only; does not auto-select an anchor).
    #[command(
        long_about = "Show one explicit batch anchor by path or id (read-only; creates and \
deletes nothing). The argument is a repo-relative path to an anchor JSON file or an \
anchor id looked up under the output root's batches/ folder. The file is validated as a \
batch anchor and must belong to the current repository; path escapes are rejected. This \
command does NOT auto-select a \"latest\" anchor. Supports `--root <path>` and `--json`.",
        after_help = "Examples:\n  \
aikit batch show <anchor-id>\n  \
aikit batch show .aikit/outputs/batches/<anchor-id>.json --json"
    )]
    Show(BatchShowArgs),
}

#[derive(Debug, Args)]
pub struct StartArgs {
    /// Override the output directory root (default: .aikit/outputs; pass .scratch/... to use scratch).
    #[arg(long, value_name = "DIR")]
    pub output: Option<String>,

    /// Record an initial snapshot of tracked files in the anchor (off by default; never a full repo scan).
    #[arg(long)]
    pub snapshot: bool,

    /// Print machine-readable JSON instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ChangedArgs {
    /// Path to the anchor JSON produced by `aikit batch start`.
    #[arg(long, value_name = "ANCHOR_JSON")]
    pub anchor: String,

    /// Print machine-readable JSON instead of human-readable text.
    #[arg(long)]
    pub json: bool,

    /// Compute a SHA-256 for each reported file.
    #[arg(long)]
    pub hash: bool,
}

#[derive(Debug, Args)]
pub struct BatchListArgs {
    /// Output root to inspect (default: .aikit/outputs; an explicit root must be under
    /// .aikit/outputs/ or .scratch/work/outputs/).
    #[arg(long, value_name = "PATH")]
    pub root: Option<String>,

    /// Print the machine-readable list to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct BatchShowArgs {
    /// Anchor to show: a repo-relative path to an anchor JSON file, or an anchor id.
    #[arg(value_name = "ANCHOR")]
    pub anchor: String,

    /// Output root used for id lookup (default: .aikit/outputs; an explicit root must be
    /// under .aikit/outputs/ or .scratch/work/outputs/).
    #[arg(long, value_name = "PATH")]
    pub root: Option<String>,

    /// Print the machine-readable anchor to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct DiffCli {
    #[command(subcommand)]
    pub command: DiffCommand,
}

#[derive(Debug, Subcommand)]
pub enum DiffCommand {
    /// Diff a batch anchor's recorded head against the current working tree.
    #[command(
        long_about = "Produce a mechanical Git diff from a batch anchor's recorded Git head \
to the current working-tree state. The argument is a repo-relative path to an anchor JSON \
file or an anchor id (looked up under .aikit/outputs/batches/). The anchor is validated \
and must belong to the current repository; its recorded `git_head` is used as the diff \
base and must still exist locally (else blocked).\n\n\
The diff (`git diff <base>`) captures committed changes since the anchor and current \
tracked working-tree/index changes. Untracked file CONTENTS are not part of a Git diff \
and are not included — use `batch changed --anchor <anchor>` for a timestamp-based file \
list. This is inspection only: it creates no review bundle or output artifact and never \
touches remotes.\n\n\
Default output includes anchor metadata, the name-status file list, and the diff stat. \
`--stat` is the explicit form of the stat output (included by default); `--patch` appends \
the full patch text; `--json` emits the structured report (patch only when `--patch`).",
        after_help = "Examples:\n  \
aikit diff anchor <anchor-id>\n  \
aikit diff anchor <anchor-id> --json\n  \
aikit diff anchor .aikit/outputs/batches/<anchor-id>.json --patch"
    )]
    Anchor(DiffAnchorArgs),
}

#[derive(Debug, Args)]
pub struct EnvCli {
    #[command(subcommand)]
    pub command: EnvCommand,
}

#[derive(Debug, Subcommand)]
pub enum EnvCommand {
    /// Capture mechanical local environment facts (read-only; creates nothing).
    #[command(
        long_about = "Capture mechanical local environment facts useful for debugging \
aikit usage. Read-only: creates no files or directories, modifies no repo files, runs no \
network commands, and never touches remotes.\n\n\
Reports the aikit version, current executable path, OS family, CPU architecture, current \
working directory, and — when inside a Git repository — the repo root, branch, HEAD, \
tracked-tree clean/dirty state, default output root, and whether `.aikit/`, `.aikit/temp/`, \
and `.aikit/outputs/` exist and `.aikit/` is ignored. Also reports legacy/informational \
shell-interpreter probes (`/bin/sh`, `/bin/zsh`), local git/Rust/Cargo versions, and the \
shell from `$SHELL` when set. These shell probes are informational only; they are NOT the \
cross-OS runner-availability/readiness model reported by `repo doctor` (`env snapshot` \
does not report runner availability).\n\n\
It deliberately does NOT dump all environment variables, the raw PATH, tokens, \
credentials, private keys, or any provider/model-specific or network-derived information. \
PATH is summarized only (entry count and whether the current executable's directory is on \
it). It works outside a Git repository too, reporting the non-repo facts. Supports human \
output and `--json`.",
        after_help = "Examples:\n  \
aikit env snapshot\n  \
aikit env snapshot --json"
    )]
    Snapshot(EnvSnapshotArgs),
}

#[derive(Debug, Args)]
pub struct EnvSnapshotArgs {
    /// Print the machine-readable snapshot to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ScanCli {
    #[command(subcommand)]
    pub command: ScanCommand,
}

#[derive(Debug, Subcommand)]
pub enum ScanCommand {
    /// Best-effort heuristic scan for likely secrets in explicit repo-local paths.
    #[command(
        long_about = "Run a local, best-effort heuristic scan for likely secrets in the \
explicit paths you provide. At least one path is required; the whole repository is never \
scanned implicitly unless you pass the repo root or `.`. Paths are resolved relative to \
the repo root; paths outside the repo and symlink/path escapes are rejected, and `.git/` \
is always excluded.\n\n\
Explicit files are scanned even when ignored; for directories, traversal respects \
`.gitignore` by default (use `--include-ignored` to include ignored files). Binary files \
and files larger than `--max-file-bytes` (default 1 MiB) are skipped.\n\n\
This is HEURISTIC and best-effort: it can false-positive and false-negative, never proves \
a file or repo is safe to share, makes no judgment about whether a finding is a live \
credential, and does not replace dedicated secret-scanning tools — inspect every finding. \
It NEVER prints raw secret values in human or JSON output; each finding reports the \
path, line, rule id, description, and severity only. It creates no output artifacts and \
never touches remotes.\n\n\
By default findings are reported and the command exits 0 (usable for inspection). With \
`--fail-on-findings`, a non-empty finding set exits 3 with blocked state \
`blocked_secret_findings`.",
        after_help = "Examples:\n  \
aikit scan secrets README.md docs\n  \
aikit scan secrets . --fail-on-findings\n  \
aikit scan secrets src --json --include-ignored --max-file-bytes 2000000"
    )]
    Secrets(ScanSecretsArgs),
}

#[derive(Debug, Args)]
pub struct ScanSecretsArgs {
    /// One or more repo-local paths (files or directories) to scan. At least one is required.
    #[arg(value_name = "PATH", required = true, num_args = 1..)]
    pub paths: Vec<String>,

    /// Print the machine-readable report to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,

    /// Include files ignored by .gitignore during directory traversal.
    #[arg(long)]
    pub include_ignored: bool,

    /// Skip files larger than N bytes (default: 1048576, i.e. 1 MiB).
    #[arg(long, value_name = "N")]
    pub max_file_bytes: Option<u64>,

    /// Exit 3 (blocked_secret_findings) when any findings are present.
    #[arg(long)]
    pub fail_on_findings: bool,
}

#[derive(Debug, Args)]
pub struct DiffAnchorArgs {
    /// Anchor to diff against: a repo-relative path to an anchor JSON file, or an anchor id.
    #[arg(value_name = "ANCHOR")]
    pub anchor: String,

    /// Explicitly include the diff stat (it is included by default).
    #[arg(long)]
    pub stat: bool,

    /// Append the full patch text (and include it in --json output).
    #[arg(long)]
    pub patch: bool,

    /// Print the machine-readable diff report to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}
