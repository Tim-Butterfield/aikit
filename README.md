# aikit

`aikit` is a personal compiled CLI for deterministic AI-agent workflow support.

## Status

- Personal tool — built primarily for the architect's own use.
- Private repo — not currently intended for public distribution, and may never be.
- Not yet pushed to any remote.
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

# After doing some work, list created/modified files since the anchor.
aikit batch changed --anchor .aikit/outputs/batches/<anchor-id>.json

# Include new untracked files (best-effort mtime heuristic) and machine-readable JSON:
aikit batch changed --anchor <anchor.json> --include-untracked --hash --json
```

Notes:

- The default output root is always `.aikit/outputs/`. `.scratch` is never
  auto-selected or auto-created; use it only by passing `--output .scratch/...`.
- Output under `.aikit/outputs/` is **local-only** and should not be committed.
- Commands that create files print the exact created paths (and include them in
  `--json` output), so you never have to infer file names.
- Tracked changes come from `git status`; untracked files require
  `--include-untracked`. Deletions are detected for tracked files only.
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
  interpreter availability (`/bin/sh`, `/bin/zsh`), the aikit version, any warnings, and
  an overall `ready` summary. It creates and modifies nothing.
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

# Files changed since a batch anchor (uses the same logic as `batch changed`):
aikit review generate --anchor .aikit/outputs/batches/<anchor-id>.json
```

- Exactly one input mode is used per run: `--files <file>...` or
  `--anchor <anchor.json>`. Supplying both, or neither, is invalid usage. The
  precomputed `--changed <changed.json>` mode is **not implemented**.
- Anchor mode bundles the files changed since the anchor (created/modified, tracked)
  and excludes unchanged files; the anchor must exist, be a valid batch anchor, and
  belong to this repo (missing/invalid/cross-repo anchors are rejected).
- Generates two files in a per-review directory:
  - `run_for_review.txt` — a readable bundle with per-file headings, SHA-256,
    size, truncation status, and fenced file contents.
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
  along with the anchor path and id.

### Governed script command family

> **Not a security sandbox.** `aikit script run` reduces *accidental* unsafe
> execution; it does not make an arbitrary script safe. The allowed-location policy
> is the primary control, and the forbidden-operation scan is best-effort (naive
> substring matching, easily bypassed, can false-positive).

`aikit script run` runs a local script through a fixed interpreter and records an
audit trail; `aikit script check` applies the same policy but never executes the
script and writes no run output:

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
- **Interpreters** come from the extension, never a shebang: `.zsh` → `/bin/zsh`,
  `.sh` → `/bin/sh`. Extensionless or unknown-extension scripts are rejected.
- **Clean-tree policy:** the default is allow-dirty. `--require-clean` blocks when
  the tracked working tree is dirty; `--allow-dirty` is the explicit default; the two
  cannot be combined. Untracked/ignored files (e.g. `.aikit/outputs/`, `.scratch/`)
  do not make the tree dirty.
- **`aikit script run --print`** validates policy and shows the planned command
  without executing (recorded as `executed: false`).
- **`aikit script check`** validates allowed location, path/symlink boundary,
  extension/interpreter, the forbidden-operation scan, and the clean-tree policy
  (`--require-clean` / `--allow-dirty`, `--json`). It never executes or copies the
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
work has added the corrected `script` command shape (Slice 1) and the `repo` family
(`repo init` / `repo doctor`, Slice 2). The precomputed `--changed <changed.json>`
review mode is not implemented (anchor mode covers the changed-since-anchor case). See
[`docs/aikit-cli-spec.md`](docs/aikit-cli-spec.md) for the CLI specification,
[`docs/aikit-implementation-plan.md`](docs/aikit-implementation-plan.md) for the
implementation plan,
[`docs/agent-usage.md`](docs/agent-usage.md) for the agent-agnostic usage guide, and
[`docs/decisions/0001-create-aikit.md`](docs/decisions/0001-create-aikit.md) for the
repo-creation decision.
