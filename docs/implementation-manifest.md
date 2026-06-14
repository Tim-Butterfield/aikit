# aikit Implementation Manifest

## 1. Purpose

- This manifest defines the expected file changes for implementation batches.
- It is a lightweight guard against file sprawl and ambiguity.
- It is not a methodology artifact or approval system.
- It must be updated/reviewed before each implementation batch.
- After each implementation batch, actual files must be compared to this manifest before commit.

## 2. Status

- Batch 1 is complete and committed.
- Batch 2 is complete and committed.
- Batch 3 is complete and committed.
- Batch 4 is complete and committed.
- Output-location policy correction is complete and committed (default output root is
  always `.aikit/outputs/`) — see "Output Location Policy Correction" below.
- Batch 5 is complete and committed (governed script runner).
- Batch 6 is complete and is being committed in this batch (local integration/polish).
- All six initial implementation batches are complete; no further batches are planned.

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

## Batch 3 Completed Scope

Batch 3 is complete and committed. It delivered:

- `aikit review generate --files <file>...`;
- explicit-file review bundle generation;
- `run_for_review.txt`;
- `manifest.json`;
- `--output <path>`;
- `--max-file-bytes <n>`;
- `--max-total-bytes <n>`;
- `--max-file-lines <n>`;
- `--json`;
- repo-relative input resolution;
- path/symlink escape rejection;
- deterministic sorting;
- SHA-256 and file-size recording;
- cap/truncation/omission reporting;
- deterministic backtick fence handling;
- review help surfaces (`aikit review --help`, `aikit review generate --help`);
- review tests;
- README usage update.

Known Batch 3 expected-vs-actual deviation:

- `Cargo.toml` and `Cargo.lock` were unchanged because no new dependency was needed.

The tables below are retained as the historical Batch 3 manifest record.

## Batch 3 Expected Committed Files - Completed

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

## Batch 3 Expected Generated or Local-Only Files - Completed

| Path / Pattern | Classification | Purpose | Commit Policy |
|---|---|---|---|
| `target/` | generated | Rust build/test output | Never commit |
| `.scratch/` | local-only | Local review/output artifacts | Never commit |
| `.claude/` | local-only | External harness state if present | Never commit |
| `.aikit/outputs/reviews/` | local-only | Fallback review output when a consuming repo lacks `.scratch/work/outputs/` | Never commit |
| `.scratch/work/outputs/aikit/reviews/` | local-only | Preferred review output when the consuming repo has `.scratch/work/outputs/` | Never commit |

## Batch 3 Deferred Files - Completed

| Path / Area | Classification | Reason Deferred |
|---|---|---|
| `aikit review generate --anchor <anchor.json>` | deferred | Batch 4 |
| precomputed `--changed <changed.json>` review mode | deferred | Only add later if a real need appears |
| `src/run.rs` | deferred | Batch 5 |
| `src/policy/` | deferred | Not needed until governed script runner |
| `docs/agent-usage.md` | deferred | Optional future documentation only |
| `.github/workflows/` | deferred | Release/CI automation deferred |

## Batch 3 Help Text Expectations - Completed

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

## Batch 3 Test Expectations - Completed

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

## Batch 3 Expected-vs-Actual Verification - Completed

Before committing Batch 3 implementation, produce an expected-vs-actual file report with:

- expected committed files created/modified;
- expected generated/local-only files observed;
- deferred files not created;
- unexpected files created;
- unexpected files removed or justified;
- final list of staged files.

## Output Location Policy Correction - Completed

A targeted correction applied before Batch 4 (commit "Default aikit outputs to .aikit"):

- The default output root is **always** `.aikit/outputs/`, with command-family
  subfolders `.aikit/outputs/{batches,inventory,reviews,runs}/` (`runs/` reserved for
  the future `aikit run script`).
- `.scratch` is **never** auto-selected, and aikit never auto-creates `.scratch/`,
  `.scratch/work/`, or `.scratch/work/outputs/`. `.scratch` is opt-in only via
  `--output .scratch/...`.
- `--output <path>` always wins and is used as the output root verbatim.
- Commands that create files print exact created artifact paths in human output and
  include them in `--json` output (`batch start` → `anchor_path`; `inventory repo`
  and `review generate` → a `written` array).
- Anchor files remain durable artifacts and are not auto-cleaned; anchor-consuming
  commands still require explicit `--anchor <anchor.json>`.

