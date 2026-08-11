# aikit CLI Specification

## 1. Purpose

`aikit` is a local CLI for deterministic AI-assisted repository workflows. It
performs mechanical, repeatable local operations that support AI-agent and
human-in-the-loop workflows, without making semantic, methodology, or governance
judgments itself.

## 2. Design Principles

- **Deterministic local operations** — same inputs and filesystem state produce the
  same output.
- **Filesystem-first behavior** — the filesystem and Git are the sources of truth.
- **Explicit root detection** — operations resolve and anchor to a detected root. The
  VCS-aware commands (setup and the script runner) detect a Git repo, a Mercurial repo, or
  a non-repo `.aikit/` folder by filesystem walk-up, with no `git`/`hg` subprocess
  required.
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

## 3. Scope

This specification covers the current `aikit` command families and their behavior:

- governed local script validation and execution (`script check` / `script run`);
- batch anchor creation, inspection, and changed-file discovery (`batch start` /
  `batch changed` / `batch list` / `batch show`);
- anchor diffing (`batch diff`);
- review bundle generation (`review generate`);
- repo inventory generation (`inventory repo`);
- readiness setup and reporting (`init` / `init --require-repo` / `init --require-folder` / `doctor` /
  `doctor --require-root`);
- serving the script runner to an AI agent over MCP (`mcp`, §5.12);
- configuration reporting (`config show`) and the global `--cwd`;
- local output-artifact management (`output list` / `output show` / `output clean`);
- environment snapshots and heuristic secret scanning (`env snapshot` / `scan secrets`);
- version reporting (`version`);
- configuration files and precedence (§5.11).

## 4. Out of Scope

Explicitly out of scope:

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

The command families. Behaviors below describe current CLI behavior.

### 5.1 `aikit script run` / `aikit script check`

The `script` command family is a noun (`script`) with verb actions (`run`, `check`).
There is exactly one public way to run a script (`aikit script run`); the earlier
`aikit run script` shape was superseded by this command and is **not** retained as an
alias.

**`aikit script run` — purpose:**
- execute a local script under policy controls;
- support audit echo;
- capture stdout/stderr/exit code;
- write run metadata;
- block unsafe operations where mechanically detectable.

**`aikit script run` — behavior:**
- detect the run root by filesystem walk-up for an enclosing `.git`, `.hg`, or `.aikit`
  marker — **no `git`/`hg` subprocess** (block `blocked_repo_not_found` when none is
  found). Works in Git repos, Mercurial repos, and non-repo `.aikit/` folders; the root's
  `vcs` (`git`/`mercurial`/`none`) is recorded in `run.json`;
- require the script path to be inside allowed project-local locations;
- reject repo escapes;
- optionally require a clean tracked tree (`--require-clean`); the dirty check is
  VCS-specific and runs only with this flag — **Git** `git status --porcelain`,
  **Mercurial** `hg status -mard` (run with `HGPLAIN=1`; the only place the runner invokes
  `hg`, erroring if absent); a non-repo root blocks `blocked_require_clean_unsupported`;
- echo the command before execution;
- capture a run record, written **atomically** (temporary + rename), since a reader cannot
  distinguish a truncated record from a run that recorded less;
- accept the script on **stdin** when the path is `-`. aikit then writes the script into
  `.aikit/temp/` itself and runs it from there, so the allowed-location policy, the
  forbidden scan and the run record apply exactly as they do to a caller-authored script —
  the caller is spared authoring a file, not exempted from the rules. `--runner` is required
  in this mode (there is no filename to infer from), and the temporary is removed on every
  exit path. Because aikit generated that file, it applies the rules it applies to any file
  it owns: a UTF-8 BOM for Windows PowerShell 5.1 (never for cmd), and the exit-code
  epilogue — so a stdin script is the one `script run` case where a PowerShell or cmd exit
  code is `epilogue` rather than `unpropagated`;
- **prune old run directories** to `output.retain_runs` (default 100; `0` disables),
  oldest-first, before the run and excluding the run just created. Only `runs/` is pruned.
  The count is reported as `pruned_runs` in the record and in human output, because
  retention deletes audit trails and a silent deletion is indistinguishable from a record
  that was never written. Ordering uses the run id (a fixed-width UTC timestamp prefix), not
  mtime, which a sync or restore can rewrite;
