# aikit

`aikit` is a personal compiled CLI for deterministic AI-agent workflow support.

## Status

- Personal tool — built primarily for the architect's own use.
- Private repo — pushed to the private GitHub remote; not currently intended for
  public distribution, and may never be.
- Implemented so far: `aikit batch start`, `aikit batch changed` (Batch 1),
  `aikit inventory repo` (Batch 2), `aikit review generate` from explicit files
  (Batch 3) or a batch anchor (Batch 4), and the `aikit script` family —
  `aikit script run` / `aikit script check` (Batch 5, then corrected to the `script`
  command shape in post-initial Slice 1) — Rust scaffold, repo-root detection, anchor
  JSON, changed-file detection, a deterministic hashed repository inventory, bounded
  review-bundle generation, and a governed local script runner/validator (not a
  security sandbox).
- Intentionally not implemented: the precomputed `--changed <changed.json>` review
  mode (anchor mode covers the changed-since-anchor case; this would only be added
  later if a real need appears).

## Purpose

`aikit` supports AI-agent workflows with **deterministic local operations**:

- batch anchoring — mark a point in time before AI-agent work begins;
- change discovery — report what was created/modified since an anchor;
- review bundle generation — produce a bounded, hashed review surface;
- repo inventory — generate a mechanical inventory of the repository;
- governed script handling — validate (`script check`) and run (`script run`) local
  scripts under explicit policy controls.

## Non-Goals

`aikit` is:

- **not** an autonomous agent;
- **not** a methodology validator;
- **not** a governance judge;
- **not** a provider/model router;
- **not** a remote execution framework;
- **not** a replacement for Git;
- **not** a copied collection of old scripts.

## Command Families

- `aikit script run`
- `aikit script check`
- `aikit batch start`
- `aikit batch changed`
- `aikit review generate`
- `aikit inventory repo`
- `aikit repo init` (post-initial Slice 2)
- `aikit repo doctor` (post-initial Slice 2)
- `aikit output list` (post-initial Slice 3)
- `aikit output show` (post-initial Slice 3)
- `aikit output clean` (post-initial Slice 3)
- `aikit batch list` (post-initial Slice 4)
- `aikit batch show` (post-initial Slice 4)
- `aikit diff anchor` (post-initial Slice 4)
- `aikit env snapshot` (post-initial Slice 5)
- `aikit scan secrets` (post-initial Slice 5)
- `aikit version`

## Implementation Direction

- Rust.
- One compiled binary named `aikit`.
- Subcommand-based.
- Machine-readable output where useful.
- No runtime dependency on shell, Python, or Node for core behavior.

## Development Posture

`aikit` is developed with a deliberately **lightweight posture**:

- Specification-first — design decisions are settled in docs before code is written.
- Minimal moving parts — one binary, no sprawling runtime dependencies.
- Incremental — build the smallest useful slice, validate it, then grow.
- Low ceremony — avoid heavyweight process, frameworks, or tooling that the
  scope does not yet justify.
- Reversible — favor choices that are easy to revisit as the tool matures.

## Relationship to Architect Toolkit

- Separate repo: `aikit` is **not** part of Architect Toolkit.
- Architect Toolkit may consume `aikit` later, but is only one possible future
  consumer.
- `aikit` is **not** Architect Toolkit-specific.

## Building and Usage

`aikit` is a standard Rust binary. Build and install locally:

```sh
cargo build            # debug build at target/debug/aikit
cargo build --release  # optimized build at target/release/aikit
cargo install --path . # install `aikit` onto your PATH
```

Run inside a Git repository. Mark an anchor before a unit of work, then list what
changed since:

```sh
# Create a batch anchor (writes JSON under the default output directory).
aikit batch start
# → Batch anchor created:
#     .aikit/outputs/batches/<anchor-id>.json

# After doing some work, list files modified since the anchor (timestamp-based).
aikit batch changed --anchor .aikit/outputs/batches/<anchor-id>.json

# Add a SHA-256 per file and machine-readable JSON:
aikit batch changed --anchor <anchor.json> --hash --json
```

Notes:

- The default output root is always `.aikit/outputs/`. `.scratch` is never
  auto-selected or auto-created; use it only by passing `--output .scratch/...`.
- Output under `.aikit/outputs/` is **local-only** and should not be committed.
- Commands that create files print the exact created paths (and include them in
  `--json` output), so you never have to infer file names.