This supersedes the earlier ".scratch/work/outputs/aikit/ preferred when present,
else .aikit/outputs/" wording recorded in the historical Batch 1/2/3 sections above;
those records are retained as history. Files touched by the correction: `src/output.rs`
(always `.aikit/outputs`), `src/batch.rs` (relative `--output` resolves under the repo
root), `src/cli.rs` (help), `src/inventory.rs` + `src/review.rs` (print a `written` array
of created paths in `--json`, without embedding paths in the durable on-disk artifacts),
the three test files, `README.md`, and `docs/aikit-implementation-plan.md` (§8). No new
dependencies; no `src/formats.rs` schema change; no runtime provider/model/agent logic.

## Batch 4 Completed Scope

Batch 4 is complete and committed. It delivered:

- `aikit review generate --anchor <anchor.json>`;
- anchor-driven review bundle generation;
- preserved `--files <file>...` mode;
- exactly-one-input-mode enforcement between `--files` and `--anchor`;
- missing/invalid/cross-repo anchor rejection;
- changed-file computation from batch anchors;
- reuse of existing review bundle pipeline;
- default `.aikit/outputs/reviews/`;
- `.scratch` only through explicit `--output`;
- review help updates;
- anchor-mode tests;
- README usage update.

Known Batch 4 expected-vs-actual deviations:

- `src/errors.rs` was unchanged because existing missing/invalid anchor states were reused.
- `Cargo.toml` and `Cargo.lock` were unchanged because no new dependency was needed.

The tables below are retained as the historical Batch 4 manifest record.

## Batch 4 Expected Committed Files - Completed

| Path | Classification | Purpose | Notes |
|---|---|---|---|
| `README.md` | modified | Add concise anchor-driven review bundle usage | Document that `--anchor <anchor.json>` uses batch changed behavior and that `--changed` remains deferred |
| `src/cli.rs` | modified | Add `--anchor <anchor.json>` option for review generation and update help text | Do not add `--changed` |
| `src/review.rs` | modified | Implement anchor-driven review generation by reusing existing review bundle behavior | Preserve explicit-file behavior; do not duplicate bundle-generation logic unnecessarily |
| `src/batch.rs` | modified | Expose/reuse changed-file computation from anchors if needed | Preserve existing `aikit batch changed` behavior |
| `src/formats.rs` | modified | Update review input metadata if needed to record anchor-driven mode | Include mode and anchor_path in manifest inputs if not already supported |
| `src/errors.rs` | modified | Add any anchor/review-specific blocked states or errors needed | Likely candidates include missing/invalid/cross-repo anchor handling if not already present |
| `tests/cli_review.rs` | modified | Add anchor-driven review generation tests | Preserve existing explicit-file tests |
| `Cargo.toml` | modified | Add any dependency needed for Batch 4 if not already present | Only add dependencies if actually needed |
| `Cargo.lock` | modified | Reflect dependency graph changes if Cargo.toml changes | No manual editing |

## Batch 4 Expected Generated or Local-Only Files - Completed

| Path / Pattern | Classification | Purpose | Commit Policy |
|---|---|---|---|
| `target/` | generated | Rust build/test output | Never commit |
| `.scratch/` | local-only | Local review/output artifacts only when explicitly requested or external tooling creates it | Never commit |
| `.claude/` | local-only | External harness state if present | Never commit |
| `.aikit/outputs/reviews/` | local-only | Default review output | Never commit |
| `.aikit/outputs/batches/` | local-only | Default batch anchor output | Never commit |
| `.scratch/work/outputs/aikit/reviews/` | local-only | Optional review output only when explicitly requested through `--output` | Never commit |

## Batch 4 Deferred Files - Completed

| Path / Area | Classification | Reason Deferred |
|---|---|---|
| precomputed `--changed <changed.json>` review mode | deferred | Only add later if a real need appears |
| `src/run.rs` | deferred | Batch 5 |
| `src/policy/` | deferred | Not needed until governed script runner |
| `docs/agent-usage.md` | deferred | Optional future documentation only |
| `.github/workflows/` | deferred | Release/CI automation deferred |

## Batch 4 Help Text Expectations - Completed

Batch 4 must update useful help for:

- `aikit review --help`
- `aikit review generate --help`

Help must make clear:

