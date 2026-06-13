# aikit Agent Usage Guide

This guide explains how an AI agent (or a human) can use `aikit` mechanically. It is
**agent-agnostic**: nothing here depends on a specific AI vendor, model, or assistant.
`aikit` is a small local command-line tool; this guide describes its commands, its
output conventions, and the assumptions a caller should and should not make.

Every command has detailed `--help`; this guide is an orientation, and the built-in
help is the authoritative per-command reference.

## Purpose

- `aikit` is a local, mechanical CLI for AI-assisted work in a Git repository.
- It helps agents and humans create batch anchors, inspect changed files, inventory a
  repository, generate bounded review bundles, and run constrained local scripts.
- It performs deterministic, repeatable operations on the filesystem and Git state.
- It does **not** decide architecture, methodology, approval, sufficiency, or
  correctness.
- It is **not** an autonomous agent — it runs one command and exits.

## Non-Goals

`aikit` is not:

- an AI provider client (it calls no model or provider);
- an agent runtime;
- a policy brain (it makes no semantic judgments);
- a security sandbox;
- a release system;
- a remote execution system;
- a package-manager orchestrator;
- a replacement for human review.

## Agent-Agnostic Contract

- Any agent can use `aikit` purely through its CLI commands and exit codes.
- Agent-specific skills or wrappers should live **outside** this repository unless their
  inclusion is explicitly approved later. `aikit` itself stays agent-agnostic.
- Durable docs and runtime help must not depend on any specific AI vendor, model, or
  agent name.
- Agents should treat the **filesystem and Git** as the source of truth, not cached
  conversation state.
- After running `aikit`, an agent should report: the exact command run, the exit code,
  the files changed, the outputs created (by path), the checks run, and any blocked
  condition encountered.

### Exit codes

`aikit` uses a small, stable exit-code convention:

- `0` — success.
- `1` — command failure (an unexpected error).
- `2` — invalid usage (bad or conflicting flags; handled by the argument parser).
- `3` — a named **blocked state** (a mechanical precondition was not met, e.g.
  `blocked_repo_not_found`, `blocked_missing_anchor`, `blocked_invalid_anchor`,
  `blocked_path_escape`, `blocked_script_not_allowed`, `blocked_dirty_tree`,
  `blocked_unsupported_mode`, `blocked_forbidden_operation`, `blocked_unreadable_file`).

A blocked state is a deliberate, named refusal — agents must surface it, not ignore it.

## Standard Local Workflow

When setting up a repository for the first time, check → prepare → re-check:

1. `aikit repo doctor` — read-only readiness report.
2. `aikit repo init` — prepare local `.aikit/temp/` and local ignore coverage
   (idempotent; safe to re-run).
3. `aikit repo doctor` — confirm the repo is now `ready`.

A typical local cycle:

1. Confirm repo state (e.g. `git status`); make sure you are inside the intended Git
   repository.
2. Start a batch anchor: `aikit batch start`.
3. Make or inspect changes (edits, generated files, etc.).
4. List what changed since the anchor: `aikit batch changed --anchor <anchor.json>`.
5. Inventory the repository when a file/hashes snapshot is needed:
   `aikit inventory repo`.
6. Generate a review bundle for an explicit set of files:
   `aikit review generate --files <file>...`.
7. Or generate a review bundle for the changed-since-anchor set:
   `aikit review generate --anchor <anchor.json>`.
8. Optionally validate a constrained local script first with
   `aikit script check <script-path>`, then run it (from an allowed location) when
   needed: `aikit script run <script-path>`.
9. Run the project's own checks (build, tests, lint) — these are the project's
   responsibility, not `aikit`'s.
10. Report exact results: command, exit code, created output paths, and any blocked
    state.

## Command Families

### `aikit repo init`

- **Purpose:** prepare the current repository for local aikit usage.
- **Typical use:** first-time setup of a repo (creates `.aikit/temp/` and ensures
  `.aikit/` is locally ignored).
