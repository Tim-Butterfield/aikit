//! Command-line interface definition and help text.
//!
//! Help text is part of the product: it must let a human or an AI agent decide
//! when and how to call each command. Each command documents its purpose, when to
//! use it, key flags, default output behavior, JSON behavior, and an example.

use clap::{ArgGroup, Args, Parser, Subcommand};

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
`review generate` writes run_for_review.txt and manifest.json under the default output \
directory .aikit/outputs/reviews/<id>/; override with --output <dir>. `.scratch` is never \
used by default and is available only via an explicit `--output .scratch/...`. Created \
artifact paths are printed; output is local-only.",
        after_help = "Example:\n  \
aikit review generate --files src/main.rs README.md\n  \
aikit review generate --anchor .aikit/outputs/batches/<anchor-id>.json --json"
    )]
    Review(ReviewCli),

    /// Run a local script under mechanical safety controls, with an audit record.
    #[command(
        long_about = "Run a local script through a fixed interpreter and record an audit \
trail. This is NOT a security sandbox: it reduces accidental unsafe execution but does not \
make an arbitrary script safe.\n\n\
Use `run script <script-path>`. The script must live under an allowed local work area \
(.aikit/temp/, .scratch/work/temp/, or .scratch/work/outputs/) — those are input locations, \
not output locations. Only `.zsh` (/bin/zsh) and `.sh` (/bin/sh) are supported; the \
interpreter is chosen from the extension, never from a shebang. The run record \
(copied script, stdout.txt, stderr.txt, run.json) is written under the default output \
directory .aikit/outputs/runs/<id>/; override with --output <dir> (`.scratch` output is \
used only when requested explicitly).",
        after_help = "Examples:\n  \
aikit run script .aikit/temp/build.sh\n  \
aikit run script .scratch/work/temp/task.zsh --print"
    )]
    Run(RunCli),
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
run_for_review.txt plus a manifest.json. Caps keep the bundle bounded: --max-file-bytes and \
--max-file-lines truncate individual files (recording truncation and the bound), and \
--max-total-bytes omits later files once the running total would be exceeded (recording \
omitted_reason/cap_hit). Every scoped file appears exactly once in the manifest whether \
included, truncated, or omitted.\n\n\
Output (run_for_review.txt + manifest.json) is written under the default local output \
directory .aikit/outputs/reviews/<id>/; override the root with --output <dir> (pass a \
.scratch/... path to use scratch, which is never used by default). Created artifact paths \
are printed in human output; with --json the manifest is printed to stdout including a \
`written` array of the created file paths.",
        after_help = "Examples:\n  \
aikit review generate --files src/main.rs README.md\n  \
aikit review generate --anchor .aikit/outputs/batches/<anchor-id>.json --json\n  \
aikit review generate --anchor <anchor.json> --max-file-bytes 200000 --max-total-bytes 2000000"
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

    /// Override the output directory root (default: .aikit/outputs; pass .scratch/... to use scratch).
    #[arg(long, value_name = "DIR")]
    pub output: Option<String>,

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
pub struct RunCli {
    #[command(subcommand)]
    pub command: RunCommand,
}

#[derive(Debug, Subcommand)]
pub enum RunCommand {
    /// Run a local script under mechanical safety controls (not a security sandbox).
    #[command(
        long_about = "Run a local script through a fixed interpreter and write an audit \
record. NOTE: this is NOT a security sandbox. The allowed-location policy is the primary \
control; the forbidden-operation scan is best-effort (naive substring matching, easily \
bypassed, can false-positive), and running a script here does not make it safe.\n\n\
The <script-path> must resolve (after symlink resolution) to a real file under an allowed \
local work area: .aikit/temp/, .scratch/work/temp/, or .scratch/work/outputs/. These are \
input locations only. Only `.zsh` (/bin/zsh) and `.sh` (/bin/sh) are supported; the \
interpreter is selected from the extension, never from a shebang — extensionless or \
unknown-extension scripts are rejected.\n\n\
Clean-tree policy: the default is allow-dirty (these scripts operate on working content). \
`--require-clean` blocks when the tracked tree is dirty; `--allow-dirty` is the explicit \
default; the two cannot be combined. With `--print`, policy is validated and the planned \
command is shown but the script is not executed (recorded as executed: false).\n\n\
On execution the script is copied into the run directory (retaining its extension), stdout \
and stderr are captured to stdout.txt / stderr.txt, and run.json records the audit metadata. \
Output is written under .aikit/outputs/runs/<id>/ by default; override with --output <dir> \
(`.scratch` output only when requested explicitly). Created artifact paths are printed (and \
included in --json). The executed script's exit code is propagated.",
        after_help = "Examples:\n  \
aikit run script .aikit/temp/build.sh\n  \
aikit run script .scratch/work/temp/task.zsh --print\n  \
aikit run script .aikit/temp/check.sh --require-clean --json"
    )]
    Script(RunScriptArgs),
}

#[derive(Debug, Args)]
pub struct RunScriptArgs {
    /// Path to the script to run; must be under an allowed local work area.
    #[arg(value_name = "SCRIPT_PATH")]
    pub script: String,

    /// Validate and print the planned command without executing the script.
    #[arg(long)]
    pub print: bool,

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
pub struct BatchCli {
    #[command(subcommand)]
    pub command: BatchCommand,
}

#[derive(Debug, Subcommand)]
pub enum BatchCommand {
    /// Create a batch anchor before AI-agent work begins.
    #[command(
        long_about = "Create a batch anchor capturing the current Git HEAD, branch, status, \
and timestamp. Use this immediately before starting a unit of AI-agent or manual work, so \
`batch changed` can later report what that work touched.\n\n\
The anchor is written as JSON under the local output directory \
.aikit/outputs/batches/ by default; override with --output <dir>. Output is local-only and \
never needs committing.\n\n\
With --json, prints the anchor path and the anchor object as machine-readable JSON.",
        after_help = "Example:\n  aikit batch start\n  aikit batch start --json"
    )]
    Start(StartArgs),

    /// List files created or modified since a batch anchor.
    #[command(
        long_about = "List files created or modified since a batch anchor. Tracked changes \
come from `git status` (working-tree state vs HEAD). Untracked files are included only with \
--include-untracked, using a best-effort filesystem mtime heuristic (newer than the anchor \
time). Deletions are detected for tracked files only; renames are reported as delete+create. \
aikit's own output directories are excluded by default.\n\n\
Results are deterministic, repo-relative, and sorted lexicographically. With --json, prints \
the full report (files, sources, sizes, counts) as machine-readable JSON; --hash adds a \
SHA-256 for each existing file.\n\n\
Limitation: mtime is a heuristic and can miss changed-then-reverted files; treat untracked \
results as best-effort.",
        after_help = "Example:\n  \
aikit batch changed --anchor .aikit/outputs/batches/<anchor-id>.json\n  \
aikit batch changed --anchor <anchor.json> --include-untracked --hash --json"
    )]
    Changed(ChangedArgs),
}

#[derive(Debug, Args)]
pub struct StartArgs {
    /// Override the output directory root (default: .aikit/outputs; pass .scratch/... to use scratch).
    #[arg(long, value_name = "DIR")]
    pub output: Option<String>,

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

    /// Only consider tracked files (ignore untracked files entirely).
    #[arg(long, conflicts_with = "include_untracked")]
    pub tracked_only: bool,

    /// Include untracked files created since the anchor (mtime heuristic).
    #[arg(long)]
    pub include_untracked: bool,

    /// Compute a SHA-256 for each reported file that still exists.
    #[arg(long)]
    pub hash: bool,
}
