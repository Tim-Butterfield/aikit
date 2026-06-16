//! Shared error and blocked-state types for aikit.
//!
//! Batch 1 keeps this intentionally small: a single error enum that distinguishes
//! deterministic *blocked states* (named, mechanical refusals → exit code 3) from
//! ordinary failures (exit code 1). Invalid CLI usage (exit code 2) is handled by
//! `clap` before these types are ever constructed.

use thiserror::Error;

/// Named blocked states. These are mechanical, deterministic refusals that callers
/// (human or AI agent) can match on. Batch 1 only needs the subset below; later
/// batches add more (path-escape, script-not-allowed, dirty-tree, …).
pub mod blocked {
    pub const REPO_NOT_FOUND: &str = "blocked_repo_not_found";
    pub const MISSING_ANCHOR: &str = "blocked_missing_anchor";
    pub const INVALID_ANCHOR: &str = "blocked_invalid_anchor";
    pub const PATH_ESCAPE: &str = "blocked_path_escape";
    pub const UNREADABLE_FILE: &str = "blocked_unreadable_file";
    pub const SCRIPT_NOT_ALLOWED: &str = "blocked_script_not_allowed";
    pub const DIRTY_TREE: &str = "blocked_dirty_tree";
    pub const FORBIDDEN_OPERATION: &str = "blocked_forbidden_operation";
    pub const ARTIFACT_NOT_FOUND: &str = "blocked_artifact_not_found";
    pub const AMBIGUOUS_ARTIFACT: &str = "blocked_ambiguous_artifact";
    pub const MISSING_BASE_COMMIT: &str = "blocked_missing_base_commit";
    pub const SECRET_FINDINGS: &str = "blocked_secret_findings";
    /// A script's extension is unknown and no other runner signal (explicit/shebang) applied.
    pub const UNKNOWN_SCRIPT_TYPE: &str = "blocked_unknown_script_type";
    /// A runner was selected but its program is not installed/available on this OS.
    pub const RUNNER_NOT_FOUND: &str = "blocked_runner_not_found";
    /// An explicit `--runner` named a value that is not a recognized runner.
    pub const RUNNER_NOT_ALLOWED: &str = "blocked_runner_not_allowed";
}

#[derive(Debug, Error)]
pub enum AikitError {
    /// A deterministic blocked state. `state` is one of the `blocked::*` constants.
    #[error("{state}: {message}")]
    Blocked {
        state: &'static str,
        message: String,
    },
    /// An ordinary, non-blocked failure (I/O, serialization, …).
    #[error("{0}")]
    Other(String),
}

impl AikitError {
    pub fn blocked(state: &'static str, message: impl Into<String>) -> Self {
        AikitError::Blocked {
            state,
            message: message.into(),
        }
    }

    pub fn other(message: impl Into<String>) -> Self {
        AikitError::Other(message.into())
    }

    /// Process exit code per the plan's exit-code policy.
    pub fn exit_code(&self) -> i32 {
        match self {
            AikitError::Blocked { .. } => 3,
            AikitError::Other(_) => 1,
        }
    }

    /// Print the error to stderr. Blocked states are printed as `state: message`
    /// so the named state is greppable in tooling and logs.
    pub fn report(&self) {
        match self {
            AikitError::Blocked { state, message } => eprintln!("{state}: {message}"),
            AikitError::Other(message) => eprintln!("error: {message}"),
        }
    }
}
