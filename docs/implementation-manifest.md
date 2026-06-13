# aikit Implementation Manifest

## 1. Purpose

- This manifest defines the expected file changes for implementation batches.
- It is a lightweight guard against file sprawl and ambiguity.
- It is not a methodology artifact or gate system.
- It must be updated/reviewed before each implementation batch.
- After each implementation batch, actual files must be compared to this manifest before commit.

## 2. Status

- Batch 1 is complete and committed.
- Batch 2 is complete and committed.
- Current manifest scope: Batch 3.
- Batch 3 has not yet been implemented.
- This Batch 3 manifest update is expected to be reviewed before source changes for Batch 3 begin, and should not be committed until reviewed.

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

## Batch 2 Completed Scope

Batch 2 is complete and committed. It delivered:

- `aikit inventory repo`;
- ignore-aware traversal;
- directory-only exclusions;
- deterministic repo-relative ordering;
- SHA-256 file hashing;
- JSON and text inventory output;
- `--json`;
- `--output <path>`;
- `--include-ignored`;
- `--max-files <n>`;
- inventory help surfaces (`aikit inventory --help`, `aikit inventory repo --help`);
- inventory tests;
- README usage update.

Known Batch 2 expected-vs-actual deviations:

- `src/main.rs` was modified for inventory command wiring even though it was not listed in the Batch 2 expected file table.
- `src/errors.rs` was not modified because no inventory-specific error expansion was needed.

The tables below are retained as the historical Batch 2 manifest record.

## Batch 2 Expected Committed Files - Completed

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

## Batch 2 Expected Generated or Local-Only Files - Completed

| Path / Pattern | Classification | Purpose | Commit Policy |
|---|---|---|---|
| `target/` | generated | Rust build/test output | Never commit |
| `.scratch/` | local-only | Local review/output artifacts | Never commit |
| `.claude/` | local-only | External harness state if present | Never commit |
| `.aikit/outputs/inventory/` | local-only | Fallback inventory output when a consuming repo lacks `.scratch/work/outputs/` | Never commit |
| `.scratch/work/outputs/aikit/inventory/` | local-only | Preferred inventory output when the consuming repo has `.scratch/work/outputs/` | Never commit |

## Batch 2 Deferred Files - Completed

| Path / Area | Classification | Reason Deferred |
|---|---|---|
| `src/review.rs` | deferred | Batch 3 / Batch 4 |
| `src/run.rs` | deferred | Batch 5 |
| `src/policy/` | deferred | Not needed until governed script runner |
| `docs/agent-usage.md` | deferred | Optional future documentation only |
| `.github/workflows/` | deferred | Release/CI automation deferred |

## Batch 2 Help Text Expectations - Completed

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

## Batch 2 Test Expectations - Completed

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

## Batch 2 Expected-vs-Actual Verification - Completed

Before committing Batch 2 implementation, produce an expected-vs-actual file report with:

- expected committed files created/modified;
- expected generated/local-only files observed;
- deferred files not created;
- unexpected files created;
- unexpected files removed or justified;
- final list of staged files.

## Batch 3 Scope

Batch 3 should:

- implement `aikit review generate --files <file>...`;
- produce `run_for_review.txt`;
- produce `manifest.json`;
- support `--output <path>`;
- support `--max-file-bytes <n>`;
- support `--max-total-bytes <n>`;
- support `--max-file-lines <n>`;
- support `--json`;
- resolve input files relative to the repo root;
- reject path escapes;
- reject symlink escapes where the resolved real path leaves the repo;
- read file contents with deterministic byte/line caps;
- compute SHA-256 and size for each file;
- sort files deterministically by repo-relative path before applying caps;
- include every file in scope in `manifest.json`, whether included, truncated, or omitted;
- handle nested backticks in file contents using a deterministic fence-length rule;
- update README usage documentation for explicit-file review bundles;
- add tests for review bundle generation from explicit files;
- compare actual files to this manifest before commit.

Batch 3 must not:

- implement `aikit review generate --anchor <anchor.json>`;
- implement any precomputed `--changed <changed.json>` mode;
- implement `aikit run script`;
- create agent skills;
- create release automation;
- push to remote.

## Batch 3 Expected Committed Files

