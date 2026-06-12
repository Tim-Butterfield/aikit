# Decision 0001: Create aikit as a Separate Personal CLI Repo

## Status

Accepted for initial repository creation.

## Context

- `architect-toolkit` is only one possible future consumer of these tools.
- Reusable AI-agent workflow tools should not be embedded inside one project.
- Previous projects used several related script/tool patterns (governed runners,
  review-bundle generation, repo inventory, timestamp-anchored change discovery),
  duplicated across efforts rather than shared.
- The architect prefers a compiled CLI rather than copied script files.
- Rust is currently preferred due to speed, correctness pressure, and
  single-binary deployability.

## Decision

- Create a separate private repo named `aikit`.
- Use binary name `aikit`.
- Use Rust as the preferred implementation technology.
- Begin with documentation/specification only.
- Do not implement yet.
- Do not push yet.

## Consequences

- `architect-toolkit` remains clean and does not own `aikit`.
- `aikit` can evolve as personal cross-project tooling.
- Implementation planning occurs in the `aikit` repo.
- Future agent skills, if any, wrap the CLI rather than replace it.
- Any later remote publication or push requires explicit architect action.
