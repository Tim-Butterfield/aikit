# aikit CLI Specification

## 1. Purpose

`aikit` is a personal compiled CLI for deterministic AI-agent workflow support. It
performs mechanical, repeatable local operations that support AI-agent and
human-in-the-loop workflows, without making semantic, methodology, or governance
judgments itself.

## 2. Design Principles

- **Deterministic local operations** — same inputs and filesystem state produce the
  same output.
- **Filesystem-first behavior** — the filesystem and Git are the sources of truth.
- **Explicit repo-root detection** — operations resolve and anchor to a detected
  repository root.
- **No hidden provider/model fallback** — `aikit` invokes no AI providers and
  performs no silent model substitution.
- **No automatic push/fetch/pull** — `aikit` never mutates remote Git state.
- **No process-state control** — `aikit` performs mechanical checks; it does not
  advance, approve, or override any external review or approval step.
- **No semantic governance judgment** — `aikit` does not decide correctness,
  sufficiency, or compliance.
- **AI agents perform interpretation and judgment** — the CLI provides inputs and
  records outputs for agents/humans to interpret.
- **The CLI performs mechanical operations** — collection, hashing, enumeration,
  controlled execution, and reporting.

## 3. Initial Scope

The initial spec covers only:

- governed local script validation and execution;
- batch anchor creation;
- changed-file discovery from an anchor;
- review bundle generation;
- repo inventory generation.

## 4. Out of Scope

Explicitly out of scope for the initial spec:

- recurring scratch validation as a core command;
- hard-coded methodology validation;
- hard-coded governance validation;
- autonomous approval/rejection decisions;
- remote execution;
- agent skills as the implementation;
- copied script wrappers;
- package-manager orchestration;
- release/distribution automation.

## 5. Command Model

The initial command families. Behaviors below are **specification intent**, not
implementation commitments.

### 5.1 `aikit script run` / `aikit script check`