- Anchor mode is **timestamp-based**: `batch changed`/`review generate` with `--anchor`
  report existing files whose filesystem mtime is newer than the anchor file. They do
  **not** use `git status`, so a file that is dirty vs `HEAD` but was last modified before
  the anchor is excluded; a file modified after the anchor is included whether tracked or
  not. Deleted files are out of scope (no content to bundle).
- Every command has detailed `--help`. `aikit` calls no AI providers and has no
  knowledge of any AI agent, CLI, or model.

### Repository setup

Recommended first-time setup for a repository — check, prepare, re-check:

```sh
aikit repo doctor   # read-only readiness report
aikit repo init     # prepare local .aikit/temp/ and local ignore coverage
aikit repo doctor   # confirm the repo is now ready
```

- `aikit repo init` creates `.aikit/` and `.aikit/temp/` if missing and ensures
  `.aikit/` is locally ignored. It adds the ignore entry to `.git/info/exclude` (local
  Git metadata, never staged) rather than modifying `.gitignore`, so it does not dirty
  tracked project files. It is idempotent, creates no output artifacts, does not create
  `.scratch/` or `.claude/`, and never touches remote Git state.
- `aikit repo doctor` is **read-only**: it reports the repo root, branch/HEAD, tracked
  clean/dirty state, whether `.aikit/`, `.aikit/temp/`, and `.aikit/outputs/` exist,
  whether `.aikit/` is ignored, the default output root, allowed script input locations,
  the aikit version, any warnings, and an overall `ready` summary. It creates and
  modifies nothing.
- **Runner readiness:** `repo doctor` reports availability for every supported script
  runner (`sh`, `bash`, `zsh`, `pwsh`, `powershell`, `cmd`, `python3`, `python`, `node`),
  each with `available` and `applicable` (OS-applicability) flags, plus
  `any_runner_available`. Readiness means local aikit state is sane **and at least one
  supported runner is available for the current OS** — it does **not** require any
  specific Unix shell (so Windows is ready with `pwsh`/`cmd`, and a host without `zsh` is
  still ready). The legacy `interpreters` field (`/bin/sh`, `/bin/zsh`) is retained as
  informational only and no longer gates readiness.
- Both support `--json`. (If your repo already ignores `.aikit/` via `.gitignore`,
  `repo init` reports that and leaves `.git/info/exclude` untouched.)

### Repository inventory

Generate a deterministic, hashed inventory of repository files:

```sh
aikit inventory repo          # human summary + writes inventory.json/.txt
aikit inventory repo --json   # also prints the inventory JSON to stdout
```

- Output is written by default under `.aikit/outputs/inventory/<id>/` (override the
  root with `--output <dir>`). Both `inventory.json` and `inventory.txt` are produced;
  the `--json` output includes a `written` array of the created file paths. Output is
  local-only.