- **Constraints:** must be run inside a Git repository, else `blocked_repo_not_found`.
  Idempotent. Adds ignore coverage to `.git/info/exclude` (local Git metadata, never
  staged), **not** `.gitignore`. Creates no output artifacts and does not create
  `.scratch/` or `.claude/`; never touches remote Git state.
- **Output:** a printed (or `--json`) record of what was already present and what was
  created, including ignore status (`aikit.repo_init`).

### `aikit repo doctor`

- **Purpose:** report repo-local aikit readiness, read-only.
- **Typical use:** confirm a repo is set up (run before/after `repo init`).
- **Constraints:** **read-only** — creates and modifies nothing (no `.aikit/`,
  `.scratch/`, `.claude/`, `.gitignore`, or `.git/info/exclude`). Must be run inside a
  Git repository, else `blocked_repo_not_found`. Exit 0 even with warnings; missing
  `.aikit/temp/` or ignore coverage are warnings, not failures.
- **Output:** a printed (or `--json`) readiness report (`aikit.repo_doctor`): repo root,
  branch/HEAD, tracked clean/dirty, `.aikit/` `.aikit/temp/` `.aikit/outputs/`
  existence, ignore status + source, default output root, allowed script locations,
  interpreter availability, version, warnings, and an overall `ready` flag.

### `aikit batch start`

- **Purpose:** capture a point-in-time anchor (Git HEAD, branch, status, timestamp)
  before a unit of work begins.
- **Typical use:** mark the start of an agent task so later steps can report what the
  task touched.
- **Constraints:** must be run inside a Git repository, else `blocked_repo_not_found`.
- **Output:** a JSON anchor under `.aikit/outputs/batches/<anchor-id>.json` by default.
  The created path is printed (and included in `--json`). Anchors are durable artifacts.

### `aikit batch changed --anchor <anchor.json>`

- **Purpose:** report files created or modified since a given anchor.
- **Typical use:** after doing work, list the change set for review or reporting.
- **Constraints:** the anchor must exist, be a valid anchor, and belong to this repo
  (else a `blocked_*` state). Tracked changes come from `git status`; untracked files
  are included only with `--include-untracked` (a best-effort mtime heuristic);
  deletions are detected for tracked files only.
- **Output:** printed report; `--json` for the machine-readable report; `--hash` adds a
  SHA-256 per existing file. This command reads state and writes no artifact directory.

### `aikit inventory repo`

- **Purpose:** produce a deterministic, hashed inventory of repository files.
- **Typical use:** capture a reproducible snapshot of repo contents for review or
  comparison.
- **Constraints:** traversal is gitignore-aware and always excludes `.git/` and common
  build/dependency/output directories (by directory name). `.gitignore`'d files are
  excluded unless `--include-ignored`; `--max-files <n>` bounds the listing
  deterministically.
- **Output:** `inventory.json` and `inventory.txt` under
  `.aikit/outputs/inventory/<id>/` by default. Created paths are printed; `--json` also
  prints the inventory and a `written` array of paths.

### `aikit review generate --files <file>...`

- **Purpose:** package an explicit set of files into a bounded, hashed review bundle.
- **Typical use:** review a known set of artifacts.
- **Constraints:** paths resolve relative to the repo root and must stay inside the repo
  (escapes via `..`, absolute paths, or symlinks are rejected). Caps
  (`--max-file-bytes`, `--max-file-lines`, `--max-total-bytes`) keep the bundle bounded
  and record truncation/omission.
- **Output:** `run_for_review.txt` and `manifest.json` under
  `.aikit/outputs/reviews/<id>/` by default. Created paths are printed (and in `--json`).

### `aikit review generate --anchor <anchor.json>`

- **Purpose:** package the files changed since an anchor into a review bundle (same
  bundle format as `--files`).
- **Typical use:** review exactly what a unit of work changed.
- **Constraints:** exactly one input mode per run — `--files` or `--anchor`, never both
  or neither (invalid usage otherwise). The anchor must be valid and belong to this
  repo. The precomputed `--changed <changed.json>` mode is **not implemented**.
