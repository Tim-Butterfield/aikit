# aikit

`aikit` is a personal compiled CLI for deterministic AI-agent workflow support.

## Status

- Personal tool — built primarily for the architect's own use.
- Private repo — not currently intended for public distribution, and may never be.
- Not yet pushed to any remote.
- Implemented so far: `aikit batch start`, `aikit batch changed` (Batch 1),
  `aikit inventory repo` (Batch 2), and `aikit review generate` from explicit files
  (Batch 3) or a batch anchor (Batch 4) — Rust scaffold, repo-root detection, anchor
  JSON, changed-file detection, a deterministic hashed repository inventory, and
  bounded review-bundle generation from explicit files or anchor-driven changes.
- Remaining: the precomputed `--changed <changed.json>` review mode and
  `aikit run script` are not implemented yet.

## Purpose

`aikit` supports AI-agent workflows with **deterministic local operations**:

- batch anchoring — mark a point in time before AI-agent work begins;
- change discovery — report what was created/modified since an anchor;
- review bundle generation — produce a bounded, hashed review surface;
- repo inventory — generate a mechanical inventory of the repository;
- governed script execution — run local scripts under explicit policy controls.

## Non-Goals

`aikit` is:

- **not** an autonomous agent;
- **not** a methodology validator;
- **not** a governance judge;
- **not** a provider/model router;
- **not** a remote execution framework;
- **not** a replacement for Git;
- **not** a copied collection of old scripts.

## Initial Command Families

- `aikit run script`
- `aikit batch start`
- `aikit batch changed`
- `aikit review generate`
- `aikit inventory repo`

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

## Building and Usage (Batch 1)

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

## Current State

Batch 1 (`batch start`, `batch changed`), Batch 2 (`inventory repo`), and Batch 3 +
Batch 4 (`review generate --files` and `review generate --anchor`) commands are
implemented. The precomputed `--changed <changed.json>` review mode and
`aikit run script` are not implemented yet. See
[`docs/aikit-cli-spec.md`](docs/aikit-cli-spec.md) for the CLI specification,
[`docs/aikit-implementation-plan.md`](docs/aikit-implementation-plan.md) for the
implementation plan, and
[`docs/decisions/0001-create-aikit.md`](docs/decisions/0001-create-aikit.md) for the
repo-creation decision.
