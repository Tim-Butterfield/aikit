# Security Policy

## Supported versions

`aikit` is early, single-author software. Only the current `main` branch is
supported for now. There are no long-term support branches or backported fixes.

## Reporting a vulnerability

If you believe you have found a security issue in `aikit`:

- **Preferred:** use GitHub's private vulnerability reporting for this repository,
  if it is enabled (Security tab → "Report a vulnerability"). This keeps the
  report private until a fix is available.
- **Otherwise:** open a minimal public issue that does **not** include sensitive
  details, simply asking for a private contact path. Do not include the specifics
  of the vulnerability in that public issue.

Please do **not** post secrets, tokens, exploit payloads, or sensitive repository
data in public issues or pull requests.

## Scope and limitations

- `aikit` is a **local CLI** for deterministic, AI-assisted repository workflows.
  It is **not a security sandbox**. The governed script runner (`aikit script run`)
  reduces *accidental* unsafe execution; it does not make an arbitrary script safe.
- `aikit scan secrets` is a **best-effort heuristic** scan. It can both
  false-positive and false-negative, it does not prove a credential is live, and
  the absence of findings does **not** prove a repository is safe to share. It does
  not replace dedicated secret-scanning tools.
- `aikit` calls no AI providers and performs no network operations as part of its
  core behavior.

## `aikit mcp`

`aikit mcp` exposes the script runner to an AI agent over the Model Context
Protocol. Understand its posture before enabling it.

- **It runs arbitrary scripts** with the privileges of the `aikit` process. The
  script is supplied by the caller in the tool call. It is **not a sandbox**, and
  neither the forbidden-pattern scan nor anything else in aikit constrains what a
  script can do once it runs. The scan is an accident guard — naive substring
  matching, trivially evaded — not containment.
- **Approval is the client's job, not aikit's.** Most MCP clients ask the user to
  confirm each tool call; aikit adds no prompt of its own and cannot tell whether
  your client asked. If you configure a client to auto-approve tools, you have
  auto-approved arbitrary local code execution. Some clients only expose MCP tools
  in modes that already permit edits.
- **It ignores the CLI's guardrails by design.** No repository, no `.aikit/`
  directory, no allowed-location allowlist and no working-tree dirty check apply.
  A script may run in any absolute directory the caller names.
- **It is not an audit trail.** No run record and no logs are written. What remains
  is whatever the client chose to retain of the call, which aikit does not control.
  If you need a durable record of what was run, use `aikit script run`, which writes
  one.
- **The generated script briefly reaches disk.** It is written under the system temp
  location, in a directory and file that are *created* owner-only rather than created
  and then narrowed — so there is no window in which they are readable in a
  world-writable place. On Windows no ACL is applied; the per-user temp directory is
  the only protection there. Deletion when the call ends is **best-effort**: a scanner
  or a surviving grandchild holding a handle can defer it. Do not treat the script
  body as a safe place for a secret.
- **Cancelling a call does not stop the script.** Clients are not required to tell
  a server that a call was abandoned, so the run continues to completion or until
  its timeout — which is why the timeout cannot be disabled.
- **Only enable it for agents you would already trust with a shell.**
