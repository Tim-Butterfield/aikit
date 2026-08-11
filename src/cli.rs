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
or model. Most commands operate on the current Git repository; setup (`init` and `doctor`, \
with their `--require-*` flags) and the script runner also work in Mercurial repositories \
and non-repo `.aikit/` folders.\n\n\
Every command that supports `--json` emits one JSON document on stdout, on success and on \
failure alike; human-readable text always goes to stderr. Exit codes: 0 success, 1 command \
failure, 2 invalid usage, 3 blocked state.",
    after_help = "Examples:\n  \
aikit batch start\n  \
aikit batch changed --anchor .aikit/outputs/batches/<anchor-id>.json\n\n\
Exit codes: 0 success, 1 command failure, 2 invalid usage, 3 blocked state."
)]
pub struct Cli {
    /// Run as if aikit had been started in this directory.
    ///
    /// Applies to every command, before anything else happens.
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        long_help = "Run as if aikit had been started in DIR. Every command that resolves a \
repository, a marker root, or a repo-relative path uses it.\n\n\
This exists so a caller does not have to navigate the shell to reach the repository. Shell \
navigation is where cross-platform wrappers break: quoting, spaces, drive-relative paths \
and `cd` semantics all differ, and a wrapper that gets it wrong fails in a way that looks \
like an aikit error. Passing the directory as an argument removes that step.\n\n\
The directory must exist; a path that cannot be entered is invalid usage (exit 2) rather \
than a blocked state, because nothing about the repository has been examined yet."
    )]
    pub cwd: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    /// Whether the invoked subcommand was given `--json`.
    ///
    /// `main` needs this *before* dispatch, because a command that fails never gets to
    /// print anything: the failure record has to be emitted by the error path, which
    /// otherwise has no idea which channel the caller asked for. Matching explicitly (rather
    /// than defaulting unknown commands to `true`) means a newly added command that forgets
    /// `--json` reports nothing on stdout instead of emitting an error document for a
    /// command that never emits records.
    pub fn wants_json(&self) -> bool {
        match &self.command {
            Command::Batch(c) => match &c.command {
                BatchCommand::Start(a) => a.json,
                BatchCommand::Changed(a) => a.json,
                BatchCommand::List(a) => a.json,
                BatchCommand::Show(a) => a.json,
                BatchCommand::Diff(a) => a.json,
            },
            Command::Env(c) => match &c.command {
                EnvCommand::Snapshot(a) => a.json,
            },
            Command::Scan(c) => match &c.command {
                ScanCommand::Secrets(a) => a.json,
            },
            Command::Inventory(c) => match &c.command {
                InventoryCommand::Repo(a) => a.json,
            },
            Command::Output(c) => match &c.command {
                OutputCommand::List(a) => a.json,
                OutputCommand::Show(a) => a.json,
                OutputCommand::Clean(a) => a.json,
            },
            Command::Review(c) => match &c.command {
                ReviewCommand::Generate(a) => a.json,
            },
            Command::Script(c) => match &c.command {
                ScriptCommand::Run(a) => a.json,
                ScriptCommand::Check(a) => a.json,
            },
            Command::Config(c) => match &c.command {
                ConfigCommand::Show(a) => a.json,
            },
            Command::Init(a) => a.json,
            Command::Doctor(a) => a.json,
            Command::Version(a) => a.json,
            // `agents-md` emits Markdown and `mcp` speaks JSON-RPC on stdio; neither has a
            // record format, so neither has `--json` to honour.
            Command::AgentsMd | Command::Mcp(_) => false,
        }
    }
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

    /// Report mechanical local environment facts (for debugging aikit usage).
    #[command(
        long_about = "Report mechanical local environment facts useful for debugging aikit \
usage. `env snapshot` prints a bounded, fixed set of facts — aikit version, current \
executable, OS family, CPU architecture, working directory, repo facts when inside a Git \
repository, legacy/informational shell-interpreter probes (`/bin/sh`, `/bin/zsh`), and \
local git/Rust/Cargo versions. These shell probes are informational only and are NOT the \
cross-OS runner-availability/readiness model used by `aikit doctor`.\n\n\
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
        long_about = "Run a local, redacted scan for likely secrets in explicit repo-local \
paths. You must pass at least one path; the whole repository is never scanned implicitly \
unless you pass the repo root or `.`.\n\n\
The rule set favours precision over coverage, so it can false-negative freely. It does not \
prove a file or repo is safe to share, does not judge whether a finding is a live \
credential, and does not replace gitleaks or trufflehog. It NEVER prints raw secret values \
in human or JSON output — only the file path, line number, rule id, description, and \
severity — so the report is safe to quote verbatim. It creates no output artifacts and \
never touches remotes.\n\n\
Severity is confidence that a match is a real credential, NOT blast radius. high: a \
self-identifying credential format, or a credential-style name assigned a long opaque \
token. medium: a credential-style name assigned a short or word-like value. low: that same \
name-based match in an example/sample/template/fixture path. Severity is monotonic, so \
`--fail-on <severity>` exits 3 (blocked_secret_findings) on that severity OR ABOVE; without \
the flag findings are reported and the command exits 0.",
        after_help = "Examples:\n  \
aikit scan secrets README.md docs\n  \
aikit scan secrets . --fail-on high\n  \
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

    /// Prepare the current directory for local aikit usage (repo-aware; idempotent).
    #[command(
        long_about = "Prepare the current directory for local aikit usage. Creates \
`.aikit/` and `.aikit/temp/` if missing.\n\n\
Adaptive by default: inside a Git or Mercurial repository it also ensures `.aikit/` is \
locally ignored (Git's `.git/info/exclude`, or a `.hg/hgrc`-registered \
`.hg/hgignore.aikit` for Mercurial — never a tracked `.gitignore`/`.hgignore`). Outside \
any repository it creates the directories only and adds no ignore coverage. Repository \
detection is filesystem-based (it looks for an enclosing `.git`/`.hg`) and does not \
require the `git`/`hg` CLI.\n\n\
Two flags turn the adaptive behaviour into an assertion, for scripts and CI where doing \
the right thing silently is the wrong answer:\n\n\
--require-repo blocks (blocked_repo_not_found) when not inside a repository. Without it, \
running in a directory you believed was a repo quietly produces a folder-mode `.aikit/` \
with no ignore coverage.\n\n\
--require-folder blocks (blocked_repo_present) when inside one, since an un-ignored \
`.aikit/` would surface as untracked there.\n\n\
Idempotent; creates no output artifacts, `.scratch/`, or `.claude/`.",
        after_help = "Examples:\n  \
aikit init\n  \
aikit init --json\n  \
aikit init --require-repo"
    )]
    Init(InitArgs),

    /// Print the agent guide embedded in this binary.
    #[command(
        name = "agents-md",
        long_about = "Print aikit's agent guide — what it does, the rules that are expensive \
to get wrong, every command with a one-line contract, and its safety posture.\n\n\
The text is compiled into the binary, so it always matches the version printing it. This is \
the same document the `agents_md` MCP tool returns, so an agent reaching aikit through a \
shell and one reaching it over MCP get identical guidance.\n\n\
Set the `AGENTS_MD` environment variable to a file path to serve that file instead. A path \
that cannot be read is an error rather than a silent fall back to the embedded text.",
        after_help = "Examples:\n  \
aikit agents-md\n  \
AGENTS_MD=/etc/aikit/AGENTS.md aikit agents-md"
    )]
    AgentsMd,

    /// Show the resolved configuration and where each value came from (read-only).
    #[command(
        long_about = "Report the fully resolved configuration: every setting's effective \
value and the source that set it — `default`, or the repo-relative config file that last \
won. Layered config is only explainable if you can see which layer won; otherwise answering \
\"why is this value what it is?\" means replaying the merge by hand.\n\n\
Read-only: it creates and modifies nothing. Loading uses the same code path every other \
command uses, so what this prints is what they will act on.\n\n\
The MCP server does NOT consult this configuration — `aikit mcp` requires an explicit \
runner on every call and applies no config-driven defaults. This command therefore explains \
the CLI only.",
        after_help = "Examples:\n  \
aikit config show\n  \
aikit config show --json"
    )]
    Config(ConfigCli),

    /// Report local aikit readiness without changing anything (read-only).
    #[command(
        long_about = "Report whether the current directory is ready for local aikit usage. \
Read-only: it creates no files or directories (no `.aikit/`, `.scratch/`, `.claude/`, or \
`.aikit/outputs/`) and does not modify `.gitignore`, `.git/info/exclude`, `.hgignore`, or \
`.hg/` state.\n\n\
Works in Git repositories, Mercurial repositories, and non-repo `.aikit/` folders. \
Detection is filesystem-based and reports a `vcs` field (git/mercurial/none); for \
Mercurial, ignore coverage is detected without invoking `hg`, while branch/HEAD come from \
`hg` when available and degrade to empty with a warning otherwise.\n\n\
It reports `runners` — each supported script runner (sh, bash, zsh, pwsh, powershell, cmd, \
python3, python, node) with `available` and OS `applicable` flags — plus \
`any_runner_available`. Readiness means sane local aikit state (`.aikit/temp/` present, \
and, in a repository, `.aikit/` ignored) AND at least one runner available for this OS. It \
does NOT require a specific Unix shell, so Windows is ready with pwsh/cmd and a host \
without zsh is still ready. The legacy `/bin/sh` and `/bin/zsh` probes are informational \
only and do not gate readiness.\n\n\
Run this BEFORE `aikit init` — it tells you whether initialization is needed rather than \
assuming it. Wherever an enclosing `.git`, `.hg`, or `.aikit` marker exists it reports \
against that root. With no marker anywhere up the tree it does not fail: it reports \
`ready: false` against the current directory and exits 0, because a read-only probe that \
errors tells a caller nothing about what to do next.\n\n\
An unanchored report is marked as such (`root_source: \"cwd_no_marker\"` in JSON) so the \
current directory is never mistaken for a real root.\n\n\
--require-root turns that into an assertion, blocking blocked_repo_not_found when there \
is no marker — for CI, where the absence of a root should fail the run. Note it requires \
a *root*, not a repository: a non-repo `.aikit/` folder satisfies it.",
        after_help = "Examples:\n  \
aikit doctor\n  \
aikit doctor --json\n  \
aikit doctor --require-root"
    )]
    Doctor(RepoDoctorArgs),

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

    /// Serve the script runner to an AI agent over the Model Context Protocol.
    #[command(
        long_about = "Serve aikit's script runner to an AI agent over the Model Context \
Protocol (MCP), on stdio. `mcp` exposes three tools: `run`, which executes a script \
supplied in the call; `list_runners`, which reports the interpreters available on this \
host; and `agents_md`, which returns aikit's agent guide.\n\n\
This is a DIFFERENT contract from `script run`, not a wrapper around it. The script arrives \
as call arguments rather than as a file you author, so nothing is written into your \
repository or working tree, no repository is required, no `.aikit/` directory is used, no \
run record is kept, and the working-tree dirty check does not apply. Auditing a run is a \
`script run` concern, where the repository the record belongs to is unambiguous. (A \
temporary script \
does exist while it runs: it is written to a directory under the system temp location that \
is created private to you, and deleted when the call ends. Deletion is best-effort — a \
virus scanner or a surviving grandchild holding the file can defer it — so treat the temp \
location as somewhere scripts may briefly persist.) Configuration is not consulted either: the runner must be named explicitly on \
every call and is never inferred from an extension, a shebang, or config, because `pwsh` \
and `powershell` execute the same .ps1 differently. An unavailable runner is an error, never \
a silent substitution.\n\n\
This is NOT a security sandbox. It runs arbitrary scripts with the privileges of the aikit \
process. The forbidden-pattern scan is an accident guard, not containment. Most MCP clients \
ask the user to confirm each tool call; aikit does not add a second prompt of its own.\n\n\
A run is stopped by its timeout (default 120s, maximum 3600s). Cancelling a call is not a \
reliable stop: when a client delivers the cancellation the process tree is killed, but \
clients are not required to send it, and one that simply stops waiting leaves the script \
running to completion.\n\n\
It is not useful to run by hand — it speaks JSON-RPC on stdin/stdout. stdout carries \
protocol traffic only; diagnostics (the negotiated protocol version and the client's \
identity) go to stderr, which some clients do not surface at all.\n\n\
Both MCP lifecycles are served: a client may open with the `initialize` handshake, or with \
`server/discover` and no handshake at all.",
        after_help = "Example client configuration:\n  \
{\"mcpServers\": {\"aikit\": {\"command\": \"aikit\", \"args\": [\"mcp\"]}}}\n\n\
Examples:\n  \
aikit mcp\n  \
aikit mcp --quiet"
    )]
    Mcp(McpServeArgs),

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
pub struct McpServeArgs {
    /// Suppress the stderr diagnostics (protocol version, client identity).
    #[arg(long)]
    pub quiet: bool,
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
pub struct InitArgs {
    /// Print the machine-readable init record to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,