- Traversal is gitignore-aware and **always** excludes `.git/` and common
  build/dependency/output directories (`target/`, `node_modules/`, `dist/`,
  `build/`, `.venv/`, `venv/`, and aikit's own output dirs), matched by directory
  name rather than substring.
- Files ignored by `.gitignore` are excluded by default; add `--include-ignored`
  to include them (the always-excluded directories still apply).
- `--max-files <n>` limits the listing deterministically (after sorting) and
  records the limitation in the output.
- Each entry records the repo-relative path, size, SHA-256, and a simple
  extension-based `kind_hint`.

### Review bundle

Package files into a bounded, hashed review bundle from one of two input modes —
explicit files, or the files changed since a batch anchor:

```sh
# Explicit files:
aikit review generate --files src/main.rs README.md
aikit review generate --files src/main.rs README.md --json   # also print manifest JSON

# Files modified since a batch anchor (timestamp-based; same as `batch changed`):
aikit review generate --anchor .aikit/outputs/batches/<anchor-id>.json
```

- Exactly one input mode is used per run: `--files <file>...` or
  `--anchor <anchor.json>`. Supplying both, or neither, is invalid usage. The
  precomputed `--changed <changed.json>` mode is **not implemented**.
- Anchor mode is **timestamp-based**: it bundles existing files whose filesystem mtime is
  newer than the anchor file, and excludes everything else. It does **not** use
  `git status`, so a file that is merely dirty vs `HEAD` but was last modified before the
  anchor is excluded; a file modified after the anchor is included whether tracked,
  untracked, staged, or unstaged. Deleted files are out of scope (no content to bundle).
  The anchor must exist, be a valid batch anchor, and belong to this repo
  (missing/invalid/cross-repo anchors are rejected). A clean Git tree is not required.
- Generates two files in a per-review directory:
  - `review_bundle.txt` — a readable bundle with per-file headings, SHA-256,
    size, truncation status, and fenced file contents. (Older local review outputs
    may use the previous name `run_for_review.txt`; new bundles use
    `review_bundle.txt`.)
  - `manifest.json` — `schema_version`, `kind`, `review_id`, `repo_root`,
    `git_head`, `generated_at`, `inputs`, `limits`, `files`, `bundle_path`, and
    `totals`.
- Output is written by default under `.aikit/outputs/reviews/<id>/` (override the root
  with `--output <dir>`). The `--json` output includes a `written` array of the
  created file paths. Output is local-only.
- Input paths are resolved relative to the repo root; paths that escape the repo
  (absolute, `..`, or via a symlink whose real target leaves the repo) are rejected.
- Files are sorted by repo-relative path before caps are applied; every requested
  file appears exactly once in the manifest, whether included, truncated, or omitted.
- Caps apply in both modes and keep the bundle bounded:
  - `--max-file-bytes <n>` / `--max-file-lines <n>` truncate an individual file's
    embedded content and record the truncation and the bound.
  - `--max-total-bytes <n>` omits later files once the running total would be
    exceeded, recording `omitted_reason` / `cap_hit`.
- For anchor mode, `manifest.json` records `inputs.mode = "changed_since_anchor"`
  along with the anchor path and id, plus `inputs.enhanced_discovery` (whether the
  enhanced discovery below was used). Each included file records the timestamp-based
  detection `source` `anchor_mtime` (never `git_status`); configured `include_files` use
  `explicit`.

#### Single-file and embedded-manifest output

Defaults are unchanged (a per-review directory with `review_bundle.txt` +
`manifest.json`). New, opt-in output shapes:

- `--single-file` writes **exactly one** bundle file with the manifest embedded and no
  review directory and no sidecar `manifest.json`. The default path is
  `tmp/review_bundle.txt` (`tmp/` is conventionally git-ignored); override it with
  `--output <file>`. The bundle is `header` → `## Manifest` (fenced JSON) → `## Files`
  (contents). It fails clearly (`blocked_path_escape`, or an error if the path is a
  directory) when the requested path cannot be satisfied.
- `--embed-manifest` embeds the manifest in the bundle text **without** changing the
  directory layout (the sidecar `manifest.json` is still written unless suppressed).
- `--no-sidecar-manifest` suppresses the sidecar `manifest.json` (directory mode only;
  single-file mode never writes one).

#### Enhanced anchor discovery

By default, anchor mode bundles existing files modified after the anchor (timestamp
walk), honoring `.gitignore` so ignored files are skipped.
`--include-ignored-batch-files` (or config) additionally:

- bundles **allowlisted ignored** files modified after the anchor — matched by
  `include_globs` and not by the exclude globs.

(Untracked **non-ignored** files modified after the anchor are already included by the
default timestamp walk, regardless of this flag. Deleted files are out of scope.)

Exclusion always applies the protective default globs (`.git/**`,
`.aikit/outputs/{raw,provider,secrets}/**`, `node_modules/**`, `target/**`, `dist/**`,
`build/**`) plus any configured `exclude_globs`; paths that escape the repo are never
included (the walk does not follow symlinks and considers only regular files).

#### Configuration

Optional config files set defaults for the options above (CLI flags take precedence):

1. built-in defaults
2. `aikit.config.json` (repo root)
3. `.aikit/config.json`
4. CLI flags

A config file may set `bundle.{single_file,embed_manifest,sidecar_manifest,output}`,
`discovery.{include_ignored_batch_files,include_globs,exclude_globs,include_files}`, and
`script_runner.{preferred_runners,detect_from_shebang,detect_from_extension,extension_map}`
(see the script section for runner detection). A higher-precedence file overrides scalar
values; the protective exclude globs are always applied and can only be added to, never
removed. A `_comment` key is accepted (and ignored) anywhere. Malformed config (bad JSON
or an unknown field) fails clearly, and **unknown runner names** in
`script_runner.preferred_runners` or `script_runner.extension_map` are rejected with
`blocked_runner_not_allowed` when a script is run/checked (rather than being silently
skipped). See `aikit.config.example.json` for a generic, annotated example.

This config (a schema-less JSON shape, not a versioned schema) is distinct from the
package version (`aikit version`) and from the per-record `schema_version`.

```sh
# One self-contained bundle (manifest embedded) from a batch anchor, including
# allowlisted ignored artifacts modified since the anchor:
aikit review generate --anchor .aikit/outputs/batches/<anchor-id>.json \
  --single-file --include-ignored-batch-files --output tmp/review_bundle.txt
```

`batch start` also records the `aikit_version` that created the anchor, and
`batch start --snapshot` optionally records an initial snapshot of tracked files (never a
full repo content scan).

### Governed script command family

> **Not a security sandbox.** `aikit script run` reduces *accidental* unsafe
> execution; it does not make an arbitrary script safe. The allowed-location policy
> is the primary control, and the forbidden-operation scan is best-effort (naive
> substring matching, easily bypassed, can false-positive).

`aikit script run` runs a local script through its detected runner (see
cross-OS runner detection below) and records an audit trail; `aikit script check`
applies the same policy but never executes the script and writes no run output:

```sh
aikit script check .aikit/temp/build.sh           # validate policy only; nothing runs
aikit script run .aikit/temp/build.sh             # run; record the audit trail
aikit script run .scratch/work/temp/task.zsh --print   # validate + show plan, do not run
aikit script run .aikit/temp/build.sh --require-clean   # block if the tracked tree is dirty
```

- **Allowed script input locations** (the script must resolve, after symlink
  resolution, to a real file under one of these): `.aikit/temp/`,
  `.scratch/work/temp/`, `.scratch/work/outputs/`. These are *input* locations, not
  output locations.
- **Cross-OS runner detection (deterministic, OS-aware).** Supported extensions:
  `.sh`, `.zsh`, `.ps1`, `.cmd`, `.bat`, `.py`, `.js`. Runner names: `sh`, `zsh`,
  `bash`, `pwsh`, `powershell`, `cmd`, `python`, `python3`, `node`. Selection order:
  1. explicit `--runner <name>`;
  2. config `script_runner.extension_map` for the extension;
  3. a recognized `#!` shebang (unless `--no-shebang` or `detect_from_shebang=false`);
  4. the built-in extension map;
  5. an OS-aware default fallback (candidate resolution filters by OS and PATH);
  6. else a clear blocked failure.

  Commands are built as argv arrays (never concatenated shell strings): PowerShell uses
  `<pwsh|powershell> -NoProfile -ExecutionPolicy Bypass -File <script>`; `.cmd`/`.bat`
  use `cmd /C <script>`; Python/Node/sh/zsh/bash use `<interp> <script>`. **On Windows,
  no Git Bash is required:** `.ps1` runs via pwsh/powershell and `.cmd`/`.bat` via cmd;
  `.sh`/`.zsh` run only when a discoverable interpreter exists. Blocked cases:
  `blocked_unknown_script_type` (no extension/shebang match),
  `blocked_runner_not_found` (selected runner unavailable on this OS),
  `blocked_runner_not_allowed` (unrecognized `--runner`). `run.json` and the
  `script check --json` report record `detected_runner`, `detection_source`,
  `used_shebang`, `used_extension_map`, the resolved `interpreter`, and the full `argv`.
- **Clean-tree policy:** the default is allow-dirty. `--require-clean` blocks when
  the tracked working tree is dirty; `--allow-dirty` is the explicit default; the two
  cannot be combined. Untracked/ignored files (e.g. `.aikit/outputs/`, `.scratch/`)
  do not make the tree dirty.
- **`aikit script run --print`** validates policy and shows the planned command
  without executing (recorded as `executed: false`).
- **`aikit script check`** validates allowed location, path/symlink boundary,
  runner detection (same order as `script run`), the forbidden-operation scan, and the
  clean-tree policy (`--require-clean` / `--allow-dirty`, `--json`; reports detection
  metadata). It never executes or copies the
  script and creates no run directory, `stdout.txt`, `stderr.txt`, or `run.json`; it
  exits 0 when the policy accepts the script and 3 with the named blocked state when it
  does not.
- **Output (`script run`):** a run directory under `.aikit/outputs/runs/<id>/` by
  default (override with `--output <dir>`; `.scratch` output only when requested
  explicitly) containing the copied script (extension retained), `stdout.txt`,
  `stderr.txt`, and `run.json` (interpreter, argv, cwd, require_clean, executed, git
  heads, exit_code, timings, paths, …). Created paths are printed (and included in
  `--json`).
- **Exit code:** an executed script's exit code is propagated; policy blocks return a
  non-zero `blocked_*` error (exit 3); invalid usage is exit 2.

### Version

```sh
aikit --version          # clap string: "aikit <version>"
aikit version            # compact human report
aikit version --json     # machine-readable record (kind: aikit.version)
```

- `aikit version` reports the package/binary version plus best-effort build metadata:
  `git_commit`, `build_profile`, `os`, `arch`, `target` (any of git/profile/target may
  be `null`). It is read-only and works outside a Git repository.
- **Build-metadata freshness:** `git_commit` is captured at build time. `build.rs` watches
  `.git/HEAD`, the current branch's ref file (`.git/refs/heads/<branch>`), and
  `.git/packed-refs`, so a normal commit on the branch triggers a rebuild and refreshes the
  commit. Limitation (best-effort, not over-engineered): detached HEAD, freshly packed
  refs, or linked-worktree gitdir layouts may still leave `git_commit` stale until the next
  rebuild; metadata never fails the build when git is unavailable.
- The package version is the Cargo package version — distinct from the per-record
  `schema_version` used by anchors/manifests/run records. It is also recorded in batch
  anchors (`aikit_version`), review manifests (`aikit_version`), and `env snapshot`.

### Output management

Manage the local artifacts aikit writes under `.aikit/outputs/` (batch anchors,
inventories, review bundles, and run records):

```sh
aikit output list                       # list known output artifacts (read-only)
aikit output show <artifact-path-or-id> # show one artifact's details (read-only)
aikit output clean --dry-run            # show what would be deleted; deletes nothing
aikit output clean --older-than 7d --execute   # delete artifacts older than 7 days
aikit output clean --all --execute      # delete all known output artifacts
```

- `output list` and `output show` are **read-only** — they create and delete nothing.
- `output clean` is **dry-run by default**: it deletes nothing unless you pass
  `--execute`, and `--execute` requires a selector (`--older-than <n>h|<n>d` or `--all`).
- Only **known** artifacts are touched (`batches/*.json` files and `inventory/`,
  `reviews/`, `runs/` subdirectories) inside the selected output root. `clean` never
  deletes outside the output root, never follows symlink escapes, and never touches
  `.aikit/temp/`, `.scratch/`, `.claude/`, `target/`, or `.git/`.
- `--family <batches|inventory|reviews|runs>` narrows the scope; `--root <path>` selects
  a different output root (restricted to `.aikit/outputs/` or `.scratch/work/outputs/`,
  so management can never be redirected at `.git/`, `target/`, or other non-output
  directories); all three support `--json`.

### Batch inspection and anchor diff

Inspect existing batch anchors and diff one against the current tree. These are
**explicit inspection** commands — they never auto-select a "latest" anchor for work;
anchor-consuming commands always take an explicit anchor:

```sh
aikit batch list                      # list batch anchors (read-only)
aikit batch show <anchor-path-or-id>  # show one explicit anchor (read-only)
aikit diff anchor <anchor-path-or-id> # diff the anchor's head vs the current tree
```

- `batch list` and `batch show` are **read-only**; `batch list` reports valid anchors and
  flags invalid files as skipped (never guessed).
- `diff anchor` uses the anchor's **recorded Git head** as the diff base (it must still
  exist locally) and reports committed changes since the anchor plus current tracked
  working-tree changes, via Git. **Untracked file contents are not part of the Git diff**
  — use `aikit batch changed --anchor <anchor>` for a timestamp-based changed-file list.
  `diff anchor` is
  mechanical inspection only: it creates no review bundle or output artifact and never
  touches remotes. `--stat` (included by default), `--patch`, and `--json` are supported.

