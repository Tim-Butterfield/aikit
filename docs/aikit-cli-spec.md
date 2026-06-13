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

**`aikit script check` — purpose:**
- validate a script against the same policy without executing it;
- report whether the policy accepts the script, and the blocked state when it does not.

**`aikit script check` — initial behavior:**
- detect repo root;
- resolve/canonicalize the script path and validate the allowed location, path/symlink
  boundary, and extension/interpreter;
- run the best-effort forbidden-operation scan;
- apply the clean-tree policy;
- do not execute the script, do not copy it, and create no run output (no run
  directory, `stdout.txt`, `stderr.txt`, or `run.json`);
- exit 0 when the policy accepts the script and exit 3 with the named blocked state when
  it does not.

### 5.2 `aikit batch start`

**Purpose:**
- create a batch anchor before AI-agent work begins.

**Initial behavior:**
- write anchor metadata;
- include a timestamp;
- include the repo root;
- include the current HEAD;
- include a current Git status summary;
- output the anchor path.

### 5.3 `aikit batch changed`

**Purpose:**
- report files created or modified since a batch anchor.

**Initial behavior:**
- read the anchor;
- compare filesystem mtimes and/or Git state;
- produce a deterministic changed-file list;
- support human-readable and machine-readable output later.

### 5.4 `aikit review generate`

**Purpose:**
- generate a bounded review bundle for AI/human review.

**Initial behavior:**
- accept explicit files, changed files from an anchor, or configured include lists;
- include file paths;
- include SHA-256 hashes;
- include sizes;
- include line counts where practical;
- apply byte/line caps;
- report truncation;
- include repo metadata.

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
  script input locations, interpreter availability (`/bin/sh`, `/bin/zsh`), the version,
  warnings, and an overall readiness summary (with `--json`);
- exit 0 when a repository is found, even with warnings; treat missing `.aikit/temp/` or
  ignore coverage as warnings rather than failures.

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
- `blocked_forbidden_git_operation`;
- `blocked_missing_anchor`;
- `blocked_invalid_anchor`;
- `blocked_unreadable_file`;
- `blocked_unsupported_mode`.

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
- best-effort policy scan rules.

Still open:

- exact config format (no config file is read yet);
- install method;
- whether to support agent wrapper files later.