    /// Require a Git or Mercurial repository; block when there is none.
    #[arg(long, conflicts_with = "require_folder")]
    pub require_repo: bool,

    /// Require a non-repository directory; block when inside a repository.
    #[arg(long, conflicts_with = "require_repo")]
    pub require_folder: bool,
}

#[derive(Debug, Args)]
pub struct RepoDoctorArgs {
    /// Print the machine-readable readiness record to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,

    /// Require a `.git`, `.hg`, or `.aikit` marker; block when there is none.
    ///
    /// Named for a *root*, not a repo: a non-repo `.aikit/` folder satisfies this, and
    /// requiring a repository would be a different and stricter thing.
    #[arg(long)]
    pub require_root: bool,
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
The run root is detected by filesystem walk-up for the nearest enclosing `.git`, `.hg`, \
or `.aikit` marker (no `git`/`hg` subprocess), so this works in a Git repo, a Mercurial \
repo, or a non-repo `.aikit/` folder; with no marker it blocks `blocked_repo_not_found`. \
The root's VCS (git/mercurial/none) is recorded as `vcs` in run.json.\n\n\
Cross-OS runner detection (deterministic, OS-aware) selects the interpreter in this \
order: (1) an explicit `--runner <name>`; (2) the config `script_runner.extension_map`; \
(3) a recognized `#!` shebang unless `--no-shebang`; (4) the built-in extension map; (5) \
an OS-aware default fallback; else a clear blocked failure. Supported extensions: .sh, \
.zsh, .ps1, .cmd, .bat, .py, .js. Runner values: sh, zsh, bash, pwsh, powershell, cmd, \
python, python3, node. On Windows, .ps1 uses pwsh/powershell and .cmd/.bat use cmd \
(no Git Bash required); .sh/.zsh run only if a discoverable interpreter exists. Unknown \
types block with blocked_unknown_script_type; a selected-but-unavailable runner blocks \
with blocked_runner_not_found; an unrecognized --runner blocks with \
blocked_runner_not_allowed. run.json records vcs, detected_runner, detection_source, \
used_shebang, used_extension_map, and the full argv.\n\n\
Clean-tree policy: the default is allow-dirty (these scripts operate on working content). \
`--require-clean` blocks when the tracked tree is dirty; `--allow-dirty` is the explicit \
default; the two cannot be combined. The dirty check is VCS-specific and runs only under \
`--require-clean`: Git uses `git status --porcelain`; Mercurial uses `hg status -mard` \
(run with HGPLAIN=1; the only place the runner invokes `hg`, erroring if `hg` is absent); \
a non-repo `.aikit/` root has no tracked tree, so `--require-clean` blocks with \
blocked_require_clean_unsupported. HEAD is recorded in run.json for Git roots only \
(empty for Mercurial/non-repo). With `--print`, policy is validated and the planned \
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
local work area: .aikit/temp/, .scratch/work/temp/, or .scratch/work/outputs/. The run \
root is detected by filesystem walk-up for an enclosing `.git`/`.hg`/`.aikit` marker (Git \
repo, Mercurial repo, or non-repo `.aikit/` folder). The check \
validates the allowed location, the path/symlink boundary, cross-OS runner detection (same \
order as `script run`: --runner, config extension_map, shebang unless --no-shebang, \
built-in extension map, OS-aware fallback), the best-effort forbidden-operation scan, and \
the clean-tree policy (Git `git status` / Mercurial `hg status -mard` under \
`--require-clean`, which blocks `blocked_require_clean_unsupported` in a non-repo folder). \
The JSON report includes detected_runner, detection_source, \
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
    /// Path to the script to run, or `-` to read the script text from stdin.
    ///
    /// With `-`, aikit writes the script into `.aikit/temp/` itself and runs it from there,
    /// so the allowed-location policy still holds — the caller is spared authoring a file,
    /// not exempted from the rule. `--runner` is then required: there is no filename to
    /// infer from, and inferring one is exactly the guess aikit refuses to make elsewhere.
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

