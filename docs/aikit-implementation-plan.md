# aikit Implementation Plan

## 1. Purpose

`aikit` is a personal compiled CLI for deterministic AI-agent workflow support. It exists to replace repeated one-off project scripts with one local Rust binary that can be called by an AI agent or by the architect directly.

The tool should automate mechanical local project tasks:

- create a batch/change anchor before AI-agent work begins;
- report files created or modified since an anchor;
- generate bounded review bundles for AI/human review;
- produce a mechanical repository inventory;
- run local scripts under explicit safety and audit controls.

`aikit` is not an autonomous agent, not a methodology validator, and not a governance judge. It should provide reliable inputs, outputs, and execution boundaries. Humans and AI agents interpret the results.

## 2. Current Repo State

Expected current repository:

```text
/Users/timothybutterfield/repos/github.com/Tim-Butterfield/aikit
```

Current durable files are expected to include:

```text
README.md
docs/aikit-cli-spec.md
docs/decisions/0001-create-aikit.md
.gitignore
```

Current local-only content may include:

```text
.scratch/README.md
```

Current known commits:

```text
a0ea5d4  Create initial aikit CLI specification
11d227b  Document lightweight development posture in README
```

The GitHub remote exists and is private, but the repository should not be pushed until the architect explicitly approves a push.

## 3. Development Posture

`aikit` is a small personal CLI. It should not follow the full Architect Toolkit / IDesign methodology.

Use lightweight engineering discipline:

- simple, reversible, normal Git workflow;
- clear README and lightweight spec;
- incremental but not tiny batches;
- ordinary commits after useful completed slices;
- tests for risky behavior;
- minimal ceremony;
- practical command behavior over formal completeness.

Do not use:

- phase gates;
- formal methodology artifacts;
- consistency review loops;
- change-request workflows;
- architecture packages;
- elaborate governance machinery.

The tool itself should still be strict where it matters: path safety, deterministic output, script execution boundaries, exit-code behavior, and refusal of dangerous operations.

## 4. Technology Decision

### 4.1 Primary Technology

Use Rust.

Reasons:

- one compiled binary;
- no runtime dependency on shell, Python, or Node for core behavior;
- strong type system and compiler pressure;
- good fit for path safety, hashing, filesystem traversal, structured errors, and deterministic CLI behavior;
- fast local execution;
- good testing ecosystem;
- acceptable personal-maintenance cost.

### 4.2 CLI Style

Use one binary named:

```text
aikit
```

Use subcommands instead of separate executables.

Initial command families:

```text
aikit batch start
aikit batch changed
aikit review generate
aikit inventory repo
aikit run script
```

### 4.3 Recommended Crates

Initial runtime dependencies:

```text
clap
serde
serde_json
thiserror
anyhow
camino
ignore
sha2
time
```

Initial dev dependencies:

```text
assert_cmd
predicates
tempfile
insta
```

Notes:

- `clap` provides subcommand parsing.
- `serde` / `serde_json` provide structured output and anchor formats.
- `thiserror` provides typed error definitions.
- `anyhow` can be used at command boundary layers for ergonomic error propagation.
- `camino` provides UTF-8 path handling, useful for CLI/report output.
- `ignore` provides `.gitignore`-aware walking and can support repo inventory/review generation better than raw `walkdir`.
- `sha2` supports SHA-256 file hashing.
- `time` supports stable timestamp handling without needing a heavier date/time stack.
- `assert_cmd`, `predicates`, and `tempfile` support CLI integration tests.
- `insta` may be useful for stable output snapshot tests, but should be used sparingly. It is optional and not required for Batch 1; add it only when a snapshot test earns its keep.

### 4.4 Cargo.lock Policy

The current `.gitignore` may ignore `Cargo.lock` because the repo was created before implementation policy was decided.

For this project, once Rust scaffolding begins, `Cargo.lock` should be committed because `aikit` is a binary application.

Implementation batch 1 should update `.gitignore` accordingly.

## 5. Non-Goals

Do not implement these in the initial product:

- AI provider calls;
- model routing;
- provider/model fallback;
- remote execution;
- SSH or file-RPC execution;
- methodology validation;
- governance validation;
- gate advancement;
- approval/rejection decisions;
- recurring scratch validation as a core command;
- package manager orchestration;
- public release automation;
- any AI-agent skills/commands as the durable implementation;
- copied shell/Python/Node scripts as durable implementation.

Historical scripts may inform behavior, but `aikit` should be implemented in Rust as a fresh tool.

## 6. High-Level Architecture

Recommended crate structure:

