# aikit Agent Usage Guide

This guide explains how an AI agent (or a human) can use `aikit` mechanically. It is
**agent-agnostic**: nothing here depends on a specific AI vendor, model, or assistant.
`aikit` is a small local command-line tool; this guide describes its commands, its
output conventions, and the assumptions a caller should and should not make.

Every command has detailed `--help`; this guide is an orientation, and the built-in
help is the authoritative per-command reference.

## Purpose

- `aikit` is a local, mechanical CLI for AI-assisted work in a Git or Mercurial
  repository (setup and the script runner also work in a non-repo `.aikit/` folder).
- It helps agents and humans create batch anchors, inspect changed files, inventory a
  repository, generate bounded review bundles, and run constrained local scripts.
- It performs deterministic, repeatable operations on the filesystem and version-control
  (Git or Mercurial) state.
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
- `2` — invalid usage: malformed arguments, or an invalid argument value. Usually no record
  is produced, because the argument parser exits before any aikit code runs. The exception
  is an invalid value only aikit can detect — a `--cwd` directory it cannot enter — where a
  record **is** emitted, since aikit is running and the parser was not.
- `3` — a named **blocked state** (a mechanical precondition was not met).
- For `script run` only, an executed script's own exit code is propagated.

A blocked state is a deliberate, named refusal — agents must surface it, not ignore it.

The blocked states form a **closed set**, listed in full in
[`aikit-cli-spec.md` §7.2](aikit-cli-spec.md). Match on it exhaustively and treat an
unlisted value as a protocol violation rather than something to guess at; a test in the
source fails the build if the set and the code drift apart.

### Machine-readable failures

**A command run with `--json` emits exactly one JSON document on stdout, whether it
succeeds or fails.** Human-readable prose always goes to stderr. Parse stdout; never scrape
stderr to find out what happened.

On failure the document is `kind: aikit.error`:

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

- `blocked_state` is a named state, or **null** for an ordinary failure. That is the field
  to branch on: a named state means "change something and retry"; null means the
  environment failed. Neither requires reading `message`.
- Two commands report their own full record instead of an error document, because the
  record *is* the answer and the non-zero exit is only a gate: `scan secrets` when
  `--fail-on` trips, and `script check` when policy rejects a script. Both carry
  `blocked_state` inside that record, so branching on `blocked_state` works either way.
- Without `--json` a failure writes nothing to stdout, so a human-mode command never emits
  JSON that was not asked for.

## Standard Local Workflow

When setting up a repository for the first time, check → prepare → re-check:

1. `aikit doctor` — read-only readiness report. Use the **top-level** form here: it works
   in a directory that has never been initialized, reporting `ready: false` with the
   remediation at exit 0, whereas `aikit doctor --require-root` blocks when there is no `.git`,
   `.hg`, or `.aikit` marker. Since the point of this step is to find out whether setup is
   needed, the form that fails when setup *is* needed answers the wrong question.
2. `aikit init` — adaptive one-step setup: in a Git or Mercurial repository it prepares
   local `.aikit/temp/` and adds the VCS-appropriate local ignore coverage; in a non-repo
   folder it creates the directories only (idempotent; safe to re-run). Use
   `aikit init --require-repo` to require a repository, or `aikit init --require-folder` to force non-repo
   treatment.
3. `aikit doctor` — confirm the directory is now `ready`.

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

After commands generate artifacts under `.aikit/outputs/` (anchors, inventories, review
bundles, run records), use `aikit output list` / `aikit output show` to inspect them and
`aikit output clean` to prune them. Generated output is local-only and should not be
committed, and cleanup is **explicit**: `output clean` is dry-run by default and deletes
only with `--execute` plus a selector — aikit never deletes outputs automatically.

## Command Families

### `aikit init` / `aikit init --require-repo` / `aikit init --require-folder`

All three create `.aikit/` and `.aikit/temp/` (idempotent), create no output artifacts,
and do not create `.scratch/` or `.claude/`. They differ only in how they treat
version control. **Repository detection is filesystem-based** (it looks for an enclosing
`.git`/`.hg`) and does **not** require the `git`/`hg` CLI to be installed or on PATH.

- **`aikit init`** — adaptive. Inside a Git or Mercurial repository it behaves like
  `init --require-repo` (adds VCS ignore coverage); outside any repository it behaves like
  `init --require-folder` (directories only, no ignore). Never errors on repo presence/absence.