### Environment snapshot

Capture a bounded, mechanical report of local environment facts useful for debugging
aikit usage:

```sh
aikit env snapshot          # human-readable report
aikit env snapshot --json   # machine-readable report
```

- **Read-only**: it creates no files or directories, modifies no repo files, runs no
  network commands, and never touches remotes. It works inside or outside a Git
  repository (outside a repo, the repo facts are reported as `null`).
- Reports the aikit version, current executable, OS family, CPU architecture, working
  directory, repo facts (root, branch, HEAD, tracked clean/dirty, default output root,
  `.aikit/` existence and ignore status), **legacy/informational shell-interpreter
  probes** (`/bin/sh`, `/bin/zsh`), local git/Rust/Cargo versions, and the shell from
  `$SHELL`. These shell probes are informational only — `env snapshot` does **not** report
  the cross-OS runner-availability/readiness model that `repo doctor` uses.
- It deliberately **does not dump all environment variables**, the raw `PATH`, tokens,
  credentials, private keys, or any provider/model-specific or network-derived
  information. `PATH` is summarized only (an entry count plus whether the current
  executable's directory is on it).

### Secret scan

Run a local, best-effort heuristic scan for likely secrets in **explicit** repo-local
paths (the whole repo is never scanned implicitly unless you pass the repo root or `.`):

```sh
aikit scan secrets README.md docs      # scan explicit files/directories
aikit scan secrets . --fail-on-findings  # scan the repo; exit 3 if anything is found
aikit scan secrets src --json --include-ignored
```

- **Best-effort and heuristic.** It can false-positive and false-negative, **does not
  prove a credential is live**, and **absence of findings does not prove a file or repo
  is safe to share**. It does not replace dedicated secret-scanning tools — inspect every
  finding yourself.
- It **never prints raw secret values** in human or JSON output. Each finding reports the
  file path, line number, rule id, description, and severity only (`redacted: true`).
- At least one path is required. Paths are resolved relative to the repo root; paths
  outside the repo and symlink/path escapes are rejected, and `.git/` is always excluded.
  Explicit files are scanned even when ignored; for directories, traversal respects
  `.gitignore` by default (use `--include-ignored` to include ignored files). Binary
  files and files larger than `--max-file-bytes` (default 1 MiB) are skipped.
- By default findings are reported and the command **exits 0** (usable for inspection).
  With `--fail-on-findings`, a non-empty finding set exits 3 (`blocked_secret_findings`).
  It creates no output artifacts and never touches remotes.

## Install for Local Use

Install the `aikit` binary so downstream repositories can call `aikit ...` directly,
without `cargo run`. `cargo install --path .` builds `aikit` and copies it into Cargo's
bin directory (normally `$HOME/.cargo/bin`). Re-run `cargo install --path .` after
pulling or building a newer local version.

macOS (zsh) — recommended:

```sh
# From the aikit repo
cargo install --path .

# Ensure Cargo-installed binaries are on PATH for zsh
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc

# Verify
aikit --help
```

- After this, other repositories can call `aikit ...` directly.
- On macOS with zsh, `~/.zshrc` is usually the right place to ensure `$HOME/.cargo/bin`
  is on `PATH`. If `$HOME/.cargo/bin` is already on `PATH`, the PATH edit is not needed.
- Avoid `sudo` and avoid copying into system directories for normal personal use.

Linux:

```sh
cargo install --path .
export PATH="$HOME/.cargo/bin:$PATH"
aikit --help
```

Put the `PATH` line in your shell startup file (for example `~/.bashrc` or `~/.zshrc`)
if `$HOME/.cargo/bin` is not already on `PATH`.

Windows:

```sh
cargo install --path .
aikit --help
```

Cargo normally installs to `%USERPROFILE%\.cargo\bin`; ensure that directory is on your
user `PATH`, and open a new terminal after changing `PATH`.

Direct binary without installing (only while developing or testing `aikit` itself):

```sh
cargo build
./target/debug/aikit --help
```

This direct-binary form is for working on `aikit` itself, not for normal downstream use.

## Current State

Batch 1 (`batch start`, `batch changed`), Batch 2 (`inventory repo`), Batch 3 +
Batch 4 (`review generate --files` and `review generate --anchor`), and the
`script` family (`script run` / `script check`) commands are implemented. Post-initial
work has added the corrected `script` command shape (Slice 1), the `repo` family
(`repo init` / `repo doctor`, Slice 2), the `output` family (`output list` /
`output show` / `output clean`, Slice 3), batch inspection + anchor diff
(`batch list` / `batch show` / `diff anchor`, Slice 4), and the `env`/`scan` families
(`env snapshot` / `scan secrets`, Slice 5). This completes the approved five-slice
post-initial command expansion. The precomputed `--changed <changed.json>` review mode
is not implemented (anchor mode covers the changed-since-anchor case). See
[`docs/aikit-cli-spec.md`](docs/aikit-cli-spec.md) for the CLI specification,
[`docs/aikit-implementation-plan.md`](docs/aikit-implementation-plan.md) for the
implementation plan,
[`docs/agent-usage.md`](docs/agent-usage.md) for the agent-agnostic usage guide,
[`docs/agent-integration-examples.md`](docs/agent-integration-examples.md) for
example external-wrapper integration patterns, and
[`docs/decisions/0001-create-aikit.md`](docs/decisions/0001-create-aikit.md) for the
repo-creation decision.