```text
Cargo.toml
Cargo.lock
src/
  main.rs
  cli.rs
  commands/
    mod.rs
    batch.rs
    review.rs
    inventory.rs
    run.rs
  core/
    mod.rs
    repo.rs
    paths.rs
    git.rs
    hashing.rs
    output.rs
    blocked.rs
    time.rs
  formats/
    mod.rs
    anchor.rs
    review.rs
    inventory.rs
    run_record.rs
  policy/
    mod.rs
    script.rs
    git_ops.rs
  fs/
    mod.rs
    walk.rs
    read.rs
    changed.rs
  errors.rs

tests/
  cli_batch.rs
  cli_inventory.rs
  cli_review.rs
  cli_run_script.rs
  fixtures/
```

This is a starting shape, not a hard architectural requirement. Keep modules practical.

Treat the tree above as an eventual map, not a Batch 1 deliverable. Do not create empty module stubs ahead of behavior. Start with a small, mostly flat module set (enough for `batch start` / `batch changed`) and split a module out only when a file actually grows. Premature structure works against the lightweight posture.

### 6.1 Design Boundaries

- `cli.rs` defines command structure and arguments.
- `commands/*` implement command orchestration.
- `core/*` contains reusable repo/path/git/output helpers.
- `formats/*` contains serializable data structures.
- `policy/*` contains deterministic blocking rules.
- `fs/*` contains file walking, reading, hashing, and changed-file mechanics.
- `errors.rs` defines typed failures and blocked states.

### 6.2 Command Output Policy

Every command should support readable terminal output.

Commands that produce structured results should support JSON:

```text
--json
```

Do not overbuild output formats initially. Support human output and JSON only.

### 6.3 Exit Code Policy

Recommended initial exit codes:

| Exit Code | Meaning |
|---:|---|
| 0 | success |
| 1 | ordinary `aikit` command failure |
| 2 | invalid usage / CLI parse failure |
| 3 | blocked state |

There is intentionally **no** dedicated "script exit propagated" code. For `aikit run script` the design choice is:

- either propagate the script's actual exit code;
- or return a fixed `aikit` failure code and record the script exit code in metadata.

Recommended initial behavior (propagate):

- if `aikit` blocks before execution, exit with `3`;
- if a real `aikit` (non-script) failure occurs, exit with `1`;
- if the script runs, return the script's exit code directly — including any value (1, 2, 3, …); aikit does not reinterpret it;
- record the script exit code in `run.json`.

Because executed scripts return their own code unchanged, codes `1`–`3` from a run-script invocation can originate from the script itself; consumers distinguish an aikit pre-execution block from a script failure by reading `blocked_state` / `exit_code` in `run.json`. This mirrors normal shell expectations and makes `aikit run script` useful in automation.

## 7. Repo Detection

Most commands require a repository root.

Initial repo-root detection:

```text
git rev-parse --show-toplevel
```

Use Git as the primary repo-root authority. Avoid inventing a parallel project-root system initially.

If no Git repo is found, commands that require a repo should fail with:

```text
blocked_repo_not_found
```

Future option:

- allow non-Git folder mode for inventory only.

Do not implement non-Git mode initially.

## 8. Output Location Convention

The default output root is always:

```text
.aikit/outputs/
```

under the detected repo root. `.scratch` is **never** auto-selected and `.scratch`
directories are never auto-created. `.scratch` may be used only when the caller
explicitly passes `--output`.

Command-family output folders (default):

```text
.aikit/outputs/batches/
.aikit/outputs/inventory/
.aikit/outputs/reviews/
.aikit/outputs/runs/
```

(`runs/` is reserved for the future `aikit run script`; it is not implemented yet.)

Output-root selection rule (deterministic):

- If `--output <dir>` is given, that directory is the output root: aikit creates the
  same per-command subfolders (`batches/`, `inventory/`, `reviews/`, `runs/`) under it.
  Passing `--output .scratch/work/outputs/aikit` is how a project opts into `.scratch`.
- If `--output` is omitted, the output root is always `.aikit/outputs/` — regardless of
  whether `.scratch/work/outputs/` happens to exist.
- aikit never auto-creates `.scratch/`, `.scratch/work/`, or `.scratch/work/outputs/`.

Reporting and ignore rules:

- `.aikit/outputs/` is local output and should not be committed.
- Commands that create files print the exact created artifact paths in human output,
  and include them (machine-readable) in JSON output (e.g. an `anchor_path` or a
  `written` array).
- Anchor files are durable output artifacts and are not auto-cleaned.
- Do not automatically modify Git ignore files; create output directories only when
  needed.

Open decision (unchanged):

- whether `.aikit/` should be auto-added to `.git/info/exclude` or only documented.
  Initial recommendation: document it; do not modify Git ignore files automatically.

## 9. Data Formats

All tool-owned data formats should include a version field.

Use JSON for generated metadata.

### 9.1 Batch Anchor Format

Recommended file name:

```text
.aikit/outputs/batches/<anchor-id>.json
```

Anchor ID format:

```text
YYYYMMDD-HHMMSS-<short-head>
```

