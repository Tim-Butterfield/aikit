# aikit Implementation Manifest

## 1. Purpose

- This manifest defines the expected file changes for implementation batches.
- It is a lightweight guard against file sprawl and ambiguity.
- It is not a methodology artifact or gate system.
- It must be updated/reviewed before each implementation batch.
- After each implementation batch, actual files must be compared to this manifest before commit.

## 2. Status

- Current manifest scope: Batch 1.
- Batch 1 has not yet been implemented.
- This manifest is expected to be committed before source files are created.

## 3. Classification Values

- `new` — file expected to be created and committed.
- `modified` — existing tracked file expected to be changed and committed.
- `generated` — generated file expected during build/test but not necessarily committed.
- `local-only` — file expected locally but not committed.
- `deferred` — file intentionally not created in the current batch.

## 4. Batch 1 Scope

Batch 1 should:

- revise `.gitignore` so `Cargo.lock` can be tracked;
- create a minimal Rust CLI project;
- implement `aikit --help`;
- implement `aikit batch --help`;
- implement `aikit batch start --help`;
- implement `aikit batch changed --help`;
- implement `aikit batch start`;
- implement `aikit batch changed --anchor <anchor.json>`;
- implement repo-root detection;
- implement output-root selection;
- implement batch anchor JSON;
- implement simple changed-file detection;
- add tests for batch start/changed;
- update README usage documentation;
- compare actual files to this manifest before commit.

Batch 1 must not:

- implement `aikit inventory repo`;
- implement `aikit review generate`;
- implement `aikit run script`;
- create agent skills;
- create release automation;
- push to remote.

## 5. Batch 1 Expected Committed Files

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

## 11. Future Batch Manifest Updates

- Before Batch 2, this manifest should be updated for inventory files.
- Before Batch 3, this manifest should be updated for review generation from explicit files.
- Before Batch 4, this manifest should be updated for review generation from anchors.
- Before Batch 5, this manifest should be updated for governed script runner files.
- Before Batch 6, this manifest should be updated for local integration/polish.