- record **`exit_code_source`** (`interpreter` | `epilogue` | `unpropagated`) so a caller
  knows whether `exit_code == 0` is load-bearing; `script run` executes the caller's own
  file and so never appends an epilogue, making PowerShell and cmd `unpropagated` here;
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

**`aikit script check` — behavior:**
- detect the run root by the same filesystem walk-up as `script run` (Git/Mercurial repo
  or non-repo `.aikit/` folder; no `git`/`hg` subprocess);
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

**Behavior:**
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
  `run_for_review.txt`; older local review outputs may still
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

**Behavior:**
- list files subject to include/exclude rules;
- include sizes and hashes;
- identify likely tooling/config files;
- avoid semantic conclusions.

### 5.6 `aikit init` / `aikit init --require-repo` / `aikit init --require-folder` / `aikit doctor` / `aikit doctor --require-root`

The `repo` command family uses the same noun-family / action grammar as the rest of the
CLI. Setup additionally provides the top-level `aikit init` and the `aikit init --require-folder`
verb. **Repository/root detection here is filesystem-based** (no `git`/`hg` subprocess),
but the marker set differs by command: `init` / `init --require-repo` / `init --require-folder` walk up for
an enclosing `.git` (file or directory) or `.hg` marker only — a non-repo `init` /
`init --require-folder` targets the current directory — while `doctor --require-root` additionally treats a
non-repo `.aikit/` folder as a root (walking `.git` / `.hg` / `.aikit`). Detection works
in Git repos, Mercurial repos, and (for `doctor --require-root`) non-repo `.aikit/` folders even on
hosts without either CLI installed. The *ignore-coverage* step differs by VCS: the
Mercurial path is also CLI-free (it writes `.hg/hgignore.aikit` and `.hg/hgrc` directly),
whereas the Git path still uses the `git` CLI (`git check-ignore` to detect existing
coverage and `git rev-parse --absolute-git-dir` to resolve the git dir for
`.git/info/exclude`) — consistent with aikit's other git operations. So Git *setup*
requires `git`; detection and Mercurial setup do not.

**`aikit init` / `aikit init --require-repo` / `aikit init --require-folder` — purpose:**
- prepare the current directory for local aikit usage. `init` is adaptive (repo mode in a
  repository, folder mode otherwise); `init --require-repo` forces repo mode; `init --require-folder` forces
  non-repo mode.

**behavior (shared):**
- create `.aikit/` and `.aikit/temp/` if missing (idempotent);
- create no output artifacts, no `.scratch/`, and no `.claude/`; run no build/test/review
  commands; never touch remote VCS state;
- report what was already present and what was created, including a `vcs` field
  (`git`/`mercurial`/`none`) and ignore status/source (with `--json`, kind
  `aikit.repo_init`).

**behavior (mode-specific):**
- **repo mode** (`init --require-repo`, or `init` inside a repo): ensure `.aikit/` is locally ignored
  using the VCS's local, never-committed mechanism — for **Git**, `.git/info/exclude`
  (local Git metadata, never staged); for **Mercurial**, a `.hg/hgignore.aikit` pattern
  (`re:^\.aikit/`) registered via `[ui] ignore.aikit` in `.hg/hgrc` (both under `.hg/`,
  never committed or cloned). Never modify a tracked `.gitignore`/`.hgignore`; add no
  duplicate entry when `.aikit/` is already ignored. `init --require-repo` blocks
  `blocked_repo_not_found` when not inside a repository;
- **folder mode** (`init --require-folder`, or `init` outside a repo): create the directories only,
  add no ignore coverage. `init --require-folder` blocks `blocked_repo_present` when run inside a Git
  or Mercurial repository.

**`aikit doctor` / `aikit doctor --require-root` — purpose:**
- report local aikit readiness without mutating anything. `doctor` is adaptive; `repo
  doctor` requires a marker. There is deliberately **no `folder doctor`**: the three-way
  init split exists because init *mutates* (the mutation differs per mode and `init --require-folder`
  must refuse inside a repo), whereas doctor writes nothing — so there would be no distinct
  behavior to provide and nothing to refuse.

**behavior (shared):**
- detect the root by walking up for a `.git`, `.hg`, or `.aikit` marker. Note this requires
  a **marker, not a repository**: a non-repo `.aikit/` folder reports normally;