Example:

```text
20260612-183015-11d227b.json
```

Recommended JSON shape:

```json
{
  "schema_version": 1,
  "kind": "aikit.batch_anchor",
  "anchor_id": "20260612-183015-11d227b",
  "created_at": "2026-06-12T18:30:15Z",
  "repo_root": "/Users/timothybutterfield/repos/github.com/Tim-Butterfield/aikit",
  "git_head": "11d227b...",
  "git_branch": "main",
  "git_status_porcelain": "",
  "filesystem_anchor_time": "2026-06-12T18:30:15Z"
}
```

Use UTC timestamps.

### 9.2 Changed Files Output Format

Recommended JSON shape:

```json
{
  "schema_version": 1,
  "kind": "aikit.batch_changed",
  "anchor": {
    "anchor_id": "20260612-183015-11d227b",
    "path": ".aikit/outputs/batches/20260612-183015-11d227b.json"
  },
  "repo_root": "/path/to/repo",
  "generated_at": "2026-06-12T18:45:00Z",
  "files": [
    {
      "path": "README.md",
      "status": "modified",
      "source": "git_status",
      "size_bytes": 1234,
      "sha256": "..."
    }
  ],
  "counts": {
    "total": 1,
    "created": 0,
    "modified": 1,
    "deleted": 0
  }
}
```

### 9.3 Review Bundle Format

Recommended default output:

```text
.aikit/outputs/reviews/<review-id>/run_for_review.txt
.aikit/outputs/reviews/<review-id>/manifest.json
```

The text bundle can keep the historically useful `run_for_review.txt` name initially because that is recognizable in existing workflows.

The command name should be modernized:

```text
aikit review generate
```

The output file may remain:

```text
run_for_review.txt
```

until a better name proves useful.

Recommended manifest shape:

```json
{
  "schema_version": 1,
  "kind": "aikit.review_bundle",
  "review_id": "20260612-184500-11d227b",
  "repo_root": "/path/to/repo",
  "git_head": "...",
  "generated_at": "2026-06-12T18:45:00Z",
  "inputs": {
    "mode": "changed_since_anchor",
    "anchor_path": "..."
  },
  "limits": {
    "max_file_bytes": 200000,
    "max_total_bytes": 2000000,
    "max_file_lines": 4000
  },
  "files": [
    {
      "path": "src/main.rs",
      "size_bytes": 1000,
      "sha256": "...",
      "included": true,
      "truncated": false,
      "lines_included": 80,
      "bytes_included": 1000,
      "omitted_reason": null,
      "cap_hit": null
    }
  ],
  "bundle_path": "run_for_review.txt",
  "totals": {
    "files_total": 1,
    "files_included": 1,
    "files_omitted": 0,
    "bytes_included": 1000
  }
}
```

Cap and ordering behavior must be deterministic:

- sort files lexicographically by repo-relative path **before** applying any caps, so cap decisions are reproducible;
- apply `--max-file-bytes` / `--max-file-lines` per file (set `truncated: true`, `cap_hit: "file_bytes"` or `"file_lines"`);
- apply `--max-total-bytes` across the sorted sequence; once the running total would be exceeded, later files are recorded with `included: false`, `omitted_reason: "max_total_bytes"`, `cap_hit: "total_bytes"` — they appear in the manifest but their content is not embedded in the bundle;
- every file in scope appears exactly once in `files[]` whether included, truncated, or omitted.

### 9.4 Inventory Format

Recommended default output:

```text
.aikit/outputs/inventory/<inventory-id>/inventory.json
.aikit/outputs/inventory/<inventory-id>/inventory.txt
```

Recommended JSON shape:

```json
{
  "schema_version": 1,
  "kind": "aikit.repo_inventory",
  "inventory_id": "20260612-184000-11d227b",
  "repo_root": "/path/to/repo",
  "git_head": "...",
  "generated_at": "2026-06-12T18:40:00Z",
  "files": [
    {
      "path": "README.md",
      "size_bytes": 1234,
      "sha256": "...",
      "kind_hint": "markdown"
    }
  ],
  "counts": {
    "files": 1,
    "bytes": 1234
  }
}
```

### 9.5 Run Record Format

Recommended default output:

```text
.aikit/outputs/runs/<run-id>/run.json
.aikit/outputs/runs/<run-id>/stdout.txt
.aikit/outputs/runs/<run-id>/stderr.txt
.aikit/outputs/runs/<run-id>/script.<ext>
```

The copied script retains its original extension (e.g. `script.zsh`) so it stays readable and re-runnable during an audit; do not rename it to a bare `script-copy`.

Recommended JSON shape:

```json
{
  "schema_version": 1,
  "kind": "aikit.script_run",
  "run_id": "20260612-185000-11d227b",
  "repo_root": "/path/to/repo",
  "script_path": ".scratch/work/temp/batch.zsh",
  "script_sha256": "...",
  "script_copy_path": "script.zsh",
  "interpreter": "/bin/zsh",
  "argv": ["/bin/zsh", "script.zsh"],
  "cwd": "/path/to/repo",
  "require_clean": false,
  "executed": true,
  "started_at": "2026-06-12T18:50:00Z",
  "ended_at": "2026-06-12T18:50:12Z",
  "duration_ms": 12000,
  "git_head_before": "...",
  "git_head_after": "...",
  "exit_code": 0,
  "blocked_state": null,
  "stdout_path": "stdout.txt",
  "stderr_path": "stderr.txt"
}
```

The `interpreter`, `argv`, `cwd`, `require_clean`, and `executed` fields make the record self-describing: an audit can reconstruct exactly what aikit ran, under which policy, and whether it actually executed. When `--print` is used (no execution), set `executed: false` and `exit_code: null`. Keep these as simple scalars/arrays — do not grow this into a heavier manifest system.

## 10. Blocked States

Initial blocked states:

| State | Meaning |
|---|---|
| `blocked_repo_not_found` | No Git repo root detected. |
| `blocked_path_escape` | Input path escapes repo or allowed root. |
| `blocked_script_not_allowed` | Script path is outside allowed execution locations. |
| `blocked_dirty_tree` | Command required a clean tracked tree and Git status was dirty. |
| `blocked_forbidden_git_operation` | Static script scan detected a forbidden Git operation. |
| `blocked_missing_anchor` | Command requires an anchor but none was provided/found. |
| `blocked_invalid_anchor` | Anchor file exists but cannot be parsed or is incompatible. |
| `blocked_unreadable_file` | Required input file cannot be read. |
| `blocked_unsupported_mode` | Requested mode is not supported. |
| `blocked_output_not_local` | Requested output path is outside the repo or local output area. |

Blocked states should be displayed clearly in human output and included in JSON output.

## 11. Command Specifications

## 11.1 `aikit batch start`

### Purpose

Create a batch anchor before AI-agent work begins.

### Initial CLI

```text
aikit batch start
```

Optional flags:

```text
--output <path>
--json
```

### Behavior

1. Detect Git repo root.
2. Read current HEAD.
3. Read current branch.
4. Capture `git status --porcelain=v1`.
5. Create output folder if needed.
6. Write anchor JSON.
7. Print anchor path.

### Human Output

Example:

```text
Batch anchor created:
  .aikit/outputs/batches/20260612-183015-11d227b.json
```

### Test Cases

- creates anchor under the default `.aikit/outputs/batches/`;
- default output stays under `.aikit/outputs/` even when `.scratch/work/outputs/` exists;
- `--output <dir>` writes under the requested directory (e.g. `.scratch/...`);
- JSON contains schema version, repo root, HEAD, branch, status, timestamp;
- fails outside Git repo;
- `--json` prints machine-readable output.

## 11.2 `aikit batch changed`

### Purpose

List files created or modified since a batch anchor.

### Initial CLI

```text
aikit batch changed --anchor <anchor.json>
```

Optional flags:

```text
--json
--tracked-only
--include-untracked
--hash
```

### Behavior

1. Detect Git repo root.
2. Read and validate anchor.
3. Compare current state to anchor.
4. Include changed tracked files from Git status.
5. Include untracked files when requested.
6. Include filesystem mtime comparison for files when appropriate.
7. Print deterministic sorted path list.
8. Optionally include JSON metadata.

### Initial Simplification

Start with Git status and mtime comparison. Do not attempt perfect historical reconstruction.

Signal sources and their roles (be explicit, because the tool is deterministic):

- **Git status is the primary signal.** Tracked files report `created` / `modified` / `deleted` from `git status --porcelain=v1` (current working-tree state relative to HEAD).
- **mtime is a supplementary heuristic, used only for untracked files** and only when `--include-untracked` is passed: a file whose mtime is newer than the anchor's `filesystem_anchor_time` is reported as `created`. mtime is **not** consulted for tracked files (Git status is authoritative there).

Stated limitations (report these, do not hide them):

- this is change *discovery*, not exact history reconstruction;
- deletions are detected only for tracked files (via Git status); untracked deletions are not detectable;
- renames are reported as a `deleted` + `created` pair, not as a rename;
- mtime is unreliable across checkouts/clones and can miss a changed-then-reverted file; treat the untracked-mtime list as best-effort.

The `status` enum is `created | modified | deleted`. When the result includes any mtime-derived entries, surface a short limitations note in human output (and a `notes` field in JSON) so the caller knows the list is heuristic.

Use deterministic behavior:

- normalize paths relative to repo root;
- sort paths lexicographically;
- exclude `.git/`;
- exclude default output directories unless explicitly requested.