The `script` command family is a noun (`script`) with verb actions (`run`, `check`).
There is exactly one public way to run a script (`aikit script run`); the earlier
`aikit run script` shape was superseded by this command and is **not** retained as an
alias (see the implementation plan's post-initial command-shape correction).

**`aikit script run` — purpose:**
- execute a local script under policy controls;
- support audit echo;
- capture stdout/stderr/exit code;
- write run metadata;
- block unsafe operations where mechanically detectable.

**`aikit script run` — initial behavior:**
- detect repo root;
- require the script path to be inside allowed project-local locations;
- reject repo escapes;
- optionally require a clean tracked tree;
- echo the command before execution;
- capture a run record;
- propagate the script's exit code unless `aikit` blocks first.

**Cross-OS runner detection.** The interpreter/runner is selected deterministically and
is OS-aware. Supported extensions: `.sh`, `.zsh`, `.ps1`, `.cmd`, `.bat`, `.py`, `.js`.
Symbolic runner names: `sh`, `zsh`, `bash`, `pwsh`, `powershell`, `cmd`, `python`,
`python3`, `node`. Selection order:

1. explicit `--runner <name>`;
2. config `script_runner.extension_map` for the extension;
3. a recognized `#!` shebang (unless `--no-shebang` or `detect_from_shebang=false`);
4. the built-in extension map;
5. an OS-aware default fallback (candidate resolution itself filters by OS and PATH);
6. a clear blocked failure.

`script_runner.preferred_runners` reorders candidates within tiers 2/4. Command
construction uses argv arrays (never concatenated shell strings): PowerShell uses
`<pwsh|powershell> -NoProfile -ExecutionPolicy Bypass -File <script>`; `.cmd`/`.bat` use
`cmd /C <script>`; Python/Node/sh/zsh/bash use `<interp> <script>`. On Windows, `.ps1`
and `.cmd`/`.bat` run with native mechanisms (no Git Bash needed); `.sh`/`.zsh` run only
when a discoverable interpreter exists. `run.json` records `detected_runner`,
`detection_source`, `used_shebang`, `used_extension_map`, the resolved `interpreter`, and
the full `argv`.

The forbidden-operation scan is OS-agnostic substring matching (it is **not** a security
sandbox), and allowed script *input* locations are unchanged by this behavior.

**`aikit script check` — purpose:**
- validate a script against the same policy without executing it;
- report whether the policy accepts the script, and the blocked state when it does not.

**`aikit script check` — initial behavior:**
- detect repo root;
- resolve/canonicalize the script path and validate the allowed location, path/symlink
  boundary, and runner detection (same order as `script run`);
- run the best-effort forbidden-operation scan;
- apply the clean-tree policy;
- do not execute the script, do not copy it, and create no run output (no run
  directory, `stdout.txt`, `stderr.txt`, or `run.json`);
- report the detection metadata (`detected_runner`, `detection_source`, `used_shebang`,
  `used_extension_map`, `argv`) in the `--json` report;
- exit 0 when the policy accepts the script and exit 3 with the named blocked state when
  it does not.

### 5.2 `aikit batch start`

**Purpose:**
- create a batch anchor before AI-agent work begins.

**Behavior:**
- write a **minimal timestamp-reference** anchor; the anchor does NOT capture Git status
  (`git_status_porcelain` is not recorded), because anchor-based changed-file discovery is
  timestamp-based against the anchor file's mtime;
- include a timestamp (`created_at` / `filesystem_anchor_time`, UTC);
- include the repo root;
- include the current HEAD and branch (identifying metadata, not working-tree status);
- include the aikit version that created the anchor (`aikit_version`);
- with `--snapshot`, include an optional initial snapshot of tracked files
  (`initial_snapshot`; off by default and never a full repo content scan);
- output the anchor path.

### 5.3 `aikit batch changed`

**Purpose:**
- report existing files modified since a batch anchor.

**Behavior (timestamp-based):**
- read the anchor; the reference point is the anchor **file's** filesystem mtime;
- report existing repo files whose filesystem mtime is newer than the anchor;
- do **not** consult `git status`: tracked/untracked/staged/unstaged status is not the
  deciding factor. A file that is dirty relative to `HEAD` but was last modified before
  the anchor is excluded; a file modified after the anchor is reported whether or not it
  is tracked. A clean Git tree is not required;
- deleted files are out of scope (no content exists on disk to bundle);
- honor `.gitignore`/`.git/info/exclude`, hard-exclude aikit's own areas (`.git/`,
  `.aikit/`, `.scratch/`, `.claude/`) and configured build/dependency directories, apply
  configured include/exclude globs, never follow symlink escapes;
- produce a deterministic, repo-relative, sorted list (status `modified`, source
  `anchor_mtime`); `--hash` adds a SHA-256; `--json` for machine output. mtime is a
  best-effort heuristic.

### 5.4 `aikit review generate`

**Purpose:**
- generate a bounded review bundle for AI/human review.

**Initial behavior:**
- accept explicit files (`--files`) or files changed since an anchor (`--anchor`);
- in anchor mode, use the **same timestamp-based discovery as `batch changed`** (existing
  files whose filesystem mtime is newer than the anchor file; not `git status`; dirty-vs-
  `HEAD`-but-older-than-anchor files are excluded; deleted files out of scope; no clean
  tree required);
- include file paths, SHA-256 hashes (hashed only for files actually included), sizes;
- include line counts where practical;
- apply byte/line caps and report truncation;
- include repo metadata.

**Output convention (default, unchanged):**
- each bundle writes two files under `.aikit/outputs/reviews/<review-id>/`: the readable
  text bundle `review_bundle.txt` and `manifest.json` (whose `bundle_path` records
  `review_bundle.txt`). The text bundle was renamed from the historical
  `run_for_review.txt` in a post-initial cleanup; older local review outputs may still
  carry the old name, but new generation uses `review_bundle.txt` and never writes both.

**Single-file / embedded-manifest output (opt-in):**
- `--single-file` writes exactly one bundle file with the manifest embedded (a fenced
  `## Manifest` JSON block followed by `## Files`), no review directory, and no sidecar
  `manifest.json` (default path `tmp/review_bundle.txt`; override with `--output <file>`).
  The contract fails clearly when it cannot be satisfied (path escape, or a directory in
  the way).
- `--embed-manifest` embeds the manifest without changing the directory layout;
  `--no-sidecar-manifest` suppresses the sidecar `manifest.json` in directory mode.
