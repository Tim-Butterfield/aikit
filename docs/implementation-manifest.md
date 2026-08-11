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
- Batch 6 is complete and committed (local integration/polish).
- All six initial implementation batches are complete; no further batches are planned.
- Multi-VCS support (Mercurial) + adaptive `init`/`folder init` is complete and committed —
  see "Post-Initial Enhancement — Multi-VCS Support (Mercurial) + Adaptive Init" below.
- Every post-initial enhancement recorded below is implemented, tested on macOS and Windows
  ARM64, and reviewed. Nothing is pending implementation.

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

- Slice 2: `aikit init --require-repo`, `aikit doctor --require-root`.
- Slice 3: `aikit output list`, `aikit output show`, `aikit output clean`.
- Slice 4: `aikit batch list`, `aikit batch show`, `aikit batch diff`.
- Slice 5: `aikit env snapshot`, `aikit scan secrets`.

## Post-Initial Command Shape — Slice 2

An approved post-initial slice adds the `repo` command family. Recorded here (not as a
new initial batch). Slice 1 and the six initial batches remain historical and complete.

### Slice 2 scope (implemented)

- Add the `repo` command family (noun-family / action grammar):
  - `aikit init --require-repo` — prepare the current repository for local aikit usage: create
    `.aikit/` and `.aikit/temp/` if missing and ensure `.aikit/` is locally ignored via
    `.git/info/exclude` (never `.gitignore`). Idempotent; no duplicate ignore entry; no
    output artifacts, `.scratch/`, or `.claude/`; no remote Git state touched.
  - `aikit doctor --require-root` — report repo-local readiness read-only (creates/modifies
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
- Slice 4: `aikit batch list`, `aikit batch show`, `aikit batch diff`.
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

Recorded as of Slice 3 (see implementation plan §22.5). (Slice 4 has since been
implemented — see the "Post-Initial Command Shape — Slice 4" section below.) No separate
roadmap document is created.

- Slice 4: `aikit batch list`, `aikit batch show`, `aikit batch diff`.
- Slice 5: `aikit env snapshot`, `aikit scan secrets`.

## Post-Initial Command Shape — Slice 4

An approved post-initial slice extends the `batch` family and adds a `diff` family.
Recorded here (not as a new initial batch). Slices 1–3 and the six initial batches remain
historical and complete.

### Slice 4 scope (implemented)

- `aikit batch list` — list valid batch anchors under the selected output root's batches/
  folder (read-only); empty success when absent; invalid files reported as skipped; sorted
  by anchor id; `--root`, `--json`. Does NOT auto-select a "latest" anchor.
- `aikit batch show <anchor-path-or-id>` — show one explicit anchor (read-only): resolve
  by path or id; reject path escapes; validate it is a batch anchor belonging to the
  current repo; `--root`, `--json`. Does NOT auto-select.
- `aikit batch diff <anchor>` — mechanical `git diff` from the anchor's
  recorded `git_head` (base) to the current working tree; base must exist locally;
  untracked file contents excluded; creates no review bundle/output artifact; never
  touches remotes; `--stat` (default), `--patch`, `--json`.
- New format kinds: `aikit.batch_list`, `aikit.batch_show`, `aikit.diff_anchor`. New
  blocked state: `blocked_missing_base_commit` (existing missing/invalid-anchor and
  path-escape states reused). No new runtime dependency.

### Slice 4 expected committed files

| Path | Classification | Purpose |
|---|---|---|
| `src/batch.rs` | modified | add `list`/`show` + anchor-resolution helpers; make `load_anchor` reusable |
| `src/diff.rs` | new | `batch diff` command + name-status parsing |
| `src/cli.rs` | modified | `batch list`/`show` args; new `Diff`/`DiffCommand::Anchor` family |
| `src/main.rs` | modified | module + dispatch for batch list/show and the anchor diff |
| `src/formats.rs` | modified | add `AnchorView`/`BatchList`/`BatchShow`/`DiffAnchor` (+ helpers) and three kinds |
| `src/errors.rs` | modified | add `blocked_missing_base_commit` |
| `src/repo.rs` | modified | add `commit_exists` + `git_diff` helpers |
| `src/output.rs` | modified | host the shared validated `resolve_output_root` (moved from `output_cmd.rs`) |
| `src/output_cmd.rs` | modified | call the shared `output::resolve_output_root` (DRY; no behavior change) |
| `tests/cli_batch.rs` | modified | add `batch list`/`show` tests |
| `tests/cli_diff.rs` | new | `batch diff` tests |
| `README.md` | modified | batch inspection + anchor diff section + command list/current-state |
| `docs/agent-usage.md` | modified | new commands in command families + workflow note |
| `docs/aikit-cli-spec.md` | modified | §5.8 batch list/show + the anchor diff (post-initial Slice 4) |
| `docs/aikit-implementation-plan.md` | modified | §22.4 Slice 4 implemented; §22.5 future slices |
| `docs/implementation-manifest.md` | modified | this section |

### Slice 4 expected-vs-actual

To be confirmed against `git status` / `git diff` before commit. **Justified deviations
from the task's likely-file list:** `src/repo.rs` is modified (added `commit_exists` /
`git_diff` git helpers used by `batch diff`); and `src/output.rs` + `src/output_cmd.rs`
are modified to centralize the validated `resolve_output_root` (moved from `output_cmd.rs`
to `output.rs`) so `batch list`/`show` reuse the same `--root` safety logic rather than
duplicating it — a DRY refactor with no behavior change to `output`. `tests/cli_integration.rs`
is expected to be **unchanged** (the existing end-to-end test needs no batch-inspection
step). `Cargo.toml` / `Cargo.lock` are expected to be unchanged (no new dependency). No
ignored/local-only files are staged.

### Remaining future slice (approved direction)