| Path | Classification | Purpose | Notes |
|---|---|---|---|
| `README.md` | modified | Add concise review bundle generation usage for explicit files | Document output behavior, caps, and manifest output |
| `src/main.rs` | modified | Register review module if required by module layout | Include because Batch 2 showed command-family wiring may require main.rs changes |
| `src/cli.rs` | modified | Add review command definitions and help text | Include useful help for `aikit review --help` and `aikit review generate --help` |
| `src/review.rs` | new | Implement `aikit review generate --files <file>...` | Include path checks, deterministic ordering, caps, bundle writing, manifest writing, and fence-length handling |
| `src/output.rs` | modified | Support review output directory helpers if needed | Reuse existing output-root behavior |
| `src/formats.rs` | modified | Add review bundle manifest data structures | Include schema_version, kind, review_id, repo_root, git_head, generated_at, inputs, limits, files, bundle_path, totals |
| `src/errors.rs` | modified | Add any review-specific errors/blocking states needed | Do not over-expand the error model |
| `tests/cli_review.rs` | new | Integration tests for `aikit review generate --files` | Use temporary Git repos and deterministic fixture files |
| `Cargo.toml` | modified | Add any dependency needed for Batch 3 if not already present | Only add dependencies if actually needed |
| `Cargo.lock` | modified | Reflect dependency graph changes if Cargo.toml changes | No manual editing |

## Batch 3 Expected Generated or Local-Only Files

| Path / Pattern | Classification | Purpose | Commit Policy |
|---|---|---|---|
| `target/` | generated | Rust build/test output | Never commit |
| `.scratch/` | local-only | Local review/output artifacts | Never commit |
| `.claude/` | local-only | External harness state if present | Never commit |
| `.aikit/outputs/reviews/` | local-only | Fallback review output when a consuming repo lacks `.scratch/work/outputs/` | Never commit |
| `.scratch/work/outputs/aikit/reviews/` | local-only | Preferred review output when the consuming repo has `.scratch/work/outputs/` | Never commit |

## Batch 3 Deferred Files

| Path / Area | Classification | Reason Deferred |
|---|---|---|
| `aikit review generate --anchor <anchor.json>` | deferred | Batch 4 |
| precomputed `--changed <changed.json>` review mode | deferred | Only add later if a real need appears |
| `src/run.rs` | deferred | Batch 5 |
| `src/policy/` | deferred | Not needed until governed script runner |
| `docs/agent-usage.md` | deferred | Optional future documentation only |
| `.github/workflows/` | deferred | Release/CI automation deferred |

## Batch 3 Help Text Expectations

Batch 3 must provide useful help for:

- `aikit review --help`
- `aikit review generate --help`

For each help surface, require:

- purpose;
- when to use;
- explicit-file input behavior;
- key flags;
- default output behavior;
- JSON behavior;
- cap/truncation behavior;
- short example where useful.

## Batch 3 Test Expectations

Expected tests should cover:

- review help is available;
- review generate help is available;
- `aikit review generate --files <file>...` generates a review directory;
- `run_for_review.txt` is created;
- `manifest.json` is created;
- explicit input files are resolved repo-relatively;
- file order is deterministic;
- files outside the repo are rejected;
- symlink escapes are rejected;
- SHA-256 and size are recorded;
- `--max-file-bytes` truncates file content and records truncation;
- `--max-file-lines` truncates file content and records truncation;
- `--max-total-bytes` omits later files deterministically and records omitted_reason/cap_hit;
- every scoped file appears exactly once in manifest files array;
- nested triple-backticks in file contents do not break the bundle;
- fallback output goes to `.aikit/outputs/reviews/` when `.scratch/work/outputs/` does not exist;
- preferred output goes to `.scratch/work/outputs/aikit/reviews/` when `.scratch/work/outputs/` exists;
- `--json` produces machine-readable command output.

## Batch 3 Expected-vs-Actual Verification

Before committing Batch 3 implementation, produce an expected-vs-actual file report with:

- expected committed files created/modified;
- expected generated/local-only files observed;
- deferred files not created;
- unexpected files created;
- unexpected files removed or justified;
- final list of staged files.

## 11. Future Batch Manifest Updates

- Before Batch 4, update this manifest for review generation from anchors.
- Before Batch 5, update this manifest for governed script runner files.
- Before Batch 6, update this manifest for local integration/polish.