- the manifest gains `embedded_manifest` / `sidecar_manifest` flags recording the shape.

**Enhanced anchor discovery (opt-in):**
- the default timestamp walk honors `.gitignore`, so ignored files are skipped (untracked
  non-ignored files modified after the anchor are already included by default).
- `--include-ignored-batch-files` (or config) additionally pulls in **allowlisted ignored**
  files modified after the anchor (per `include_globs` minus the exclude globs).
- protective default excludes (`**/.git/**`, `.aikit/outputs/{raw,provider,secrets}/**`,
  `**/node_modules/**`, `**/target/**`, `**/dist/**`, `**/build/**`) are always applied
  and can only be added to via `exclude_globs`; aikit's own areas (`.git/`, `.aikit/`,
  `.scratch/`, `.claude/`) are hard-excluded. Dependency/build directory names are matched
  anywhere in the tree (nested `pkg/node_modules/` is protected too). Paths that escape the
  repo are never included.

**Manifest detection source.** In anchor mode every included file records the
timestamp-based source `anchor_mtime` (never `git_status`); configured `include_files`
record `explicit`. Explicit-files mode (`--files`) has no mechanical change classification,
so `source` is omitted/null there — aikit does not invent a classification it cannot derive
deterministically. Deleted files are out of scope and produce no manifest entry. The
manifest also records the generating `aikit_version`.

**Configuration:**
- defaults for the options above may be set in `aikit.config.json` (repo root) or
  `.aikit/config.json`, with precedence: built-in defaults < `aikit.config.json` <
  `.aikit/config.json` < CLI flags. Keys: `bundle.{single_file, embed_manifest,
  sidecar_manifest, output}` and `discovery.{include_ignored_batch_files, include_globs,
  exclude_globs, include_files}`. Malformed config fails clearly. See
  `aikit.config.example.json`.

### 5.5 `aikit inventory repo`

**Purpose:**
- generate a mechanical repo inventory.

**Initial behavior:**
- list files subject to include/exclude rules;
- include sizes and hashes;
- identify likely tooling/config files;
- avoid semantic conclusions.

### 5.6 `aikit repo init` / `aikit repo doctor`

