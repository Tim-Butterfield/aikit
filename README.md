# aikit

`aikit` is a personal compiled CLI for deterministic AI-agent workflow support.

## Status

- Personal tool — built primarily for the architect's own use.
- Private repo — not currently intended for public distribution, and may never be.
- Not yet pushed to any remote.
- No implementation yet.
- Specification-first: this repo currently contains documentation and design
  decisions only.

## Purpose

`aikit` supports AI-agent workflows with **deterministic local operations**:

- batch anchoring — mark a point in time before AI-agent work begins;
- change discovery — report what was created/modified since an anchor;
- review bundle generation — produce a bounded, hashed review surface;
- repo inventory — generate a mechanical inventory of the repository;
- governed script execution — run local scripts under explicit policy controls.

## Non-Goals

`aikit` is:

- **not** an autonomous agent;
- **not** a methodology validator;
- **not** a governance judge;
- **not** a provider/model router;
- **not** a remote execution framework;
- **not** a replacement for Git;
- **not** a copied collection of old scripts.

## Initial Command Families

- `aikit run script`
- `aikit batch start`
- `aikit batch changed`
- `aikit review generate`
- `aikit inventory repo`

## Implementation Direction

- Rust.
- One compiled binary named `aikit`.
- Subcommand-based.
- Machine-readable output where useful.
- No runtime dependency on shell, Python, or Node for core behavior.

## Development Posture

`aikit` is developed with a deliberately **lightweight posture**:

- Specification-first — design decisions are settled in docs before code is written.
- Minimal moving parts — one binary, no sprawling runtime dependencies.
- Incremental — build the smallest useful slice, validate it, then grow.
- Low ceremony — avoid heavyweight process, frameworks, or tooling that the
  scope does not yet justify.
- Reversible — favor choices that are easy to revisit as the tool matures.

## Relationship to Architect Toolkit

- Separate repo: `aikit` is **not** part of Architect Toolkit.
- Architect Toolkit may consume `aikit` later, but is only one possible future
  consumer.
- `aikit` is **not** Architect Toolkit-specific.

## Current State

Specification-first. No source code, no `src/`, no Rust scaffolding yet.
See [`docs/aikit-cli-spec.md`](docs/aikit-cli-spec.md) for the CLI specification and
[`docs/decisions/0001-create-aikit.md`](docs/decisions/0001-create-aikit.md) for the
repo-creation decision.