- **`aikit init --require-repo`** — force repo mode. Errors `blocked_repo_not_found` when not
  inside a Git or Mercurial repository. Ensures `.aikit/` is locally ignored using the
  VCS's local, never-committed mechanism: for **Git**, an entry in `.git/info/exclude`
  (local Git metadata, never staged); for **Mercurial**, a `.hg/hgignore.aikit` pattern
  registered via `[ui] ignore.aikit` in `.hg/hgrc` (both under `.hg/`, never committed or
  cloned). Tracked ignore files (`.gitignore` / `.hgignore`) are never modified; no
  duplicate entry is added when `.aikit/` is already ignored.
- **`aikit init --require-folder`** — force non-repo mode (directories only, no ignore). Errors
  `blocked_repo_present` when run inside a Git or Mercurial repository (use `init --require-repo` or
  `init` there so `.aikit/` is ignored).
- **Output:** a printed (or `--json`) record (`aikit.repo_init`) of what was already
  present and what was created, including a `vcs` field (`git` / `mercurial` / `none`) and
  ignore status/source.

### `aikit doctor` / `aikit doctor --require-root`

- **Purpose:** report local aikit readiness, read-only.
- **Typical use:** **`aikit doctor`** — run it before `init` to find out whether setup is
  needed, and after to confirm `ready`. It is the form to reach for by default: it answers
  in a directory that has never been initialized instead of failing there.
- **Which form:** `doctor` is adaptive; `doctor --require-root` requires a `.git`, `.hg`, or
  `.aikit` marker and blocks `blocked_repo_not_found` without one. Both are co-equal and
  share one implementation — `doctor --require-root` is not deprecated. There is no `folder doctor`:
  doctor writes nothing, so there is no mode-specific behavior to provide.
- **Constraints:** **read-only** — creates and modifies nothing (no `.aikit/`,
  `.scratch/`, `.claude/`, `.gitignore`, `.git/info/exclude`, `.hgignore`, or `.hg/`
  state), including on the no-marker path. Works in Git repos, Mercurial repos, and
  non-repo `.aikit/` folders. Exit 0 even with warnings; missing `.aikit/temp/` or (in a
  repo) ignore coverage are warnings, not failures. For Mercurial, ignore coverage is
  detected without invoking `hg`; branch/HEAD come from `hg` (run with `HGPLAIN=1`) when
  available and degrade to empty with a warning otherwise.
- **Output:** a printed (or `--json`) readiness report (`aikit.repo_doctor` for both
  forms): repo root, `root_source`, `vcs` (`git`/`mercurial`/`none`), branch/HEAD, tracked
  clean/dirty, `.aikit/` `.aikit/temp/` `.aikit/outputs/` existence, ignore status +
  source, default output root, allowed script locations, runner availability, version,
  warnings, and an overall `ready` flag. A non-repo `.aikit/` folder needs no ignore
  coverage to be `ready`.
- **Reading an unanchored report:** with no marker, `aikit doctor` sets `root_source:
  "cwd_no_marker"` and `ready: false`, and `repo_root` is the current directory rather than
  a real root. Check `root_source` before treating `repo_root` as an anchor — the two cases
  are otherwise indistinguishable, since both are just a path.

### `aikit output list` / `aikit output show` / `aikit output clean`

- **Purpose:** manage the local artifacts aikit writes under `.aikit/outputs/` (batch
  anchors, inventories, review bundles, run records).
- **Typical use:** inspect what local output exists (`list`/`show`) and prune it
  explicitly when it accumulates (`clean`).
- **Constraints:** must be run inside a Git repository, else `blocked_repo_not_found`.
  `list` and `show` are **read-only**. `clean` is **dry-run by default** and deletes only
  with `--execute` plus a selector (`--older-than <n>h|<n>d` or `--all`); it deletes only
  known artifacts (`batches/*.json`, `inventory/`, `reviews/`, `runs/` subdirectories)
  inside the selected output root, never outside it, never via symlink escapes, and never
  `.aikit/temp/`, `.scratch/`, `.claude/`, `target/`, or `.git/`. `--family` narrows
  scope; `--root <path>` selects a different in-repo output root.
- **Output:** `list` → `aikit.output_list`; `show` → `aikit.output_show` (artifact
  family/id/path, contained files, a compact metadata summary); `clean` →
  `aikit.output_clean` (mode, filters, candidates, deleted paths). All support `--json`.
  This is inspection/management only — no judgment about output correctness.