### Test Cases

- detects modified tracked file;
- detects newly created untracked file when `--include-untracked` is set;
- excludes generated `aikit` output by default;
- fails on missing anchor;
- fails on anchor from another repo;
- JSON output is stable.

## 11.3 `aikit inventory repo`

### Purpose

Generate a mechanical inventory of repository files.

### Initial CLI

```text
aikit inventory repo
```

Optional flags:

```text
--output <path>
--json
--include-ignored
--max-files <n>
```

### Behavior

1. Detect Git repo root.
2. Walk repository using ignore-aware traversal.
3. Exclude `.git/` always.
4. Exclude common build/dependency folders by default.
5. Collect path, size, SHA-256, and kind hint.
6. Write JSON and text inventory.
7. Print output path.

### Default Excludes

```text
.git/
target/
node_modules/
dist/
build/
.venv/
venv/
.scratch/work/outputs/aikit/
.aikit/outputs/
```

Configure these in the `ignore` crate builder as **directory-only** globs (e.g. `**/target/`, `**/node_modules/`), not bare substrings, so a legitimately-named file such as `build` or `target` is not excluded accidentally.

### Test Cases

- inventories simple repo;
- excludes `.git/`;
- respects `.gitignore` by default;
- includes ignored files only with `--include-ignored`;
- computes SHA-256;
- produces deterministic ordering.

## 11.4 `aikit review generate`

### Purpose

Generate a bounded review bundle for AI/human review.

### Initial CLI

```text
aikit review generate --files <file>...
aikit review generate --anchor <anchor.json>
```

Choose only one initial mode for the first implementation batch.

Recommended initial mode:

```text
aikit review generate --files <file>...
```

Then add a single anchor-driven mode after `batch changed` is stable. Use **`--anchor <anchor.json>`** as the one spelling for "bundle the files changed since this anchor." Do not introduce a second `--changed` flag for the same behavior; a distinct `--changed <changed.json>` mode (consume a precomputed `batch changed` JSON) is **deferred** and only added later if a real need appears.

Optional flags:

```text
--output <path>
--max-file-bytes <n>
--max-total-bytes <n>
--max-file-lines <n>
--json
```

### Behavior

1. Detect Git repo root.
2. Resolve input files relative to repo root.
3. Reject path escapes.
4. Read file contents with caps.
5. Compute SHA-256 and size.
6. Produce `run_for_review.txt` and `manifest.json`.
7. Report truncation.

### Bundle Text Format

Use a simple, readable format (outer fence shown with four backticks so the nested
triple-backtick block renders correctly in this document):

````text
# aikit Review Bundle

Repo: <repo root>
HEAD: <head>
Generated: <timestamp>

## Files

### path/to/file
SHA-256: <hash>
Size: <bytes>
Truncated: false

```text
<file contents>
```
````

Implementation note:

When writing this Markdown format from Rust, nested code fences must not break the bundle. Use a deterministic fence-length rule: scan each file's content for the longest run of consecutive backticks, then wrap that file's content in a fence one backtick longer than that run (minimum three). This guarantees the file's own backticks can never prematurely close the wrapper fence. Cover this with a test using content that itself contains triple backticks.

### Test Cases

- generates bundle for explicit file;
- rejects file outside repo;
- rejects a symlinked input file whose real path escapes the repo;
- caps large file;
- reports truncation;
- handles file content containing triple backticks without breaking the bundle (fence-length rule);
- applies `--max-total-bytes` deterministically, omitting later files with `omitted_reason` recorded in the manifest;
- includes hashes and sizes;
- manifest matches text bundle;
- deterministic file order.

## 11.5 `aikit run script`

### Purpose

Execute a local script under mechanical safety controls and create an audit record.

### Initial CLI

```text
aikit run script <script-path>
```

Optional flags:

```text
--print
--require-clean
--allow-dirty
--output <path>
--json
```

### Allowed Initial Script Locations

Allow only scripts under:

```text
.scratch/work/temp/
.scratch/work/outputs/
.aikit/temp/
```

Do not initially allow arbitrary `scripts/` or `tools/` execution. That can be added later.

### Behavior

1. Detect Git repo root.
2. Resolve script path relative to repo root.
3. Reject absolute path escapes and `..` escapes.
4. Canonicalize the path (resolve symlinks) and confirm the **resolved** path is still inside the repo root and under an allowed location; reject if a symlink points outside (see Path Safety below).
5. Read script content.
6. Static-scan for forbidden operations.
7. Apply the clean-tree policy (see Clean-Tree Policy below).
8. Echo command.
9. If `--print`, do not execute; print script and policy summary.
10. Create run output folder.
11. Copy script into run folder (retaining its extension).
12. Execute through explicit interpreter.
13. Capture stdout/stderr.
14. Write `run.json`.
15. Exit with script exit code if executed.