- explicit-file mode remains available through `--files <file>...`;
- anchor-driven mode is available through `--anchor <anchor.json>`;
- exactly one input mode should be used at a time;
- `--changed <changed.json>` is not implemented;
- default output is `.aikit/outputs/reviews/`;
- `.scratch` is available only through explicit `--output`;
- created artifact paths are printed;
- JSON behavior;
- cap/truncation behavior;
- short examples where useful.

## Batch 4 Test Expectations - Completed

Expected tests should cover:

- review generate help advertises `--anchor`;
- review generate help does not advertise `--changed`;
- explicit-file review generation still works;
- `aikit review generate --anchor <anchor.json>` creates a review directory;
- anchor-driven mode creates `run_for_review.txt`;
- anchor-driven mode creates `manifest.json`;
- anchor-driven manifest inputs record anchor-driven mode and anchor path;
- changed files from an anchor are included in the review bundle;
- unchanged files are not included;
- missing anchor is rejected;
- invalid anchor is rejected;
- anchor from another repo is rejected;
- using both `--files` and `--anchor` is invalid usage;
- default output goes to `.aikit/outputs/reviews/`;
- presence of `.scratch/work/outputs/` does not change the default output;
- explicit `--output .scratch/work/outputs/aikit/reviews` uses `.scratch` as requested;
- cap/truncation/omission behavior from explicit-file mode still works for anchor mode;
- `--json` output includes machine-readable created artifact paths.

## Batch 4 Expected-vs-Actual Verification - Completed

Before committing Batch 4 implementation, produce an expected-vs-actual file report with:

- expected committed files created/modified;
- expected generated/local-only files observed;
- deferred files not created;
- unexpected files created;
- unexpected files removed or justified;
- final list of staged files.

## Batch 5 Completed Scope

Batch 5 is complete and committed (in this batch). It delivered:

- `aikit run script <script-path>` with `--print`, `--require-clean`, `--allow-dirty`,
  `--output`, and `--json`;
- `--require-clean` + `--allow-dirty` rejected together (invalid usage); default is allow-dirty;
- allowed script inputs only under `.aikit/temp/`, `.scratch/work/temp/`, `.scratch/work/outputs/`;
- canonicalized path resolution rejecting missing scripts, directories, symlink escapes, and out-of-allowlist paths;
- interpreter chosen from extension (`.zsh` → `/bin/zsh`, `.sh` → `/bin/sh`); extensionless/unknown rejected; shebangs not trusted;
- best-effort static forbidden-operation scan (documented as not a security boundary);
- `--print` validates and shows the plan without executing (`executed: false`);
- default output `.aikit/outputs/runs/<id>/` (`.scratch` only via explicit `--output`); script copied with its extension; `stdout.txt`, `stderr.txt`, and `run.json` written; created paths printed (and in `--json`);
- run metadata (interpreter, argv, cwd, require_clean, allow_dirty, executed, timings, git heads, exit_code, blocked_state, paths);
- executed script exit code propagated;
- a `src/policy/` module (`mod.rs` + `script.rs`) for the deterministic, best-effort policy rules;
- run-script tests, README usage + safety warning, and help on `aikit run --help` / `aikit run script --help` that clearly states it is not a security sandbox.

Known Batch 5 expected-vs-actual deviations:

- `Cargo.toml` and `Cargo.lock` were unchanged because no new dependency was needed (reused serde/serde_json/sha2/time).
- All other Batch 5 expected files (README, main, cli, run, policy/mod, policy/script, output, repo, formats, errors, tests/cli_run_script) were created/modified as expected.

The sections below are retained as the historical Batch 5 manifest record.

## Batch 5 Allowed Script Input Locations - Completed

Initial script inputs are allowed only under:

- `.aikit/temp/`
- `.scratch/work/temp/`
- `.scratch/work/outputs/`

Clarifications:

- These are allowed input locations for scripts, not default output locations.
- The default output location for run records remains `.aikit/outputs/runs/`.
- `.scratch` output is used only when explicitly requested through `--output`.

## Batch 5 Forbidden Operation Scan - Completed

The initial best-effort static scan should block clear textual matches such as:

- `git push`
- `git fetch`
- `git pull`
- `gh repo create`
- `gh repo delete`
- `rm -rf /`
- `sudo`

Explicitly:

- The scan is crude and best-effort.
- It can false-positive.
- It can be bypassed intentionally.
- It is a guard against obvious accidental mistakes, not a security boundary.
- The allowed-location policy is the primary control.
- `aikit run script` does not make arbitrary scripts safe.