### `aikit batch start`

- **Purpose:** create a minimal timestamp-reference anchor before a unit of work begins.
- **Typical use:** mark the start of an agent task so later steps can report what the
  task touched.
- **Constraints:** must be run inside a Git repository, else `blocked_repo_not_found`.
- **Output:** a JSON anchor under `.aikit/outputs/batches/<anchor-id>.json` by default.
  The created path is printed (and included in `--json`). Anchors are durable artifacts.

### `aikit batch list` / `aikit batch show`

- **Purpose:** inspect existing batch anchors (read-only).
- **Typical use:** see which anchors exist, and review one explicitly before using it for
  anchor-consuming work.
- **Constraints:** must be inside a Git repository, else `blocked_repo_not_found`. Both
  are read-only (create/delete nothing). `batch list` reports valid anchors and flags
  invalid files as skipped. `batch show <anchor-path-or-id>` validates the anchor and that
  it belongs to the current repo; path escapes are rejected. **Neither auto-selects a
  "latest" anchor** — always pass an explicit anchor to anchor-consuming commands.
- **Output:** `batch list` → `aikit.batch_list`; `batch show` → `aikit.batch_show`. Both
  support `--json` and `--root`.

### `aikit batch diff <anchor>`

- **Purpose:** produce a mechanical Git diff from an anchor's recorded head to the current
  working tree.
- **Typical use:** see what changed since an explicit anchor was created.
- **Constraints:** validates the anchor and that it belongs to this repo; uses the
  anchor's recorded `git_head` as the diff base (must still exist locally, else blocked).
  Reports committed changes since the anchor and current tracked working-tree changes.
  **Untracked file contents are not part of the Git diff** — use
  `batch changed --anchor <anchor>` for a timestamp-based file list. Inspection only: it
  creates no review bundle or output artifact, advances no workflow state, and never
  touches remotes.
- **Output:** `aikit.diff_anchor` (anchor metadata, base/current head, name-status files,
  counts, stat, notes). `--stat` (default), `--patch`, and `--json` supported.

### `aikit env snapshot`

- **Purpose:** capture mechanical local environment facts useful for debugging aikit
  usage.
- **Typical use:** a debugging/reporting aid — record the local toolchain and repo state
  when diagnosing an aikit problem or describing an environment.
- **Constraints:** **read-only** — creates no files or directories, modifies no repo
  files, runs no network commands, and never touches remotes. Works inside or outside a
  Git repository (outside a repo, repo facts are `null` and a warning is recorded). It
  deliberately **does not dump all environment variables**, the raw `PATH`, tokens,
  credentials, or keys; `PATH` is summarized only (entry count plus an on-PATH boolean).
- **Output:** `aikit.env_snapshot` (version, current exe, OS/arch, working directory,
  repo facts, interpreter availability, local git/Rust/Cargo versions, `$SHELL`,
  warnings). Supports `--json`.

### `aikit scan secrets <path>...`

- **Purpose:** run a local, best-effort heuristic scan for likely secrets in explicit
  repo-local paths.
- **Typical use:** a **pre-share / pre-review** best-effort check — sweep files before
  bundling them for review or handing them off, to catch obvious committed secrets.
- **Constraints:** must be inside a Git repository, else `blocked_repo_not_found`. At
  least one explicit path is required; the whole repo is never scanned implicitly unless
  the path is the repo root or `.`. Paths are resolved relative to the repo root; paths
  outside the repo and symlink/path escapes are rejected (`blocked_path_escape`), and
  `.git/` is always excluded. Explicit files are scanned even when ignored; directory
  traversal respects `.gitignore` by default (`--include-ignored` to include ignored
  files). Binary files and files over `--max-file-bytes` (default 1 MiB) are skipped.
- **Results are a signal, not proof.** The rule set favours precision over coverage, so it
  false-negatives freely; a finding is **not** proof of a live credential, and **no
  findings does not prove a file is safe to share**. It does not replace gitleaks or
  trufflehog, which have far larger rule sets, scan history, and can verify liveness.
  Agents must surface findings (and this caveat) for human inspection rather than acting
  on them as a verdict.
