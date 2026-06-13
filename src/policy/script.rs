//! Script-runner policy: allowed input locations, interpreter selection, and a
//! best-effort forbidden-operation scan.
//!
//! IMPORTANT: this is not a security sandbox. The forbidden-operation scan is naive
//! substring matching — trivially evaded (aliases, variables, whitespace, encoding)
//! and prone to false positives. The allowed-location allowlist is the primary
//! control; the scan only guards against obvious accidental mistakes. Running a
//! script through `aikit run script` does not make it safe.

use std::path::Path;

use crate::errors::{blocked, AikitError};

/// Repo-relative subtrees a script may be read from. These are *input* locations,
/// not output locations (run records default to `.aikit/outputs/runs/`).
pub const ALLOWED_SCRIPT_DIRS: &[&str] = &[
    ".aikit/temp/",
    ".scratch/work/temp/",
    ".scratch/work/outputs/",
];

/// Best-effort forbidden textual patterns. Easily bypassed; not a security boundary.
pub const FORBIDDEN_PATTERNS: &[&str] = &[
    "git push",
    "git fetch",
    "git pull",
    "gh repo create",
    "gh repo delete",
    "sudo",
    "rm -rf /",
];

/// Map a script's extension to its interpreter. Only `.zsh` and `.sh` are supported;
/// extensionless or unknown-extension scripts are rejected (the interpreter comes
/// from the extension, never from a shebang).
pub fn interpreter_for(rel: &str) -> Result<&'static str, AikitError> {
    match Path::new(rel).extension().and_then(|e| e.to_str()) {
        Some("zsh") => Ok("/bin/zsh"),
        Some("sh") => Ok("/bin/sh"),
        _ => Err(AikitError::blocked(
            blocked::UNSUPPORTED_MODE,
            format!(
                "unsupported or missing script extension for {rel:?}: only .zsh (/bin/zsh) and .sh (/bin/sh) are supported"
            ),
        )),
    }
}

/// Whether a repo-relative (forward-slash) path lies under an allowed script dir.
pub fn is_in_allowed_location(rel: &str) -> bool {
    ALLOWED_SCRIPT_DIRS.iter().any(|d| rel.starts_with(d))
}

/// Return the first forbidden pattern found in the script content, if any. This is
/// a best-effort substring scan, not a security check.
pub fn scan_forbidden(content: &str) -> Option<&'static str> {
    FORBIDDEN_PATTERNS
        .iter()
        .copied()
        .find(|p| content.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpreter_mapping() {
        assert_eq!(interpreter_for(".aikit/temp/x.zsh").unwrap(), "/bin/zsh");
        assert_eq!(interpreter_for(".aikit/temp/x.sh").unwrap(), "/bin/sh");
        assert!(interpreter_for(".aikit/temp/x").is_err());
        assert!(interpreter_for(".aikit/temp/x.py").is_err());
    }

    #[test]
    fn allowed_locations() {
        assert!(is_in_allowed_location(".aikit/temp/x.sh"));
        assert!(is_in_allowed_location(".scratch/work/temp/x.sh"));
        assert!(is_in_allowed_location(".scratch/work/outputs/x.sh"));
        assert!(!is_in_allowed_location("src/x.sh"));
        assert!(!is_in_allowed_location(".aikit/outputs/x.sh"));
    }

    #[test]
    fn forbidden_scan() {
        assert_eq!(scan_forbidden("echo hi\ngit push\n"), Some("git push"));
        assert_eq!(scan_forbidden("echo ok\n"), None);
    }
}