### Path Safety

`..` and absolute-path rejection alone are insufficient: a file at an allowed relative path can be a symlink to somewhere outside the repo or outside the allowed script locations. Before reading, copying, or executing, canonicalize the script path (resolve all symlink components) and require the resolved real path to remain inside the repo root **and** under an allowed location. Reject otherwise with `blocked_path_escape` (or `blocked_script_not_allowed` when the resolved path leaves the allowlist). Apply the same canonicalize-then-check rule to `--output` directory components and to `review generate` input files. The simplest safe initial stance is to refuse symlinked script inputs outright.

### Clean-Tree Policy

`--require-clean` and `--allow-dirty` are mutually exclusive; supplying both is invalid usage (exit `2`).

- **Default (neither flag): allow-dirty.** These scripts operate on scratch/working content, so a dirty tree is normal; aikit does not block on it by default.
- `--require-clean` blocks with `blocked_dirty_tree` when the tracked tree is dirty before execution.
- Record the effective policy in `run.json` (`require_clean: true|false`) and in human output, so a dirty-by-default run is never mistaken for a clean-tree guarantee.

### Interpreters

Initial implementation may support:

```text
.zsh → /bin/zsh
.sh  → /bin/sh
```

Do not support Python/Node execution initially unless needed.

Initially, a **known extension is required**. aikit selects the interpreter from the extension and invokes that interpreter explicitly. Scripts with no extension, or an unrecognized extension, are rejected with `blocked_unsupported_mode` rather than being run via their shebang or native `+x` execution. This is deliberate: choosing the interpreter from a fixed map (instead of trusting an arbitrary shebang) keeps execution predictable and is part of why `run script` echoes exactly what it will run. Shebang-based or extra-extension support can be added later if a concrete need appears.

### Forbidden Operation Scan

Initial static scan should block clear textual matches:

```text
git push
git fetch
git pull
gh repo create
gh repo delete
rm -rf /
sudo
```

This is not a security sandbox. It is a mechanical policy guard.

The scan is intentionally crude: it is naive substring/line matching, so it is **trivially evaded** (aliases, variable expansion, unusual whitespace, encoding, indirection) and can also false-positive on the same strings appearing inside comments or quoted text. The **allowed-location allowlist is the primary control**; the static scan is only a secondary guard against obvious accidental mistakes.

Document clearly:

- scans are best-effort and easily bypassed;
- `aikit` reduces accidental unsafe execution, it does not prevent intentional unsafe execution;
- it does not make arbitrary scripts safe.

Surface the "not a security sandbox" warning where the user actually sees it — in `aikit run script --help` and in the command's human output / `--print` policy summary — not only in the README and this plan.

### Test Cases

- rejects script outside repo;
- rejects script outside allowed locations;
- rejects a symlinked script whose real path escapes the repo or the allowlist;
- rejects an extensionless or unknown-extension script;
- rejects `--require-clean` and `--allow-dirty` supplied together (invalid usage);
- defaults to allow-dirty when neither flag is given, and records the policy in `run.json`;
- supports `--print` without execution (and records `executed: false`);
- blocks forbidden Git operations;
- enforces `--require-clean`;
- captures stdout/stderr;
- propagates exit code;
- writes run record (including interpreter, argv, cwd);
- copies script into run folder retaining its extension.

## 12. Implementation Batches

The work should be executed in larger coherent batches, not tiny one-command loops, but still keep risky behavior isolated.

### Pre-Batch 1: Expected File Manifest

- Create `docs/implementation-manifest.md`.
- Define the exact files expected for Batch 1.
- Review the manifest for completeness and ambiguity before implementation.
- Do not create source files until the manifest is accepted.
- After implementation, compare actual files to the manifest before committing.

(Do not create the manifest as part of planning; create it when implementation begins, before any source files exist. See §18.)

### Batch 1: Rust Scaffold and Batch Commands

Scope:

- review/accept `docs/implementation-manifest.md` for Batch 1 before writing any source (see Pre-Batch 1 above);
- revise `.gitignore` so `Cargo.lock` is tracked;
- create a minimal Rust project — only the modules needed for `batch start` / `batch changed` (do not stub out the full §6 tree; grow modules as files actually warrant splitting);
- add initial dependencies;
- implement `aikit batch start`;
- implement `aikit batch changed` with anchor reading and simple changed-file detection;
- implement repo-root detection;
- implement output path selection;
- implement anchor JSON format;
- implement useful help text for `aikit --help`, `aikit batch --help`, `aikit batch start --help`, and `aikit batch changed --help` (purpose, when to use, inputs, important flags, default output locations, JSON behavior, examples — see §19);
- add tests for batch start/changed;
- include tests or captured checks that help text is available and contains the critical usage information;
- update README usage section;
- compare actual created files to `docs/implementation-manifest.md`, and explain or remove any unexpected files, before committing.

