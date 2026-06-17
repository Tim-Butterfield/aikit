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
