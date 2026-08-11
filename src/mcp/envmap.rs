//! Environment construction for the MCP `run` tool.
//!
//! `env_base` is `"inherit"` (default) or `"minimal"`. Minimal does not mean *empty*: it
//! means a curated floor under which a normal shell works, not merely one under which
//! process creation succeeds. Omitting `PATH` would make every command in the script fail
//! with "not recognized" — an error that looks nothing like its cause, which is exactly
//! what the floor exists to prevent.
//!
//! Floor names are **immutable**: a `null` value cannot remove them. If `null` could delete
//! `SystemRoot` or `ComSpec` the floor would guarantee nothing.

use std::collections::BTreeMap;

use tokio::process::Command;

/// Variables preserved by `env_base: "minimal"`.
pub const MINIMAL_FLOOR: &[&str] = &[
    // Windows process creation and shell basics.
    "SystemRoot",
    "windir",
    "ComSpec",
    "PATHEXT",
    // A working shell needs to find programs and modules.
    "PATH",
    "PSModulePath",
    // Temp and profile locations that ordinary tooling assumes.
    "TEMP",
    "TMP",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "ProgramFiles",
    "ProgramData",
    // POSIX equivalents.
    "HOME",
    "SHELL",
    "LANG",
    "LC_ALL",
    "TZ",
];

/// Whether two environment variable names refer to the same variable on this platform.
///
/// Windows environment names are case-insensitive, so `path` and `PATH` are one variable and
/// must be treated as such — otherwise `path` could shadow the floor's `PATH`. On POSIX they
/// are two genuinely different variables, and folding them would silently discard one.
fn same_name(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// Whether `name` is part of the immutable floor.
pub fn is_floor(name: &str) -> bool {
    MINIMAL_FLOOR.iter().any(|f| same_name(f, name))
}

/// Apply the caller's environment overlay to a command.
///
/// `overrides` maps a name to `Some(value)` (set) or `None` (remove). Removal never applies
/// to a floor name under `minimal`. Names that the *platform* considers the same variable
/// are collapsed before the command is spawned, so on Windows a caller cannot smuggle two
/// spellings of one variable — and on POSIX two spellings stay two variables.
pub fn apply(cmd: &mut Command, overrides: &BTreeMap<String, Option<String>>, minimal: bool) {
    if minimal {
        cmd.env_clear();
        for name in MINIMAL_FLOOR {
            if let Some(val) = std::env::var_os(name) {
                cmd.env(name, val);
            }
        }
    }

    // Collapse duplicates the OS considers the same variable, last spelling wins, so the
    // child never sees two names that resolve to one.
    let mut resolved: Vec<(String, Option<String>)> = Vec::new();
    for (name, value) in overrides {
        if let Some(slot) = resolved
            .iter_mut()
            .find(|(existing, _)| same_name(existing, name))
        {
            slot.1 = value.clone();
        } else {
            resolved.push((name.clone(), value.clone()));
        }
    }

    for (name, value) in resolved {
        match value {
            Some(v) => {
                cmd.env(&name, v);
            }
            None => {
                if minimal && is_floor(&name) {
                    // Immutable: the floor is a guarantee, not a default.
                    continue;
                }
                cmd.env_remove(&name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_matching_follows_platform_case_rules() {
        assert!(is_floor("PATH"));
        assert!(is_floor("SystemRoot"));
        assert!(!is_floor("AIKIT_NOT_A_FLOOR_VAR"));

        // `path` IS `PATH` on Windows and is NOT on POSIX. Folding case everywhere would
        // make `env: {"path": "..."}` collide with the floor on Linux and macOS, where the
        // two are unrelated variables.
        assert_eq!(is_floor("path"), cfg!(windows));
        assert_eq!(is_floor("systemroot"), cfg!(windows));
    }

    #[test]
    fn distinct_posix_names_are_not_collapsed() {
        // Regression guard: a case-insensitive collapse on POSIX silently discards one of
        // two genuinely different variables.
        let mut overrides = BTreeMap::new();
        overrides.insert("aikit_probe".to_string(), Some("lower".to_string()));
        overrides.insert("AIKIT_PROBE".to_string(), Some("upper".to_string()));

        let mut cmd = Command::new("does-not-need-to-run");
        apply(&mut cmd, &overrides, false);

        let seen = cmd.as_std().get_envs().count();
        assert_eq!(seen, if cfg!(windows) { 1 } else { 2 });
    }

    #[test]
    fn floor_includes_path_and_psmodulepath() {
        // Regression guard: a floor that only makes process creation work, rather than one
        // under which a normal shell works, is the defect this list exists to prevent.
        assert!(MINIMAL_FLOOR.contains(&"PATH"));
        assert!(MINIMAL_FLOOR.contains(&"PSModulePath"));
        assert!(MINIMAL_FLOOR.contains(&"ComSpec"));
    }
}
