# aikit Implementation Manifest

## 1. Purpose

- This manifest defines the expected file changes for implementation batches.
- It is a lightweight guard against file sprawl and ambiguity.
- It is not a methodology artifact or gate system.
- It must be updated/reviewed before each implementation batch.
- After each implementation batch, actual files must be compared to this manifest before commit.

## 2. Status

- Batch 1 is complete and committed.
- Current manifest scope: Batch 2.
- Batch 2 has not yet been implemented.
- This Batch 2 manifest update is expected to be reviewed before source changes for Batch 2 begin, and should not be committed until reviewed.

## 3. Classification Values

- `new` — file expected to be created and committed.
- `modified` — existing tracked file expected to be changed and committed.
- `generated` — generated file expected during build/test but not necessarily committed.
- `local-only` — file expected locally but not committed.
- `deferred` — file intentionally not created in the current batch.

## 4. Batch 1 Completed Scope

Batch 1 is complete and committed. It delivered:

- Rust scaffold (minimal module layout);
- `aikit batch start`;
- `aikit batch changed --anchor <anchor.json>`;
- repo-root detection, output-root selection, batch anchor JSON, and simple changed-file detection;
- root and batch help surfaces (`aikit --help`, `aikit batch --help`, `aikit batch start --help`, `aikit batch changed --help`);
- tests for batch behavior;
- README usage update;
- `Cargo.lock` tracking.

The tables in sections 5–10 below are retained as the historical Batch 1 manifest record.

## 5. Batch 1 Expected Committed Files - Completed

| Path | Classification | Purpose | Notes |
|---|---|---|---|
| `README.md` | modified | Add build/use examples for Batch 1 commands | Keep concise; personal/local-first; no public distribution promise |
| `.gitignore` | modified | Stop ignoring `Cargo.lock` for binary application policy | Keep `target/`, `.scratch/`, `.claude/`, `.DS_Store` ignored as appropriate |
| `Cargo.toml` | new | Rust package manifest for the `aikit` binary | Include only dependencies needed for Batch 1 unless clearly justified |
| `Cargo.lock` | new | Locked dependency graph for the binary application | Must be tracked once Rust scaffolding exists |
| `src/main.rs` | new | Binary entry point | Should delegate to CLI command handling; keep small |
| `src/cli.rs` | new | clap CLI definition and top-level command dispatch | Include help text for root and batch commands |
| `src/batch.rs` | new | `batch start`/`changed` command implementation | Keep Batch 1 implementation minimal; split later if needed |
| `src/repo.rs` | new | Repo-root detection and Git helper functions | May use `git rev-parse --show-toplevel` and `git status --porcelain=v1` |
| `src/output.rs` | new | Output-root selection and local output directory helpers | Implement `.scratch/work/outputs/aikit/` if parent exists, else `.aikit/outputs/` |
| `src/formats.rs` | new | Serializable Batch 1 data structures | Include batch anchor and changed-file output structures only |
| `src/errors.rs` | new | Shared Batch 1 error/blocking types | Include initial blocked states needed by batch commands |
| `tests/cli_batch.rs` | new | Integration tests for `batch start`/`changed` and help availability | Use temporary Git repos; verify JSON shape, anchor creation, changed-file behavior, and help text |

## 6. Batch 1 Expected Generated or Local-Only Files

| Path / Pattern | Classification | Purpose | Commit Policy |
|---|---|---|---|
| `target/` | generated | Rust build output | Never commit |
| `.scratch/` | local-only | Local-only scratch/review output | Never commit |
| `.claude/` | local-only | External harness state if present | Never commit |
| `.aikit/outputs/` | local-only | Fallback local output when a consuming repo lacks `.scratch/work/outputs/` | Never commit |
| `.scratch/work/outputs/aikit/` | local-only | Preferred local command output when the consuming repo has `.scratch/work/outputs/` | Never commit |

## 7. Batch 1 Deferred Files

| Path / Area | Classification | Reason Deferred |
|---|---|---|
| `src/inventory.rs` | deferred | Batch 2 |
| `src/review.rs` | deferred | Batch 3 / Batch 4 |
| `src/run.rs` | deferred | Batch 5 |
| `src/policy/` | deferred | Not needed until governed script runner |
| `docs/agent-usage.md` | deferred | Optional future documentation only |
| `.github/workflows/` | deferred | Release/CI automation deferred |

## 8. Batch 1 Help Text Expectations

Batch 1 must provide useful help for:

- `aikit --help`
- `aikit batch --help`
- `aikit batch start --help`
- `aikit batch changed --help`

For each help surface, require:

- purpose;
- when to use;
- key flags;
- default output behavior;
- JSON behavior where available;
- short example where useful.

## 9. Batch 1 Test Expectations

Expected tests should cover:

- root help is available;
- batch help is available;
- batch start help is available;
- batch changed help is available;
- `batch start` creates an anchor;
- anchor JSON includes schema version, kind, anchor id, created_at, repo_root, git_head, git_branch, git_status_porcelain, filesystem_anchor_time;
- command fails outside a Git repo with `blocked_repo_not_found`;
- output-root selection uses `.scratch/work/outputs/aikit/` when `.scratch/work/outputs/` exists;
- output-root selection falls back to `.aikit/outputs/` when `.scratch/work/outputs/` does not exist;
- `batch changed` detects modified tracked files;
- `batch changed --include-untracked` detects new untracked files by mtime heuristic;
- generated aikit output folders are excluded from changed-file results by default;
- paths are deterministic and repo-relative.

