# AGENTS.md — using aikit

aikit performs deterministic, local, mechanical operations that support AI-agent and
human-in-the-loop workflows. It calls no AI providers, performs no model or provider logic,
and has no knowledge of any specific agent, CLI, slash command, or model.

This file is embedded in the `aikit` binary. Over MCP, call the `agents_md` tool to read it;
from a shell, run `aikit agents-md`. Either way the text matches the binary you are running.

## Two ways to reach aikit

**As a CLI.** Every command is a subcommand of `aikit`. Output is machine-readable where
useful — pass `--json` to any command that supports it and parse that rather than the
human-readable text, which is not a stable interface.

**As an MCP server** (`aikit mcp`). This exposes three tools and nothing else: `run`,
`list_runners`, and `agents_md`. If `aikit` is on PATH you can still reach the full CLI from
inside a `run` call.

## Rules that are expensive to get wrong

These are the mistakes worth naming explicitly, because each looks like success:

- **Check `exit_code_source` before trusting `exit_code == 0`.** `interpreter` means the
  interpreter propagates the status, so 0 means success. `epilogue` means aikit appended the
  propagation — reliable for native commands, but a failing *cmdlet* or an early `exit`
  bypasses it. `unpropagated` (PowerShell/cmd under `script run`, where aikit runs your own
  file and cannot modify it) means **0 tells you it ran, not that it succeeded**.
- **`containment` names what would kill the script's descendants** — `job_object` on
  Windows, `process_group` on Unix — because "kills the process tree" is not unconditionally
  true. A process using explicit breakaway (Windows) or `setsid` (Unix) escapes either.
- **A non-zero exit code is a SUCCESSFUL call.** `run` reports it in `exit_code`. It is not
  a tool error; do not retry on it.
- **On Windows, `exit_code == 0` is weak evidence.** `cmd` batch files propagate ERRORLEVEL
  lossily, and `powershell`/`pwsh -File` exit 0 when a native command failed unless the
  script checks `$LASTEXITCODE`. Check the output, not just the code.
- **The runner is required and never inferred.** `pwsh` and `powershell` execute the same
  `.ps1` differently, so aikit refuses to guess. Call `list_runners` and pass an exact name.
  A runner that is not installed is an error, never a silent substitution to a sibling.
- **Cancelling a call does not stop the script.** If your client delivers the cancellation
  the process tree is killed, but clients are not required to send it — many simply stop
  waiting, and the script runs to completion. The timeout is the only guaranteed stop, which
  is why it cannot be disabled, only adjusted.
- **Run records are pruned.** `script run` keeps the newest `output.retain_runs` run
  directories (default 100; `0` disables) and deletes older ones, reporting the count as
  `pruned_runs`. Only `runs/` is pruned. Do not assume an old run directory still exists.
- **`aikit config show`** reports every effective setting and the layer that set it. Use it
  instead of guessing why a value is what it is. It describes the CLI only — the MCP server
  consults no configuration.
- **Every command takes `--cwd <PATH>`.** Use it instead of navigating the shell; quoting
  and `cd` semantics differ per platform and a mistake there looks like an aikit failure.
- **`.aikit/` is local state, never committed.** Ignore coverage uses the VCS's local,
  never-committed mechanism (`.git/info/exclude`, or a `.hg/hgrc`-registered
  `.hg/hgignore.aikit`). Tracked ignore files are never modified.
- **Blocked states are deliberate refusals, not failures to work around.** They exit 3 with
  a named `blocked_*` reason. Surface the reason; do not fall back to ad-hoc shell.
- **`--json` gives you one JSON document on stdout even when the command fails.** Parse
  stdout; never scrape stderr, which carries the human text. A failure is
  `kind: aikit.error` with `blocked_state` (a named state, or `null` for an ordinary
  failure), `exit_code`, and `message`. The blocked states are a closed set — treat an
  unlisted one as a protocol violation. Exit codes: 0 success, 1 failure, 2 invalid usage,
  3 blocked state. Exit 2 usually carries no record because the parser exits before aikit
  runs; the exception is a `--cwd` directory aikit cannot enter, which does emit one. `scan secrets` and
  `script check` report their own record instead, with `blocked_state` inside it.

## MCP tools