## Batch 5 Expected Committed Files - Completed

| Path | Classification | Purpose | Notes |
|---|---|---|---|
| `README.md` | modified | Add governed script runner usage and safety warning | Clearly state not a security sandbox |
| `src/main.rs` | modified | Register run module if required by module layout | Include because command-family wiring may require main.rs changes |
| `src/cli.rs` | modified | Add run command definitions and help text | Include useful help for `aikit run --help` and `aikit run script --help` |
| `src/run.rs` | new | Implement `aikit run script <script-path>` | Include path checks, policy checks, print mode, execution, stdout/stderr capture, run metadata |
| `src/policy/mod.rs` | new | Policy module root for script-runner rules | Keep policy limited to deterministic local script-runner checks |
| `src/policy/script.rs` | new | Allowed-location, interpreter, extension, and forbidden-operation scan policy | Must clearly remain best-effort, not sandbox semantics |
| `src/output.rs` | modified | Support run output directory helpers | Default `.aikit/outputs/runs/`; `--output` override |
| `src/repo.rs` | modified | Support clean-tree checks and git head before/after if needed | Reuse existing Git helpers where possible |
| `src/formats.rs` | modified | Add run metadata data structures | Include schema_version, kind, run_id, repo_root, script_path, script_sha256, script_copy_path, interpreter, argv, cwd, require_clean, executed, timestamps, duration_ms, git heads, exit_code, blocked_state, stdout_path, stderr_path |
| `src/errors.rs` | modified | Add script-runner blocked states/errors as needed | Include path/script/policy/unsupported-mode cases without over-expanding the model |
| `tests/cli_run_script.rs` | new | Integration tests for governed script runner | Use temporary Git repos and small scripts; avoid dangerous commands except harmless static-scan fixtures |
| `Cargo.toml` | modified | Add any dependency needed for Batch 5 if not already present | Only add dependencies if actually needed |
| `Cargo.lock` | modified | Reflect dependency graph changes if Cargo.toml changes | No manual editing |

## Batch 5 Expected Generated or Local-Only Files - Completed

| Path / Pattern | Classification | Purpose | Commit Policy |
|---|---|---|---|
| `target/` | generated | Rust build/test output | Never commit |
| `.aikit/outputs/runs/` | local-only | Default run output | Never commit |
| `.aikit/temp/` | local-only | Allowed local script input location | Never commit |
| `.scratch/` | local-only | Local work/review artifacts or explicit output override only | Never commit |
| `.scratch/work/temp/` | local-only | Allowed local script input location | Never commit |
| `.scratch/work/outputs/` | local-only | Allowed local script input location and optional output only when explicitly requested | Never commit |
| `.claude/` | local-only | External harness state if present | Never commit |

## Batch 5 Deferred Files - Completed

| Path / Area | Classification | Reason Deferred |
|---|---|---|
| `docs/agent-usage.md` | deferred | Optional future documentation only |
| `.github/workflows/` | deferred | Release/CI automation deferred |
| remote execution | deferred | Out of initial scope |
| Python/Node script execution | deferred | Initial interpreter map supports only `.zsh` and `.sh` |
| automatic cleanup commands | deferred | Old anchors/runs remain human-cleanup artifacts for now |

## Batch 5 Help Text Expectations - Completed

Batch 5 must provide useful help for:

- `aikit run --help`
- `aikit run script --help`

Help must make clear:

- purpose;
- when to use;
- allowed script locations;
- supported extensions/interpreters;
- `--print` behavior;
- `--require-clean` behavior;
- `--allow-dirty` behavior;
- default allow-dirty behavior;
- `--require-clean` and `--allow-dirty` cannot be combined;
- forbidden-operation scan is best-effort;
- this is not a security sandbox;
- default output is `.aikit/outputs/runs/`;
- `.scratch` output is available only through explicit `--output`;
- created artifact paths are printed;
- JSON behavior if supported;
- exit-code propagation;
- short examples where useful.

## Batch 5 Test Expectations - Completed

Expected tests should cover:

