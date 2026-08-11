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
    /// `folder init` was run inside a Git/Mercurial repository, where it refuses to
    /// create an un-ignored `.aikit/`; use `repo init` or `init` instead.
    pub const REPO_PRESENT: &str = "blocked_repo_present";
    /// `--require-clean` was requested for a non-repo aikit root, where there is no
    /// VCS working tree to compare against.
    pub const REQUIRE_CLEAN_UNSUPPORTED: &str = "blocked_require_clean_unsupported";
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

    /// Every blocked state aikit can emit, as a closed set.
    ///
    /// This is the contract: a caller may match on these and treat an unlisted value as a
    /// protocol violation rather than a state to guess at. Adding one is a schema change,
    /// which is why they live in one list instead of being spelled inline at each throw
    /// site. `ALL` is asserted against the constants by a test, so a new constant that is
    /// not registered here fails the build's test run rather than shipping undocumented.
    ///
    /// Not referenced by the binary at runtime — nothing needs to enumerate blocked states
    /// to *emit* one. Its consumers are the contract test below and the published docs, so
    /// the dead-code warning is expected rather than a sign it should be deleted.
    #[allow(dead_code)]
    pub const ALL: &[&str] = &[
        REPO_NOT_FOUND,
        REPO_PRESENT,
        REQUIRE_CLEAN_UNSUPPORTED,
        MISSING_ANCHOR,
        INVALID_ANCHOR,
        PATH_ESCAPE,
        UNREADABLE_FILE,
        SCRIPT_NOT_ALLOWED,
        DIRTY_TREE,
        FORBIDDEN_OPERATION,
        ARTIFACT_NOT_FOUND,
        AMBIGUOUS_ARTIFACT,
        MISSING_BASE_COMMIT,
        SECRET_FINDINGS,
        UNKNOWN_SCRIPT_TYPE,
        RUNNER_NOT_FOUND,
        RUNNER_NOT_ALLOWED,
    ];
}

#[derive(Debug, Error)]
pub enum AikitError {
    /// A deterministic blocked state. `state` is one of the `blocked::*` constants.
    #[error("{state}: {message}")]
    Blocked {
        state: &'static str,
        message: String,
        /// Whether the command already wrote its own JSON record to stdout before failing.
        ///
        /// `scan secrets` and `script check` report a full record *and* exit non-zero: the
        /// record is the answer, and the non-zero exit is the gate. Emitting a second
        /// object after it would leave two JSON documents on stdout, which no caller can
        /// parse as one. This flag is what keeps stdout to exactly one document.
        json_emitted: bool,
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
            json_emitted: false,
        }
    }

    /// A blocked state raised *after* the command already printed its JSON record.
    ///
    /// Use only when a record really was written to stdout — see the field docs.
    pub fn blocked_after_json(state: &'static str, message: impl Into<String>) -> Self {
        AikitError::Blocked {
            state,
            message: message.into(),
            json_emitted: true,
        }
    }

    pub fn other(message: impl Into<String>) -> Self {
        AikitError::Other(message.into())
    }

    /// The named blocked state, or `None` for an ordinary failure.
    pub fn blocked_state(&self) -> Option<&'static str> {
        match self {
            AikitError::Blocked { state, .. } => Some(state),
            AikitError::Other(_) => None,
        }
    }

    /// The human-readable detail, without the `state:` prefix the `Display` impl adds.
    ///
    /// The JSON record carries `blocked_state` as its own field, so repeating it inside
    /// `message` would make a caller strip it back off to show the detail.
    pub fn detail(&self) -> &str {
        match self {
            AikitError::Blocked { message, .. } => message,
            AikitError::Other(message) => message,
        }
    }

    /// Whether stdout already carries this command's JSON record.
    pub fn json_emitted(&self) -> bool {
        matches!(
            self,
            AikitError::Blocked {
                json_emitted: true,
                ..
            }
        )
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
    ///
    /// Prose always goes to stderr, including under `--json`: stdout stays the machine
    /// channel, so a caller can parse it without first stripping human text.
    pub fn report(&self) {
        match self {
            AikitError::Blocked { state, message, .. } => eprintln!("{state}: {message}"),
            AikitError::Other(message) => eprintln!("error: {message}"),
        }
    }

    /// Print a structured failure record to **stdout**, for `--json` callers.
    ///
    /// Without this, `--json` produced nothing parseable exactly when a caller most needs
    /// structure — leaving it to scrape prose from stderr to find out what happened.
    /// `blocked_state` is null for an ordinary failure, which is how a caller tells a
    /// deterministic refusal (retryable only after changing something) from an I/O error.
    pub fn report_json(&self) {
        print_error_json(self.blocked_state(), self.exit_code(), self.detail());
    }
}

/// Print an `aikit.error` document to **stdout**.
///
/// Standalone rather than a method because not every failure that owes a caller a record is
/// an `AikitError`: a `--cwd` directory that cannot be entered is rejected in `main` before
/// any command runs, and carries exit code 2 rather than this type's 1. One function keeps
/// those emitters producing an identical shape.
pub fn print_error_json(blocked_state: Option<&str>, exit_code: i32, message: &str) {
    let record = serde_json::json!({
        "schema_version": crate::formats::SCHEMA_VERSION,
        "kind": crate::formats::KIND_ERROR,
        "ok": false,
        "blocked_state": blocked_state,
        "exit_code": exit_code,
        "message": message,
    });
    // Pretty-printed to match every other `--json` record; a caller parses either.
    //
    // A serialisation failure is ignored deliberately: it cannot realistically happen
    // for this fixed shape, and the stderr report has already carried the actual error.
    // Failing loudly here would replace a real diagnosis with a spurious one.
    if let Ok(text) = serde_json::to_string_pretty(&record) {
        println!("{text}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_blocked_constant_is_registered_in_all() {
        // The constants are the throw sites; `ALL` is the published contract. If they drift,
        // a caller matching exhaustively on the documented set silently loses a case.
        let src = include_str!("errors.rs");
        let declared: Vec<&str> = src
            .lines()
            .filter_map(|l| l.trim().strip_prefix("pub const "))
            .filter_map(|l| l.split_once(": &str = "))
            .map(|(_, v)| v.trim().trim_end_matches(';').trim_matches('"'))
            .filter(|v| v.starts_with("blocked_"))
            .collect();

        assert!(!declared.is_empty(), "no blocked_* constants parsed");
        for state in &declared {
            assert!(
                blocked::ALL.contains(state),
                "{state} is not registered in blocked::ALL"
            );
        }
        assert_eq!(
            declared.len(),
            blocked::ALL.len(),
            "blocked::ALL has entries that are not declared constants"
        );
    }

    #[test]
    fn error_json_names_the_blocked_state_and_exit_code() {
        let err = AikitError::blocked(blocked::PATH_ESCAPE, "outside the repo");
        assert_eq!(err.blocked_state(), Some(blocked::PATH_ESCAPE));
        assert_eq!(err.exit_code(), 3);
        assert!(!err.json_emitted());

        // An ordinary failure is distinguishable by a null blocked_state, not by prose.
        let other = AikitError::other("disk on fire");
        assert_eq!(other.blocked_state(), None);
        assert_eq!(other.exit_code(), 1);
    }

    #[test]
    fn blocked_after_json_suppresses_a_second_document() {
        let err = AikitError::blocked_after_json(blocked::SECRET_FINDINGS, "findings");
        assert!(err.json_emitted());
        assert_eq!(err.exit_code(), 3);
    }
}
