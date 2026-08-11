# Changelog

Notable changes to aikit, following [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Nothing has been released yet: there are no tags and no GitHub releases, and `Cargo.toml`
sets `publish = false`. Install from the repository as the README describes. Until a first
release is cut, the entry below records what the current `main` provides.

## [Unreleased]

### Added

- **Governed script runner.** `aikit script run` executes a script from an allowed local
  work area and records what ran: the interpreter it detected, how that detection was made,
  and how far the resulting exit code can be trusted. Run records are written atomically and
  pruned to `output.retain_runs`.
- **Batch and anchor lifecycle.** `batch start`, `batch changed`, `batch show` and
  `batch diff`, with anchors addressable by id or repo-relative path.
- **Secret scanning** with severity thresholds, gated by `--fail-on`.
- **Repository and folder setup.** `init` with `--require-repo|folder|root`, plus a
  marker-based `doctor`.
- **MCP server.** `aikit mcp` serves `run`, `list_runners` and `agents_md` over stdio. It is
  a separate contract from `script run`: the script arrives in the call, nothing is written
  into a repository, no repository is required, and no configuration is consulted.
- **Machine-readable failures.** `--json` yields exactly one document on stdout whether a
  command succeeds or fails; `blocked_state` is a closed, published set.
- **Multi-VCS support** for Git and Mercurial.

[Unreleased]: https://github.com/Tim-Butterfield/aikit/commits/main