- run help is available;
- run script help is available;
- help clearly states not a security sandbox;
- script outside repo is rejected;
- script outside allowed locations is rejected;
- symlinked script whose resolved path leaves repo or allowlist is rejected;
- extensionless script is rejected;
- unknown-extension script is rejected;
- `.zsh` script runs through `/bin/zsh`;
- `.sh` script runs through `/bin/sh`;
- `--print` does not execute the script;
- `--print` records/reports `executed: false`;
- default policy is allow-dirty when neither clean flag is supplied;
- `--require-clean` blocks when tracked tree is dirty;
- `--allow-dirty` allows dirty tracked tree;
- `--require-clean` and `--allow-dirty` together are invalid usage;
- forbidden-operation scan blocks obvious forbidden text;
- stdout is captured to `stdout.txt`;
- stderr is captured to `stderr.txt`;
- `run.json` is written;
- run metadata includes interpreter, argv, cwd, require_clean, executed, git heads, exit_code, blocked_state, stdout_path, stderr_path, script_copy_path;
- executed script exit code is propagated;
- default output goes to `.aikit/outputs/runs/`;
- explicit `--output .scratch/work/outputs/aikit/runs` uses `.scratch` as requested;
- copied script retains its extension;
- commands print exact created artifact paths.

## Batch 5 Expected-vs-Actual Verification - Completed

Before committing Batch 5 implementation, produce an expected-vs-actual file report with:

- expected committed files created/modified;
- expected generated/local-only files observed;
- deferred files not created;
- unexpected files created;
- unexpected files removed or justified;
- final list of staged files.

## Batch 6 Completed Scope

Batch 6 is complete and is being committed in this batch. It is final local
integration and polish only — no new behavior, no new command family, no broadening
of the Batch 5 script-runner policy. It delivered:

- a docs/help/spec consistency review across README, the CLI spec, and every durable
  help surface (`aikit --help`; `batch`, `inventory`, `review`, `run` parents; and the
  `start`, `changed`, `inventory repo`, `review generate`, `run script` leaves);
- small CLI help polish: the `aikit run --help` parent now carries an `Examples:`
  block, matching the example blocks already present on `batch`, `inventory`, and
  `review` (cross-family help consistency);
- README final alignment: the "Building and Usage" heading no longer carries a stale
  per-batch label now that it documents all command families, and the unimplemented
  precomputed `--changed <changed.json>` review mode is described as intentionally not
  implemented (rather than as remaining work);
- CLI spec final alignment: the "Implementation Direction" section no longer states
  that no `src/` exists / that implementation is pending, and instead points at the
  implementation plan and this manifest for the realized layout;
- a new end-to-end integration test (`tests/cli_integration.rs`) exercising the
  intended local workflow in one throwaway Git repo — anchor → modify a tracked file →
  `batch changed --anchor` → `inventory repo` → `review generate --anchor` → stage a
  harmless script under `.aikit/temp/` → `run script --print` — asserting the
  artifacts and metadata line up across commands. The test is deterministic (it
  changes a tracked file so detection is via `git status`, not the mtime heuristic, and
  uses `--print` so no interpreter is invoked and no run directory is created).

Confirmed during Batch 6 and unchanged:

- the default output root remains `.aikit/outputs/`, with `.scratch` opt-in only via
  explicit `--output`;
- batch anchors remain durable and explicitly passed via `--anchor`;
- `review generate` supports `--files` and `--anchor`; the precomputed
  `--changed <changed.json>` mode remains absent;
- `run script` allowed input locations, the `.zsh`/`.sh`-only interpreter map, and the
  best-effort (not-a-sandbox) framing are unchanged;
- no remote execution, Python/Node execution, package-manager orchestration, cleanup
  commands, release/install automation, or runtime provider/model/agent logic was
  added;
- generated/local-only artifacts remain uncommitted.

## Batch 6 Expected-vs-Actual Verification - Completed

Allowed Batch 6 committed file changes and what actually changed:

| Path | Allowed Classification | Actual |
|---|---|---|
| `README.md` | modified (final usage/example alignment) | modified — heading label + `--changed` wording |
| `docs/aikit-cli-spec.md` | modified (final behavior/help/output alignment) | modified — Implementation Direction status lines |
| `docs/implementation-manifest.md` | modified (record Batch 6 completion) | modified — this update |
| `src/cli.rs` | modified (help polish only) | modified — `run` parent `Examples:` block |
| `tests/cli_integration.rs` | new (optional end-to-end test) | new — local-workflow integration test |
| `src/output.rs` | modified only if needed | unchanged — no wording/path change needed |
| `src/formats.rs` | modified only if needed | unchanged — no schema change needed |
| `src/errors.rs` | modified only if needed | unchanged — no error wording change needed |
| `tests/cli_batch.rs` | modified only if needed | unchanged — existing coverage sufficient |
| `tests/cli_inventory.rs` | modified only if needed | unchanged — existing coverage sufficient |
| `tests/cli_review.rs` | modified only if needed | unchanged — existing coverage sufficient |
| `tests/cli_run_script.rs` | modified only if needed | unchanged — existing coverage sufficient |