- create and modify nothing (read-only): no `.aikit/`, `.scratch/`, `.claude/`,
  `.aikit/outputs/`, `.gitignore`, `.git/info/exclude`, `.hgignore`, or `.hg/` state;
- report repo root, `vcs` (`git`/`mercurial`/`none`), branch, HEAD, tracked clean/dirty
  state, `.aikit/` `.aikit/temp/` `.aikit/outputs/` existence, ignore status and source,
  the default output root, allowed script input locations, the version, warnings, and an
  overall readiness summary (with `--json`). For **Mercurial**, ignore coverage is detected
  without invoking `hg`; branch/HEAD come from `hg` (run with `HGPLAIN=1`) and the
  tracked-tree check uses `hg status -mard`. When the VCS CLI is unavailable (reachable
  because detection is filesystem-based — for **either** Git or Mercurial), branch/HEAD are
  empty and the tracked-tree check degrades to "clean", and a single warning records that
  the CLI was unavailable (the dirty check is fallible for both VCSes, so doctor never
  silently misreports a dirty tree as clean);
- report **runner availability** (`runners`) for every supported runner (`sh`, `bash`,
  `zsh`, `pwsh`, `powershell`, `cmd`, `python3`, `python`, `node`), each with `available`
  and `applicable` (OS-applicability) flags, plus `any_runner_available`. Runner
  availability mirrors `policy::script` and is deterministic per OS. The legacy
  `interpreters` field (`/bin/sh`, `/bin/zsh`) is retained as informational only;
- **readiness** = sane local aikit state (temp dir present, and — **in a repository** —
  `.aikit/` ignored) **and** at least one supported runner available for the current OS. A
  non-repo `.aikit/` folder has nothing to ignore against, so it is ready without ignore
  coverage. It does not require any specific Unix shell, so Windows is ready with
  `pwsh`/`cmd` and a host without `zsh` is still ready; `zsh` is optional unless a `.zsh`
  script is actually selected;
- exit 0 when a root is found, even with warnings; treat missing `.aikit/temp/`, ignore
  coverage (in a repo), or no available runner as warnings rather than failures;
- report `root_source`: `marker` when a marker was found, `cwd_no_marker` otherwise. Both
  are just a path in `repo_root`, so without this a caller cannot tell an anchored root
  from an unanchored one.

**behavior (mode-specific), when no marker is found anywhere up the tree:**
- **`aikit doctor`** reports against the current directory with `root_source:
  "cwd_no_marker"`, `vcs: "none"`, `ready: false`, and a warning carrying the `aikit init`
  remediation — at **exit 0**. The documented loop is "doctor first, `init` only if doctor
  reports a gap"; a read-only probe that *fails* in a fresh folder tells a caller nothing
  about what to do next, which is the one place that loop used to break. Text output names
  the unanchored state explicitly rather than printing the directory as a root;
- **`aikit doctor --require-root`** blocks `blocked_repo_not_found`, unchanged.

`ready` is never true for an unanchored report: with no root there is nothing to be ready.

### 5.7 `aikit output list` / `aikit output show` / `aikit output clean`

The `output` command family manages local aikit output artifacts under an output
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

### 5.8 `aikit batch list` / `aikit batch show` / `aikit batch diff`

These are mechanical inspection/diff commands: they do not auto-select a "latest" anchor,
advance workflow state, perform semantic review, create review bundles, or touch remotes.
All three belong to the `batch` family, because all three operate on a batch anchor.
`batch diff` was previously the top-level `aikit diff anchor`; a `diff` namespace holding a
single anchor-scoped subcommand advertised a general diff facility aikit does not provide.

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

**`aikit batch diff <anchor>` — behavior:**
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

`env snapshot`
is an `env` family; `scan secrets` is a `scan` family. Neither calls AI providers,
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
  `doctor --require-root` reports (§5.6); `env snapshot` does not report runner availability;
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
- run a rule set of two kinds: **format rules**, matching self-identifying credential
  formats (private-key block headers, and the fixed prefixes used by VCS-host, chat,
  payment, cloud, model-provider, and package-registry tokens, plus signed-web-token
  structure), and one **name-based rule**, matching a credential-style name assigned a
  literal value. The set is deliberately not a comprehensive corpus: it favours precision
  over coverage, so it can false-negative freely, never proves a file or repo is safe to
  share, and never judges whether a finding is live;
