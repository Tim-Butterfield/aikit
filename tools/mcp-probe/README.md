# MCP client probe

A diagnostic MCP server that answers **"what does my MCP client actually do?"**

MCP leaves several behaviours to the client, and clients differ. Before relying on any MCP
server — including `aikit mcp` — it is worth knowing whether yours:

- forwards `structuredContent` into the model's context, or only `content`
- shows the model anything for an `isError` result with an empty content array
- delivers `notifications/cancelled` to the server when a call is abandoned
- imposes its own per-request timeout
- honours `notifications/progress`
- runs your server inside a Job Object (Windows), which affects process-tree cleanup

Each tool isolates one behaviour and has a control beside it, so a difference is
attributable rather than guessed at.

Nothing here is specific to any vendor. Paths are never assumed: you supply them.

## Requirements

Python 3.8+. No third-party packages.

## 1. Check the probe itself

```
python3 tools/mcp-probe/selftest.py
```

This drives `probe_server.py` directly, with no MCP client involved. If it passes, any
difference you later see through a real client is the *client's* behaviour — which is the
point. If it fails, stop: the probe is broken, not your client.

## 2. Register the probe with your client

Use whatever mechanism your client provides. The command is your Python interpreter and the
absolute path to `probe_server.py`. A typical stdio-server config entry looks like:

```json
{
  "mcpServers": {
    "probe": {
      "command": "python3",
      "args": ["/absolute/path/to/tools/mcp-probe/probe_server.py"],
      "transport": "stdio"
    }
  }
}
```

Where that file lives is client-specific — consult your client's documentation. Many CLIs
also offer an `mcp add` subcommand.

> Most clients ignore MCP servers supplied per-session and use only their own configuration.
> If the probe's tools never appear, that is usually why.

## 3. Ask your client to call the tools

Any client will do — IDE, chat UI or CLI. Ask it to call `probe_structured`, `probe_both`,
`probe_error_empty` and `probe_error_text`, and to report **verbatim** what it received for
each, saying explicitly whether anything looked empty.

For ACP-capable agent CLIs there is a driver that does this non-interactively:

```
python3 tools/mcp-probe/acp_driver.py --command /path/to/your-agent --arg acp --suite delivery
python3 tools/mcp-probe/acp_driver.py --command /path/to/your-agent --arg --acp --suite env
python3 tools/mcp-probe/acp_driver.py --command ... --suite timeout --seconds 240 --no-progress
python3 tools/mcp-probe/acp_driver.py --command ... --suite cancel --seconds 120
```

`--arg` is repeatable and is whatever flag or subcommand puts your binary into ACP mode;
different CLIs use different ones. The driver approves only probe-related permission
requests and refuses everything else.

## 4. Compare against the wire

The probe appends every message, both directions, to a log file. Default location is
`aikit-probe.log` in your OS temp directory — never inside a checkout. Override with
`$AIKIT_PROBE_LOG`; the path is printed to stderr at startup and returned by `probe_env`.

Read the log rather than trusting the client's narration. If the model says a result was
empty, the log shows exactly what the server sent, and the disagreement is the finding.

## Tools

| Tool | What it tells you |
|---|---|
| `probe_echo` | sanity check |
| `probe_structured` | `structuredContent` only, empty `content` — does it reach the model? |
| `probe_both` | control: same payload plus a text block |
| `probe_error_empty` | `isError` with empty `content` — is any error visible? |
| `probe_error_text` | control: `isError` with a text message |
| `probe_slow` | client timeout; `emit_progress: false` isolates whether progress matters |
| `probe_env` | OS, PID, TTY, console, Windows Job Object membership, log path |

## Interpreting the results

- **`probe_structured` empty, `probe_both` readable** → your client does not forward
  `structuredContent` to the model. Any server you use must always send a text block.
- **`probe_error_empty` shows nothing** → an error carrying only `structuredContent` is
  invisible; errors need text too.
- **`probe_slow` completes** → no short per-request timeout at that duration. Re-run with
  `--no-progress` to learn whether progress is what kept it alive.
- **Cancel suite reports no cancellation** → the client abandons calls without telling the
  server, so only a server-side timeout can stop work, and an abandoned call keeps running.
- **`windows_in_job_object: true`** → your server is already inside a Job Object, so nested
  job assignment for process-tree kills can fail unless breakaway is handled.