**`list_runners`** — returns every supported runner with `applicable` (does it apply to this
OS), `available` (was the interpreter found), plus `path`, `version`, and a `reason` token
(`available` | `available_via_alias` | `not_found_on_path` | `not_applicable_on_this_os`).
Use `version` to tell `powershell` 5.1 from `pwsh` 7 — they differ in output encoding and
exit-code behaviour. `available_via_alias` means only a Windows App Execution Alias was
found: sometimes a working Store install, sometimes a stub that exits 9009. Call this before
`run`. Read-only.

**`run`** — executes a script supplied in the call. Required: `runner`, `script`, `cwd`
(absolute path to an existing directory). Optional: `stdin`, `env`, `env_base`
(`inherit` | `minimal`), and `limits` (`timeout_ms` default 120000 max 3600000, `max_bytes`
default 32 MiB, `on_output_limit` `truncate` | `kill`). Unknown keys are rejected rather
than ignored, so a typo like `timeoutMs` is an error rather than a silently discarded bound.

`run` is a **different contract from `aikit script run`**, not a wrapper around it: the
script arrives as call arguments rather than as a file you author, so nothing is written
into the repository or working tree, no repository is required, `.aikit/` is not used, no
run record is produced, and the dirty check does not apply. Configuration is not consulted.

**`agents_md`** — returns this document.

## CLI commands

Run `aikit <command> --help` for the full contract of any of these.

**Setup and readiness**
- `aikit doctor` — read-only readiness report. Run this *before* `init` to find out whether
  setup is needed; it reports rather than failing in a directory that was never initialized.
  Add `--require-root` to make the absence of a root an error instead.
- `aikit init` — adaptive setup: inside a Git or Mercurial repository it creates `.aikit/`
  and `.aikit/temp/` and adds local ignore coverage; outside one it creates the directories
  only. Idempotent. `--require-repo` demands a repository; `--require-folder` demands a
  non-repo directory.

**Change tracking**
- `aikit batch start` — create an anchor recording the current state.
- `aikit batch changed --anchor <anchor.json>` — report what changed since an anchor.
- `aikit batch list` / `aikit batch show` — inspect existing anchors.
- `aikit batch diff` — mechanical Git diff report from an anchor.

**Inspection**
- `aikit inventory repo` — mechanical inventory of repository files.
- `aikit review generate` — bounded review bundle from explicit files or an anchor.
- `aikit env snapshot` — mechanical local environment facts.
- `aikit scan secrets` — best-effort heuristic scan of explicit paths. Both false-positive
  and false-negative; it does not prove a credential is live, and finding nothing does not
  prove a repository is safe to share.

**Scripts**
- `aikit script check` — validate a script without running it.
- `aikit script run <path>` — run a repo-local script under mechanical safety controls, with
  a run record written under `.aikit/outputs/runs/<id>/`. Use this when you want an audit
  trail; use the MCP `run` tool when you want no local footprint.

**Artifacts**
- `aikit output list` / `show` / `clean` — manage local aikit output artifacts.

**Other**
- `aikit version` — version and build metadata.
- `aikit mcp` — serve the script runner over MCP (this is the server, not a client command).

## Recommended loop

1. `aikit doctor --json` — check readiness. It answers even in an uninitialized directory.
2. `aikit init` — only if doctor reports a gap. It writes local VCS metadata, so prefer to
   surface that rather than running it silently.
3. `aikit doctor --json` — confirm `ready` is now true.

For a unit of work: `aikit batch start`, make changes, then
`aikit batch changed --anchor <anchor.json>` to report exactly what moved.

## Safety posture

aikit is **not a sandbox**. `aikit script run` and the MCP `run` tool both execute arbitrary
code with the privileges of the aikit process. The forbidden-pattern scan (`git push`,
`sudo`, `rm -rf /`, and similar) is an accident guard against a plausible mistake, not a
security boundary — it is naive substring matching and trivially evaded.

Over MCP, approval is the client's responsibility: aikit adds no prompt of its own and cannot
tell whether your client asked. A client configured to auto-approve tools has auto-approved
arbitrary local code execution.

The generated script does briefly reach disk: it is written under the system temp location,
in a directory created private to the user, and deleted afterwards on a best-effort basis. Do
not put a secret in a script body on the assumption that it vanishes.
