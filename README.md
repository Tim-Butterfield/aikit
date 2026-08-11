# aikit

`aikit` is a local CLI for deterministic AI-assisted repository workflows.

It is a single compiled binary that performs **local, mechanical operations** —
anchoring a unit of work, discovering what changed, generating a bounded review
bundle, inventorying a repo, and running local scripts under explicit policy. It
calls **no AI providers**, performs no network operations as part of its core
behavior, and makes no autonomous decisions.

## Install

A Rust toolchain (via [`rustup`](https://rustup.rs)) is the only prerequisite, **Rust 1.88
or newer**. **`make` is not required** — every command below is plain `cargo`.

An older toolchain is refused up front by Cargo rather than failing partway through a build.
CI compiles against 1.88 on every change, so that floor is tested and not merely claimed.

Install the latest directly from GitHub — no clone required:

```sh
cargo install --git https://github.com/Tim-Butterfield/aikit --locked
```

This builds `aikit` and puts it on your PATH (`~/.cargo/bin`). Pin a version with
`--tag <tag>` or `--rev <sha>`.

Or install globally from a clone — the same result, and what you want if you also intend to
read or modify the source:

```sh
git clone https://github.com/Tim-Butterfield/aikit
cd aikit
cargo install --path . --locked
```

Verify it:

```sh
aikit --version
aikit doctor        # read-only readiness report; creates nothing
```

Notes:

- **`--locked` builds the dependency versions this repository committed and CI tested.**
  Without it, cargo resolves fresh versions at install time, so your binary may be built
  against dependencies no one has tested together. It does not modify your clone either way.
- **To upgrade, re-run the same command** — cargo replaces the installed binary in place;
  no `--force` and no uninstall step is needed.
- To build without installing: `cargo build --release`, then use `target/release/aikit`.
- `cargo uninstall aikit` removes it.

## Status

- Early, personal-use-oriented software — built primarily for the author's own
  workflows — and available as open source under the MIT license.
- License: MIT (see [`LICENSE`](LICENSE)).
- `publish = false` is set in `Cargo.toml`: `aikit` is distributed as source and
  installed from this repository (see [Install](#install)), not published to
  crates.io.
- What `aikit` is **not**: not an autonomous agent, not a security sandbox, not a
  methodology validator or governance judge, and not a provider/model router. It
  calls no AI providers. (See [Non-Goals](#non-goals).)
- Generated outputs under `.aikit/outputs/` are **local-only** and should not be
  committed.

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

## Available Commands

- `aikit init`
- `aikit init --require-repo`
- `aikit init --require-folder`
- `aikit doctor`
- `aikit doctor --require-root`
- `aikit batch start`
- `aikit batch changed`
- `aikit batch list`
- `aikit batch show`
- `aikit batch diff`
- `aikit inventory repo`
- `aikit review generate`
- `aikit script check`
- `aikit script run`
- `aikit mcp`
- `aikit output list`
- `aikit output show`
- `aikit output clean`
- `aikit env snapshot`
- `aikit scan secrets`
- `aikit config show`
- `aikit agents-md`
- `aikit version`

Every command accepts a global `--cwd <PATH>` to run as if aikit had been started in that
directory, so a wrapper never has to navigate the shell to reach the repository.

## Design

- Rust.
- One compiled binary named `aikit`.
- Subcommand-based.
- No runtime dependency on shell, Python, or Node for core behavior.

### Output contract

**A command run with `--json` emits exactly one JSON document on stdout — on success and on
failure alike.** Human-readable prose always goes to stderr, so stdout stays a channel a
program can parse without stripping text out of it first.

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | Success. |
| 1 | Ordinary failure (I/O, serialization, a subprocess that could not run). |
| 2 | Invalid usage — malformed arguments or an invalid argument value. Usually no record: the argument parser exits before any aikit code runs. Where aikit itself detects the invalid value (a `--cwd` directory it cannot enter), a record **is** emitted. |
| 3 | A named blocked state — a deterministic, mechanical refusal. |

On failure under `--json`, the document is `kind: aikit.error`:

```json
{
  "schema_version": 1,
  "kind": "aikit.error",
  "ok": false,
  "blocked_state": "blocked_missing_anchor",
  "exit_code": 3,
  "message": "anchor file not found or unreadable: missing.json"
}
```

`blocked_state` names one of a **closed set** of blocked states (listed in full in
[the CLI spec](docs/aikit-cli-spec.md)), or is `null` for an ordinary failure — so a caller
can tell "change something and retry" from "the environment failed" without reading the
message. The set is closed deliberately: match on it exhaustively and treat anything else as
a protocol violation. A test fails the build if the documented set and the code drift apart.

Two commands report their own record rather than an error document, because there the record
*is* the answer and the non-zero exit is only a gate: `scan secrets` when `--fail-on` trips,
and `script check` when policy rejects a script. Both carry `blocked_state` inside that
record. Without `--json`, a failure writes nothing to stdout.

## Usage

See [Install](#install) above to put the `aikit` binary on your PATH.

Run inside a Git repository. Mark an anchor before a unit of work, then list what
changed since:

```sh
# Create a batch anchor (writes JSON under the default output directory).
aikit batch start
# → Batch anchor created:
#     .aikit/outputs/batches/<anchor-id>.json

# After doing some work, list files modified since the anchor (timestamp-based).
aikit batch changed --anchor .aikit/outputs/batches/<anchor-id>.json

# Add a SHA-256 per file and machine-readable JSON:
aikit batch changed --anchor <anchor.json> --hash --json
```

Notes:

- The default output root is always `.aikit/outputs/`. `.scratch` is never
  auto-selected or auto-created; use it only by passing `--output .scratch/...`.
- Output under `.aikit/outputs/` is **local-only** and should not be committed.
- Commands that create files print the exact created paths (and include them in
  `--json` output), so you never have to infer file names.
- Anchor mode is **timestamp-based**: `batch changed`/`review generate` with `--anchor`
  report existing files whose filesystem mtime is newer than the anchor file. They do
  **not** use `git status`, so a file that is dirty vs `HEAD` but was last modified before
  the anchor is excluded; a file modified after the anchor is included whether tracked or
  not. Deleted files are out of scope (no content to bundle).
- Every command has detailed `--help`. `aikit` calls no AI providers and has no
  knowledge of any AI agent, CLI, or model.

### Repository setup

Recommended first-time setup — check, prepare, re-check:

```sh
aikit doctor        # read-only readiness report (works before anything is set up)
aikit init          # prepare local .aikit/temp/ and (in a repo) ignore coverage
aikit doctor        # confirm it is now ready
```

`aikit` works in **Git** repositories, **Mercurial** repositories, and **non-repo
folders**. Repository detection is filesystem-based (it looks for an enclosing
`.git`/`.hg`) and does **not** require the `git`/`hg` CLI to be installed.

- `aikit init` is the adaptive one-step setup: it creates `.aikit/` and `.aikit/temp/`,
  and **inside a repository** it also ensures `.aikit/` is locally ignored. Use
  `aikit init --require-repo` to require a repository (errors `blocked_repo_not_found` otherwise), or
  `aikit init --require-folder` to force non-repo treatment (errors `blocked_repo_present` inside a
  repo). All three are idempotent, create no output artifacts, and do not create
  `.scratch/` or `.claude/`.
- **Ignore coverage uses the VCS's local, never-committed mechanism**, so it never dirties
  tracked files: for **Git**, an entry in `.git/info/exclude`; for **Mercurial**, a
  `.hg/hgignore.aikit` pattern registered via `[ui] ignore.aikit` in `.hg/hgrc` (both under
  `.hg/`, never committed or cloned). Tracked ignore files (`.gitignore` / `.hgignore`) are
  never modified. In a non-repo folder no ignore coverage is added.
- `aikit doctor` is **read-only**: it reports the repo root, the `vcs`
  (`git`/`mercurial`/`none`), branch/HEAD, tracked clean/dirty state, whether `.aikit/`,
  `.aikit/temp/`, and `.aikit/outputs/` exist, whether `.aikit/` is ignored (and the
  source), the default output root, allowed script input locations, the aikit version, any
  warnings, and an overall `ready` summary. For Mercurial it detects ignore coverage
  without invoking `hg` and reads branch/HEAD via `hg` (with `HGPLAIN=1`) when available,
  degrading to empty with a warning otherwise. It creates and modifies nothing.
- **`aikit doctor` works before setup.** In a directory with no `.git`, `.hg`, or `.aikit`
  it reports `ready: false` with the remediation and exits 0, marking the report
  `root_source: "cwd_no_marker"` so the current directory is not mistaken for a root. Use
  `aikit doctor --require-root` when you want the absence of a root to be an error
  (`blocked_repo_not_found`) instead; the two share one implementation and neither is
  deprecated.
- **Runner readiness:** `doctor --require-root` reports availability for every supported script
  runner (`sh`, `bash`, `zsh`, `pwsh`, `powershell`, `cmd`, `python3`, `python`, `node`),
  each with `available` and `applicable` (OS-applicability) flags, plus
  `any_runner_available`. Readiness means local aikit state is sane **and at least one
  supported runner is available for the current OS** — it does **not** require any
  specific Unix shell (so Windows is ready with `pwsh`/`cmd`, and a host without `zsh` is
  still ready). The legacy `interpreters` field (`/bin/sh`, `/bin/zsh`) is retained as
  informational only and no longer gates readiness.
- All support `--json`. (If your repo already ignores `.aikit/` via a tracked
  `.gitignore`/`.hgignore`, `init --require-repo`/`init` reports that and adds no local entry.)

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

# Files modified since a batch anchor (timestamp-based; same as `batch changed`):
aikit review generate --anchor .aikit/outputs/batches/<anchor-id>.json
```

- Exactly one input mode is used per run: `--files <file>...` or
  `--anchor <anchor.json>`. Supplying both, or neither, is invalid usage. The
  precomputed `--changed <changed.json>` mode is **not implemented**.
- Anchor mode is **timestamp-based**: it bundles existing files whose filesystem mtime is
  newer than the anchor file, and excludes everything else. It does **not** use
  `git status`, so a file that is merely dirty vs `HEAD` but was last modified before the
  anchor is excluded; a file modified after the anchor is included whether tracked,
  untracked, staged, or unstaged. Deleted files are out of scope (no content to bundle).
  The anchor must exist, be a valid batch anchor, and belong to this repo
  (missing/invalid/cross-repo anchors are rejected). A clean Git tree is not required.
- Generates two files in a per-review directory:
  - `review_bundle.txt` — a readable bundle with per-file headings, SHA-256,
    size, truncation status, and fenced file contents. (Older local review outputs
    may use the previous name `run_for_review.txt`; new bundles use
    `review_bundle.txt`.)
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
  along with the anchor path and id, plus `inputs.enhanced_discovery` (whether the
  enhanced discovery below was used). Each included file records the timestamp-based
  detection `source` `anchor_mtime` (never `git_status`); configured `include_files` use
  `explicit`.

#### Single-file and embedded-manifest output

Defaults are unchanged (a per-review directory with `review_bundle.txt` +
`manifest.json`). New, opt-in output shapes:

- `--single-file` writes **exactly one** bundle file with the manifest embedded and no
  review directory and no sidecar `manifest.json`. The default path is
  `tmp/review_bundle.txt` (`tmp/` is conventionally git-ignored); override it with
  `--output <file>`. The bundle is `header` → `## Manifest` (fenced JSON) → `## Files`
  (contents). It fails clearly (`blocked_path_escape`, or an error if the path is a
  directory) when the requested path cannot be satisfied.
- `--embed-manifest` embeds the manifest in the bundle text **without** changing the
  directory layout (the sidecar `manifest.json` is still written unless suppressed).
- `--no-sidecar-manifest` suppresses the sidecar `manifest.json` (directory mode only;
  single-file mode never writes one).

#### Enhanced anchor discovery

By default, anchor mode bundles existing files modified after the anchor (timestamp
walk), honoring `.gitignore` so ignored files are skipped.
`--include-ignored-batch-files` (or config) additionally:

- bundles **allowlisted ignored** files modified after the anchor — matched by
  `include_globs` and not by the exclude globs.

(Untracked **non-ignored** files modified after the anchor are already included by the
default timestamp walk, regardless of this flag. Deleted files are out of scope.)

Exclusion always applies the protective default globs (`.git/**`,
`.aikit/outputs/{raw,provider,secrets}/**`, `node_modules/**`, `target/**`, `dist/**`,
`build/**`) plus any configured `exclude_globs`; paths that escape the repo are never
included (the walk does not follow symlinks and considers only regular files).

#### Configuration

Optional config files set defaults for the options above (CLI flags take precedence):

1. built-in defaults
2. `aikit.config.json` (repo root)
3. `.aikit/config.json`
4. CLI flags

A config file may set `bundle.{single_file,embed_manifest,sidecar_manifest,output}`,
`discovery.{include_ignored_batch_files,include_globs,exclude_globs,include_files}`, and
`script_runner.{preferred_runners,detect_from_shebang,detect_from_extension,extension_map}`
(see the script section for runner detection). A higher-precedence file overrides scalar
values; the protective exclude globs are always applied and can only be added to, never
removed. A `_comment` key is accepted (and ignored) anywhere. Malformed config (bad JSON
or an unknown field) fails clearly, and **unknown runner names** in
`script_runner.preferred_runners` or `script_runner.extension_map` are rejected with
`blocked_runner_not_allowed` when a script is run/checked (rather than being silently
skipped). See `aikit.config.example.json` for a generic, annotated example.

This config (a schema-less JSON shape, not a versioned schema) is distinct from the
package version (`aikit version`) and from the per-record `schema_version`.

```sh
# One self-contained bundle (manifest embedded) from a batch anchor, including
# allowlisted ignored artifacts modified since the anchor:
aikit review generate --anchor .aikit/outputs/batches/<anchor-id>.json \
  --single-file --include-ignored-batch-files --output tmp/review_bundle.txt
```

`batch start` also records the `aikit_version` that created the anchor, and
`batch start --snapshot` optionally records an initial snapshot of tracked files (never a
full repo content scan).

### Governed script command family

> **Not a security sandbox.** `aikit script run` reduces *accidental* unsafe
> execution; it does not make an arbitrary script safe. The allowed-location policy
> is the primary control, and the forbidden-operation scan is best-effort (naive
> substring matching, easily bypassed, can false-positive).

`aikit script run` runs a local script through its detected runner (see
cross-OS runner detection below) and records an audit trail; `aikit script check`
applies the same policy but never executes the script and writes no run output:

```sh
aikit script check .aikit/temp/build.sh           # validate policy only; nothing runs
aikit script run .aikit/temp/build.sh             # run; record the audit trail
aikit script run .scratch/work/temp/task.zsh --print   # validate + show plan, do not run
aikit script run .aikit/temp/build.sh --require-clean   # block if the tracked tree is dirty

# Supply the script on stdin instead of authoring a file (one call, nothing to clean up).
echo 'echo hi' | aikit script run - --runner sh
```

- **A script may be read from stdin** by passing `-` instead of a path. aikit writes it into
  `.aikit/temp/` itself and runs it from there, so the allowed-location policy, the
  forbidden-operation scan and the audit record all apply exactly as they do to a file you
  wrote — you are spared authoring the file, not exempted from the rules. `--runner` is
  required, because there is no filename to infer from. The temporary is removed when the
  command exits; the run directory keeps the copy.

- **Run root detection is filesystem-based and VCS-agnostic.** The command anchors to
  the nearest enclosing `.git`, `.hg`, or `.aikit` directory, so it works in a Git repo, a
  Mercurial repo, or a non-repo `.aikit/` folder with **no** `git`/`hg` subprocess for
  detection (blocks `blocked_repo_not_found` when no marker is found). The root's `vcs`
  (`git`/`mercurial`/`none`) is recorded in `run.json`.
- **Allowed script input locations** (the script must resolve, after symlink
  resolution, to a real file under one of these): `.aikit/temp/`,
  `.scratch/work/temp/`, `.scratch/work/outputs/`. These are *input* locations, not
  output locations.
- **Cross-OS runner detection (deterministic, OS-aware).** Supported extensions:
  `.sh`, `.zsh`, `.ps1`, `.cmd`, `.bat`, `.py`, `.js`. Runner names: `sh`, `zsh`,
  `bash`, `pwsh`, `powershell`, `cmd`, `python`, `python3`, `node`. Selection order:
  1. explicit `--runner <name>`;
  2. config `script_runner.extension_map` for the extension;
  3. a recognized `#!` shebang (unless `--no-shebang` or `detect_from_shebang=false`);
  4. the built-in extension map;
  5. an OS-aware default fallback (candidate resolution filters by OS and PATH);
  6. else a clear blocked failure.

  Commands are built as argv arrays (never concatenated shell strings): PowerShell uses
  `<pwsh|powershell> -NoProfile -ExecutionPolicy Bypass -File <script>`; `.cmd`/`.bat`
  use `cmd /d /c <script>` (`/d` suppresses the registry `AutoRun` command, which would
  otherwise run before the script and can rewrite `PATH` — the same ambient mutation
  `-NoProfile` refuses); Python/Node/sh/zsh/bash use `<interp> <script>`. On Windows the
  script path is passed with native separators, because `cmd` rejects forward slashes; the
  recorded `argv` shows that executed form, while `script_path` keeps the portable
  forward-slash spelling. **On Windows,
  no Git Bash is required:** `.ps1` runs via pwsh/powershell and `.cmd`/`.bat` via cmd;
  `.sh`/`.zsh` run only when a discoverable interpreter exists. Blocked cases:
  `blocked_unknown_script_type` (no extension/shebang match),
  `blocked_runner_not_found` (selected runner unavailable on this OS),
  `blocked_runner_not_allowed` (unrecognized `--runner`). `run.json` and the
  `script check --json` report record `detected_runner`, `detection_source`,
  `used_shebang`, `used_extension_map`, the resolved `interpreter`, and the full `argv`.
- **Clean-tree policy:** the default is allow-dirty. `--require-clean` blocks when
  the tracked working tree is dirty; `--allow-dirty` is the explicit default; the two
  cannot be combined. The dirty check is VCS-specific and runs **only** with
  `--require-clean`: **Git** uses `git status --porcelain`; **Mercurial** uses
  `hg status -mard` (run with `HGPLAIN=1` — the only place the runner invokes `hg`, with a
  clear error if `hg` is absent). In a non-repo `.aikit/` folder there is no working tree,
  so `--require-clean` blocks with `blocked_require_clean_unsupported`. Untracked/ignored
  files (e.g. `.aikit/outputs/`, `.scratch/`) do not make the tree dirty.
- **Forbidden-operation acknowledgement.** When the scan refuses a script it names the
  pattern and the line that matched. If the operation is genuinely intended, re-run with
  `--acknowledge-forbidden <pattern>` (repeatable), passing the exact pattern text from the
  refusal. Acknowledging one pattern never disables the others, and nothing is remembered
  between invocations — each run states its own intent. The acknowledged set is recorded in
  `run.json` as `acknowledged_forbidden`, so the audit trail shows what was waived.
- **`aikit script run --print`** validates policy and shows the planned command
  without executing (recorded as `executed: false`).
- **`aikit script check`** validates allowed location, path/symlink boundary,
  runner detection (same order as `script run`), the forbidden-operation scan, and the
  clean-tree policy (`--require-clean` / `--allow-dirty`, `--json`; reports detection
  metadata). It never executes or copies the
  script and creates no run directory, `stdout.txt`, `stderr.txt`, or `run.json`; it
  exits 0 when the policy accepts the script and 3 with the named blocked state when it
  does not.
- **Output (`script run`):** a run directory under `.aikit/outputs/runs/<id>/` by
  default (override with `--output <dir>`; `.scratch` output only when requested
  explicitly) containing the copied script (extension retained), `stdout.txt`,
  `stderr.txt`, and `run.json` (`vcs`, interpreter, argv, cwd, require_clean, executed,
  head(s) — populated for Git, empty for Mercurial/non-repo — exit_code, timings, paths,
  …). Created paths are printed (and included in `--json`).
- **Run-record retention.** Run directories accumulate once per invocation, so `script run`
  keeps the newest **100** and deletes older ones oldest-first. Set
  `output.retain_runs` in `.aikit/config.json` to change it, or `0` to keep everything:

  ```json
  { "output": { "retain_runs": 250 } }
  ```

  Only `runs/` is pruned — batches, reviews and inventories are created deliberately and are
  never touched. Deletion is never silent: the count appears as `pruned_runs` in `run.json`
  and in the human output, so a missing earlier run is explicable as a deliberate deletion
  rather than a record that was never written. `run.json` itself is written atomically, so a
  reader never sees a half-written record.
- **Exit code:** an executed script's exit code is propagated; policy blocks return a
  non-zero `blocked_*` error (exit 3); invalid usage is exit 2.

### Version

```sh
aikit --version          # clap string: "aikit <version>"
aikit version            # compact human report
aikit version --json     # machine-readable record (kind: aikit.version)
```

- `aikit version` reports the package/binary version plus best-effort build metadata:
  `git_commit`, `build_profile`, `os`, `arch`, `target` (any of git/profile/target may
  be `null`). It is read-only and works outside a Git repository.
- **Build-metadata freshness:** `git_commit` is captured at build time. `build.rs` watches
  `.git/HEAD`, the current branch's ref file (`.git/refs/heads/<branch>`), and
  `.git/packed-refs`, so a normal commit on the branch triggers a rebuild and refreshes the
  commit. Limitation (best-effort, not over-engineered): detached HEAD, freshly packed
  refs, or linked-worktree gitdir layouts may still leave `git_commit` stale until the next
  rebuild; metadata never fails the build when git is unavailable.
- The package version is the Cargo package version — distinct from the per-record
  `schema_version` used by anchors/manifests/run records. It is also recorded in batch
  anchors (`aikit_version`), review manifests (`aikit_version`), and `env snapshot`.

### Output management

Manage the local artifacts aikit writes under `.aikit/outputs/` (batch anchors,
inventories, review bundles, and run records):

```sh
aikit output list                       # list known output artifacts (read-only)
aikit output show <artifact-path-or-id> # show one artifact's details (read-only)
aikit output clean --dry-run            # show what would be deleted; deletes nothing
aikit output clean --older-than 7d --execute   # delete artifacts older than 7 days
aikit output clean --all --execute      # delete all known output artifacts
```

- `output list` and `output show` are **read-only** — they create and delete nothing.
- `output clean` is **dry-run by default**: it deletes nothing unless you pass
  `--execute`, and `--execute` requires a selector (`--older-than <n>h|<n>d` or `--all`).
- Only **known** artifacts are touched (`batches/*.json` files and `inventory/`,
  `reviews/`, `runs/` subdirectories) inside the selected output root. `clean` never
  deletes outside the output root, never follows symlink escapes, and never touches
  `.aikit/temp/`, `.scratch/`, `.claude/`, `target/`, or `.git/`.
- `--family <batches|inventory|reviews|runs>` narrows the scope; `--root <path>` selects
  a different output root (restricted to `.aikit/outputs/` or `.scratch/work/outputs/`,
  so management can never be redirected at `.git/`, `target/`, or other non-output
  directories); all three support `--json`.

### Batch inspection and anchor diff

Inspect existing batch anchors and diff one against the current tree. These are
**explicit inspection** commands — they never auto-select a "latest" anchor for work;
anchor-consuming commands always take an explicit anchor:

```sh
aikit batch list                      # list batch anchors (read-only)
aikit batch show <anchor-path-or-id>  # show one explicit anchor (read-only)
aikit batch diff <anchor> # diff the anchor's head vs the current tree
```

- `batch list` and `batch show` are **read-only**; `batch list` reports valid anchors and
  flags invalid files as skipped (never guessed).
- `batch diff` uses the anchor's **recorded Git head** as the diff base (it must still
  exist locally) and reports committed changes since the anchor plus current tracked
  working-tree changes, via Git. **Untracked file contents are not part of the Git diff**
  — use `aikit batch changed --anchor <anchor>` for a timestamp-based changed-file list.
  `batch diff` is
  mechanical inspection only: it creates no review bundle or output artifact and never
  touches remotes. `--stat` (included by default), `--patch`, and `--json` are supported.

### Environment snapshot

Capture a bounded, mechanical report of local environment facts useful for debugging
aikit usage:

```sh
aikit env snapshot          # human-readable report
aikit env snapshot --json   # machine-readable report
```

- **Read-only**: it creates no files or directories, modifies no repo files, runs no
  network commands, and never touches remotes. It works inside or outside a Git
  repository (outside a repo, the repo facts are reported as `null`).
- Reports the aikit version, current executable, OS family, CPU architecture, working
  directory, repo facts (root, branch, HEAD, tracked clean/dirty, default output root,
  `.aikit/` existence and ignore status), **legacy/informational shell-interpreter
  probes** (`/bin/sh`, `/bin/zsh`), local git/Rust/Cargo versions, and the shell from
  `$SHELL`. These shell probes are informational only — `env snapshot` does **not** report
  the cross-OS runner-availability/readiness model that `doctor --require-root` uses.
- It deliberately **does not dump all environment variables**, the raw `PATH`, tokens,
  credentials, private keys, or any provider/model-specific or network-derived
  information. `PATH` is summarized only (an entry count plus whether the current
  executable's directory is on it).

### Secret scan

Run a local, redacted scan for likely secrets in **explicit** repo-local paths (the whole
repo is never scanned implicitly unless you pass the repo root or `.`):

```sh
aikit scan secrets README.md docs           # scan explicit files/directories
aikit scan secrets . --fail-on high         # exit 3 only on high-confidence findings
aikit scan secrets src --json --include-ignored
```

**What this is for.** It is a cheap, dependency-free signal to run before handing a path
to something that will publish or transmit it — a bundle, a paste, an upload. It is
**not** a security audit and does not replace [gitleaks](https://github.com/gitleaks/gitleaks)
or [trufflehog](https://github.com/trufflesecurity/trufflehog), which carry far larger
rule sets, scan Git history, and can verify whether a credential is live. `aikit` scans
only the working-tree bytes you name, and never checks a credential against its provider.
**Absence of findings does not prove a file or repo is safe to share.**

**Severity is confidence, not blast radius.** It answers "how sure is aikit that this
match is a real credential?" — never "how bad would it be if it leaked":

| Severity | Meaning |
| --- | --- |
| `high` | Either the match is in a **self-identifying credential format** — the bytes announce what they are (a private-key block header, a provider token prefix) — or a credential-shaped name is assigned a **long, opaque, token-like value**. Low false-positive rate. |
| `medium` | A credential-shaped name is assigned a short or word-like value. Real secrets and placeholders are indistinguishable here, so read every one. |
| `low` | The same name-based match, in a path that labels itself an example, sample, template, or fixture. Most likely deliberate placeholder text. |

Only the name-based rule is demoted by an example path. A real private key committed under
`testdata/` keeps its `high` severity — a leaked key is leaked wherever it sits.

Severity is monotonic, so `--fail-on <severity>` means "that severity **or above**":
`--fail-on high` fails on `high` only, `--fail-on medium` on `high`+`medium`, and
`--fail-on low` on any finding at all. Without the flag the command reports findings and
**exits 0**, which is the right mode for inspection. When the gate trips it exits 3 with
`blocked_secret_findings`.

- It **never prints raw secret values** in human or JSON output. Each finding reports the
  file path, line number, rule id, description, and severity only (`redacted: true`) — so
  the report is itself safe to paste into an agent transcript or an issue.
- At least one path is required. Paths are resolved relative to the repo root; paths
  outside the repo and symlink/path escapes are rejected, and `.git/` is always excluded.
  Explicit files are scanned even when ignored; for directories, traversal respects
  `.gitignore` by default (use `--include-ignored` to include ignored files). Binary
  files and files larger than `--max-file-bytes` (default 1 MiB) are skipped.
- It creates no output artifacts and never touches remotes.

### Configuration

```sh
aikit config show          # every effective setting and where it came from
aikit config show --json
```

Configuration is layered — built-in defaults, then `aikit.config.json` at the repo root,
then `.aikit/config.json`, then CLI flags. `config show` reports each effective value with
the layer that set it (`default`, or the file), so "why is this value what it is?" does not
require replaying the merge by hand. It is read-only, and loading uses the same code path
every other command uses, so what it prints is what they will act on.

**The MCP server does not consult configuration at all** — `aikit mcp` requires an explicit
runner on every call and applies no config-driven defaults. `config show` explains the CLI.

### Agent guide

```sh
aikit agents-md                              # print the guide
AGENTS_MD=/etc/aikit/AGENTS.md aikit agents-md   # serve a different file instead
```

`aikit agents-md` prints the agent-facing guide to aikit: what it does, the rules that are
expensive to get wrong, a one-line contract per command, and its safety posture. The text is
compiled into the binary, so it always describes the version printing it and works with no
repository present. It is the same document the `agents_md` MCP tool returns, so an agent
reaching aikit through a shell and one reaching it over MCP get identical guidance.

Set `AGENTS_MD` to a readable file path to serve that file instead. The variable is
deliberately unprefixed — the command name already says whose guide it is. A path that
cannot be read is an error, never a silent fall back to the embedded text.

### MCP server

```sh
aikit mcp        # serve on stdio; configure as {"command": "aikit", "args": ["mcp"]}
```

`aikit mcp` serves the script runner to an AI agent over the Model Context Protocol on
stdio. It exposes three tools:

| Tool | Purpose |
| --- | --- |
| `run` | Execute a script supplied in the call itself. |
| `list_runners` | Report which interpreters this host actually has. |
| `agents_md` | Return the guide above, as Markdown. |

This is a **separate contract from `aikit script run`, not a wrapper**. The script arrives
as call arguments rather than as a file you author, so nothing is written into your
repository or working tree, no repository is required, `.aikit/` is unused, and no run
record is produced. A temporary script does exist while it runs, under the system temp
location in a directory created private to you; deletion afterwards is best-effort, so do
not put a secret in a script body on the assumption it vanishes.

`runner` is **required on every call and never inferred** — no shebang or extension-map
tier applies, because `pwsh` and `powershell` interpret the same `.ps1` differently. Call
`list_runners` first and pass an exact name. Limits can be adjusted but not removed: a
timeout always applies (default 120 s, capped at 1 h) and output capture is bounded
(default 32 MiB across both streams). Concurrent calls are capped at 8 and over-capacity
calls are rejected rather than queued, because an abandoned call holds its slot for its
full timeout.

**It is not a security sandbox.** It runs arbitrary scripts with the privileges of the
aikit process. The forbidden-pattern scan is an accident guard, not containment, and
approval is the MCP client's responsibility — aikit adds no prompt of its own. See
[SECURITY.md](SECURITY.md).

## Install — detailed setup (PATH, per-OS)

The [Install](#install) section above covers the common cases (the `cargo install --git`
one-liner and a local clone). This section adds per-OS `PATH` setup so other repositories
can call `aikit ...` directly. `cargo install` copies the binary into Cargo's bin
directory (normally `$HOME/.cargo/bin`); re-run it after pulling a newer version, and make
sure that directory is on your `PATH`.

Nothing here needs `make`. A `Makefile` is included as a convenience for working on `aikit`
itself, but it only wraps `cargo`, and `make` is often absent on Windows:

| Convenience | Equivalent without `make` |
| --- | --- |
| `make install` | `cargo install --path . --locked --force` |
| `make uninstall` | `cargo uninstall aikit` |
| `make build` | `cargo build` |
| `make test` | `cargo test` |
| `make fmt` | `cargo fmt` |
| `make lint` | `cargo clippy --all-targets -- -D warnings` |
| `make verify` | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` |

(`make verify` additionally type-checks the Windows-only code paths from a non-Windows host
via `cargo clippy --target x86_64-pc-windows-msvc`, which needs that target installed —
`rustup target add x86_64-pc-windows-msvc`. It is redundant when you are already on Windows.)

macOS (zsh) — recommended:

```sh
# From the aikit repo
cargo install --path . --locked

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
cargo install --path . --locked
export PATH="$HOME/.cargo/bin:$PATH"
aikit --help
```

Put the `PATH` line in your shell startup file (for example `~/.bashrc` or `~/.zshrc`)
if `$HOME/.cargo/bin` is not already on `PATH`.

Windows:

```sh
cargo install --path . --locked
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

## Documentation

- [`docs/aikit-cli-spec.md`](docs/aikit-cli-spec.md) — CLI behavior and command reference.
- [`docs/agent-usage.md`](docs/agent-usage.md) — agent-agnostic usage guidance.
- [`docs/agent-integration-examples.md`](docs/agent-integration-examples.md) — example wrapper patterns for external integrations.