Commit:

```text
Implement batch anchor and changed-file commands
```

Do not push.

### Batch 2: Inventory Command

Scope:

- implement ignore-aware file walking;
- implement SHA-256 hashing;
- implement `aikit inventory repo`;
- output JSON and text inventory;
- add deterministic ordering;
- add tests;
- update/review `docs/implementation-manifest.md` for this batch before implementation and compare actual files to the manifest before committing.

Commit:

```text
Implement repository inventory command
```

Do not push.

### Batch 3: Review Bundle Generation

Scope:

- implement `aikit review generate --files`;
- produce `run_for_review.txt` and `manifest.json`;
- implement byte and line caps;
- implement truncation reporting;
- add path escape checks;
- add tests;
- update README usage;
- update/review `docs/implementation-manifest.md` for this batch before implementation and compare actual files to the manifest before committing.

Commit:

```text
Implement review bundle generation
```

Do not push.

### Batch 4: Review Generation from Batch Anchor

Scope:

- connect `aikit review generate` to batch changed output;
- support `--anchor <anchor.json>` mode for review generation from a batch anchor; keep any precomputed `--changed <changed.json>` mode deferred unless a real need appears;
- add tests;
- update/review `docs/implementation-manifest.md` for this batch before implementation and compare actual files to the manifest before committing.

Commit:

```text
Generate review bundles from batch anchors
```

Do not push.

### Batch 5: Governed Script Runner

Scope:

- implement `aikit run script`;
- support `--print`;
- support `--require-clean` / `--allow-dirty`;
- implement allowed script locations;
- implement forbidden operation scan;
- capture stdout/stderr;
- write run metadata;
- propagate script exit code;
- add tests for risky behavior;
- ensure `aikit run script --help` clearly states it is not a security sandbox (see §19);
- update README warning that this is not a security sandbox;
- update/review `docs/implementation-manifest.md` for this batch before implementation and compare actual files to the manifest before committing.

Commit:

```text
Implement governed script runner
```

Do not push.

### Batch 6: Local Integration and Polish

Scope:

- run full test suite;
- run formatter/linter;
- install locally with `cargo install --path .` or equivalent;
- test from a separate local repo;
- review all subcommand help text for completeness against §19 before considering commands done;
- update README examples;
- update spec to reflect implemented behavior;
- record remaining deferred items;
- update/review `docs/implementation-manifest.md` for this batch before implementation and compare actual files to the manifest before committing.

Commit:

```text
Polish local aikit workflow
```

Do not push unless explicitly authorized.

## 13. Test Strategy

Test risky and behavior-defining areas.

Prioritize tests for:

- repo-root detection;
- path escape rejection, including symlink escapes (resolved-path outside repo/allowlist);
- anchor creation;
- changed-file detection, including the untracked-only mtime rule and deletion reporting;
- deterministic inventory ordering;
- hashing;
- review bundle caps/truncation, nested-backtick fencing, and total-byte omission;
- script location allowlist;
- extension/interpreter selection (unknown/extensionless rejected);
- forbidden operation blocking;
- clean-tree requirement and the allow-dirty default / both-flags rejection;
- stdout/stderr capture;
- exit-code propagation.

Do not over-test trivial CLI plumbing.

Recommended test types:

- unit tests for pure helpers;
- integration tests using `assert_cmd` and `tempfile`;
- minimal snapshot tests for stable output if useful.

## 14. Documentation Updates During Implementation

Keep docs practical.

README should eventually include:

- short purpose;
- install/build instructions;
- command examples;
- warning that `run script` is not a security sandbox;
- note that `aikit` is personal/local-first;
- no public distribution promises.

Spec should eventually be updated to reflect actual behavior, not idealized future behavior.

Decision records should not multiply unless there is a meaningful fork in direction.

## 15. Deferred Items

Do not implement initially:

- plugin system;
- config file unless needed;
- AI-agent skill wrappers;
- GitHub Actions;
- release packaging;
- shell completions;
- remote execution;
- Windows-specific behavior;
- non-Git folder mode;
- automatic `.git/info/exclude` modification;
- semantic methodology/governance validation.

## 16. Risks and Mitigations

| Risk | Mitigation |
|---|---|
| CLI scope expands too quickly | Keep initial commands limited to included set. |
| `run script` is mistaken for a sandbox | Document clearly and block obvious unsafe operations. |
| Review bundle output becomes too complex | Start with explicit files and simple caps. |
| Batch changed logic is misleading | Test changed-file scenarios and report mode/limitations clearly. |
| Tool becomes Architect Toolkit-specific | Keep command names generic and methodology validation out of scope. |
| Too much ceremony returns | Use normal Git workflow and lightweight docs only. |

## 17. Agent-Agnostic Runtime Boundary

