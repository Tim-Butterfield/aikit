# Agent Integration Examples

This document describes **example** patterns for external agents, tools, skills,
slash commands, or workflow wrappers that call the installed `aikit` CLI. It is
documentation only: it does not add behavior to `aikit`, and it does not define
built-in support for any specific assistant.

## Purpose

`aikit` is a small, mechanical, local CLI. Agents and tools get value from it by
calling its commands and reading its output — not by embedding anything
assistant-specific into the tool itself. This document gives **example wrapper
patterns** an external integration can follow when it wants to drive `aikit`
mechanically: how to call a command, which JSON fields and output paths to
preserve, which blocked states to surface, and which operations to keep behind
explicit confirmation.

Every pattern here is illustrative. The integrations themselves live **outside**
this repository so that `aikit` stays agent-agnostic. The installed CLI and
[`docs/agent-usage.md`](agent-usage.md) remain the authoritative source of
command behavior; this document only shows how an external caller might compose
those commands.

## Non-Goals

This document explicitly does **not**:

- add agent-specific behavior to `aikit`;
- create built-in Claude, Codex, or Gemini integrations (or any other
  assistant-specific integration);
- define provider routing or model routing;
- define governance, approval, sufficiency, or methodology logic;
- replace [`docs/agent-usage.md`](agent-usage.md);
- replace each command's built-in `--help`;
- permit automatic `git push`, `fetch`, or `pull` (`aikit` never touches remotes,
  and wrappers must not add remote operations on their own);
- permit automatic cleanup execution (`output clean --execute` is never
  automatic);
- permit hidden "latest anchor" selection (anchors are always explicit).

## Source Guidance

The example patterns below are synthesized from three sources, not invented:

- [`docs/agent-usage.md`](agent-usage.md) — the agent-agnostic usage contract,
  exit-code convention, blocked-state names, and per-command constraints.
- The installed `aikit --help` surface (root command plus every subcommand),
  which is the authoritative per-command reference.
- Three independent command audits (one each from the Claude, Codex, and Gemini
  perspectives) that classified every installed command as a strong, limited, or
  not-recommended candidate for wrapping.