Notes and acceptable deviations:

- `Cargo.toml` / `Cargo.lock` unchanged — no new dependency was needed (Batch 6 added
  no features).
- The four pre-existing test files were left unchanged because the consistency review
  found their per-family coverage adequate; the integration gap was filled by the new
  `tests/cli_integration.rs` rather than by expanding them.
- Generated/local-only artifacts observed and not staged: `target/`, `.scratch/`,
  external harness state, `docs/.DS_Store`, and any `.aikit/outputs/` produced during
  checks.
- Deferred files remain absent: `docs/agent-usage.md` and `.github/workflows/`.
- Deferred behaviors remain absent: remote execution, Python/Node execution,
  package-manager orchestration, automatic cleanup commands, release/install
  automation, and the precomputed `--changed <changed.json>` review mode.

## 11. Future Batch Manifest Updates

- None planned. All six initial implementation batches (Batch 1–6) are complete. No
  further batch manifest updates are planned for the initial implementation unless new
  user-approved work is added later.

## Post-Initial Documentation

After the initial six-batch implementation was complete, the user approved creating the
previously deferred optional `docs/agent-usage.md` document. It was added as
post-initial documentation, not as a new implementation batch:

- `docs/agent-usage.md` is an agent-agnostic guide describing how an AI agent (or human)
  uses `aikit` mechanically — its command families, output conventions, exit-code
  meanings, and the assumptions callers should and should not make.
- It adds **no** runtime behavior: no Rust source, tests, or `Cargo.*` were changed; no
  new command family or flag was introduced.
- It creates **no** agent-specific skills, wrappers, prompts, or command files, and
  names no specific AI vendor, model, or agent. It notes only that such wrappers could
  be built **outside** this repository while keeping `aikit` itself agent-agnostic.
- `README.md` was updated minimally to link to the new guide.
- Dogfooding the default output root surfaced that `.gitignore` did not actually
  exclude `.aikit/`, even though the durable docs describe `.aikit/outputs/` (and
  `.aikit/temp/`) as local-only and not to be committed. `.gitignore` was updated
  (added `/.aikit/`) so the repository mechanically enforces that the default output
  root stays local-only. This is repo hygiene, not runtime behavior.

This note supersedes, going forward, the "deferred / optional future documentation"
status for `docs/agent-usage.md` recorded in the historical Batch 1–5 deferred tables
and the Batch 6 records above; those records are retained as history. All six initial
implementation batches remain complete, and no further initial batch manifest updates
are planned.

## Post-Initial Command Shape — Slice 1

After the initial six batches, an approved post-initial slice corrects the script
command grammar. This is recorded here (not as a new initial batch). The six initial
batches remain historical and complete.

### Slice 1 scope (implemented)

- Replace the verb-first `aikit run script <script-path>` with the noun-family / action
  form:
  - `aikit script run <script-path>` — preserves the previous run behavior;
  - `aikit script check <script-path>` — validates a script against the same policy
    without executing it and without creating any run output.
- The old `aikit run script` command shape (and the top-level `aikit run`) is **removed,
  not aliased**: there is exactly one public way to run a script (`aikit script run`).
- The run-record format (`aikit.script_run` / run.json) is unchanged; `script check`
  adds a new `aikit.script_check` report kind. No new runtime dependency.

### Slice 1 expected committed files