- assign each finding a severity meaning **confidence that the match is a real
  credential** — never blast radius. `high`: a format rule matched, or the name-based rule
  matched a long opaque token-like value. `medium`: the name-based rule matched a short or
  word-like value. `low`: the name-based rule matched inside a path that labels itself an
  example, sample, template, or fixture. An example path demotes **only** the name-based
  rule; format-rule findings keep `high` wherever they sit;
- **never** print raw matched secret values (human or JSON); each finding reports path,
  line, rule id, description, severity, and `redacted: true`; create no output artifacts;
- by default report findings and exit 0; with `--fail-on <high|medium|low>`, exit 3 with
  `blocked_secret_findings` when a finding at **that severity or above** is present
  (severity is monotonic, so `--fail-on low` fails on any finding). Record the chosen
  threshold as `fail_on` (null when the flag is absent) and per-severity totals in
  `counts`;
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

### 5.12 `aikit mcp`

- serve the script runner to an AI agent over the Model Context Protocol, on stdio;
- exposes exactly three tools: **`run`** (execute a script supplied in the call),
  **`list_runners`** (report the interpreters available on this host), and **`agents_md`**
  (return the agent guide for aikit itself);
- **a separate contract from `script run`, not a wrapper.** The script arrives as call
  arguments rather than as a file the caller authors, so nothing is written into the user's
  repository or working tree, no repository is required, `.aikit/` is not used, no run
  record is produced, and the `--require-clean` dirty check does not apply. A temporary
  script does exist while it runs, in a directory under the system temp location. The
  directory and the script are **created** with owner-only permissions rather than created
  and then narrowed, so there is no window in which they are readable in a world-writable
  location; on Windows no ACL is applied and the per-user temp directory is the only
  protection. Deletion when the call ends is **best-effort**: a scanner or a surviving
  grandchild holding a handle can defer it, so the claim is that nothing is written to the
  user's repository, not that nothing reaches disk;
- **`runner` is required on every call and never inferred** — no shebang, extension-map, or
  OS-default tier applies here, because `pwsh` and `powershell` interpret the same `.ps1`
  differently. Runners never fall back to a sibling; an unavailable runner is an error;
- **configuration is not consulted** (`script_runner.extension_map` / `preferred_runners`
  exist only to *choose* a runner, and the caller has already chosen);
- `run` parameters: `runner`, `script`, `cwd` (all required; `cwd` must be an absolute path
  to an existing **directory**), plus optional `stdin`, `env`, `env_base`
  (`"inherit"` | `"minimal"`), and `limits { timeout_ms, max_bytes, on_output_limit }`;
- **limits cannot be removed, only adjusted.** `timeout_ms` defaults to 120 000, is capped
  at 3 600 000, and rejects `null`; `max_bytes` defaults to 32 MiB combined across streams
  and likewise rejects `null`. Clients are not obliged to tell a server that a call was
  abandoned, so the timeout is the only guaranteed stop, and uncapped capture across
  concurrent calls is an unrecoverable failure rather than a slow one;
- `agents_md` takes no parameters and returns the guide as **raw Markdown in the text
  block**, not as a JSON string, so a client that shows the model only the text block gets
  readable prose rather than an escaped blob. The document is compiled into the binary from
  the repository's `AGENTS.md`, so a released binary answers identically wherever it runs
  and with no repository present. Setting the `AGENTS_MD` environment variable to a
  readable file path serves that file instead — the variable is deliberately unprefixed,
  since the tool name already says whose guide it is. An unset override is the normal case;
  a set-but-unreadable path is an error rather than a silent fall back to the embedded copy;
- the forbidden-pattern scan applies to `run` as it does to `script run`, and refusals name
  the pattern and the line. A caller that genuinely intends the operation repeats the call
  with **`acknowledge_forbidden`**, listing the exact advertised patterns it accepts.
  Acknowledgement is per call and per pattern: it is never remembered across calls, and
  acknowledging one pattern never disables the others. The CLI spelling is
  `--acknowledge-forbidden <pattern>` (repeatable) on `script run` and `script check`;
- results carry **both** a text block and `structuredContent` against a declared
  `outputSchema`. `outputSchema` governs non-error results only, and not every client
  forwards `structuredContent` into model context, so a structured-only result can reach
  the model as an empty response;