- **Severity means confidence, not blast radius** — how sure aikit is that the match is a
  real credential. `high`: a self-identifying credential format (private-key header,
  provider token prefix), or a credential-style name assigned a long opaque token.
  `medium`: a credential-style name assigned a short or word-like value — placeholders and
  real secrets are indistinguishable, so read each one. `low`: the same name-based match in
  a path labelled example/sample/template/fixture. Only the name-based rule is demoted by
  an example path; a real key under `testdata/` stays `high`.
- **Privacy:** raw secret values are **never** printed in human or JSON output — each
  finding carries only path, line, rule id, description, severity, and `redacted: true`.
  This makes the report safe to quote back into a transcript verbatim.
- **Exit behavior:** by default findings are reported and the command exits 0 (usable for
  inspection). `--fail-on <high|medium|low>` exits 3 (`blocked_secret_findings`) when a
  finding at that severity **or above** is present — severity is monotonic, so
  `--fail-on low` fails on any finding. Creates no output artifacts.
- **Output:** `aikit.scan_secrets` (inputs, counts, findings, skipped files). Supports
  `--json`.

### `aikit config show`

- **Purpose:** report every effective configuration value and the layer that set it.
- **Typical use:** answer "why is this value what it is?" without opening each candidate
  file and replaying the merge. Sources are reported in precedence order, and each value
  names `default` or the file that last won.
- **Constraints:** read-only; creates nothing. **The MCP server does not consult
  configuration at all** — this explains the CLI only.
- **Output:** `aikit.config_show` with `sources` and a `settings` array of
  `{key, value, source}`. Supports `--json`.

### Global `--cwd <PATH>`

Every command accepts `--cwd`, running as if aikit had been started there. Prefer it over
navigating the shell: quoting, spaces, drive-relative paths and `cd` semantics differ across
platforms, and getting them wrong produces a failure that looks like an aikit error. A
directory that cannot be entered is invalid usage (exit 2), not a blocked state — but under
`--json` you still get an `aikit.error` record on stdout (`blocked_state: null`), because
aikit detected it rather than the argument parser.

### `aikit agents-md`

- **Purpose:** print aikit's own agent guide — what it does, the rules that are expensive to
  get wrong, a one-line contract per command, and its safety posture.
- **Typical use:** run it once when you first encounter aikit in a repository, instead of
  inferring contracts from `--help` output or guessing. It is the shell-side twin of the
  `agents_md` MCP tool and serves the identical document.
- **Constraints:** read-only; creates nothing; needs no repository and works outside one.
  The text is compiled into the binary, so it always describes the version printing it.
  `AGENTS_MD` (no `AIKIT_` prefix — the command name already scopes it) may name a readable
  file to serve instead; an unreadable path is an error, not a silent fallback.
- **Output:** Markdown on stdout. No `--json`: the payload is prose, and a JSON string
  wrapper would only make it harder to read.

### `aikit batch changed --anchor <anchor.json>`

- **Purpose:** report existing files modified since a given anchor.
- **Typical use:** after doing work, list the change set for review or reporting.
- **Constraints:** the anchor must exist, be a valid anchor, and belong to this repo
  (else a `blocked_*` state). Discovery is **timestamp-based** (filesystem mtime newer
  than the anchor file), not `git status`: it includes existing non-ignored files
  modified after the anchor whether tracked or untracked, and excludes files merely dirty
  vs `HEAD` but older than the anchor. Deleted files are out of scope. A clean tree is not
  required.
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
- **Output:** `review_bundle.txt` and `manifest.json` under
  `.aikit/outputs/reviews/<id>/` by default. Created paths are printed (and in `--json`);
  agents should report or cite the generated paths rather than guessing them.

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

- **Purpose:** run a constrained local script through its **detected runner** and record
  an audit trail.