The implemented `aikit` binary must remain agent-agnostic.

- `aikit` must not know about any specific AI agent, AI-agent CLI, slash command,
  review skill, model, or provider.
- The binary should expose deterministic CLI behavior and useful help text.
- AI agents may choose to wrap or call `aikit`, but those wrappers are outside the
  binary and not part of this implementation.
- The CLI should not contain provider/model logic.
- The CLI should not contain agent-specific prompts, slash commands, or skill logic.

External review tools may be used during development to review the plan, source,
expected file manifest, help text, and implementation completeness. These tools are
development aids only; the `aikit` binary has no knowledge of them.

## 18. Implementation Manifest Requirement

Before implementation, create a reviewed expected-file manifest. It is a durable repo
document, expected at:

```text
docs/implementation-manifest.md
```

The manifest should:

- list every file expected to be created or modified for the implementation batch;
- include source files, tests, docs updates, and config/build files;
- classify each file as one of: `new`, `modified`, `generated`, `local-only`, `deferred`;
- state the purpose of each file.

After each implementation batch, compare expected files to actual files:

- expected and created;
- expected but missing;
- unexpected created files;
- expected modifications;
- unexpected modifications.

Unexpected files should be explained or removed before committing.

The manifest is not heavy methodology; it is a lightweight guard against file sprawl
and ambiguity.

## 19. CLI Help Completeness Requirement

Help text is part of the product.

- `aikit --help` and all subcommand help must be useful to humans and AI agents.
- Help must explain command purpose, when to use it, inputs, important flags, default
  output locations, JSON behavior, safety limits, and examples where useful.
- At minimum, implementation should review help for:

  ```text
  aikit --help
  aikit batch --help
  aikit batch start --help
  aikit batch changed --help
  aikit review --help
  aikit review generate --help
  aikit inventory --help
  aikit inventory repo --help
  aikit run --help
  aikit run script --help
  ```

- `aikit run script --help` must clearly say it is not a security sandbox.
- Help output should be reviewed before considering a command complete.
- The help should be sufficient for an AI agent to decide when and how to call the
  command effectively.

## 20. Optional Future Agent-Usage Documentation

- Future documentation may describe how AI agents can use `aikit`.
- This may include a document such as:

  ```text
  docs/agent-usage.md
  ```

- Such documentation is optional and deferred.
- It may help agents or agent skill authors wrap `aikit`.
- It must not make the `aikit` binary agent-specific.
- Do not create this document yet.

## 21. Implementation Readiness

- The plan has been reviewed and refined.
- It is ready to guide creation of the implementation manifest and then Batch 1.
- Batch 1 should not begin until the expected-file manifest is created and reviewed.
- Batch 1 remains focused on:
  - Rust scaffold;
  - `aikit batch start`;
  - `aikit batch changed`;
  - tests for batch behavior;
  - basic CLI help.
- No deferred commands should be implemented in Batch 1.

## 22. Post-Initial Command-Shape Correction and Approved Slices

The initial six batches are complete (see the implementation manifest). The sections
above (including §11.5) are retained as the historical record of the original design,
which spelled the script command as `aikit run script`. This section records an
approved post-initial correction and the approved direction for further slices.

### 22.1 Command-shape correction (Slice 1 — implemented)

- The script command family is corrected from the verb-first `aikit run script` to the
  noun-family / action form:
  - `aikit script run <script-path>` — run a script under policy controls (preserves the
    previous `aikit run script` behavior);
  - `aikit script check <script-path>` — validate a script against the same policy
    without executing it and without writing any run output.
- `aikit run script` (and the top-level `aikit run`) is **superseded and removed**. It
  is **not** retained as a compatibility alias — there is exactly one public way to run a
  script (`aikit script run`).
- Where earlier sections (e.g. §11.5) describe `aikit run script`, read them as the
  historical spec of the behavior now provided by `aikit script run`.

### 22.2 Approved post-initial slices (direction only)

The following slices are approved direction. **Only Slice 1 is implemented now**; the
rest are recorded so the command grammar stays coherent, and are not implemented in this
slice. No separate roadmap document is created — this section is the record.

- **Slice 1 (implemented):** `aikit script run`, `aikit script check`; remove
  `aikit run script`.
- **Slice 2 (approved, not implemented):** `aikit repo init`, `aikit repo doctor`.
- **Slice 3 (approved, not implemented):** `aikit output list`, `aikit output show`,
  `aikit output clean`.
- **Slice 4 (approved, not implemented):** `aikit batch list`, `aikit batch show`,
  `aikit diff anchor`.
- **Slice 5 (approved, not implemented):** `aikit env snapshot`, `aikit scan secrets`.

These follow the same noun-family / action grammar as Slice 1. Each future slice will be
implemented only under its own explicitly approved task, with the same checks / review /
manifest discipline used for the initial batches.