- `stop_reason` ∈ `exited` | `timeout` | `output_limit` | `server_limit` | `spawn_failed` |
  `setup_failed`. There is deliberately no `cancelled`: the protocol forbids responding to
  a cancelled request. A non-zero exit code is a **successful** call;
- results report **`exit_code_source`** (`interpreter` | `epilogue` | `unpropagated`),
  because `exit_code: 0` is not equally meaningful across runners: PowerShell exits 0 when
  the last *native* command failed. Where aikit generates the script it appends a
  propagation (`epilogue`); `script run`, which executes the caller's own file, cannot, so
  PowerShell and cmd report `unpropagated` there and `0` means "it ran". Also reported by
  `script run --print`, so the trustworthiness is known before the run;
- results report **`containment`** (`job_object` on Windows, `process_group` on Unix) — the
  mechanism that contains and kills descendants, reported rather than claimed. Neither is
  absolute: explicit breakaway escapes a job, and `setsid`/`setpgid` leaves a process group.
  Establishing the mechanism is a precondition for running, so an executing call always has
  one in force;
- concurrency is capped at 8; over-capacity calls are **rejected** (`server_limit`) rather
  than queued, because an abandoned call holds its slot for its full timeout;
- **not a security sandbox.** It runs arbitrary scripts with the privileges of the aikit
  process; the forbidden-pattern scan is an accident guard, not containment. Approval is
  the MCP client's responsibility — aikit adds no prompt of its own. See `SECURITY.md`;
- **protocol:** both lifecycles are served. A client may open with the `initialize`
  handshake, or with **`server/discover`** and no handshake at all — the latter answers
  with the supported versions, capabilities, instructions and server identity in one
  request. A discover request uses the inline lifecycle and must therefore carry
  self-contained `_meta` (`io.modelcontextprotocol/protocolVersion`, `/clientInfo`,
  `/clientCapabilities`); omitting it is a malformed request, not an unsupported one.
- **revisions:** the server negotiates whichever revision the client proposes, among those
  the SDK supports (`2024-11-05` through `2026-07-28`), and falls back to `2025-11-25` for
  anything it does not recognise. There is no argument to pin or restrict this; the set is
  the SDK's default and is what `server/discover` advertises.
- **state:** per-request lifecycle state only (a concurrency semaphore, a cancellation
  registry). Nothing a client can address survives a `tools/call`, which is what allows the
  stateless flow to work without a session.

### 5.13 `aikit config show`

- report the fully resolved configuration: every setting's effective value and the source
  that set it — `"default"`, or the repo-relative config file that last won;
- provenance is recorded **per assignment**, not per file: a later file that sets one key
  does not become the source of the others, which is the only way the report answers "which
  layer won?" rather than merely "which files were read?";
- loading uses the same code path every other command uses, so what this prints is what
  they will act on — a reimplementation could be wrong in exactly the case the command
  exists to diagnose;
- read-only; creates and modifies nothing; supports `--json`
  (`kind: aikit.config_show`, with `sources` in precedence order and a `settings` array of
  `{key, value, source}`);
- states explicitly that the **MCP server does not consult configuration**, since the
  CLI-consults / MCP-does-not split is the part most likely to be misread.

### 5.14 Global `--cwd <PATH>`

- accepted by every command; aikit runs as if started in that directory;
- applied **before dispatch**, because every command resolves its root from the working
  directory — threading it through each command would leave gaps;
- exists so a caller need not navigate the shell to reach the repository. Shell navigation
  is where cross-platform wrappers break (quoting, spaces, drive-relative paths, `cd`
  semantics), and a wrapper that gets it wrong fails in a way that looks like an aikit error;
- a directory that cannot be entered is **invalid usage (exit 2)**, not a blocked state:
  nothing about a repository has been examined yet, so there is no repository to be blocked
  on, and an invalid argument *value* is the same class of error `clap` reports as 2 when it
  can detect it (`--fail-on bogus`). This one needs the filesystem, so aikit detects it —
  and, unlike the parser, aikit is running, so under `--json` it **does** emit an
  `aikit.error` record (`blocked_state: null`, `exit_code: 2`) before exiting.

### 5.15 `aikit agents-md`

- print the aikit agent guide to stdout — the same document the `agents_md` MCP tool
  returns, from the same compiled-in source, so the two entry points cannot disagree;