- **Output:** same as `--files`; `manifest.json` records
  `inputs.mode = "changed_since_anchor"` plus the anchor path and id.

### `aikit script run <script-path>`

- **Purpose:** run a constrained local script through a fixed interpreter and record an
  audit trail.
- **Typical use:** execute a small, repo-local helper script with a recorded run record.
- **Constraints:** see [Script Runner Use](#script-runner-use). This is **not** a
  security sandbox.
- **Output:** a run directory under `.aikit/outputs/runs/<id>/` by default (copied
  script, `stdout.txt`, `stderr.txt`, `run.json`). The executed script's exit code is
  propagated.

### `aikit script check <script-path>`

- **Purpose:** validate a script against the same policy `script run` uses, without
  executing it.
- **Typical use:** confirm a generated script will be accepted (allowed location,
  path/symlink boundary, extension/interpreter, forbidden-operation scan, clean-tree
  policy) before running it.
- **Constraints:** same policy as `script run`; optional `--require-clean` /
  `--allow-dirty` (default allow-dirty) and `--json`. There is no `--print` (the command
  already never executes). This is **not** a security sandbox.
- **Output:** a printed (or `--json`) report with `accepted` / `blocked_state`; the
  script is never executed or copied, and **no** run directory, `stdout.txt`,
  `stderr.txt`, or `run.json` is created. Exit 0 when accepted, exit 3 with the named
  blocked state when blocked.

## Output Locations

- The default output root is always `.aikit/outputs/`.
- Command-family defaults:
  - `.aikit/outputs/batches/`
  - `.aikit/outputs/inventory/`
  - `.aikit/outputs/reviews/`
  - `.aikit/outputs/runs/`
- `--output <dir>` overrides the output root; a relative `--output` resolves under the
  repo root.
- `.scratch` is **never** auto-selected and is never auto-created. It is used only when
  explicitly requested, e.g. `--output .scratch/work/outputs/aikit`.
- Generated output under `.aikit/outputs/` (and any explicit `.scratch` output) is
  local-only and should not be committed.

## Review Bundles

- `review generate --files` is for reviewing an explicit set of artifacts.
- `review generate --anchor` is for reviewing the changed-since-anchor set.
- The precomputed `--changed <changed.json>` review mode is **intentionally not
  implemented** (anchor mode covers the changed-since-anchor case); it would only be
  added later if a real need appears.
- Every bundle produces `run_for_review.txt` (readable, with per-file headings,
  SHA-256, size, truncation status, and fenced contents) and `manifest.json` (schema
  version, ids, repo metadata, inputs, limits, per-file records, totals).
- Agents should report or cite the generated paths rather than guessing them.

## Script Runner Use

- `aikit script run` runs **constrained local scripts only**; `aikit script check`
  applies the exact same policy without executing anything and without writing any run
  output. Use `script check` to confirm a script will be accepted before running it.
- Neither is a **security sandbox**, and running a script does **not** make the script
  safe. The allowed-location policy is the primary control; the forbidden-operation scan
  is best-effort and is **not** a security boundary.
- **Allowed script input locations** (the script must resolve, after symlink
  resolution, to a real file under one of these):
  - `.aikit/temp/`
  - `.scratch/work/temp/`
  - `.scratch/work/outputs/`
- **Supported extensions** (interpreter chosen by extension, never by shebang):
  - `.zsh` via `/bin/zsh`
  - `.sh` via `/bin/sh`
  - extensionless or unknown extensions are rejected.
- `--print` validates policy and prints the planned command **without executing**
  (recorded as `executed: false`; no run directory is created).
- `--require-clean` blocks when the tracked working tree is dirty
  (`blocked_dirty_tree`).
- `--allow-dirty` permits a dirty tracked tree; this is the **default** when neither
  flag is given. `--require-clean` and `--allow-dirty` cannot be combined.
- On execution the output run directory contains the copied script (extension
  retained), `stdout.txt`, `stderr.txt`, and `run.json` (interpreter, argv, cwd, git
  heads, exit code, timings, paths, …).

## Agent-Generated Script Rules

`aikit` validates *where the script file lives* (it must resolve to a real file under an
allowed input location) and records the run. But once execution begins, the script runs
from the repository root through `/bin/sh` or `/bin/zsh`, and `aikit script run` does
**not** constrain which paths the script touches after it starts — it is **not a
filesystem sandbox**. A script under `.aikit/temp/do_stuff.sh` can therefore read and
write files across the repository, which is exactly what makes useful commands (`sed`,
`awk`, build, test, format, inventory, review/dogfood) possible — but a poorly written
script could also reach outside the repository. The agent that generates the script is
responsible for keeping it within the intended repository-local boundary.

> `aikit script run` validates where the script file lives and records the run, but it
> does not sandbox every path the script touches after execution. The agent is
> responsible for generating scripts that operate only within the intended
> repository-local boundary unless the user explicitly approves a wider scope.

**Allowed script behavior:**

- read files under the current repository root;
- write files under the current repository root when the task requires edits;
- write generated/local-only outputs under `.aikit/` or `.scratch/`;
- run local project checks such as build, test, lint, formatting, inventory, and review
  commands;
- use explicit repo-relative paths rooted at the repository root;
- create temporary local working files only in approved local areas, especially
  `.aikit/` or `.scratch/`.

**Disallowed script behavior, unless the user explicitly approves it:**

- `cd ..`, `pushd ..`, or otherwise moving above the repository root;
- reading or writing parent directories;
- reading or writing sibling repositories;
- using `../` paths to escape the repository;
- using arbitrary absolute paths outside the repository;
- touching remote Git state;
- installing packages or tools;
- deleting broad directory trees;
- changing global, user, shell, OS, or system configuration;
- using network operations;
- modifying files outside the intended repository.

**Script style guidance:**

- start with `set -euo pipefail`;
- print section headers before major steps;
- use explicit repo-relative paths;
- avoid hidden side effects;
- avoid destructive commands;
- keep the script small enough to review before execution;
- report what the script is expected to change;
- prefer `--require-clean` when the script is expected only to verify state;
- use `--allow-dirty` only when the script is intentionally operating on a dirty working
  tree.

## What Agents Must Not Assume

- Do **not** assume the "latest" anchor automatically — always pass an explicit
  `--anchor <anchor.json>`.
- Do **not** assume `.scratch` is the default output — the default is `.aikit/outputs/`.
- Do **not** assume `script run` makes a script safe — it does not.
- Do **not** assume any remote/push/fetch/pull behavior exists — `aikit` never touches
  remotes.
- Do **not** assume cleanup is automatic — old anchors, runs, and outputs persist until
  a human or external tooling removes them.
- Do **not** assume unimplemented modes exist (e.g. precomputed
  `--changed <changed.json>`).
- Do **not** treat any database or cache state as authoritative — the filesystem and Git
  are the source of truth.
- Do **not** silently ignore a blocked state (exit `3`) — surface it.
- Do **not** expect `aikit` to switch providers, models, or external tools — it has no
  such behavior.

## Building Agent-Specific Skills or Wrappers Outside aikit

Agent-specific integrations can wrap `aikit`, but they belong **outside** this
repository (so `aikit` stays agent-agnostic). Guidance for such wrappers:

- Call `aikit` CLI commands rather than re-implementing or duplicating its policy.
- Parse the `--json` output where available instead of scraping human text.
- Keep generated outputs local-only; do not commit `.aikit/outputs/` or explicit
  `.scratch` output.
- Do not modify `aikit`'s behavior or change its durable docs to mention a specific
  agent unless that is explicitly approved later.

A minimal wrapper pattern:

1. Confirm the repository (and that it is the intended one).
2. Run the `aikit` command.
3. Capture the command string, exit code, and stdout/stderr.
4. Report the generated output paths.
5. Do not push, fetch, or pull unless a human explicitly instructs it.

## Invocation Modes

There are three ways to invoke `aikit`; pick the one that matches the context.

- **Downstream projects (normal usage).** In a repository that consumes `aikit`,
  invoke the installed binary directly, assuming it is on `PATH`:

  ```sh
  aikit script run .aikit/temp/task.sh --require-clean --json
  ```

  This is the pattern agents should use in real work.

- **Direct local binary testing (inside this repository, before installation).** To
  exercise the compiled binary in the `aikit` repo itself without installing it,
  prefer the built executable directly:

  ```sh
  ./target/debug/aikit script run .aikit/temp/task.sh --require-clean --json
  ```

- **Development convenience (inside this repository only).** `cargo run -- ...` is a
  build-and-run shortcut used while developing `aikit`; it is **not** the normal
  downstream usage pattern:

  ```sh
  cargo run -- script run .aikit/temp/task.sh --require-clean --json
  ```

  Note that `cargo run -- script run ...` is **not** a clean permission-consolidation
  test (see below): if the environment has already granted permission to run
  `cargo run`, the outer invocation is pre-approved and tells you nothing about prompt
  reduction. Agent-facing wrappers should prefer direct `aikit ...` invocation once the
  binary is available, not `cargo run -- ...`.

## Permission-Consolidated Script Runner Pattern

When an agent runs many local checks, each separate command may trigger its own
approval prompt. The script runner can consolidate those into a single top-level
invocation: the inner commands run as child processes of one `aikit script run` call
rather than as separate tool calls.

- **Goal:** reduce repeated per-command approval prompts by making exactly one
  top-level CLI invocation.
- **Put the repeated checks inside one script** in an allowed script input location,
  for example `.aikit/temp/local-checks.sh`. The script holds the commands (build,
  format, lint, test, help surfaces, dogfood runs, etc.).
- **Then invoke exactly one top-level command:**

  ```sh
  aikit script run .aikit/temp/local-checks.sh --require-clean --json
  ```

  Inside this repository, before installation, the equivalent direct-binary form is:

  ```sh
  ./target/debug/aikit script run .aikit/temp/local-checks.sh --require-clean --json
  ```

- **Do not** wrap `aikit script run` inside a larger shell batch when the goal is
  permission consolidation — that reintroduces the outer shell as the thing being
  approved.
- **Do not** run setup commands, `--print`, output inspections, and verification
  commands as separate tool calls if the goal is to measure prompt reduction; put the
  repeated commands in the script instead and make the single `aikit script run` call.
- **Review or trust the script before running it.** `aikit script run` still does
  **not** make a script safe and is **not** a security sandbox — consolidation is about
  fewer prompts, not about safety.
- If the UI still prompts once for the single outer `aikit script run` call, that is
  expected; the goal is to avoid a prompt for every inner command.
- Some environments do not expose permission-prompt behavior to the terminal. When that
  is the case, an agent should report only what it can actually observe (e.g. that the
  inner commands were not separate tool calls) and say plainly that the UI prompt
  behavior was not observable.

## Minimal Command Examples

```sh
# Start a batch anchor (prints the created anchor path).
aikit batch start

# List files changed since an anchor (machine-readable).
aikit batch changed --anchor .aikit/outputs/batches/<anchor-id>.json --json

# Generate a review bundle from explicit files.
aikit review generate --files README.md docs/aikit-cli-spec.md

# Generate a review bundle from the changed-since-anchor set.
aikit review generate --anchor .aikit/outputs/batches/<anchor-id>.json

# Validate a script against the run policy without executing it (no run output).
aikit script check .aikit/temp/task.sh --json

# Validate and show a script's run plan without executing it.
aikit script run .aikit/temp/task.sh --print

# Run a script and record the audit trail under an explicit output location.
aikit script run .aikit/temp/task.sh --output .scratch/work/outputs/aikit

# Permission-consolidated: run many local checks via one top-level invocation
# (the repeated commands live inside the script).
aikit script run .aikit/temp/local-checks.sh --require-clean --json
```

See each command's `--help` for the full set of flags and behavior.