| Path | Classification | Purpose |
|---|---|---|
| `src/script.rs` | new (rename/refactor of `src/run.rs`) | `script run` + `script check`, sharing one validation path |
| `src/run.rs` | removed | superseded by `src/script.rs` |
| `src/main.rs` | modified | module + dispatch (`run` → `script`; `run`/`check` actions) |
| `src/cli.rs` | modified | `Script`/`ScriptCommand::{Run,Check}` family; remove `Run`/`RunCommand` |
| `src/formats.rs` | modified | add `ScriptCheck` + `aikit.script_check` kind |
| `src/policy/script.rs` | modified (comment only) | existing policy reused by both actions; module doc comment updated to the new command name |
| `src/errors.rs` | unchanged | existing blocked states reused |
| `tests/cli_script.rs` | new (rename/refactor of `tests/cli_run_script.rs`) | `script run` + `script check` + removal tests |
| `tests/cli_run_script.rs` | removed | superseded by `tests/cli_script.rs` |
| `tests/cli_integration.rs` | modified | use `script run --print` instead of `run script --print` |
| `README.md` | modified | `script run` / `script check` usage |
| `docs/agent-usage.md` | modified | `script run` / `script check` across sections |
| `docs/aikit-cli-spec.md` | modified | §5.1 corrected to the `script` family |
| `docs/aikit-implementation-plan.md` | modified | §22 post-initial correction + approved slices |
| `docs/implementation-manifest.md` | modified | this section |

### Slice 1 expected-vs-actual

To be confirmed against `git status` / `git diff` before commit: the committed set
should match the table above (with `src/run.rs` → `src/script.rs` and
`tests/cli_run_script.rs` → `tests/cli_script.rs` shown by Git as deletions + additions,
which Git may report as renames). `Cargo.toml` / `Cargo.lock` are expected to be
unchanged (no new dependency). `src/errors.rs` is expected to be unchanged (existing
blocked states are reused); `src/policy/script.rs` is changed only by a doc-comment
update to the new command name (its policy behavior is unchanged and is reused by both
`script run` and `script check`). No ignored/local-only files are staged.

### Future slices (approved direction, not implemented)

Recorded as approved direction only; **not** implemented in Slice 1 (see the
implementation plan §22.3). No separate roadmap document is created. (Slice 2 has since
been implemented — see the "Post-Initial Command Shape — Slice 2" section below.)

- Slice 2: `aikit repo init`, `aikit repo doctor`.
- Slice 3: `aikit output list`, `aikit output show`, `aikit output clean`.
- Slice 4: `aikit batch list`, `aikit batch show`, `aikit diff anchor`.
- Slice 5: `aikit env snapshot`, `aikit scan secrets`.

## Post-Initial Command Shape — Slice 2

An approved post-initial slice adds the `repo` command family. Recorded here (not as a
new initial batch). Slice 1 and the six initial batches remain historical and complete.

### Slice 2 scope (implemented)

- Add the `repo` command family (noun-family / action grammar):
  - `aikit repo init` — prepare the current repository for local aikit usage: create
    `.aikit/` and `.aikit/temp/` if missing and ensure `.aikit/` is locally ignored via
    `.git/info/exclude` (never `.gitignore`). Idempotent; no duplicate ignore entry; no
    output artifacts, `.scratch/`, or `.claude/`; no remote Git state touched.
  - `aikit repo doctor` — report repo-local readiness read-only (creates/modifies
    nothing); exit 0 even with warnings; only `blocked_repo_not_found` is an error.
- New format kinds: `aikit.repo_init`, `aikit.repo_doctor`. No new runtime dependency;
  existing blocked states reused (`blocked_repo_not_found`).

### Slice 2 expected committed files

| Path | Classification | Purpose |
|---|---|---|
| `src/repo.rs` | modified | add `init` + `doctor` command functions and ignore/dir helpers (alongside existing repo helpers) |
| `src/cli.rs` | modified | add `Repo`/`RepoCommand::{Init,Doctor}` family + args |
| `src/main.rs` | modified | dispatch `repo init` / `repo doctor` |
| `src/formats.rs` | modified | add `RepoInit`, `RepoDoctor`, `PathStatus` + the two kinds |
| `src/errors.rs` | unchanged | existing `blocked_repo_not_found` reused (no new states) |
| `tests/cli_repo.rs` | new | help, init (create/ignore/idempotent/outside-repo), doctor (read-only/ready/dirty/locations/interpreters) |
| `README.md` | modified | repo setup section + command list/current-state |
| `docs/agent-usage.md` | modified | repo commands in workflow + command families |
| `docs/aikit-cli-spec.md` | modified | §5.6 repo init/doctor (post-initial Slice 2) |
| `docs/aikit-implementation-plan.md` | modified | §22.2 Slice 2 implemented; §22.3 future slices |
| `docs/implementation-manifest.md` | modified | this section |

