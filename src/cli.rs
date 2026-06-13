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