Recorded as of Slice 4. (Slice 5 has since been implemented — see the "Post-Initial
Command Shape — Slice 5" section below.) No separate roadmap document is created.

- Slice 5: `aikit env snapshot`, `aikit scan secrets`.

## Post-Initial Command Shape — Slice 5

The final approved post-initial slice adds an `env` family and a `scan` family. Recorded
here (not as a new initial batch). Slices 1–4 and the six initial batches remain historical
and complete. **This slice completes the approved five-slice post-initial command
expansion.**

### Slice 5 scope (implemented)

- `aikit env snapshot` — a bounded, read-only local environment report: aikit version,
  current executable, OS family, CPU architecture, working directory, repo facts when inside
  a Git repo (root, branch, HEAD, tracked clean/dirty, default output root, `.aikit/`
  `.aikit/temp/` `.aikit/outputs/` existence, `.aikit/` ignore status), interpreter
  availability (`/bin/sh`, `/bin/zsh`), local git/Rust/Cargo versions, and `$SHELL`. Works
  outside a repo (repo facts `null` + warning). Creates nothing; runs no network commands.
  Does NOT dump all environment variables, the raw `PATH`, tokens, credentials, or keys;
  `PATH` is summarized only (entry count + on-PATH boolean). `--json`.
- `aikit scan secrets <path>...` — redacted secret scan over explicit repo-local paths.
  Blocks outside a repo (`blocked_repo_not_found`); requires ≥1 explicit path; resolves
  paths relative to the repo root; rejects out-of-repo and symlink/path escapes
  (`blocked_path_escape`); always excludes `.git/`. Scans explicit files even when ignored;
  directory traversal respects `.gitignore` by default (`--include-ignored`); skips binary
  files and files over `--max-file-bytes` (default 1 MiB). Rule set = format rules matching
  self-identifying credential formats (`private_key_block`, `access_key_id`,
  `vcs_host_token`, `chat_platform_token`, `payment_live_secret_key`, `cloud_api_key`,
  `model_provider_key`, `package_registry_token`, `signed_web_token`) plus one name-based
  rule (`long_token_assignment` / `credential_assignment`). Precision over coverage — not a
  comprehensive corpus, and no replacement for gitleaks/trufflehog. Severity = confidence
  the match is a real credential, not blast radius: `high` for a format rule or a
  credential-style name assigned a long opaque token; `medium` for a name assigned a short
  or word-like value; `low` for that name-based match in an example/sample/template/fixture
  path (an example path demotes the name-based rule ONLY). NEVER prints raw secret values
  (human or JSON); findings carry path, line, rule id, description, severity,
  `redacted: true`. Default exit 0; `--fail-on <high|medium|low>` exits 3 with
  `blocked_secret_findings` when a finding at that severity or above exists (monotonic).
  Records `fail_on` (null when absent) and per-severity `counts`. Creates no artifacts.
  `--json`.
- New format kinds: `aikit.env_snapshot`, `aikit.scan_secrets`. New blocked state:
  `blocked_secret_findings`. New runtime dependency: `regex` (materially simplifies the
  secret rule set).

### Slice 5 expected committed files

| Path | Classification | Purpose |
|---|---|---|
| `Cargo.toml` | modified | add the `regex` dependency |
| `Cargo.lock` | modified | lock `regex` (and its transitive deps already present via `ignore`) |
| `src/env.rs` | new | `env snapshot` command (mechanical environment report) |
| `src/scan.rs` | new | `scan secrets` command (heuristic scan + rules) |
| `src/cli.rs` | modified | new `Env`/`Scan` families and their args |
| `src/main.rs` | modified | module declarations + dispatch for env/scan |
| `src/formats.rs` | modified | add `EnvSnapshot`/`ScanSecrets` (+ nested structs) and two kinds |
| `src/errors.rs` | modified | add `blocked_secret_findings` |
| `src/repo.rs` | modified | add `detect_root_opt` (non-blocking repo detection for `env snapshot`); harden the porcelain status probes with `--no-optional-locks` so the dirty check never rewrites `.git/index` (review remediation) |
| `tests/cli_env.rs` | new | `env snapshot` tests |
| `tests/cli_scan.rs` | new | `scan secrets` tests |
| `README.md` | modified | env snapshot + secret scan sections + command list/current-state |
| `docs/agent-usage.md` | modified | new commands in command families + examples + exit-code |
| `docs/aikit-cli-spec.md` | modified | §5.9 env snapshot / scan secrets (post-initial Slice 5) |
| `docs/aikit-implementation-plan.md` | modified | §22.5 Slice 5 implemented; §22.6 expansion complete |
| `docs/implementation-manifest.md` | modified | this section |

### Slice 5 expected-vs-actual

To be confirmed against `git status` / `git diff` before commit. **Justified deviation from
the task's likely-file list:** `src/repo.rs` is modified for two reasons: (1) adds
`detect_root_opt`, a non-blocking repo-detection helper so `env snapshot` can report
non-repo facts outside a Git repo; (2) cross-AI-review remediation — the porcelain status
probes (`git_status_porcelain` / `git_status_changed`) gained `--no-optional-locks` so the
dirty-tree probe used by `env snapshot` (and `repo doctor` / `batch diff`) never rewrites
`.git/index`, making the read-only guarantee strictly true. `Cargo.toml` / `Cargo.lock` are
modified (the justified `regex` dependency). `tests/cli_integration.rs` is expected to be
**unchanged** (the existing end-to-end test needs no env/scan step). No ignored/local-only
files are staged.

**Cross-AI review (report mode) outcome.** Iterating reviewer (codex gpt-5.5, deep) +
cross-check (gemini-3.1-pro-preview, deep); both model identities verified, no Class E, no
halt. Gemini approved with four `pass` confirmations (type-level no-secret-leak guarantee,
PATH summarization, path/symlink-escape blocking, binary/UTF-8 sniff). Codex raised two
valid findings, both remediated before commit: (1) `env snapshot` dirty probe could rewrite
`.git/index` → fixed via `--no-optional-locks` (test `env_snapshot_does_not_write_git_index`);
(2) nested `.git/` explicit files were scannable → `is_under_git` made component-based (test
`scan_secrets_excludes_nested_git_explicit_file`). Run dir under
`.scratch/work/outputs/plan-review/slice5-review/` (local-only).

### Approved post-initial command expansion — complete

Slices 1–5 are all implemented. There are no remaining approved post-initial command
slices; any further command work requires a new explicitly approved task. No separate
roadmap document is created.

## Post-Initial Cleanup — Review bundle filename

A small post-initial cleanup (not a new command slice) renamed the generated review-bundle
text artifact from `run_for_review.txt` to **`review_bundle.txt`**. The initial six batches
and the five post-initial command slices remain historical and complete.

### Cleanup scope (implemented)

- `aikit review generate` (both `--files` and `--anchor` modes) writes `review_bundle.txt`
  plus `manifest.json`; the manifest's `bundle_path` records `review_bundle.txt`. The old
  file is no longer written and no compatibility duplicate is created.
- Filename change only: command family, flags, bundle content format, path-safety, caps,
  hashing, and anchor behavior are unchanged.
- `aikit output show` already recognizes review artifacts by the `reviews/` family and the
  `manifest.json` summary, and lists the directory's files generically — so it handles new
  (`review_bundle.txt`) and older historical (`run_for_review.txt`) local review outputs
  with no code change. Existing local `run_for_review.txt` artifacts are left untouched (no
  migration of ignored local outputs).

### Cleanup expected committed files

| Path | Classification | Purpose |
|---|---|---|
| `src/review.rs` | modified | `BUNDLE_NAME` constant `run_for_review.txt` → `review_bundle.txt` (drives the path builder, manifest `bundle_path`, `written`, and human output) |
| `src/cli.rs` | modified | review help text references the new bundle file name |
| `tests/cli_review.rs` | modified | expect `review_bundle.txt`; assert old file is not created; assert human output names it |
| `tests/cli_output.rs` | modified | add `output show` recognition of a review artifact containing `review_bundle.txt` |
| `tests/cli_integration.rs` | modified | end-to-end review `written` reports `review_bundle.txt` |
| `README.md` | modified | active bundle name → `review_bundle.txt` (historical note) |
| `docs/agent-usage.md` | modified | active bundle name → `review_bundle.txt` |
| `docs/aikit-cli-spec.md` | modified | §5.4 output convention → `review_bundle.txt` |
| `docs/aikit-implementation-plan.md` | modified | §9.3 design + new §22.7 cleanup note |
| `docs/implementation-manifest.md` | modified | this section |

### Cleanup expected-vs-actual

To be confirmed against `git status` / `git diff` before commit. **Justified deviations
from the task's likely-file list:** `src/formats.rs` is expected to be **unchanged** — the
manifest's `bundle_path` is populated from `review.rs`'s `BUNDLE_NAME` constant, and the
`ReviewManifest.bundle_path` field is filename-agnostic, so no struct change is needed.
`src/output_cmd.rs` is expected to be **unchanged** — `output show` lists a review
directory's files generically and reads `manifest.json`; it never hard-codes the bundle
filename, so it recognizes the renamed bundle without modification (covered by a new
`tests/cli_output.rs` test). `tests/cli_integration.rs` IS modified (the end-to-end test
asserts the reported bundle filename). Historical Batch 3/4 records in this manifest that
mention `run_for_review.txt` (as originally delivered/tested) are retained as historical and
are **superseded by this rename**. No ignored/local-only files are staged.

## Post-Initial Enhancement — Multi-VCS Support (Mercurial) + Adaptive Init

A post-initial enhancement (not a new command slice from the original plan) adds
**Mercurial** support alongside Git and introduces an adaptive top-level `aikit init` plus
an explicit `aikit init --require-folder` for non-repository folders. The initial six batches and the
five post-initial command slices remain historical and complete. This section exists for
the manifest-compare gate; it is implemented and pending cross-AI review + commit.

### Enhancement scope (implemented)

- **Filesystem-based VCS/root detection (no subprocess).** Detection walks up for an
  enclosing `.git` (file or directory), `.hg`, or `.aikit` marker. It requires **no**
  `git`/`hg` CLI invocation, so it works on hosts without either binary and on Windows.
  - `repo::find_vcs_root()` — nearest `.git`/`.hg` (Git precedence) for the init commands.
  - `repo::detect_marker_root()` — nearest `.git`/`.hg`/`.aikit`, returning the root and an
    `Option<Vcs>`, for the `script run`/`check` gate (`blocked_repo_not_found` when none).
  - The Git-only commands (`batch`, `diff`, `inventory`, `review`, `scan`, `env`,
    `output`) are **unchanged**; they continue to use the existing git-CLI `detect_root()`
    and remain Git-only by design.
- **New setup verbs (adaptive + explicit):**
  - `aikit init` — adaptive: repo mode (dirs + VCS ignore) inside a Git/Mercurial repo,
    else folder mode (dirs only). Never errors on repo presence/absence.
  - `aikit init --require-folder` — force non-repo mode (dirs only); errors `blocked_repo_present`
    inside a repository.
  - `aikit init --require-repo` — unchanged contract (force repo mode), now also works in Mercurial
    repos; still errors `blocked_repo_not_found` outside a repository.
- **Mercurial ignore mechanism.** In an hg repo, ignore coverage is written to
  `.hg/hgignore.aikit` (pattern `re:^\.aikit/`) and registered via `[ui] ignore.aikit` in
  `.hg/hgrc` — both under `.hg/`, never committed or cloned, mirroring Git's local-only
  `.git/info/exclude`. A tracked `.hgignore`/`.gitignore` is never modified; existing
  coverage is detected (filesystem) and not duplicated.
- **`aikit doctor` is VCS-aware.** Works in Git repos, Mercurial repos, and non-repo
  `.aikit/` folders; reports a `vcs` field. For Mercurial: ignore coverage is detected
  without invoking `hg`; branch/HEAD use `hg` (run with `HGPLAIN=1`) when available and
  degrade to empty with a warning otherwise; the tracked-tree check uses `hg status -mard`
  and degrades to "clean" + a warning when `hg` is absent. Readiness does not require
  ignore coverage in a non-repo folder. Read-only and exit-0-in-a-root behavior unchanged.
- **`aikit script run` / `script check` are VCS-aware.** The gate uses
  `detect_marker_root()` (filesystem; non-repo runs incur zero subprocesses). The
  `--require-clean` dirty check is VCS-specific and runs only with that flag: Git
  `git status --porcelain`; Mercurial `hg status -mard` (HGPLAIN; the only place the runner
  invokes `hg`; errors if absent); a non-repo root blocks
  `blocked_require_clean_unsupported`. `run.json` records `vcs`; HEAD is populated for Git
  and empty for Mercurial/non-repo.
- **New blocked states:** `blocked_repo_present`, `blocked_require_clean_unsupported`.
- **New JSON fields (no new format kinds):** `vcs` added to `aikit.repo_init`,
  `aikit.repo_doctor`, and `aikit.script_run`. `RepoDoctor.git_branch`/`git_head` field
  names are retained for schema stability and documented as VCS-generic.
- **HGPLAIN policy.** All aikit-internal `hg` invocations go through `repo::hg_command()`,
  which sets `HGPLAIN=1` per call. HGPLAIN is **not** set globally on aikit's process, so
  user scripts run via `script run` keep their normal environment.
- No new runtime dependency (`Cargo.toml` / `Cargo.lock` unchanged).

### Enhancement expected committed files

| Path | Classification | Purpose |
|---|---|---|
| `src/repo.rs` | modified | `Vcs` enum; filesystem detection (`markers_in`/`walk_up`/`find_vcs_root`/`detect_marker_root`); hg helpers (`hg_command`/`run_hg`/`hg_tracked_tree_dirty`/`vcs_branch`/`vcs_head`/`hg_aikit_ignore_source`); hg ignore writer (`ensure_aikit_ignored_hg` + hgrc/hgignore helpers); `do_init` + `init`/`init_auto`/`init_folder`; VCS-aware `doctor` |
| `src/cli.rs` | modified | add top-level `Init` + `Folder`/`FolderCommand::Init` + args; update `init`/`folder init`/`repo init`/`repo doctor`/`repo` group/top-level long_abouts |
| `src/main.rs` | modified | dispatch `aikit init` and its `--require-*` modes |
| `src/errors.rs` | modified | add `blocked_repo_present`, `blocked_require_clean_unsupported` |
| `src/formats.rs` | modified | add `vcs` to `RepoInit`, `RepoDoctor`, `ScriptRun`; doc updates (VCS-generic `git_branch`/`git_head`) |
| `src/script.rs` | modified | `Located.vcs`; gate via `detect_marker_root`; VCS-aware `--require-clean`; `vcs` in `run.json`; head probe git-only |
| `tests/cli_repo.rs` | modified | hg `repo init` tests; `init`/`folder init` tests (git/hg/non-repo, refusals); hg + non-repo `doctor` tests |
| `tests/cli_script.rs` | modified | non-repo `.aikit` run; hg-marker run; non-repo `--require-clean` block; markerless block |
| `README.md` | modified | command list; repo setup (Git/Mercurial/non-repo); script run detection + `--require-clean` + `run.json` |
| `docs/agent-usage.md` | modified | purpose; blocked-state list; workflow; init/folder/doctor sections; Script Runner Use |
| `docs/aikit-cli-spec.md` | modified | §5.6 init family + doctor; §5.1 script run/check; root-detection principle; blocked-states list |
| `docs/implementation-manifest.md` | modified | this section + Status line |

### Enhancement expected-vs-actual

To be confirmed against `git status` / `git diff` before commit; the committed set should
match the table above. **Justified deviations / non-changes:**

- `src/policy/script.rs` is **unchanged** — the allowed-location allowlist and runner
  detection are reused as-is; only the root anchoring (in `script.rs`) changed.
- `Cargo.toml` / `Cargo.lock` are **unchanged** — no new dependency (hg is shelled out to,
  not linked; detection is pure `std::fs`).
- `tests/cli_integration.rs` is **unchanged** — the end-to-end flow runs in a Git repo and
  is unaffected; new VCS paths are covered by `tests/cli_repo.rs` / `tests/cli_script.rs`.
- `docs/aikit-implementation-plan.md` is **unchanged** — the plan documents the original
  batches/slices; this enhancement is recorded here in the manifest rather than retro-fitted
  into the historical plan. (Flagged as a deliberate deviation from prior slices, which did
  update the plan; revisit if a living-plan entry is preferred.)
- Mercurial is not installed in the dev/CI environment, so hg tests exercise the
  filesystem-detectable paths (a `.hg/` marker, ignore-file writing, doctor degradation +
  warning) **without** requiring the `hg` binary; the `hg`-invoking paths (`hg status`
  dirty check, `hg` branch/head) are covered by code review rather than execution here.
- No ignored/local-only files are staged; Mercurial ignore writes (`.hg/hgignore.aikit`,
  `.hg/hgrc`) and Git `.git/info/exclude` writes are local VCS metadata and are never
  staged.

## Enhancement: `aikit mcp` (MCP server)

The command's behaviour is specified in `docs/aikit-cli-spec.md` §5.12; its execution
posture is in `SECURITY.md`.

- **New command family:** `aikit mcp`, an MCP server on stdio exposing three tools,
  `run`, `list_runners` and `agents_md`. It is a **separate contract** from `aikit script run`, not a
  wrapper: the script arrives as call arguments, so no file is written, no repository or
  `.aikit/` directory is required, the dirty check does not apply, no run record is kept,
  and configuration is not consulted.
- **`runner` is required and never inferred.** No shebang, extension-map or OS-default
  tier applies on this path, because `pwsh` and `powershell` interpret the same `.ps1`
  differently. Runners never fall back to a sibling; an unavailable runner is an error.
- **Result shape.** Every result carries a `TextContent` block **and** `structuredContent`
  against a declared `outputSchema`. `outputSchema` governs non-error results only, and not
  every client forwards `structuredContent` into model context, so a structured-only result
  can reach the model as an empty response. `stop_reason` is one of `exited`, `timeout`,
  `output_limit`, `server_limit`, `spawn_failed`, `setup_failed`; there is no `cancelled`,
  because the protocol forbids responding to a cancelled request.
- **Limits.** `timeout_ms` defaults to 120 000 and is capped at 3 600 000; **null is
  rejected** — clients are not required to report an abandoned call, so this timeout is the
  only thing that can stop a runaway script. `max_bytes` defaults to 32 MiB combined across
  streams with per-stream `truncated` flags; `on_output_limit` defaults to `truncate`,
  which keeps draining both pipes so the child never blocks. Concurrency is capped at 8 and
  over-capacity calls are **rejected** (`server_limit`) rather than queued.
- **Process control.** stdout, stderr and stdin each run on their own task (a large stdin
  write to a child that fills stdout first would otherwise deadlock both sides); the capture
  loop shares the child's deadline; POSIX places the child in its own process group and
  signals with `killpg`; Windows creates the child suspended, assigns it to a kill-on-close
  Job Object, then resumes it.
- **`env_base`** is `"inherit"` (default) or `"minimal"`. Minimal is a curated floor under
  which a normal shell works — including `PATH` and `PSModulePath`, not merely the variables
  process creation needs — and floor names are immutable against `null` removal.
- **Temp scripts are ephemeral, not absent.** They exist while running; the OS share-locks
  an executing script, so deletion is deferred and best-effort, backed by a per-instance
  directory named with a random nonce (not a PID, which is reused), created with restrictive
  permissions and swept by acquiring its lock.
- **No new format kinds and no new blocked states**: the MCP surface reports through the
  tool result, not through aikit's CLI record formats.
- **New runtime dependencies:** `rmcp` (MCP protocol), `tokio` (async runtime) and
  `tokio-util` (the `CancellationToken` rmcp already puts in `RequestContext`, used directly
  rather than bridged onto a second signal).

### MCP expected committed files

| Path | Classification | Purpose |
|---|---|---|
| `src/mcp/mod.rs` | new | module root; contract summary; runtime setup |
| `src/mcp/server.rs` | new | `ServerHandler`: tool listing, argument validation, dispatch, result shaping |
| `src/mcp/tools.rs` | new | tool names, descriptions, hand-written input/output JSON schemas |
| `src/mcp/exec.rs` | new | process execution: concurrent stdio, byte budget, timeout, cancellation |
| `src/mcp/platform.rs` | new | POSIX process groups; Windows Job Object suspend/assign/resume |
| `src/mcp/envmap.rs` | new | `env_base` floor and the set/remove overlay |
| `src/mcp/workdir.rs` | new | per-instance temp dir, lock, script writing (extension/CRLF/BOM), sweep |
| `src/policy/script.rs` | modified | additive public surface for the MCP path (`is_known_runner_name`, `all_runner_names`, `resolve_runner_program`, `mcp_runner_flags`, `runner_script_extension`, `runner_wants_crlf`, `runner_wants_bom`) |
| `src/cli.rs` | modified | add the `Mcp(McpServeArgs)` leaf command and its long_about |
| `src/main.rs` | modified | declare `mcp` module; dispatch `mcp` |
| `tests/cli_mcp.rs` | new | help surface + full stdio protocol exchange against the real binary |
| `tools/mcp-probe/*` | new | client-agnostic MCP diagnostic server, self-test, ACP driver, README |
| `tests/cli_repo.rs` | modified | help assertions rewritten to be wrap-position independent |
| `tests/cli_script.rs` | modified | help assertions rewritten to be wrap-position independent |
| `Makefile` | new | `build test fmt fmt-check lint lint-windows verify install uninstall clean`; `verify` is the default goal |
| `Cargo.toml` / `Cargo.lock` | modified | add `rmcp`, `tokio`, `tokio-util`; enable clap's `wrap_help` |
| `.gitignore` | modified | unanchor `tmp/` so it matches at any depth; `.aikit/` moved to `.git/info/exclude` |
| `SECURITY.md` | modified | MCP execution posture; temp-script disk exposure |
| `README.md` | modified | add `aikit mcp` to the command list |
| `docs/aikit-cli-spec.md` | modified | §5.12 `mcp` (placed after §5.11); scope bullet |
| `docs/agent-usage.md` | modified | `aikit mcp` command-family entry |
| `docs/aikit-implementation-plan.md` | modified | drop the reference to the removed decision record |
| `docs/decisions/0001-create-aikit.md` | **deleted** | see below |
| `docs/implementation-manifest.md` | modified | this section |

### MCP expected-vs-actual

To be confirmed against `git status` / `git diff` before commit. **Justified deviations /
non-changes:**

- `src/script.rs` is **unchanged** — the CLI runner keeps its allowed-location gate, dirty
  check and run record. The MCP path shares the runner *table*, not the runner.
- **`mcp_runner_flags` deliberately diverges** from the CLI's `runner_flags`: PowerShell
  additionally gets `-NonInteractive`. The CLI runs a script the user approved from a
  repo-local path and can tolerate a prompt; an MCP call closes stdin, so a prompt would
  block silently for the whole timeout instead of failing. This is the one sanctioned
  divergence in the shared policy module.
- `src/formats.rs` and `src/errors.rs` are **unchanged** — no new CLI record kind or blocked
  state; MCP failures are reported in-band as tool results.
- Windows-only code is compiled but not executed here: it is typechecked with
  `cargo check --target x86_64-pc-windows-msvc --all-targets`. Behaviour under a real Job
  Object (including whether the host process is already inside one, which makes nested
  assignment fail) is unverified until it runs on Windows. The *validation* tests are
  platform-neutral (`any_runner()` / `any_dir()`) so they assert what they exist to assert
  on Windows rather than failing on an unresolvable `sh`; the execute-path tests are
  `#[cfg]`-split, with a `cmd` counterpart covering the Windows path.
- **Cancellation passes the request's own `RequestContext::ct` to the executor**, unmediated.
  Do not substitute a hand-rolled signal: checking an `AtomicBool` and then awaiting a
  `Notify` has a lost-wakeup window — a cancel arriving between the load and the waiter's
  registration is missed by `notify_waiters`, leaving the call parked until its timeout.
  Nor bridge one signal onto another with a spawned task: it outlives every call that is
  never cancelled.
- **Argument validation enforces `additionalProperties: false` itself.** The schemas are
  hand-written, so nothing else does. Unknown keys at the top level and inside `limits` are
  rejected rather than ignored: accepting `{"limits": {"timeoutMs": 5000}}` and then running
  for the 120 s default is the worst available outcome, because the caller believes the call
  is bounded. `max_bytes` is capped for the same reason `null` is refused — an unbounded
  budget bounds nothing.
- **Environment-name case folding is platform-conditional.** Windows environment names are
  case-insensitive, so `path` and `PATH` are one variable and must be collapsed; on POSIX
  they are two, and collapsing silently discarded one.
- **The temp directory and script are created with owner-only permissions**, via `mkdir`'s
  mode and `OpenOptions::mode`. Do not create them and narrow afterwards: the system temp
  location is world-writable, so that leaves a readable window. Windows applies no ACL, and
  that is stated wherever the behaviour is documented rather than covered by the phrase
  "restrictive permissions".
- `tools/mcp-probe/` writes its log to `$AIKIT_PROBE_LOG` or the OS temp directory, never
  into a checkout, and assumes no paths — the client binary is always supplied by the user.
- **`docs/decisions/` is removed.** These documents record why past choices were made, which
  duplicates what git already holds and competes with the documents that state what is
  currently true. Design rationale now lives with the thing it explains:
  `docs/aikit-cli-spec.md` for behaviour, `SECURITY.md` for posture, this manifest for the
  change set. Nothing was lost — the only content not already stated elsewhere (the MCP
  server's protocol and lifecycle handling) was moved into §5.12 first.

## Command shape: `aikit init` / `aikit doctor` and their `--require-*` flags

Readiness and initialization are two top-level commands, `aikit doctor` and `aikit init`,
each adaptive by default with strictness selected by a flag rather than by a different
command name: `init --require-repo`, `init --require-folder`, `doctor --require-root`.
There is no `repo` or `folder` command group.

- **The flag is the mode, because the mode is a constraint, not a different operation.**
  `aikit init` and `aikit init --require-repo` do the same thing; the flag only says what
  the caller is willing to accept. Encoding that as a separate command name made the noun
  (`repo`, `folder`) look like a subject the command acts on, when it is really a
  precondition on the environment.
- **Adaptive is the default because the documented loop depends on it.** The guidance is
  "run `doctor` first, `init` only on a reported gap". A command that blocks when no marker
  is present breaks that loop in exactly the fresh folder the loop exists for, so the
  unqualified form reports rather than refuses, and `--require-root` is available when a
  caller genuinely wants the refusal.
- **Doctor requires a marker, not a repository.** Root resolution goes through
  `detect_marker_root()`, which accepts `.git`, `.hg`, **or `.aikit`**, and `do_doctor`
  handles `vcs: None` inline throughout — so a non-repo `.aikit/` folder reports normally.
  `--require-root` blocks `blocked_repo_not_found` when no marker is found.
- **One implementation, one entry point per command.** `doctor()` is a thin wrapper over
  `do_doctor(kind, args)`, with `DoctorKind` governing root resolution only; `init` wraps
  `do_init` the same way. Strictness never forks the body.
- **Init keeps a three-way distinction because init *mutates*.** The mutation differs per
  mode, and `--require-folder` must refuse inside a repo. Doctor writes nothing, so it needs
  only the two states (report, or refuse without a marker).
- **Schema:** the same `aikit.repo_doctor` kind, with one **added** field, `root_source`
  (`"marker"` | `"cwd_no_marker"`). No existing field is repurposed. The field exists
  because `repo_root` is just a path either way, so without it a caller cannot distinguish
  an anchored root from a directory that merely happens to be where the command ran.
- **`ready` is never true for an unanchored report**, independently of the other checks —
  otherwise a folder that happened to contain `temp/` could look initialized.

### `init` / `doctor` expected committed files

| Path | Classification | Purpose |
|---|---|---|
| `src/repo.rs` | modified | `DoctorKind`; `do_doctor()` shared body; no-marker report path |
| `src/formats.rs` | modified | add `root_source` to `RepoDoctor`; `KIND_REPO_DOCTOR` unchanged |
| `src/cli.rs` | modified | top-level `Doctor(RepoDoctorArgs)`; `InitArgs { require_repo, require_folder }`; `RepoDoctorArgs { require_root }`; `repo`/`folder` groups removed |
| `src/main.rs` | modified | dispatch `Command::Doctor` / `Command::Init` |
| `tests/cli_repo.rs` | modified | adaptive doctor in git / hg / non-repo-`.aikit` / no-marker; `--require-root` blocks; read-only on the no-marker path; per-mode init behavior |
| `docs/aikit-cli-spec.md` | modified | §5.6 init/doctor family; scope bullet |
| `docs/agent-usage.md` | modified | doctor-first loop; command entries |
| `docs/agent-integration-examples.md` | modified | readiness wrapper uses `aikit doctor` |
| `README.md` | modified | setup snippet; command list; unanchored-report note |

## Enhancement: embedded agent guide (`agents_md` / `aikit agents-md`)

One document, `AGENTS.md`, compiled into the binary with `include_str!` and served by two
entry points: the `agents_md` MCP tool and the `aikit agents-md` command.

- **Embedded rather than read from the repository.** A guide read from the working
  directory describes whatever repository the agent happens to be standing in, which may be
  a different aikit version — or absent entirely, since the MCP server requires no
  repository. Compiling it in makes the answer a property of the binary, so it is identical
  across hosts and always matches the version answering.
- **The full document, not an abridged one.** An abridged variant would be a second thing to
  keep true, and the two would drift. The compile-time cost is a string constant.
- **Raw Markdown in the MCP text block, not a JSON string.** The tool returns prose, and a
  client that shows the model only the text block would otherwise deliver an escaped blob.
  This is the one tool whose payload is not a record, so it does not carry
  `structuredContent`.
- **`AGENTS_MD` override, deliberately unprefixed.** Every other aikit environment knob
  would be ambiguous without a prefix; this one is named by a tool that already says whose
  guide it is, so `AIKIT_AGENTS_MD` would only repeat the word. An unreadable override is an
  **error** naming the variable and the path, never a silent fall back to the embedded text
  — a silent fallback would answer with the wrong document while looking correct.
- **Shared resolution.** `mcp::resolve_agents_md()` returns `(content, source)` and backs
  both entry points, so the CLI and the MCP tool cannot disagree. `source` is
  `"embedded"` or `"override:<path>"`.
- **No `--json` on the CLI.** The payload is Markdown; wrapping it in a JSON string would
  make it less usable, not more.

### Agent-guide expected committed files

| Path | Classification | Purpose |
|---|---|---|
| `AGENTS.md` | new | The guide itself; the single source for both entry points |
| `src/mcp/tools.rs` | modified | `EMBEDDED_AGENTS_MD` (`include_str!`), `AGENTS_MD_OVERRIDE`, `agents_md` schemas/description |
| `src/mcp/server.rs` | modified | `resolve_agents_md()`; `agents_md()` returning raw Markdown |
| `src/mcp/mod.rs` | modified | re-export `resolve_agents_md` |
| `src/cli.rs` | modified | `AgentsMd` leaf command + long_about/after_help |
| `src/main.rs` | modified | dispatch `Command::AgentsMd` |
| `tests/cli_mcp.rs` | modified | tool is listed; embedded content served; override honored; unreadable override errors |
| `docs/aikit-cli-spec.md` | modified | §5.12 tool list + `agents_md` bullet; new §5.13 |
| `docs/agent-usage.md` | modified | `aikit agents-md` entry; MCP tool list |
| `README.md` | modified | command list; Agent guide + MCP server sections |

## Enhancement: forbidden-operation acknowledgement

The forbidden-operation scan previously had one outcome: refuse. A caller whose operation
was genuinely intended had no way to proceed except to rewrite the script until the
substring no longer matched — which defeats an accident guard rather than satisfying it.

- **`--acknowledge-forbidden <pattern>` (repeatable) on `script run` and `script check`**,
  and `acknowledge_forbidden: string[]` on the MCP `run` tool.
- **Per pattern, never global.** Each entry must equal an advertised pattern exactly, so
  acknowledging one never disables the others. The MCP schema's `enum` is the same
  `FORBIDDEN_PATTERNS` constant `scan_forbidden_all` enforces, so the accepted values and
  the refused ones cannot drift apart.
- **Per invocation, never remembered.** There is no stored allowlist; each run restates its
  own intent. A persisted acknowledgement would silently cover future scripts nobody
  reviewed.
- **The refusal reports every match, not just the first.** `scan_forbidden_all()` returns
  `ForbiddenMatch { pattern, line, column, offset }` for all matches, so one refusal names
  everything that must be acknowledged instead of forcing a match-fix-rerun cycle.
- **`run.json` records `acknowledged_forbidden`**, so the audit trail shows what was waived
  rather than showing a clean run indistinguishable from one that triggered nothing.

## Enhancement: Windows execution correctness

Behaviours found by running the suite on Windows, each a case where the command appeared to
succeed while doing something other than what was recorded.

- **Verbatim-path containment.** `std::fs::canonicalize` returns `\\?\C:\...` (and
  `\\?\UNC\server\share\...` for a mapped drive) on Windows, so `strip_prefix` against a
  non-verbatim repo root failed and path-escape checks fell through to "file missing" —
  reporting the wrong blocked state for a real traversal attempt. `formats::canonicalize`
  now strips the `\\?\UNC\` and `\\?\` prefixes, and is the single canonicalizer the
  containment checks use.
- **`.sh` scripts silently ran under WSL.** `bash.exe` and `wsl.exe` under `System32` are
  WSL launchers: the script executed in a Linux filesystem namespace where the repo-relative
  paths meant something else. They are hard-excluded from runner resolution.
- **App Execution Aliases are deprioritised, not excluded.** Windows installs 0-byte reparse
  points on `PATH` that open the Store when executed. Excluding zero-byte candidates
  outright also removed genuinely Store-installed interpreters such as `pwsh`, so a real
  executable now wins and the alias is kept only as a fallback.
- **Exit codes were masked by the interpreter.** A PowerShell script whose last native
  command failed still exited 0, and batch files propagate `ERRORLEVEL` lossily. The MCP
  path appends `runner_exit_epilogue()` (`exit $LASTEXITCODE` / `exit /b %ERRORLEVEL%`)
  after a forced newline. Known limits are documented at the function: `$LASTEXITCODE`
  tracks native commands only, and a script calling `exit` itself terminates first — which
  is correct, an explicit exit being the caller's stated intent.
- **`cmd` runs with `/d`.** Without it, `Command Processor\AutoRun` from the registry
  executes before the script and can rewrite `PATH` — the same class of ambient mutation
  `-NoProfile` refuses for PowerShell.
- **Environment-variable names compare case-insensitively only on Windows** (`same_name()`
  under `cfg!(windows)`), matching the platform rather than the developer's host.
- **Job Objects kill the process tree**, verified against a surviving grandchild.
- **argv carries the script path in native separators.** `cmd.exe` does not accept `/` as a
  path separator: given `.aikit/temp/h.cmd` it answers `'.aikit' is not recognized as an
  internal or external command` and exits 1 with an empty stderr, so the failure looks like
  the script's own. `exec_path()` converts on Windows for both the spawn and the recorded
  `argv`, which keeps the record equal to what actually ran; the portable forward-slash
  spelling stays in `script_path`. Conversion is unconditional on Windows rather than
  per-runner — PowerShell, python, node and the POSIX shells all accept either form, and one
  rule is easier to keep true than a table of exceptions.
- **A runner backed by a real executable beats one backed only by an App Execution Alias.**
  Windows ships a zero-byte `python3.exe` alias with no Python behind it, so `.py` selected
  `python3`, reported the runner as available, and then failed at exec with 9009 — a
  "command not found" surfacing as the script's own exit code. The preference is applied
  across *proposals* in `detect_runner`, not only across candidates within one runner spec,
  because `python3` and `python` are separate specs and the alias-only one is proposed
  first. An alias is still accepted when nothing real is proposed, since a Store-installed
  PowerShell 7 has no other PATH entry. An explicit `--runner` bypasses the preference: one
  name was requested, so there is no alternative to prefer.

## Cross-AI review pass (pre-publication)

An independent two-reviewer pass over the pre-publication change set: codex `gpt-5.5`
iterating, agy `Gemini 3.1 Pro` cross-check, both read-only, both model-verified. Report
mode (single pass per primitive), primitive P1 in its no-upstream form — internal
consistency of the artifact against its own stated goals. Verdict `request_changes`; nine
findings adjudicated valid and applied, one invalid.

The theme of the valid findings is the same one this whole effort keeps surfacing: **the
documentation described the behaviour that was intended, not the behaviour that was
written.**

- **Two findings concerned the MCP `record` option, which no longer exists.** The reviewers
  found that it executed the script *before* checking a record could be written (so the
  documented "fails rather than running unrecorded" was false — the script had already run),
  and that a wrong-typed `record` was coerced to `false`, reintroducing the silently-dropped
  audit request the option existed to prevent. Both were fixed, then the option itself was
  removed — see the section below.
- **`script check` leaked its stdin temporary** on three of four blocked paths, because
  `process::exit` runs no destructors and only the first path released the guard.
  `emit_blocked_check` now takes the guard **by value**, so the compiler enforces what a
  comment previously asked for — a fifth blocked path cannot silently regress it.
- **`ScriptCheck` gained `acknowledged_forbidden`**: `script check` accepts the flag and
  promises every acknowledgement is recorded, but only `script run` delivered on it.
- **`max_bytes` declared no `maximum`** while the code enforces 128 MiB, so a client
  generating requests from the advertised schema could be rejected for a schema-valid one.
- **Two stale claims corrected**: "two tools" in the CLI help and the unknown-tool
  diagnostic, and "MCP writes nothing into the repository" in README, AGENTS.md, the spec
  and this manifest, which the `record` option had made untrue. Removing that option made
  the unqualified claim true again, and it is stated without exception everywhere.

**One finding adjudicated invalid.** The cross-check reviewer flagged clap's exit-2 path
emitting no JSON as a contract break. It is a deliberate, documented exception (invalid usage
is rejected before any aikit code runs) already pinned by a test, so it was recorded
`reported-invalid` rather than applied.

**Two findings were held back from this pass as behaviour changes rather than corrections,
then decided separately.** Both are now settled and reflected in the sections below: MCP
records were unprunable because MCP consults no configuration, which was resolved by
removing the record entirely; and a `--cwd` directory that cannot be entered now emits an
`aikit.error` document under `--json` while keeping exit 2.

## Test coverage: cancellation is now verified end-to-end

Cancellation was implemented (the request's `CancellationToken` reaches the executor, which
kills the process tree and returns nothing) but only *asserted in help text*. Timeout was
already covered by two tests; cancellation had none.

- **Both halves of the contract are now observed from outside the process.** The script
  writes a marker, waits, then writes a second marker: a killed script leaves the first and
  never writes the second. The test asserts the second marker is absent (the tree died) and
  that the cancelled request id is never answered (the protocol forbids responding, which is
  also why `stop_reason` has no `cancelled` variant to report).
- **The timeout is set far above the script's runtime**, so the timeout path cannot be what
  stops it — otherwise the test would silently re-test timeout instead of cancellation.
- **The "no response" assertion is ordered, not raced.** The test waits past the point where
  an un-cancelled script would have finished and queued its response, then issues a fresh
  request and checks which ids arrived ahead of it.
- **The test was verified to fail.** Removing only the cancellation notification makes it
  fail with "a cancelled request must receive no response, but id 2 was answered" — so it
  discriminates rather than passing vacuously.
- Passes on both platforms, which means it exercises the Unix process-group kill and the
  Windows Job Object path.

## Correctness pass: stale help, stale output, and the documentation gaps

Found by auditing the shipped surface against the code rather than against memory.

- **`scan secrets --help` still advertised `--fail-on-findings`.** The outer `scan` group's
  help had been updated when the flag was replaced; the inner `secrets` subcommand's had
  not — and that is the one a user reads. It documented a flag that no longer parses.
- **`aikit batch diff` printed the header `aikit diff anchor:`**, naming a command that no
  longer exists.
- **A stdin script now gets the same treatment as any file aikit generates.** `script run -`
  materializes the script itself, so the rule "where aikit generates the file it applies its
  own rules" should hold there too. It now writes a UTF-8 BOM for Windows PowerShell 5.1
  (never for cmd) and appends the exit-code epilogue — making a stdin script the one
  `script run` case where a PowerShell or cmd exit code is `epilogue` rather than
  `unpropagated`. Verified on Windows: the installed binary reports `source=epilogue`.
- **Documentation gaps closed.** Run-record retention was undocumented in every
  user-facing doc despite deleting run directories by default; `aikit config show` and the
  global `--cwd` were absent from the spec and `agent-usage.md`. Added, plus spec §5.13
  (`config show`) and §5.14 (`--cwd`).
- **Remaining references to removed shapes were swept** from source doc comments, test
  comments, and current-state prose in the manifest and plan. What remains is only
  deliberate "was previously X" explanation of a rename.

**Windows install verified.** `cargo install --path . --locked --force` on the Windows ARM64
target, then checked against the *installed* binary rather than the build tree: the new
surface answers (`config show`, `batch diff`, `agents-md`, `--fail-on`, `--require-root`,
stdin scripts), the removed surface exits 2 (`diff anchor`, `--fail-on-findings`), and the
help no longer advertises the replaced flag.

## Command shape: the MCP server writes nothing, without exception

`aikit mcp` exposes `run`, `list_runners` and `agents_md`, and none of them writes into a
repository. There is no opt-in audit record; auditability is a `script run` concern, where
the repository the record belongs to is unambiguous.

An opt-in `record` parameter existed briefly and was removed before publication. The
reasoning that removed it is worth keeping, because it generalises:

- **Retention has no correct scope on this surface.** One MCP server serves many folders —
  repo and non-repo — in a single IDE workspace, and the workspace's folder set changes
  mid-session. A session-wide retention limit therefore cannot describe the directories it
  governs, and a per-repo one re-introduces configuration to a surface that deliberately
  consults none. Every available answer was wrong in a different way.
- **The pressure to add it came from symmetry, not need.** "Auditability should not depend
  on transport" is a tidy principle, but the surface that needs an audit trail is the one
  that runs a file *in* a repository, and that surface already writes one. MCP runs a script
  that exists only in the call.
- **Removing it restores an unqualified invariant.** "Nothing is written into your
  repository, and no repository is required" is now true without a footnote — which is what
  makes it worth anything to a client deciding whether to approve the server.

The pre-publication window is what made this cheap: the option had never shipped, so removal
cost one revert instead of a deprecation.

## Command shape: the anchor lifecycle lives under `batch`

`aikit diff anchor` became `aikit batch diff`, and the top-level `diff` namespace is
removed. Every anchor reference — `batch show`, `batch diff`, and now `batch changed
--anchor` — accepts an anchor **id or a repo-relative path**, resolved by one shared
resolver.

- **A namespace should describe what it contains.** `diff` held exactly one subcommand,
  which operated on a batch anchor. The name advertised a general diff facility aikit does
  not have and has no plans to grow, while hiding an anchor operation away from the anchor
  commands.
- **Done now because it is breaking.** aikit is pre-1.0 and not yet public, which is the
  only period in which this costs a note rather than a migration. Deferring would have made
  it permanently more expensive to fix, for no benefit.
- **Ids and paths both work everywhere.** `batch changed --anchor` previously required a
  path while `batch diff` accepted either, which made the storage layout part of the
  interface for half the anchor commands: a caller who moved the output root, or who only
  ever saw the id printed by `batch start`, could use one and not the other.
- **One behavioural change, deliberately kept.** An out-of-repo anchor path given to
  `batch changed` now reports `blocked_path_escape` rather than `blocked_invalid_anchor`,
  because containment is now checked before the anchor is parsed — the same answer
  `batch diff` already gave for the same input. The ownership check still exists for the
  case containment cannot catch (an anchor sitting inside this repo that records another
  repo's root), and has its own test so consolidating the resolver could not silently drop
  it.

## Enhancement: bounded run records, `config show`, and wrapper-friendly invocation

- **Retention at write time.** `output.retain_runs` (default 100, `0` disables) keeps the
  newest N run directories, pruned oldest-first. `output clean` already existed but is a
  command a human has to remember, and per-invocation directories accumulate at agent speed
  — this repository reached 288 of them, 22 MB, in a day. Only `runs/` is pruned; batches,
  reviews and inventories are created deliberately. Never silent: `pruned_runs` appears in
  `run.json` and in the human output, because a missing earlier run must be explicable as a
  deliberate deletion rather than a record that was never written. Pruning runs *before* the
  script and excludes the run just created, so a run can never be its own victim, and a
  failed prune is not an error — losing an audit record to protect disk space is the wrong
  trade. Ordering uses the run id, not mtime: ids start with a fixed-width UTC timestamp, so
  lexical order is chronological and a sync or restore rewriting mtimes cannot reorder them.
- **Atomic `run.json`.** Written to a sibling temporary, synced, then renamed. A reader
  cannot distinguish a truncated record from a run that recorded less, so a torn write would
  read as corruption rather than as an unfinished run. Two parts of that finding were already
  satisfied and left alone: collision-resistant ids (`unique_run_dir` retries with a numeric
  suffix) and the privacy boundary (no environment values are recorded at all).
- **`aikit config show`.** Every effective setting with the source that set it — `default`,
  or the repo-relative file that last won. Provenance is recorded per *assignment*, not per
  file, so a later file that sets one key does not become the source of the others. The
  command states plainly that the MCP server does not consult configuration at all, since
  that split is the one most likely to be misread.
- **`--cwd <PATH>`,** global. Shell navigation is where cross-platform wrappers break —
  quoting, spaces, drive-relative paths and `cd` semantics all differ — and a wrapper that
  gets it wrong fails in a way that looks like an aikit error. Applied before dispatch rather
  than threaded through each command, because every command resolves its root from the
  working directory. A directory that cannot be entered is exit 2, not a blocked state:
  nothing about a repository has been examined yet, and an unusable argument *value* is the
  same class of error as `--fail-on bogus`, which the parser already rejects with 2. It is
  nonetheless the one exit-2 path that emits a record under `--json`, because clap's
  exception rests on nothing of aikit's having run yet — and here aikit is running, so the
  reason to stay silent does not apply. `print_error_json` is shared with `AikitError` so
  the two emitters cannot drift into different shapes.
- **`script run -` / `script check -`** read the script from stdin. This spares a caller
  *authoring* a file — for an agent, a second tool call and a second approval for something
  never meant to persist — without exempting it from the rules: aikit writes the script into
  `.aikit/temp/` itself, so the allowed-location policy, the forbidden scan and the run
  record all apply exactly as they do to a caller-authored script. `--runner` is required,
  because there is no filename to infer from and guessing is the substitution aikit refuses
  to make everywhere else. The temporary is removed on every exit path via a guard, plus an
  explicit drop before `process::exit`, which skips `Drop`.

## Enhancement: reporting what aikit does not know

Three places where a result stated more than aikit could actually guarantee. Each reports
the uncertainty instead of asserting past it.

- **`exit_code_source` — how far `exit_code` can be trusted.** `0` is not equally meaningful
  across runners: a POSIX shell, python or node exits with the last command's status, but
  PowerShell exits 0 when the last *native* command failed. Where aikit generates the script
  it appends a propagation (`epilogue`); where it runs a file the caller owns — every
  `script run` — it cannot (`unpropagated`), and `0` there means "it ran", not "it
  succeeded". Reported on both surfaces, and on `--print` too, so a caller learns before the
  run whether the code it plans to gate on will mean anything. `interpreter` |
  `epilogue` | `unpropagated`, derived from `runner_exit_epilogue` so the field and the
  behaviour cannot disagree.
- **`containment` — the mechanism, not a promise.** "Kills the process tree" is not
  unconditionally true, and a caller who believes it will not go looking for the orphan that
  outlived a timeout. `job_object` (Windows) means descendants cannot be spawned outside the
  job, though explicit breakaway escapes; `process_group` (Unix) means the group is
  signalled, though a descendant calling `setsid`/`setpgid` survives. Establishing the
  mechanism is a precondition for running at all — a failed `setpgid` fails the spawn, a
  failed job assignment is `setup_failed` — so a call that executes always has one in force.
- **The WSL exclusion now covers the Store alias.** `is_wsl_launcher` matched only
  `System32`, but a Windows ARM64 host had `WindowsApps\bash.EXE` as the *only* `bash` on
  PATH — the same launcher, reached through a Store App Execution Alias, which would run a
  `.sh` inside a Linux distribution rather than on Windows. The check now also matches a
  `WindowsApps` parent directory, and stays narrow by still applying only to the two
  launcher names, so Git for Windows' `bash.exe` resolves normally.

Also fixes a schema violation introduced by the runner-introspection change: `list_runners`
declares `additionalProperties: false`, so the added `path`/`version`/`reason` fields made
every result invalid against the server's own advertised `outputSchema` — accepted by
lenient clients, rejected by strict ones. A new test validates each tool's
`structuredContent` against its declared schema (required fields present, no undeclared
fields when the schema is closed), which is the general form of that mistake rather than a
patch for the one instance.

## Enhancement: machine-readable failure output

`--json` now yields exactly one JSON document on stdout whether a command succeeds or fails,
and the blocked states are published as a closed set. Specified in
`docs/aikit-cli-spec.md` §7.

- **The gap this closes.** Six of nine sampled failure paths wrote *nothing* to stdout under
  `--json`, leaving a machine caller to scrape prose off stderr precisely when it most
  needed structure — the one situation where guessing is most expensive. Measured before
  and after rather than assumed.
- **One document, always.** `main` captures `Cli::wants_json()` before dispatch, because a
  command that fails never gets to print anything and the error path would otherwise not
  know which channel the caller asked for. Prose still goes to stderr in both modes, so
  stdout stays parseable without stripping text out of it.
- **Never two documents.** `scan secrets` (when `--fail-on` trips) and `script check` (when
  policy rejects) report a full record *and* exit non-zero: the record is the answer, the
  exit is only the gate. `AikitError::blocked_after_json` marks those so no second object is
  appended — two JSON documents on stdout are unparseable as one. `script check` was already
  safe, exiting via `process::exit` rather than returning an error.
- **Opt-in.** Without `--json` a failure writes nothing to stdout, so piping a human-mode
  command never yields JSON that was not requested.
- **`blocked_state` is the field to branch on**: a named state means "change something and
  retry", `null` means the environment failed. `message` carries the detail *without* the
  `state:` prefix the stderr line uses, since the state is already its own field.
- **Exit code 2 stays clap's, and its silence is clap's too.** Invalid usage is rejected
  before any aikit code runs, so no record is emitted — pinned by a test, so a later change
  cannot start emitting a record for a command that was never dispatched. The one exit-2
  path aikit owns, a `--cwd` directory it cannot enter, does emit a record: the silence
  follows from nothing having run, not from the exit code, and there aikit is running.
  `errors::print_error_json` is a standalone function precisely so that emitter and
  `AikitError::report_json` cannot produce different shapes for the same document.
- **Pinned by `tests/cli_json_errors.rs`**, which asserts the document count on stdout
  rather than only its content — a second document is the failure mode that no amount of
  correct fields protects a caller from.
- **The blocked set is closed.** `errors::blocked::ALL` publishes it and a test fails the
  build if a declared constant is missing from it, so the docs cannot silently fall behind
  the code. Writing it down immediately caught `blocked_unsupported_mode`, documented in
  `agent-usage.md` but never defined in the source.

## Enhancement: structured runner introspection

Runner records on both surfaces (`aikit doctor` and the MCP `list_runners` tool) carry
`path`, `version` and `reason` alongside `name`, `available` and `applicable`.

- **`available` alone is not actionable.** Two hosts both reporting `powershell: available`
  may be running 5.1 and 7, which differ in output encoding and exit-code behaviour — the
  distinction that produced the exit-code-masking defect. `path` makes the report
  reproducible and `version` makes the two distinguishable without running anything.
- **Both surfaces, same shape.** `list_runners` existed only over MCP while the larger
  consumer class drives the CLI; a client reaching aikit through a shell should not get a
  thinner answer than one reaching it over MCP.
- **`reason` is a closed token set**, so a caller can act without parsing prose:
  `available`, `available_via_alias`, `not_found_on_path`, `not_applicable_on_this_os`.
  `available_via_alias` reports that the only thing on PATH is a Windows App Execution
  Alias. That is deliberately not called broken — a Store-installed PowerShell 7 resolves
  this way and answers `7.6.4` — but it is also how Windows ships a `python3.exe` with no
  Python behind it, which opens the Store and exits 9009. Plain `available` would state more
  than aikit knows.
- **Version probing uses each interpreter's own idiom**, not a guessed universal flag:
  `$PSVersionTable.PSVersion` for PowerShell (5.1 predates `--version`), `ver` for `cmd`,
  `--version` elsewhere. A guessed flag would return an error message in the version field.
  Probing is best-effort and skipped for unavailable runners, so it spawns at most one
  short-lived process per installed interpreter; `null` means "not determined", never "old"
  or "broken" (a POSIX `sh` that is really `dash` rejects `--version`).

## Enhancement: `scan secrets` severity and `--fail-on <severity>`

Replaces the boolean `--fail-on-findings` gate with a severity threshold, and expands the
rule set. Specified in `docs/aikit-cli-spec.md` §5.9.

- **Severity means confidence that the match is a real credential — never blast radius.**
  Blast radius is a property of the credential's privileges, which a local text scan cannot
  observe; confidence is a property of the match, which it can. Publishing the wrong
  definition would invite callers to treat `low` as "minor leak" rather than "probably a
  placeholder".
- **Monotonic, so `--fail-on X` means "X or above".** `--fail-on low` therefore reproduces
  the old `--fail-on-findings` behaviour exactly, and no capability is lost in the swap.
- **Format rules over a name corpus.** Nine rules match self-identifying credential formats
  — text whose own bytes announce what it is. Rule ids are vendor-neutral
  (`vcs_host_token`, not a brand) while the patterns are necessarily vendor-specific. This
  is deliberately **not** a comprehensive corpus: aikit is a pre-share signal, not a
  replacement for gitleaks or trufflehog, and precision is what makes a `high` finding worth
  gating on.
- **`low` earns its meaning from the path, and only for the name-based rule.**
  `is_example_path()` demotes `password = hunter2` in `config.example`, because that is
  almost certainly placeholder text. A real private key under `testdata/` keeps `high` — a
  leaked key is leaked wherever it sits.
- **Redaction is unchanged and is what makes the report quotable.** Findings carry path,
  line, rule id, description and severity; the matched value is never emitted, so the report
  can be pasted into a transcript or an issue verbatim.
- **Record fields:** `fail_on` (the chosen threshold, null when the flag is absent) and
  per-severity `counts`, so a caller can see both what was found and what was gated on.

### `scan secrets` expected committed files

| Path | Classification | Purpose |
|---|---|---|
| `src/scan.rs` | modified | `SEVERITY_*` consts + semantics doc; `FormatRule` table; `scan_line`; `is_example_path`; severity-threshold gate |
| `src/cli.rs` | modified | `Severity` ValueEnum; `--fail-on <severity>` replacing `--fail-on-findings` |
| `src/formats.rs` | modified | `fail_on: Option<String>` on `ScanSecrets` |
| `tests/cli_scan.rs` | modified | threshold behavior per severity; example-path demotion applies to the name-based rule only; format matches are `high` |
| `docs/aikit-cli-spec.md` | modified | §5.9 rule kinds, severity definition, gate |
| `docs/agent-usage.md` | modified | severity bullet; exit behavior; example |
| `docs/agent-integration-examples.md` | modified | basis command; blocked-state condition; safety boundary |
| `README.md` | modified | Secret scan section rewritten with severity table and scanner pointers |