- **Typical use:** execute a small, repo-local helper script with a recorded run record.
- **Constraints:** see [Script Runner Use](#script-runner-use). This is **not** a
  security sandbox.
- **Script on stdin:** pass `-` instead of a path to supply the script text on stdin. aikit
  writes it into `.aikit/temp/` itself and runs it from there, so every policy still
  applies — you are spared authoring a file, not exempted from the rules. `--runner` is
  required (no filename to infer from), and the temporary is removed when the command exits.
- **Output:** a run directory under `.aikit/outputs/runs/<id>/` by default (copied
  script, `stdout.txt`, `stderr.txt`, `run.json`, written atomically). The executed script's
  exit code is propagated — check **`exit_code_source`** before trusting `0`:
  `interpreter` means the interpreter propagated the status, `unpropagated` (PowerShell and
  cmd here, since aikit runs your own file and cannot modify it) means 0 tells you it *ran*.
- **Retention:** run directories accumulate once per invocation, so the newest
  `output.retain_runs` (default 100; `0` disables) are kept and older ones deleted
  oldest-first. Only `runs/` is pruned. The count is reported as `pruned_runs`, so a missing
  earlier run is explicable rather than mysterious — surface it if a caller depended on it.

### `aikit script check <script-path>`

- **Purpose:** validate a script against the same policy `script run` uses, without
  executing it.
- **Typical use:** confirm a generated script will be accepted (allowed location,
  path/symlink boundary, runner detection, forbidden-operation scan, clean-tree
  policy) before running it.
- **Constraints:** same policy as `script run`; optional `--require-clean` /
  `--allow-dirty` (default allow-dirty) and `--json`. There is no `--print` (the command
  already never executes). This is **not** a security sandbox.
- **Output:** a printed (or `--json`) report with `accepted` / `blocked_state`; the
  script is never executed or copied, and **no** run directory, `stdout.txt`,
  `stderr.txt`, or `run.json` is created. Exit 0 when accepted, exit 3 with the named
  blocked state when blocked.

### `aikit mcp`

- **Purpose:** serve the script runner to an AI agent over the Model Context Protocol, on
  stdio. The agent calls tools instead of writing and running a script file.
- **Typical use:** register it with an MCP client
  (`{"command": "aikit", "args": ["mcp"]}`), then call **`list_runners`** followed
  by **`run`**. It is not useful to invoke by hand — it speaks JSON-RPC on stdin/stdout.
- **Tools:** `run`, `list_runners`, and **`agents_md`** — the last returns aikit's own agent
  guide as raw Markdown in the text block (not a JSON string), compiled into the binary so
  it matches the version answering. Call it once at the start of a session instead of
  guessing at command contracts.
- **Why it exists:** running a set of commands through `script run` costs an agent two
  approvals — one to write the script file, one to run it. Here the script travels in the
  call, so it costs one.
- **How it differs from `script run` — do not assume they match:**
  - the runner is **required and never inferred**; call `list_runners` and pass an exact
    name. An unavailable runner is an error, never a substitution;
  - no repository, no `.aikit/`, no allowed-location allowlist, no dirty check;
  - **no run record.** Nothing is written into your repository or working tree, and there
    is no audit trail. If you need one, use `script run`, which writes one;
  - configuration is not consulted;
  - `cwd` must be an absolute path to an existing directory.
- **Constraints:** every result carries both a text block and structured content; a
  non-zero exit code is a **successful** call; the timeout (default 120 s, max 3600 s)
  cannot be disabled, and **cancelling a call is not a reliable stop** — a client that
  simply stops waiting leaves the script running to completion. Concurrency is capped at 8
  and over-capacity calls are rejected rather than queued. This is **not** a security
  sandbox: it executes arbitrary scripts, and approval is the MCP client's responsibility.
- **Output:** the tool result only. A temporary script exists under the system temp
  location while it runs, in a directory created private to you, and is deleted when the
  call ends — best-effort, so it can briefly outlive the call if something holds the file.

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
- Every bundle produces `review_bundle.txt` (readable, with per-file headings,
  SHA-256, size, truncation status, and fenced contents) and `manifest.json` (schema
  version, ids, repo metadata, inputs, limits, per-file records, totals). Older local
  review outputs may use the previous name `run_for_review.txt`.
- Agents should report or cite the generated paths rather than guessing them.

## Script Runner Use

- `aikit script run` runs **constrained local scripts only**; `aikit script check`
  applies the exact same policy without executing anything and without writing any run
  output. Use `script check` to confirm a script will be accepted before running it.
- Neither is a **security sandbox**, and running a script does **not** make the script
  safe. The allowed-location policy is the primary control; the forbidden-operation scan
  is best-effort and is **not** a security boundary.
- **When the forbidden-operation scan refuses a script,** the refusal names the pattern and
  the matching line. Do not rewrite the script to slip past the pattern — that defeats an
  accident guard rather than satisfying it. Either drop the operation, or, if it is
  genuinely intended, re-run with `--acknowledge-forbidden <pattern>` (repeatable) using the
  exact pattern text from the refusal, and say in your report what you waived and why.
  Acknowledgement is per invocation and per pattern: acknowledging one never disables the
  others, and nothing carries over to the next run. Both records carry the waived set as
  `acknowledged_forbidden` — `run.json` for `script run`, and the `aikit.script_check`
  report for `script check` — so the audit trail shows what was waived at either step.
- **Allowed script input locations** (the script must resolve, after symlink
  resolution, to a real file under one of these):
  - `.aikit/temp/`
  - `.scratch/work/temp/`
  - `.scratch/work/outputs/`
- **Cross-OS runner detection (deterministic, OS-aware).** The runner is detected, not
  fixed. Supported extensions: `.sh`, `.zsh`, `.ps1`, `.cmd`, `.bat`, `.py`, `.js`.
  Supported runner names: `sh`, `zsh`, `bash`, `pwsh`, `powershell`, `cmd`, `python`,
  `python3`, `node`. Selection order:
  1. explicit `--runner <name>`;
  2. config `script_runner.extension_map` for the extension;
  3. a recognized `#!` shebang (unless `--no-shebang` or `detect_from_shebang=false`);
  4. the built-in extension map;
  5. an OS-aware default fallback;
  6. else a clear blocked failure.

  `script_runner.preferred_runners` may reorder candidates. On Windows, `.ps1` uses
  `pwsh`/`powershell` and `.cmd`/`.bat` use `cmd` — **Git Bash is not required**;
  `.sh`/`.zsh` run only when a discoverable compatible runner exists. An unknown script
  type blocks with `blocked_unknown_script_type`; a selected-but-unavailable runner blocks
  with `blocked_runner_not_found`; an unknown `--runner` name blocks with
  `blocked_runner_not_allowed`.
- **Run root detection is filesystem-based and VCS-agnostic.** `script run` / `script
  check` anchor to the nearest enclosing `.git`, `.hg`, or `.aikit` directory — so they
  work in a Git repo, a Mercurial repo, **or** a non-repo `.aikit/` folder, with **no**
  `git`/`hg` subprocess for detection. When no such marker is found the command blocks
  with `blocked_repo_not_found`. The detected root is recorded as `vcs`
  (`git`/`mercurial`/`none`) in `run.json`.
- `--print` validates policy and prints the planned command **without executing**
  (recorded as `executed: false`; no run directory is created).
- `--require-clean` blocks when the tracked working tree is dirty (`blocked_dirty_tree`).
  The dirty check is VCS-specific and runs only with this flag: **Git** uses
  `git status --porcelain`; **Mercurial** uses `hg status -mard` (run with `HGPLAIN=1`;
  the only place `script run` invokes `hg`, and it errors clearly if `hg` is not
  installed). In a non-repo `.aikit/` folder there is no working tree to check, so
  `--require-clean` blocks with `blocked_require_clean_unsupported`.
- `--allow-dirty` permits a dirty tracked tree; this is the **default** when neither
  flag is given (and does no VCS work). `--require-clean` and `--allow-dirty` cannot be
  combined.
- On execution the output run directory contains the copied script (extension
  retained), `stdout.txt`, `stderr.txt`, and `run.json`, which records runner metadata:
  `vcs`, `detected_runner`, `detection_source`, `used_shebang`, `used_extension_map`, the
  resolved interpreter, `argv`, cwd, head(s) (populated for Git; empty for Mercurial /
  non-repo), exit code, timings, and paths.

## Agent-Generated Script Rules

`aikit` validates *where the script file lives* (it must resolve to a real file under an
allowed input location) and records the run. But once execution begins, the script runs
from the repository root through its detected runner, and `aikit script run` does
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
  `--anchor <anchor.json>` (or explicit anchor argument). Use `aikit batch list` /
  `aikit batch show` to inspect which anchors exist and choose one deliberately; these
  commands never select an anchor for you.
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
repository (so `aikit` stays agent-agnostic). For worked example wrapper patterns,
see [`agent-integration-examples.md`](agent-integration-examples.md) (documentation
only; it adds no agent-specific behavior to `aikit`). Guidance for such wrappers:

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

# Report local environment facts (read-only; no full env dump).
aikit env snapshot --json

# Redacted secret scan over explicit paths (raw values never printed).
aikit scan secrets README.md docs --json
aikit scan secrets . --fail-on high

# Permission-consolidated: run many local checks via one top-level invocation
# (the repeated commands live inside the script).
aikit script run .aikit/temp/local-checks.sh --require-clean --json
```

See each command's `--help` for the full set of flags and behavior.