The `repo` command family is **post-initial Slice 2** (not part of the completed initial
implementation batches; see the implementation plan's post-initial slice section). It
uses the same noun-family / action grammar as the rest of the CLI.

**`aikit repo init` — purpose:**
- prepare the current repository for local aikit usage.

**`aikit repo init` — behavior:**
- detect the repo root (block `blocked_repo_not_found` outside a repository);
- create `.aikit/` and `.aikit/temp/` if missing (idempotent);
- ensure `.aikit/` is locally ignored, preferring `.git/info/exclude` (local Git
  metadata, never staged) and never modifying `.gitignore`; add no duplicate entry when
  `.aikit/` is already ignored by any Git ignore source;
- create no output artifacts, no `.scratch/`, and no `.claude/`; run no build/test/review
  commands; never touch remote Git state;
- report what was already present and what was created (with `--json`).

**`aikit repo doctor` — purpose:**
- report repo-local aikit readiness without mutating the repository.

**`aikit repo doctor` — behavior:**
- detect the repo root (block `blocked_repo_not_found` outside a repository);
- create and modify nothing (read-only): no `.aikit/`, `.scratch/`, `.claude/`,
  `.aikit/outputs/`, `.gitignore`, or `.git/info/exclude`;
- report repo root, branch, HEAD, tracked clean/dirty state, `.aikit/` `.aikit/temp/`
  `.aikit/outputs/` existence, ignore status and source, the default output root, allowed
  script input locations, the version, warnings, and an overall readiness summary (with
  `--json`);
- report **runner availability** (`runners`) for every supported runner (`sh`, `bash`,
  `zsh`, `pwsh`, `powershell`, `cmd`, `python3`, `python`, `node`), each with `available`
  and `applicable` (OS-applicability) flags, plus `any_runner_available`. Runner
  availability mirrors `policy::script` and is deterministic per OS. The legacy
  `interpreters` field (`/bin/sh`, `/bin/zsh`) is retained as informational only;
- **readiness** = sane local aikit state (temp dir present, `.aikit/` ignored) **and** at
  least one supported runner available for the current OS. It does not require any
  specific Unix shell, so Windows is ready with `pwsh`/`cmd` and a host without `zsh` is
  still ready; `zsh` is optional unless a `.zsh` script is actually selected;
- exit 0 when a repository is found, even with warnings; treat missing `.aikit/temp/`,
  ignore coverage, or no available runner as warnings rather than failures.

### 5.7 `aikit output list` / `aikit output show` / `aikit output clean`

The `output` command family is **post-initial Slice 3** (not part of the completed
initial implementation batches). It manages local aikit output artifacts under an output
root (default `.aikit/outputs/`), using the noun-family / action grammar. Known artifacts
are `batches/*.json` files and `inventory/`, `reviews/`, and `runs/` subdirectories;
arbitrary files elsewhere are not treated as aikit output artifacts.

**`aikit output list` — behavior:**
- detect the repo root (block `blocked_repo_not_found` outside a repository);
- inspect the selected output root; if it does not exist, succeed with an empty list;
- list known artifacts only, sorted by family then artifact id, with family, id, path,
  type, size, and modified time;
- read-only (create/delete nothing); support `--family`, `--root`, `--json`.

**`aikit output show <artifact-path-or-id>` — behavior:**
- detect the repo root (block `blocked_repo_not_found` outside a repository);
- resolve the argument as a path under the output root or as an artifact id matched
  against the known family folders; reject ambiguous ids and paths that resolve outside
  the output root (`blocked_path_escape`); a missing artifact is `blocked_artifact_not_found`;
- report the artifact family/id/path, the files it contains, and a compact summary of its
  main JSON; read-only; support `--root`, `--json`. Inspection only — no correctness
  judgment.

**`aikit output clean` — behavior:**
- safe by default: dry-run unless `--execute`, and `--execute` requires a selector
  (`--older-than <n>h|<n>d` or `--all`); with neither selector, list candidates in dry-run
  and delete nothing;
- delete only known artifacts inside the selected output root; never outside the root,
  never via symlink escapes, and never `.aikit/temp/`, `.scratch/`, `.claude/`, `target/`,
  or `.git/`; leave family directories in place;
- `--older-than` and `--all` are mutually exclusive; `--older-than` is parsed safely
  (overflowing values are rejected); an explicit `--root` is restricted to
  `.aikit/outputs/` or `.scratch/work/outputs/` so management cannot be redirected at
  non-output directories; support `--family`, `--root`, `--json`; report mode, filters,
  candidates, and the exact deleted paths.

### 5.8 `aikit batch list` / `aikit batch show` / `aikit diff anchor`

These commands are **post-initial Slice 4** (not part of the completed initial batches).
They are mechanical inspection/diff commands: they do not auto-select a "latest" anchor,
advance workflow state, perform semantic review, create review bundles, or touch remotes.
`batch list`/`batch show` extend the `batch` family; `diff anchor` is a new `diff` family.

**`aikit batch list` — behavior:**
- detect the repo root (block `blocked_repo_not_found` outside a repository);
- inspect the selected output root's `batches/` folder; empty success if it is absent;
- list only valid batch anchor JSON files, sorted by anchor id; report invalid files as
  skipped (not guessed); read-only; support `--root`, `--json`;
- it does NOT auto-select any anchor.

**`aikit batch show <anchor-path-or-id>` — behavior:**
- detect the repo root (block outside a repository);
- resolve the argument as a repo-relative anchor path or an id under the batches/ folder;
  reject path escapes (`blocked_path_escape`); validate it is a batch anchor belonging to
  the current repo (else `blocked_missing_anchor` / `blocked_invalid_anchor`);
- read-only; support `--root`, `--json`; does NOT auto-select.

**`aikit diff anchor <anchor-path-or-id>` — behavior:**
- detect the repo root (block outside a repository); resolve and validate the explicit
  anchor (same blocked states as `batch show`);
- use the anchor's recorded `git_head` as the diff base; the base must exist locally, else
  `blocked_missing_base_commit`;
- generate a deterministic `git diff <base>` against the current working tree (committed
  changes since the anchor plus current tracked worktree/index changes); untracked file
  contents are not included (callers use `batch changed` for a timestamp-based file list);
- create no review bundle or output artifact; never touch remotes; support `--stat`
  (included by default), `--patch`, and `--json`.

### 5.9 `aikit env snapshot` / `aikit scan secrets`

These commands are **post-initial Slice 5** (not part of the completed initial batches),
the final slice of the approved five-slice post-initial command expansion. `env snapshot`
is a new `env` family; `scan secrets` is a new `scan` family. Neither calls AI providers,
touches remotes, makes semantic governance decisions, or creates durable output artifacts
by default; both prefer stdout plus `--json`.

**`aikit env snapshot` — behavior:**
- report a bounded, mechanical set of local environment facts for debugging aikit usage;
- read-only: create no files or directories, modify no repo files (including
  `.git/info/exclude`), run no network commands, and never touch remotes;
- detect the Git repo when inside one and report repo facts (root, branch, HEAD, tracked
  clean/dirty, default output root, `.aikit/` `.aikit/temp/` `.aikit/outputs/` existence,
  `.aikit/` ignore status); when outside a repo, still report the non-repo facts and record
  a warning (repo facts `null`);
- also report the aikit version, current executable, OS family, CPU architecture, working
  directory, **legacy/informational shell-interpreter probes** (`/bin/sh`, `/bin/zsh`),
  local git/Rust/Cargo versions, and the shell from `$SHELL`. These shell probes are
  informational only and are NOT the cross-OS runner-availability/readiness model that
  `repo doctor` reports (§5.6); `env snapshot` does not report runner availability;
- **do not** dump all environment variables, the raw `PATH`, tokens, credentials, private
  keys, SSH-agent/cloud credentials, or any provider/model-specific or network-derived
  information; `PATH` is summarized only (entry count plus an on-PATH boolean);
- support human output and `--json` (`kind: aikit.env_snapshot`).

**`aikit scan secrets <path>...` — behavior:**
- detect the repo root (block `blocked_repo_not_found` outside a repository);
- require at least one explicit path (the whole repo is never scanned implicitly unless a
  path is the repo root or `.`); resolve paths relative to the repo root; reject paths
  outside the repo and symlink/path escapes (`blocked_path_escape`); always exclude `.git/`;
- scan explicit files even when ignored; for directories, traverse deterministically and
  respect `.gitignore` by default (`--include-ignored` includes ignored files); skip binary
  files and files larger than `--max-file-bytes` (default 1 MiB);
- run a best-effort heuristic rule set for obvious likely secrets (private-key block
  markers, credential-style assignments, long token-like values, generic access-key
  identifiers); the scan is heuristic only — it can false-positive/false-negative, never
  proves a file or repo is safe to share, and does not judge whether a finding is live;
- **never** print raw matched secret values (human or JSON); each finding reports path,
  line, rule id, description, severity, and `redacted: true`; create no output artifacts;
- by default report findings and exit 0; with `--fail-on-findings`, exit 3 with
  `blocked_secret_findings` when findings are present;
- support human output and `--json` (`kind: aikit.scan_secrets`).

### 5.10 `aikit version`

- report the package/binary version and build metadata;
- `aikit --version` prints the standard clap string (`aikit <version>`); `aikit version`
  prints a compact human report; `aikit version --json` emits `kind: aikit.version` with
  `name`, `version`, `git_commit`, `build_profile`, `os`, `arch`, `target`, and
  `rust_profile` (git/profile/target are best-effort build-time values and may be null);
- read-only; creates nothing; works outside a Git repository.
- The package version is recorded in batch anchors (`aikit_version`) and review manifests
  (`aikit_version`), and reported by `env snapshot`.
- **Build-metadata freshness:** `git_commit` is captured by `build.rs`, which watches
  `.git/HEAD`, the current branch ref (`.git/refs/heads/<branch>`), and `.git/packed-refs`
  so a normal commit refreshes it on the next build. Limitation (best-effort): detached
  HEAD, freshly packed refs, or linked-worktree gitdir layouts may leave `git_commit`
  stale until the next rebuild. Build metadata never fails the build when git is
  unavailable.

### 5.11 Configuration files

- Optional, layered config (lowest to highest precedence): built-in defaults <
  `aikit.config.json` (repo root) < `.aikit/config.json` < CLI flags.
- Sections: `bundle` (`single_file`, `embed_manifest`, `sidecar_manifest`, `output`),
  `discovery` (`include_ignored_batch_files`, `include_globs`, `exclude_globs`,
  `include_files`), and `script_runner` (`preferred_runners`, `detect_from_shebang`,
  `detect_from_extension`, `extension_map`). A `_comment` key is accepted (and ignored)
  anywhere so an annotated example can be copied verbatim.
- Protective exclude globs are always applied and can only be added to, never removed.
- Malformed config (invalid JSON or an unknown field) fails clearly. Unknown runner names
  in `script_runner.preferred_runners` or `script_runner.extension_map` are validated
  before runner detection and rejected with `blocked_runner_not_allowed` (not silently
  skipped, and not surfaced as a misleading `blocked_runner_not_found`). See
  `aikit.config.example.json` for a generic, annotated example.
- Version concepts are distinct: the Cargo package version (`aikit version`), and the
  per-record `schema_version` (anchors/manifests/run records). They are never conflated.

## 6. Output Conventions

- The default output root is always `.aikit/outputs/` under the detected repo root.
- Command-family default output directories are:
  - `.aikit/outputs/batches/`
  - `.aikit/outputs/inventory/`
  - `.aikit/outputs/reviews/`
  - `.aikit/outputs/runs/`
- `--output <path>` overrides the default output root; a relative `--output` resolves
  under the repo root.
- `.scratch` is never auto-selected and is never auto-created. It may be used only when
  explicitly requested through `--output` (for example, `--output .scratch/work/outputs/aikit`).
- Commands that write files print the exact created artifact paths in human output, and
  commands that support `--json` include machine-readable artifact paths in JSON output.
- Generated/local output directories are local-only and should not be committed.

## 7. Blocked States

Blocked states are explicit, named, mechanical conditions. Examples:

- `blocked_repo_not_found`;
- `blocked_path_escape`;
- `blocked_script_not_allowed`;
- `blocked_dirty_tree`;
- `blocked_forbidden_operation`;
- `blocked_missing_anchor`;
- `blocked_invalid_anchor`;
- `blocked_unreadable_file`;
- `blocked_unknown_script_type` (script extension/shebang yields no known runner);
- `blocked_runner_not_found` (selected runner has no available interpreter on this OS);
- `blocked_runner_not_allowed` (explicit `--runner` is not a recognized runner);
- `blocked_missing_base_commit`;
- `blocked_secret_findings`.

## 8. Historical Sources and Patterns

`aikit` may be **informed by** prior local tools and patterns, but the initial spec
copies **no** scripts directly. Source patterns include:

- IDesign `run-batch.zsh` (path-prefix safe-zone runner + audit echo);
- legacy `run-step.mjs` (manifest runner, concurrency lock, clean-tree preflight,
  no commit/push);
- the historical "newer" timestamp anchor pattern (changed-file discovery);
- the `run_for_review` / `archtool` review-bundle pattern (hashed, capped bundles);
- the `repo_inventory` pattern (mechanical repo enumeration).

These are **source patterns, not implementation commitments**. Durable behavior must
be **re-specified and implemented in Rust**, not ported verbatim.

## 9. Implementation Direction

- Rust preferred.
- Single compiled binary.
- Subcommands.
- Implemented in Rust as a single `aikit` binary with subcommands; see
  `aikit-implementation-plan.md` and `implementation-manifest.md` for the realized
  module layout and per-command details.

## 10. Open Decisions

Settled during implementation (see `aikit-implementation-plan.md` and
`implementation-manifest.md` for specifics):

- crate / module structure;
- anchor, review-bundle, inventory, and run-record formats and their JSON schemas;
- default include/exclude rules;
- best-effort policy scan rules;
- config files and precedence (see §5.11): `aikit.config.json` and `.aikit/config.json`
  are read, layered over built-in defaults and under CLI flags, with `bundle`,
  `discovery`, and `script_runner` sections;
- cross-OS script runner detection (see §5.1).

Still open:

- install method;
- whether to support agent wrapper files later;
- whether to expose a dedicated config-schema version (currently config has no separate
  schema version; the package version and per-record `schema_version` remain distinct).
