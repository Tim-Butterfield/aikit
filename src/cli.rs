//! Command-line interface definition and help text.
//!
//! Help text is part of the product: it must let a human or an AI agent decide
//! when and how to call each command. Each command documents its purpose, when to
//! use it, key flags, default output behavior, JSON behavior, and an example.

use clap::{Args, Parser, Subcommand};

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
aikit batch changed --anchor .scratch/work/outputs/aikit/batches/<anchor-id>.json\n\n\
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
(.scratch/work/outputs/aikit/batches/ when .scratch/work/outputs/ exists, otherwise \
.aikit/outputs/batches/); output is local-only and never needs committing. Both \
subcommands support --json for machine-readable output.",
        after_help = "Examples:\n  \
aikit batch start\n  \
aikit batch changed --anchor .scratch/work/outputs/aikit/batches/<anchor-id>.json --json"
    )]
    Batch(BatchCli),

    /// Generate a mechanical inventory of repository files.
    #[command(
        long_about = "Generate a mechanical inventory of repository files: a deterministic, \
hashed listing of every included file. Use it to capture a reproducible snapshot of repo \
contents for review or comparison. Traversal is gitignore-aware and always excludes `.git/` \
and common build/dependency/output directories (matched by directory name, not substring).\n\n\
The `repo` subcommand writes inventory.json and inventory.txt under the local output \
directory (.scratch/work/outputs/aikit/inventory/<id>/ when .scratch/work/outputs/ exists, \
otherwise .aikit/outputs/inventory/<id>/); output is local-only. Key flags (on `inventory \
repo`): --json (also print JSON to stdout), --include-ignored (include .gitignore'd files; \
always-excluded dirs still apply), --max-files <n> (limit deterministically after sorting), \
and --output <dir> (override the output root).",
        after_help = "Example:\n  \
aikit inventory repo\n  \
aikit inventory repo --json --include-ignored --max-files 500"
    )]
    Inventory(InventoryCli),

    /// Generate a bounded review bundle from explicit files.
    #[command(
        long_about = "Generate a bounded, hashed review bundle for AI/human review. Use it to \
package a fixed set of files into a single reviewable text bundle plus a manifest, with \
deterministic ordering and size caps so the surface stays bounded.\n\n\
Batch 3 supports explicit files only, via `review generate --files <file>...`; the \
`--anchor` and `--changed` modes are not available (anchor-based generation is deferred). \
Key flags (on `review generate`): --files <file>... (required inputs, resolved under the \
repo root), --max-file-bytes / --max-file-lines (truncate a file and record it), \
--max-total-bytes (omit later files once the running total is exceeded), --output <dir> \
(override the output root), and --json (also print the manifest JSON to stdout).\n\n\
`review generate` writes run_for_review.txt and manifest.json under the local output \
directory (.scratch/work/outputs/aikit/reviews/<id>/ when .scratch/work/outputs/ exists, \
otherwise .aikit/outputs/reviews/<id>/); output is local-only.",
        after_help = "Example:\n  \
aikit review generate --files src/main.rs README.md\n  \
aikit review generate --files src/main.rs --max-file-bytes 200000 --json"
    )]
    Review(ReviewCli),
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
inventory.txt) is written under the local output directory: \
.scratch/work/outputs/aikit/inventory/<id>/ when .scratch/work/outputs/ exists, otherwise \
.aikit/outputs/inventory/<id>/; override the root with --output <dir>. With --json the \
inventory is also printed to stdout as machine-readable JSON. --max-files <n> limits the \
listing deterministically (after sorting) and records the limitation.",
        after_help = "Examples:\n  \
aikit inventory repo\n  \
aikit inventory repo --json\n  \
aikit inventory repo --include-ignored --max-files 500"
    )]
    Repo(InventoryRepoArgs),
}

#[derive(Debug, Args)]
pub struct InventoryRepoArgs {
    /// Override the output directory root (default: .scratch/work/outputs/aikit, else .aikit/outputs).
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
    /// Generate a review bundle from an explicit list of files.
    #[command(
        long_about = "Generate a review bundle from an explicit list of files. Each \
`--files` path is resolved relative to the repository root and must resolve (after \
symlink resolution) to a real path inside the repo; paths that escape the repo are \
rejected. Files are sorted by repo-relative path, hashed (SHA-256), and packaged into \
run_for_review.txt plus a manifest.json.\n\n\
When to use: to hand a fixed, bounded, hashed set of files to a reviewer (human or AI \
agent). Caps keep the bundle bounded: --max-file-bytes and --max-file-lines truncate \
individual files (recording truncation and the bound), and --max-total-bytes omits later \
files once the running total would be exceeded (recording omitted_reason/cap_hit). Every \
requested file appears exactly once in the manifest whether included, truncated, or \
omitted.\n\n\
Output (run_for_review.txt + manifest.json) is written under the local output directory: \
.scratch/work/outputs/aikit/reviews/<id>/ when .scratch/work/outputs/ exists, otherwise \
.aikit/outputs/reviews/<id>/; override the root with --output <dir>. With --json the \
manifest is also printed to stdout. Batch 3 supports explicit files only; --anchor and \
--changed modes are not available.",
        after_help = "Examples:\n  \
aikit review generate --files src/main.rs README.md\n  \
aikit review generate --files src/*.rs --max-file-bytes 200000 --max-total-bytes 2000000 --json"
    )]
    Generate(ReviewGenerateArgs),
}

#[derive(Debug, Args)]
pub struct ReviewGenerateArgs {
    /// Explicit files to include, resolved relative to the repo root (one or more).
    #[arg(long, value_name = "FILE", num_args = 1.., required = true)]
    pub files: Vec<String>,

    /// Override the output directory root (default: .scratch/work/outputs/aikit, else .aikit/outputs).
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
The anchor is written as JSON under the local output directory: \
.scratch/work/outputs/aikit/batches/ when .scratch/work/outputs/ already exists, otherwise \
.aikit/outputs/batches/. Output is local-only and never needs committing.\n\n\
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
aikit batch changed --anchor .scratch/work/outputs/aikit/batches/<anchor-id>.json\n  \
aikit batch changed --anchor <anchor.json> --include-untracked --hash --json"
    )]
    Changed(ChangedArgs),
}

#[derive(Debug, Args)]
pub struct StartArgs {
    /// Override the output directory root (default: .scratch/work/outputs/aikit, else .aikit/outputs).
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