The three audits agree on the high-value core (`repo doctor`, `batch start`,
`batch changed`, `review generate`, `scan secrets`, and `script check` as a
preflight) and on the sharp edges (`output clean --execute`, `script run`, and
broad/ignored scans need explicit confirmation; anchors are never auto-selected).
Where they differed — for example, how eager `repo init` and `inventory repo`
should be — this document takes the more conservative reading: prepare and
snapshot deliberately rather than automatically. The synthesis is summarized in
[Recommended Minimal Integration Set](#recommended-minimal-integration-set); the
audits are not quoted at length.

## Wrapper Design Rules

An external wrapper around `aikit` should follow these rules:

- **Use the installed `aikit` binary** from `PATH`. Do not call
  `./target/debug/aikit` or `cargo run` in real integrations; those forms are for
  developing `aikit` itself.
- **Prefer `--json`.** Parse the machine-readable output instead of scraping human
  text; `--json` is available on every command a wrapper is likely to call.
- **Preserve exact output paths.** When a command reports a `written` array (or a
  `bundle_path`/`anchor_path`), report those exact paths rather than guessing or
  reconstructing file names.
- **Preserve anchor paths and IDs explicitly.** Capture the anchor path and
  `anchor_id` from `batch start` and thread them through every anchor-consuming
  command.
- **Never assume the latest anchor.** `aikit` deliberately does not select one;
  wrappers must not invent a "latest anchor" abstraction either.
- **Surface exit code 3 blocked states.** A `blocked_*` state is a deliberate,
  named refusal (e.g. `blocked_repo_not_found`, `blocked_missing_anchor`,
  `blocked_invalid_anchor`, `blocked_path_escape`, `blocked_script_not_allowed`,
  `blocked_dirty_tree`, `blocked_secret_findings`). Report it; never swallow it.
- **Treat `scan secrets` as heuristic, not proof.** Findings are not proof of a
  live credential, and *no findings does not prove a file is safe to share*.
- **Treat `script run` as not a sandbox.** The allowed-location policy is the only
  real control; running a script does not make it safe.
- **Keep `output clean --execute` confirmation-gated.** It deletes local
  artifacts; it must never run automatically.
- **Keep broad scans, broad review bundles, and patch output confirmation-gated.**
  `scan secrets .`, `--include-ignored` scans, broad `review generate`/`inventory
  repo`, and `diff anchor --patch` can surface more content than intended.
- **Never hide truncation, omission, caps, or skipped files.** If a manifest
  records `truncated`, `omitted_reason`, `cap_hit`, or skipped files, surface it.
- **Never add provider/model/agent-specific behavior to `aikit`.** All
  assistant-specific logic belongs in the external wrapper layer.

## Recommended Minimal Integration Set

The smallest useful set of external integrations to build first:

- **Repo readiness:** `repo doctor`, with guarded `repo init`.
- **Task anchoring:** `batch start`, `batch changed`, with optional `batch list` /
  `batch show` as helpers.
- **Change inspection:** `diff anchor`, stat-oriented by default; patch output
  confirmation-gated.
- **Review packaging:** `review generate`, using explicit files or an explicit
  anchor.
- **Secret pre-check:** `scan secrets` over explicit paths before sharing or
  bundling for review.
- **Script workflow:** `script check` always before `script run`; `script run`
  only after explicit approval.
- **Diagnostics:** `env snapshot`, plus `output list` / `output show` as helpers.
- **On-demand snapshot:** `inventory repo` only for explicit audit/snapshot
  workflows, not routine automatic use.

## Example: Repository Readiness Wrapper

- **Problem solved:** confirm the repository is a Git repo and is prepared for
  local `aikit` usage before any other command runs.
- **Basis commands:** `aikit repo doctor --json`; guarded `aikit repo init
  --json`.
- **Inputs:** none beyond the current working directory.
- **Output to preserve:** `repo_root`, `git_branch`, `git_head`,
  `tracked_tree_clean`, `ready`, `warnings`, the `.aikit/temp/` existence flag,
  and (after `init`) what was created vs. already present.
- **Blocked states to surface:** `blocked_repo_not_found` (not inside a Git
  repository) — stop and report; do not fall back to ad-hoc shell.
- **Safety boundary:** `repo doctor` is read-only. `repo init` is idempotent but
  mutates local Git metadata (`.git/info/exclude`) and creates `.aikit/temp/`; it
  never edits `.gitignore` and never touches remotes.
- **Can run automatically?** `repo doctor` yes. `repo init` should be guarded —
  run it only when `doctor` reports a readiness gap, and prefer to surface that it
  will write local Git metadata.
- **Explicit confirmation required?** Not for `doctor`. For `init`, treat the
  local-metadata mutation as worth disclosing; run it on a clear readiness gap,
  not speculatively.

## Example: Task Anchor Wrapper

- **Problem solved:** mark a point in time before a unit of work so later steps
  can report what the work touched.
- **Basis commands:** `aikit batch start --json`; optional helpers `aikit batch
  list --json` and `aikit batch show <anchor> --json`.
- **Inputs:** optional `--output <dir>` to override the output root.
- **Output to preserve:** the `anchor_path` and `anchor_id`, plus the recorded
  branch/HEAD/timestamp. The wrapper must persist these for the rest of the task;
  `aikit` will not recall them for you.
- **Blocked states to surface:** `blocked_repo_not_found`; for `batch show`,
  invalid/cross-repo anchors and path escapes.
- **Safety boundary:** `batch start` writes a durable local artifact under
  `.aikit/outputs/batches/`; nothing is auto-cleaned. `list`/`show` are read-only.
- **Can run automatically?** `batch start` can run at the deliberate start of a
  bounded task; avoid creating many anchors for trivial read-only questions.
  `list`/`show` are safe to run automatically as helpers.
- **Explicit confirmation required?** No — but the wrapper must never auto-select
  a "latest" anchor from `batch list`; anchor choice stays explicit.

## Example: Change Reporting Wrapper

- **Problem solved:** report which files were modified since an explicit anchor
  (timestamp-based), and optionally show a mechanical diff.
- **Basis commands:** `aikit batch changed --anchor <anchor> --json` (optionally
  `--hash`); `aikit diff anchor <anchor> --json` (stat-oriented by default).
- **Inputs:** an **explicit** anchor path or id (from the Task Anchor Wrapper).
- **Output to preserve:** the file list, per-source counts, sizes, hashes (when
  `--hash`), and for `diff anchor` the base/current heads, name-status list, and
  stat.
- **Blocked states to surface:** `blocked_missing_anchor`,
  `blocked_invalid_anchor`, cross-repo rejection, and (for `diff anchor`) a
  recorded base head that no longer exists locally.
- **Safety boundary:** both are inspection-only and write no artifacts.
  `batch changed` untracked detection is a best-effort mtime heuristic; `diff
  anchor` does not include untracked file *contents*. Disclose both rather than
  implying completeness.
- **Can run automatically?** Yes, with an explicit anchor and stat-oriented diff
  output.
- **Explicit confirmation required?** Stat output, no. `diff anchor --patch`
  should be confirmation-gated because patches can be large or reveal sensitive
  content.

## Example: Review Bundle Wrapper

- **Problem solved:** package a bounded, hashed set of files into a single
  reviewable bundle for human or external-tool review.
- **Basis commands:** `aikit review generate --files <file>... --json` **or**
  `aikit review generate --anchor <anchor> --json` (exactly one mode per run).
- **Inputs:** either an explicit file list or an explicit anchor; optional caps
  (`--max-file-bytes`, `--max-file-lines`, `--max-total-bytes`) and `--output`.
- **Output to preserve:** the `written` paths (`review_bundle.txt` and
  `manifest.json`), `review_id`, `bundle_path`, and the per-file records including
  `truncated`, `omitted_reason`, and `cap_hit`.
- **Blocked states to surface:** path escapes (absolute/`..`/symlink), invalid or
  cross-repo anchors, and invalid usage when both or neither input mode is given
  (the precomputed `--changed <changed.json>` mode is not implemented).
- **Safety boundary:** writes durable local artifacts under
  `.aikit/outputs/reviews/`; output is local-only and must not be committed.
- **Can run automatically?** Yes for scoped, explicit inputs.
- **Explicit confirmation required?** Yes for broad anchors, large bundles, or
  suspected-sensitive paths. Never hide truncation or omitted files from the
  report.

## Example: Secret Scan Pre-Check

- **Problem solved:** a best-effort sweep for likely committed secrets before
  sharing files or building a review bundle.
- **Basis commands:** `aikit scan secrets <explicit paths> --json` (optionally
  `--fail-on-findings`).
- **Inputs:** one or more **explicit** repo-local paths. The whole repo is never
  scanned implicitly unless the path is the repo root or `.`.
- **Output to preserve:** finding counts and per-finding metadata — `path`,
  `line`, `rule_id`, `severity`, and `redacted: true` — plus the skipped-files
  list. Raw secret values are never emitted and must never be reconstructed.
- **Blocked states to surface:** `blocked_path_escape` for out-of-repo/symlink
  paths; `blocked_secret_findings` (exit 3) when `--fail-on-findings` is used and
  findings exist.
- **Safety boundary:** read-only, creates no artifacts, never prints raw values.
  Results are heuristic: a finding is not proof of a live credential, and clean
  output is not proof a file is safe to share.
- **Can run automatically?** Yes, on explicit, scoped paths — as a pre-bundle or
  pre-share step, reporting findings rather than acting on them as a verdict.
- **Explicit confirmation required?** Yes for `scan secrets .` and for
  `--include-ignored` scans, which inspect more local content than a scoped path.

## Example: Script Check and Script Run Workflow

- **Problem solved:** validate, then optionally execute, a constrained local
  helper script with a recorded audit trail — and consolidate many local checks
  behind a single top-level invocation.
- **Basis commands:** `aikit script check <script> --json` **always first**, then
  `aikit script run <script> --json` only after explicit approval.
- **Inputs:** a script under an allowed input location (`.aikit/temp/`,
  `.scratch/work/temp/`, or `.scratch/work/outputs/`). The runner is **detected**
  cross-OS, not fixed: supported extensions are `.sh`, `.zsh`, `.ps1`, `.cmd`, `.bat`,
  `.py`, `.js`, and detection order is explicit `--runner` → config
  `script_runner.extension_map` → recognized shebang (unless disabled) → built-in
  extension map → OS-aware fallback. On Windows, `.ps1` uses `pwsh`/`powershell` and
  `.cmd`/`.bat` use `cmd` (no Git Bash required). Optional `--require-clean` /
  `--allow-dirty`.
- **Output to preserve:** for `check`, `accepted` / `blocked_state` plus the runner
  metadata (`detected_runner`, `detection_source`, `used_shebang`, `used_extension_map`,
  resolved interpreter, `argv`). For `run`, the run directory and its `written` paths
  (copied script, `stdout.txt`, `stderr.txt`, `run.json` with the same runner metadata)
  and the propagated exit code.
- **Blocked states to surface:** `blocked_script_not_allowed` (disallowed location),
  `blocked_unknown_script_type` (unknown extension/no runner signal),
  `blocked_runner_not_found` (selected runner unavailable on this OS),
  `blocked_runner_not_allowed` (unknown `--runner` name), `blocked_path_escape`, and
  `blocked_dirty_tree` (under `--require-clean`).
- **Safety boundary:** `script run` is **not a security sandbox**; once execution
  starts the script can touch any path. The wrapper — not `aikit` — is responsible
  for keeping the script within the intended repository-local boundary. Do not
  wrap `script run` inside a larger shell batch when the goal is consolidating
  approvals.
- **Can run automatically?** `script check` yes (it never executes). `script run`
  **no** — never automatic.
- **Explicit confirmation required?** Yes for `script run`, every time, unless the
  user has already explicitly requested the exact reviewed script/workflow.

## Example: Diagnostic Snapshot Workflow

- **Problem solved:** capture bounded local environment and artifact state for
  debugging an `aikit` integration or for a bug report.
- **Basis commands:** `aikit env snapshot --json`; helpers `aikit output list
  --json` and `aikit output show <artifact> --json`.
- **Inputs:** none for `env snapshot`; optional `--family`/`--root` for `output
  list`, an explicit artifact path/id for `output show`.
- **Output to preserve:** aikit version, current executable, OS/arch, repo facts,
  interpreter availability, and local git/Rust/Cargo versions; for `output`, the
  artifact family/id/path and contained-file summaries.
- **Blocked states to surface:** ambiguous ids or out-of-root paths for `output
  show`.
- **Safety boundary:** all read-only. `env snapshot` deliberately omits the full
  environment, raw `PATH`, tokens, and keys — include it as-is, do not augment it
  with secrets.
- **Can run automatically?** Yes — all three are safe diagnostics.
- **Explicit confirmation required?** No.

## Helper Commands

These are useful supporting commands, not headline integrations. Wrap them thinly
and only in service of the patterns above:

- `aikit batch list` / `aikit batch show` — inspect anchors and validate a chosen
  one. Helpers only; never auto-select an anchor for downstream commands.
- `aikit output list` / `aikit output show` — discover and inspect local
  artifacts for reporting and housekeeping triage. Read-only.
- `aikit output clean --dry-run` — preview what cleanup *would* remove. Safe; the
  destructive `--execute` form is gated below.

## Commands Requiring Explicit Confirmation

A wrapper must obtain explicit confirmation before invoking any of these:

- `script run` — arbitrary local code execution; not a sandbox.
- `output clean --execute` — deletes local artifacts.
- `scan secrets .` — scans the whole repository rather than a scoped path.
- `scan secrets --include-ignored` — inspects ignored content too.
- broad `review generate` — large or sensitive anchor/file sets packaged into a
  bundle.
- broad `inventory repo` — especially `--include-ignored` or unbounded listings.
- `diff anchor --patch` — full patch text can be large or reveal sensitive
  content.

## Commands That Should Remain Manual

These should stay deliberate, human-driven actions — never frictionless,
never automatic:

- `output clean --execute` — destructive pruning of local artifacts.
- automatic "latest anchor" selection — anchor choice is always explicit.
- automatic script execution — `script run` is a deliberate, approved step.
- automatic remote Git operations — `aikit` never touches remotes, and wrappers
  must not add push/fetch/pull on their own.
- broad inventory or secret scans of ignored files — only when explicitly
  requested.

## Anti-Patterns

Avoid these wrapper designs:

- **Hiding blocked states.** Collapsing an exit-3 `blocked_*` refusal into a
  generic success/failure label hides a deliberate, named signal.
- **Silent "latest anchor" selection.** Picking an anchor for the user defeats the
  explicit-anchor safety property `aikit` enforces.
- **Treating `script run` as a sandbox.** It records and constrains *where the
  script lives*, not what it does once running.
- **Treating clean `scan secrets` output as proof of safety.** Absence of findings
  is not assurance; the scan is heuristic.
- **Auto-deleting local artifacts.** Cleanup via `output clean --execute` must be
  explicit; automatic deletion can destroy evidence the user still needs.
- **Committing `.aikit/outputs/` or `.scratch/`.** Generated output is local-only.
- **Adding Claude/Codex/Gemini-specific behavior to `aikit`.** Assistant-specific
  logic belongs in the external wrapper, never in the tool.
- **Changing `aikit` to satisfy one agent wrapper.** Keep the tool agent-agnostic;
  adapt the wrapper, not the CLI.

## Example Output Reporting

A concise template a wrapper can use to report each `aikit` invocation. Populate
only the fields that apply; never include raw secret values.

```text
aikit command:        <exact command run, e.g. `aikit review generate --files ... --json`>
exit code:            <0 | 1 | 2 | 3>
blocked state:        <blocked_* name, or none>
generated paths:      <exact paths from the `written` array, or none>
anchor path / id:     <anchor_path and anchor_id, if any>
review bundle path:   <bundle_path / review dir, if any>
scan findings:        <count, plus per-finding path:line rule_id severity (redacted), or none>
truncation/omission:  <files truncated / omitted / cap_hit / skipped, or none>
tracked tree state:   <clean | dirty (and what changed)>
```

## Summary

These are example patterns only. An external agent or tool can wrap the installed
`aikit` CLI by calling its commands, parsing `--json`, preserving exact output
paths and anchor identifiers, and surfacing every blocked state — while keeping
`script run`, `output clean --execute`, broad scans/bundles, and patch output
behind explicit confirmation. The high-value core to build first is repo
readiness, task anchoring, change reporting, review packaging, a secret
pre-check, and a `script check` → `script run` workflow, with diagnostics and an
on-demand inventory as supporting pieces. `aikit` itself stays agent-agnostic; the
installed CLI and [`docs/agent-usage.md`](agent-usage.md) remain the source of
command behavior.