## 10. Batch 1 Expected-vs-Actual Verification

Before committing Batch 1 implementation, produce an expected-vs-actual file report with:

- expected committed files created/modified;
- expected generated/local-only files observed;
- deferred files not created;
- unexpected files created;
- unexpected files removed or justified;
- final list of staged files.

## Batch 2 Scope

Batch 2 should:

- implement ignore-aware file walking;
- implement SHA-256 hashing for inventory entries;
- implement `aikit inventory repo`;
- implement inventory JSON output;
- implement inventory text output;
- implement deterministic file ordering;
- support `--output <path>`;
- support `--json`;
- support `--include-ignored`;
- support `--max-files <n>`;
- exclude `.git/` always;
- exclude default build/dependency/output directories by directory-only rules;
- update README usage documentation for inventory;
- add tests for inventory behavior;
- compare actual files to this manifest before commit.

Batch 2 must not:

- implement `aikit review generate`;
- implement `aikit run script`;
- create agent skills;
- create release automation;
- push to remote.

## Batch 2 Expected Committed Files

| Path | Classification | Purpose | Notes |
|---|---|---|---|
| `README.md` | modified | Add concise inventory command usage | Document output behavior and JSON/text outputs |
| `src/cli.rs` | modified | Add inventory command definitions and help text | Include useful help for `aikit inventory --help` and `aikit inventory repo --help` |
| `src/inventory.rs` | new | Implement `aikit inventory repo` | Include ignore-aware traversal, deterministic ordering, hashing, JSON/text output |
| `src/output.rs` | modified | Support inventory output directory helpers if needed | Reuse existing output-root behavior |
| `src/formats.rs` | modified | Add inventory JSON data structures | Include schema_version, kind, inventory_id, repo_root, git_head, generated_at, files, counts |
| `src/errors.rs` | modified | Add any inventory-specific errors/blocking states needed | Do not over-expand the error model |
| `tests/cli_inventory.rs` | new | Integration tests for `aikit inventory repo` | Use temporary Git repos and deterministic fixture files |
| `Cargo.toml` | modified | Add any dependency needed for Batch 2 if not already present | Only add dependencies if actually needed |
| `Cargo.lock` | modified | Reflect dependency graph changes if Cargo.toml changes | No manual editing |

## Batch 2 Expected Generated or Local-Only Files

| Path / Pattern | Classification | Purpose | Commit Policy |
|---|---|---|---|
| `target/` | generated | Rust build/test output | Never commit |
| `.scratch/` | local-only | Local review/output artifacts | Never commit |
| `.claude/` | local-only | External harness state if present | Never commit |
| `.aikit/outputs/inventory/` | local-only | Fallback inventory output when a consuming repo lacks `.scratch/work/outputs/` | Never commit |
| `.scratch/work/outputs/aikit/inventory/` | local-only | Preferred inventory output when the consuming repo has `.scratch/work/outputs/` | Never commit |

## Batch 2 Deferred Files

| Path / Area | Classification | Reason Deferred |
|---|---|---|
| `src/review.rs` | deferred | Batch 3 / Batch 4 |
| `src/run.rs` | deferred | Batch 5 |
| `src/policy/` | deferred | Not needed until governed script runner |
| `docs/agent-usage.md` | deferred | Optional future documentation only |
| `.github/workflows/` | deferred | Release/CI automation deferred |

## Batch 2 Help Text Expectations

Batch 2 must provide useful help for:

- `aikit inventory --help`
- `aikit inventory repo --help`

For each help surface, require:

- purpose;
- when to use;
- key flags;
- default output behavior;
- JSON behavior;
- ignored-file behavior;
- short example where useful.

## Batch 2 Test Expectations

Expected tests should cover:

- inventory help is available;
- inventory repo help is available;
- `aikit inventory repo` inventories a simple Git repo;
- `.git/` is always excluded;
- output is deterministic and repo-relative;
- JSON output includes schema_version, kind, inventory_id, repo_root, git_head, generated_at, files, counts;
- text output is created in the inventory output directory;
- SHA-256 is computed for included files;
- `.gitignore` is respected by default;
- ignored files are included only with `--include-ignored`;
- default build/dependency/output directories are excluded by directory-only rules;
- `--max-files <n>` limits the inventory deterministically and reports the limitation;
- fallback output goes to `.aikit/outputs/inventory/` when `.scratch/work/outputs/` does not exist;
- preferred output goes to `.scratch/work/outputs/aikit/inventory/` when `.scratch/work/outputs/` exists.

## Batch 2 Expected-vs-Actual Verification

Before committing Batch 2 implementation, produce an expected-vs-actual file report with:

- expected committed files created/modified;
- expected generated/local-only files observed;
- deferred files not created;
- unexpected files created;
- unexpected files removed or justified;
- final list of staged files.

## 11. Future Batch Manifest Updates

- Before Batch 3, update this manifest for review generation from explicit files.
- Before Batch 4, update this manifest for review generation from anchors.
- Before Batch 5, update this manifest for governed script runner files.
- Before Batch 6, update this manifest for local integration/polish.