### Slice 2 expected-vs-actual

To be confirmed against `git status` / `git diff` before commit: the committed set should
match the table above. `tests/cli_integration.rs` is expected to be **unchanged** (the
existing end-to-end test already exercises the prior families and needs no repo step;
listed as a likely-touched file in the task but not required). `Cargo.toml` / `Cargo.lock`
and `src/errors.rs` are expected to be unchanged (no new dependency; existing blocked
states reused). No ignored/local-only files are staged; `repo init`'s `.git/info/exclude`
writes are local Git metadata and are never staged.

### Remaining future slices (approved direction, not implemented)

Recorded as of Slice 2 (see implementation plan §22.4). (Slice 3 has since been
implemented — see the "Post-Initial Command Shape — Slice 3" section below.) No separate
roadmap document is created.

- Slice 3: `aikit output list`, `aikit output show`, `aikit output clean`.
- Slice 4: `aikit batch list`, `aikit batch show`, `aikit diff anchor`.
- Slice 5: `aikit env snapshot`, `aikit scan secrets`.

## Post-Initial Command Shape — Slice 3

An approved post-initial slice adds the `output` command family. Recorded here (not as a
new initial batch). Slices 1–2 and the six initial batches remain historical and complete.

### Slice 3 scope (implemented)

- Add the `output` command family (noun-family / action grammar) to manage local aikit
  output artifacts under an output root (default `.aikit/outputs/`). Known artifacts:
  `batches/*.json` files and `inventory/`, `reviews/`, `runs/` subdirectories.
  - `aikit output list` — list known artifacts (read-only); empty success when the output
    root is absent.
  - `aikit output show <artifact-path-or-id>` — show one artifact (read-only); resolve by
    path under the output root or by id; reject out-of-root paths and ambiguous ids;
    missing → `blocked_artifact_not_found`.
  - `aikit output clean` — dry-run by default; `--execute` requires `--older-than`/`--all`;
    deletes only known artifacts inside the output root; never outside the root, via
    symlink escapes, or into `.aikit/temp/`/`.scratch/`/`.claude/`/`target/`/`.git/`.
- New format kinds: `aikit.output_list`, `aikit.output_show`, `aikit.output_clean`. New
  blocked states: `blocked_artifact_not_found`, `blocked_ambiguous_artifact`. No new
  runtime dependency.

### Slice 3 expected committed files

| Path | Classification | Purpose |
|---|---|---|
| `src/output_cmd.rs` | new | `output list`/`show`/`clean` command logic + discovery/safety helpers |
| `src/cli.rs` | modified | `Output`/`OutputCommand::{List,Show,Clean}` family + args + `OutputFamily` enum |
| `src/main.rs` | modified | module + dispatch for the output family |
| `src/formats.rs` | modified | add `OutputArtifact`/`OutputList`/`OutputShow`/`OutputClean` (+ helpers) and three kinds |
| `src/errors.rs` | modified | add `blocked_artifact_not_found`, `blocked_ambiguous_artifact` |
| `tests/cli_output.rs` | new | help, list, show, clean (incl. dry-run/execute/selector/safety) |
| `README.md` | modified | output management section + command list/current-state |
| `docs/agent-usage.md` | modified | output commands in command families + workflow |
| `docs/aikit-cli-spec.md` | modified | §5.7 output list/show/clean (post-initial Slice 3) |
| `docs/aikit-implementation-plan.md` | modified | §22.3 Slice 3 implemented; §22.4 future slices |
| `docs/implementation-manifest.md` | modified | this section |

### Slice 3 expected-vs-actual

To be confirmed against `git status` / `git diff` before commit: the committed set should
match the table above. `src/output.rs` is expected to be **unchanged** (existing
output-root helpers reused; the command logic lives in the new `src/output_cmd.rs`).
`tests/cli_integration.rs` is expected to be **unchanged** (the existing end-to-end test
needs no output-management step). `Cargo.toml` / `Cargo.lock` are expected to be unchanged
(no new dependency). No ignored/local-only files are staged.

### Remaining future slices (approved direction, not implemented)

Slices 4–5 remain approved direction only (see implementation plan §22.4); not
implemented in Slice 3. No separate roadmap document is created.

- Slice 4: `aikit batch list`, `aikit batch show`, `aikit diff anchor`.
- Slice 5: `aikit env snapshot`, `aikit scan secrets`.