- the guide is embedded at build time from the repository's `AGENTS.md`. It therefore works
  with no repository present and outside any repository, and a released binary carries the
  guide that matches its own behavior rather than whatever happens to be in the current
  working directory;
- `AGENTS_MD` (unprefixed, because the command name already scopes it) overrides the source
  with a readable file path. An unreadable override is an error, never a silent fallback;
- read-only; creates nothing; no `--json` (the payload is Markdown, and wrapping prose in a
  JSON string would make it less usable, not more).

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

## 7. Exit Codes and Blocked States

### 7.1 Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Success. |
| 1 | Ordinary command failure (I/O, serialization, a subprocess that could not be run). |
| 2 | Invalid usage: malformed arguments, or an invalid argument value. Mostly owned by `clap`, which rejects arguments **before** any aikit code runs, so no record is produced. A `--cwd` directory that cannot be entered is the same class of error but needs the filesystem to detect, so aikit rejects it — and, being the one running, emits a record under `--json`. |
| 3 | A named blocked state: a deterministic, mechanical refusal. |
| *other* | For `script run` only, the executed script's own exit code is propagated. |

### 7.2 Blocked states — the closed set

Blocked states are explicit, named, mechanical conditions. This is the **complete** set: a
caller may match on it exhaustively and treat an unlisted value as a protocol violation
rather than a state to guess at. Adding one is a schema change. The list is mirrored by
`errors::blocked::ALL` in the source, and a test fails the build if a declared constant is
missing from it, so this table cannot silently fall behind.

| State | Raised when |
| --- | --- |
| `blocked_repo_not_found` | No repository (or, for marker-based commands, no `.git`/`.hg`/`.aikit` marker) was found. |
| `blocked_repo_present` | `init --require-folder` was run inside a Git/Mercurial repository. |
| `blocked_require_clean_unsupported` | `--require-clean` in a non-repo `.aikit/` folder, where there is no working tree to compare. |
| `blocked_path_escape` | A path resolved outside the repository, or through a symlink that does. |
| `blocked_script_not_allowed` | A script resolved outside the allowed input locations. |
| `blocked_dirty_tree` | `--require-clean` was given and the tracked working tree is dirty. |
| `blocked_forbidden_operation` | The static scan matched a forbidden pattern that was not acknowledged. |
| `blocked_missing_anchor` | The named anchor file does not exist or is unreadable. |
| `blocked_invalid_anchor` | The anchor file exists but is not a valid anchor. |
| `blocked_unreadable_file` | A required file is missing, unreadable, or not a regular file. |
| `blocked_unknown_script_type` | The extension and shebang yielded no known runner. |
| `blocked_runner_not_found` | The selected runner has no available interpreter on this OS. |
| `blocked_runner_not_allowed` | An explicit `--runner` named an unrecognized runner. |
| `blocked_missing_base_commit` | The base commit required for a diff is absent. |
| `blocked_secret_findings` | `scan secrets --fail-on <severity>` found a finding at that severity or above. |
| `blocked_ambiguous_artifact` | An artifact id matched more than one artifact. |
| `blocked_artifact_not_found` | No known output artifact matched the given id. |

### 7.3 Failure output under `--json`

A command invoked with `--json` emits **exactly one JSON document on stdout**, whether it
succeeds or fails. Human-readable prose always goes to stderr, so stdout stays a clean
machine channel that a caller can parse without stripping text out of it first.

On failure the document is `kind: aikit.error`:

```json
{
  "schema_version": 1,
  "kind": "aikit.error",
  "ok": false,
  "blocked_state": "blocked_missing_anchor",
  "exit_code": 3,
  "message": "anchor file not found or unreadable: missing.json"
}
```

- `blocked_state` is one of the states above, or **null** for an ordinary failure. That
  distinction is the point of the field: a named state means "change something and retry",
  a null one means the environment failed. Neither requires parsing `message`.
- `message` carries the detail **without** the `state:` prefix that the human stderr line
  uses, since `blocked_state` is already its own field.
- Commands that report a full record *and* exit non-zero — `scan secrets` when its
  `--fail-on` gate trips, and `script check` when policy rejects a script — emit that record
  and **not** an additional error document. The record is the answer and the non-zero exit
  is the gate; a second document would leave stdout unparseable.
- Without `--json`, a failure writes **nothing** to stdout. The error document is opt-in, so
  piping a human-mode command never yields JSON that was not asked for.