    /// Proceed despite a forbidden-pattern match, naming the exact pattern (repeatable).
    ///
    /// The scan is an accident guard, not a security boundary, so a deliberate operation
    /// needs a compliant route rather than none. Each value must match an advertised
    /// pattern exactly, so acknowledging one never disables the rest, and every
    /// acknowledgement is recorded.
    #[arg(long, value_name = "PATTERN")]
    pub acknowledge_forbidden: Vec<String>,

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

    /// Proceed despite a forbidden-pattern match, naming the exact pattern (repeatable).
    ///
    /// The scan is an accident guard, not a security boundary, so a deliberate operation
    /// needs a compliant route rather than none. Each value must match an advertised
    /// pattern exactly, so acknowledging one never disables the rest, and every
    /// acknowledgement is recorded.
    #[arg(long, value_name = "PATTERN")]
    pub acknowledge_forbidden: Vec<String>,

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

    /// Diff a batch anchor's recorded head against the current working tree.
    #[command(
        long_about = "Produce a mechanical Git diff from a batch anchor's recorded Git head \
to the current working-tree state. The anchor may be given as an anchor id or as a \
repo-relative path to an anchor JSON file. The anchor is validated and must belong to the \
current repository; its recorded `git_head` is used as the diff base and must still exist \
locally (else blocked).\n\n\
The diff (`git diff <base>`) captures committed changes since the anchor and current \
tracked working-tree/index changes. Untracked file CONTENTS are not part of a Git diff and \
are not included — use `batch changed --anchor <anchor>` for a timestamp-based file list. \
This is inspection only: it creates no review bundle or output artifact and never touches \
remotes.\n\n\
This lives under `batch` because it operates on a batch anchor. It was previously the \
top-level `aikit diff anchor`, which implied a general diff facility aikit does not have.",
        after_help = "Examples:\n  \
aikit batch diff <anchor-id>\n  \
aikit batch diff <anchor-id> --patch --json"
    )]
    Diff(DiffAnchorArgs),

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
cross-OS runner-availability/readiness model reported by `aikit doctor` (`env snapshot` \
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

#[derive(Debug, Args)]
pub struct ConfigCli {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the resolved configuration with the source of each value.
    Show(ConfigShowArgs),
}

#[derive(Debug, Args)]
pub struct ConfigShowArgs {
    /// Print the machine-readable report to stdout instead of human-readable text.
    #[arg(long)]
    pub json: bool,
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
The rule set favours precision over coverage, so it can false-negative freely. It never \
proves a file or repo is safe to share, makes no judgment about whether a finding is a live \
credential, and does not replace gitleaks or trufflehog — inspect every finding. It NEVER \
prints raw secret values in human or JSON output; each finding reports the path, line, rule \
id, description, and severity only, so the report is safe to quote verbatim. It creates no \
output artifacts and never touches remotes.\n\n\
Severity is confidence that a match is a real credential, NOT blast radius. high: a \
self-identifying credential format, or a credential-style name assigned a long opaque \
token. medium: a credential-style name assigned a short or word-like value. low: that same \
name-based match in an example/sample/template/fixture path.\n\n\
By default findings are reported and the command exits 0 (usable for inspection). Severity \
is monotonic, so `--fail-on <high|medium|low>` exits 3 with blocked state \
`blocked_secret_findings` when a finding at that severity OR ABOVE is present.",
        after_help = "Examples:\n  \
aikit scan secrets README.md docs\n  \
aikit scan secrets . --fail-on high\n  \
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

    /// Exit 3 (blocked_secret_findings) when findings at this severity **or above** exist.
    ///
    /// Severity is confidence that a match is a real credential, not blast radius:
    /// `high` is a self-identifying credential format; `medium` is a credential-named
    /// field with an ordinary value; `low` is the same in an example/template file.
    /// `--fail-on high` is the useful CI setting — `medium` and `low` include the
    /// placeholder and documentation cases that a name-based rule cannot distinguish.
    ///
    /// Omit the flag to report without gating, which is the default.
    #[arg(long, value_name = "SEVERITY", value_enum)]
    pub fail_on: Option<Severity>,
}

/// Finding confidence, ordered. See `ScanSecretsArgs::fail_on`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Severity {
    /// A credential format that identifies itself (fixed prefix and shape).
    High,
    /// A credential-named field assigned an ordinary value.
    Medium,
    /// As `medium`, in a file whose path marks it an example, template, or fixture.
    Low,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
        }
    }
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
