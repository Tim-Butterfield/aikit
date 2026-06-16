//! Script-runner policy: allowed input locations, cross-OS runner detection, and a
//! best-effort forbidden-operation scan.
//!
//! IMPORTANT: this is not a security sandbox. The forbidden-operation scan is naive
//! substring matching — trivially evaded (aliases, variables, whitespace, encoding)
//! and prone to false positives. The allowed-location allowlist is the primary
//! control; the scan only guards against obvious accidental mistakes. Running a
//! script through `aikit script run` does not make it safe.
//!
//! Runner detection is deterministic and OS-aware. The selection order is:
//!   1. explicit `--runner <name>` override,
//!   2. config `script_runner.extension_map` for the extension,
//!   3. a recognized `#!` shebang line (when enabled),
//!   4. the built-in extension map,
//!   5. an OS-aware default fallback (candidate resolution is itself OS-aware), then
//!   6. a clear blocked failure.
//!
//! Within tiers 2/4, `script_runner.preferred_runners` reorders candidates.

use std::path::Path;

use crate::config::ScriptRunnerConfig;
use crate::errors::{blocked, AikitError};

/// Repo-relative subtrees a script may be read from. These are *input* locations,
/// not output locations (run records default to `.aikit/outputs/runs/`).
pub const ALLOWED_SCRIPT_DIRS: &[&str] = &[
    ".aikit/temp/",
    ".scratch/work/temp/",
    ".scratch/work/outputs/",
];

/// Best-effort forbidden textual patterns. Easily bypassed; not a security boundary.
/// These are OS-agnostic substring checks (git/gh operations work the same on every
/// platform); the Unix-shaped entries are harmless to match on other OSes.
pub const FORBIDDEN_PATTERNS: &[&str] = &[
    "git push",
    "git fetch",
    "git pull",
    "gh repo create",
    "gh repo delete",
    "sudo",
    "rm -rf /",
];

/// A recognized runner: its symbolic name, the program candidates to resolve on PATH
/// (in order), and OS constraints. Candidates may be absolute (`/bin/sh`) or bare names
/// resolved via PATH (`bash`).
struct RunnerSpec {
    name: &'static str,
    candidates: &'static [&'static str],
    windows_only: bool,
    unix_only: bool,
}

const RUNNERS: &[RunnerSpec] = &[
    RunnerSpec {
        name: "sh",
        candidates: &["/bin/sh", "sh"],
        windows_only: false,
        unix_only: false,
    },
    RunnerSpec {
        name: "bash",
        candidates: &["bash", "/bin/bash"],
        windows_only: false,
        unix_only: false,
    },
    RunnerSpec {
        name: "zsh",
        candidates: &["/bin/zsh", "zsh"],
        windows_only: false,
        unix_only: false,
    },
    RunnerSpec {
        name: "pwsh",
        candidates: &["pwsh"],
        windows_only: false,
        unix_only: false,
    },
    RunnerSpec {
        name: "powershell",
        candidates: &["powershell"],
        windows_only: true,
        unix_only: false,
    },
    RunnerSpec {
        name: "cmd",
        candidates: &["cmd"],
        windows_only: true,
        unix_only: false,
    },
    RunnerSpec {
        name: "python3",
        candidates: &["python3"],
        windows_only: false,
        unix_only: false,
    },
    RunnerSpec {
        name: "python",
        candidates: &["python"],
        windows_only: false,
        unix_only: false,
    },
    RunnerSpec {
        name: "node",
        candidates: &["node"],
        windows_only: false,
        unix_only: false,
    },
];

/// The outcome of runner detection.
#[derive(Debug)]
pub struct Detection {
    /// Symbolic runner name (`sh`, `pwsh`, `python3`, …).
    pub runner: String,
    /// Resolved program path/name used to invoke the script.
    pub program: String,
    /// Flags that go before the script path in argv (e.g. `-File`, `/C`).
    pub argv_flags: Vec<String>,
    /// `explicit_runner` | `config` | `shebang` | `extension_map` | `default_fallback`.
    pub source: String,
    pub used_shebang: bool,
    pub used_extension_map: bool,
}

/// Detect the runner for a script. `content` is the script text (for shebang parsing);
/// `explicit` is `--runner`; `allow_shebang` is false when `--no-shebang` was given.
pub fn detect_runner(
    rel: &str,
    content: &str,
    cfg: &ScriptRunnerConfig,
    explicit: Option<&str>,
    allow_shebang: bool,
) -> Result<Detection, AikitError> {
    // Configured runner names must be valid before they can influence detection.
    validate_config_runners(cfg)?;

    // Tier 1: explicit --runner override.
    if let Some(raw) = explicit {
        let name = raw.to_lowercase();
        if !is_known_runner(&name) {
            return Err(AikitError::blocked(
                blocked::RUNNER_NOT_ALLOWED,
                format!(
                    "--runner {raw:?} is not a recognized runner (known: {})",
                    known_runner_names().join(", ")
                ),
            ));
        }
        let program = resolve_runner(&name).ok_or_else(|| {
            AikitError::blocked(
                blocked::RUNNER_NOT_FOUND,
                format!("runner {name:?} is not available on this system (no interpreter found)"),
            )
        })?;
        return Ok(Detection {
            runner: name.clone(),
            program,
            argv_flags: runner_flags(&name),
            source: "explicit_runner".to_string(),
            used_shebang: false,
            used_extension_map: false,
        });
    }

    let ext = ext_of(rel);
    let shebang_on = allow_shebang && cfg.detect_from_shebang;
    let ext_on = cfg.detect_from_extension;

    // Build candidate proposals in tier order (config map → shebang → built-in map).
    // Each proposal carries the source label and which signal it represents.
    let mut proposals: Vec<(String, &'static str, bool, bool)> = Vec::new();
    if ext_on {
        if let Some(cands) = cfg.extension_map.get(&ext) {
            for n in reorder(cands, &cfg.preferred_runners) {
                proposals.push((n, "config", false, true));
            }
        }
    }
    if shebang_on {
        if let Some(n) = shebang_runner(content) {
            proposals.push((n, "shebang", true, false));
        }
    }
    if ext_on {
        if let Some(cands) = default_ext_map(&ext) {
            for n in reorder(&cands, &cfg.preferred_runners) {
                proposals.push((n, "extension_map", false, true));
            }
        }
    }

    let had_proposals = !proposals.is_empty();
    let mut attempted: Vec<String> = Vec::new();
    for (name, source, used_shebang, used_ext) in &proposals {
        if !is_known_runner(name) {
            continue;
        }
        if !attempted.contains(name) {
            attempted.push(name.clone());
        }
        if let Some(program) = resolve_runner(name) {
            return Ok(Detection {
                runner: name.clone(),
                program,
                argv_flags: runner_flags(name),
                source: source.to_string(),
                used_shebang: *used_shebang,
                used_extension_map: *used_ext,
            });
        }
    }

    if had_proposals {
        Err(AikitError::blocked(
            blocked::RUNNER_NOT_FOUND,
            format!(
                "no available runner for {rel:?} on this system (tried: {})",
                attempted.join(", ")
            ),
        ))
    } else {
        Err(AikitError::blocked(
            blocked::UNKNOWN_SCRIPT_TYPE,
            format!(
                "unknown script type for {rel:?}: no extension mapping or shebang matched a known runner"
            ),
        ))
    }
}

/// Availability of one supported runner on the current OS.
pub struct RunnerAvailability {
    pub name: &'static str,
    /// Whether the runner's program was found on this system.
    pub available: bool,
    /// Whether the runner can apply to the current OS at all (OS constraints).
    pub applicable: bool,
}

/// Report availability for every supported runner, in the built-in table order. Used by
/// `repo doctor` to model runner readiness instead of hard-coding `/bin/sh` + `/bin/zsh`.
pub fn runner_availability() -> Vec<RunnerAvailability> {
    RUNNERS
        .iter()
        .map(|r| {
            let applicable = (!r.windows_only || cfg!(windows)) && (!r.unix_only || !cfg!(windows));
            let available = resolve_runner(r.name).is_some();
            RunnerAvailability {
                name: r.name,
                available,
                applicable,
            }
        })
        .collect()
}

/// Validate that every runner name referenced by config (`preferred_runners` and the
/// `extension_map` values) is a recognized runner, failing clearly otherwise. Catches
/// typos at use time rather than letting them be silently skipped or surface later as a
/// misleading `blocked_runner_not_found`.
///
/// Match is exact (case-sensitive): configured runner names must be the lowercase
/// symbolic names. A mixed-case value like `"Bash"` is rejected rather than silently
/// normalized, so config matches detection (which also compares case-sensitively).
pub fn validate_config_runners(cfg: &ScriptRunnerConfig) -> Result<(), AikitError> {
    let check = |name: &str, origin: &str| -> Result<(), AikitError> {
        if is_known_runner(name) {
            Ok(())
        } else {
            Err(AikitError::blocked(
                blocked::RUNNER_NOT_ALLOWED,
                format!(
                    "script_runner config references unknown runner {name:?} in {origin}; \
runner names must be lowercase symbolic names (known: {})",
                    known_runner_names().join(", ")
                ),
            ))
        }
    };
    for name in &cfg.preferred_runners {
        check(name, "preferred_runners")?;
    }
    for (ext, names) in &cfg.extension_map {
        for name in names {
            check(name, &format!("extension_map[{ext:?}]"))?;
        }
    }
    Ok(())
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

/// The lowercased extension (with leading dot) of a path, or `""` when absent.
fn ext_of(rel: &str) -> String {
    Path::new(rel)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default()
}

/// Built-in extension → ordered candidate runners.
fn default_ext_map(ext: &str) -> Option<Vec<String>> {
    let v: &[&str] = match ext {
        ".sh" => &["sh", "bash"],
        ".zsh" => &["zsh"],
        ".bash" => &["bash", "sh"],
        ".ps1" => &["pwsh", "powershell"],
        ".cmd" => &["cmd"],
        ".bat" => &["cmd"],
        ".py" => &["python3", "python"],
        ".js" => &["node"],
        _ => return None,
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

/// Flags that precede the script path in argv for a given runner.
fn runner_flags(name: &str) -> Vec<String> {
    match name {
        "pwsh" | "powershell" => ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        "cmd" => vec!["/C".to_string()],
        _ => Vec::new(),
    }
}

fn is_known_runner(name: &str) -> bool {
    RUNNERS.iter().any(|r| r.name == name)
}

fn known_runner_names() -> Vec<&'static str> {
    RUNNERS.iter().map(|r| r.name).collect()
}

/// Resolve a runner's program to an available path, honoring OS constraints. Returns
/// `None` when the runner is not applicable to this OS or no candidate is installed.
fn resolve_runner(name: &str) -> Option<String> {
    let spec = RUNNERS.iter().find(|r| r.name == name)?;
    if spec.windows_only && !cfg!(windows) {
        return None;
    }
    if spec.unix_only && cfg!(windows) {
        return None;
    }
    spec.candidates.iter().find_map(|c| which(c))
}

/// Locate a program: an absolute/relative path is checked directly; a bare name is
/// searched on PATH (with PATHEXT extensions on Windows). Returns the resolved path.
fn which(program: &str) -> Option<String> {
    let p = Path::new(program);
    if program.contains('/') || program.contains('\\') || p.is_absolute() {
        return p.is_file().then(|| program.to_string());
    }
    let path_var = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .ok()
            .map(|s| s.split(';').map(|e| e.to_string()).collect())
            .unwrap_or_else(|| {
                [".COM", ".EXE", ".BAT", ".CMD"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            })
    } else {
        Vec::new()
    };
    for dir in std::env::split_paths(&path_var) {
        let direct = dir.join(program);
        if direct.is_file() {
            return Some(direct.to_string_lossy().to_string());
        }
        for e in &exts {
            let candidate = dir.join(format!("{program}{e}"));
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    None
}

/// Reorder candidate runners so any present in `preferred` come first (in preferred
/// order), then the remaining candidates in their original order.
fn reorder(cands: &[String], preferred: &[String]) -> Vec<String> {
    if preferred.is_empty() {
        return cands.to_vec();
    }
    let mut out: Vec<String> = Vec::new();
    for p in preferred {
        if cands.iter().any(|c| c == p) && !out.contains(p) {
            out.push(p.clone());
        }
    }
    for c in cands {
        if !out.contains(c) {
            out.push(c.clone());
        }
    }
    out
}

/// Parse a `#!` shebang line and map it to a known runner name, handling
/// `#!/usr/bin/env <interp>`. Returns `None` when there is no shebang or it is not
/// recognized.
fn shebang_runner(content: &str) -> Option<String> {
    let first = content.lines().next()?;
    let rest = first.strip_prefix("#!")?.trim();
    let mut tokens = rest.split_whitespace();
    let prog = tokens.next()?;
    let mut base = Path::new(prog)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(prog)
        .to_string();
    if base == "env" {
        // `#!/usr/bin/env python3` — the interpreter is the next non-flag token.
        if let Some(next) = tokens.find(|t| !t.starts_with('-')) {
            base = Path::new(next)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(next)
                .to_string();
        }
    }
    runner_for_basename(&base)
}

/// Map an interpreter basename to a known runner name.
fn runner_for_basename(base: &str) -> Option<String> {
    let name = match base.to_lowercase().as_str() {
        "sh" => "sh",
        "bash" => "bash",
        "zsh" => "zsh",
        "pwsh" => "pwsh",
        "powershell" => "powershell",
        "python" => "python",
        "python3" => "python3",
        "node" | "nodejs" => "node",
        _ => return None,
    };
    Some(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ScriptRunnerConfig;

    fn cfg() -> ScriptRunnerConfig {
        ScriptRunnerConfig::default()
    }

    #[test]
    fn sh_resolves_via_extension_when_no_shebang() {
        if !Path::new("/bin/sh").exists() {
            return;
        }
        let d = detect_runner(".aikit/temp/x.sh", "echo hi\n", &cfg(), None, true).unwrap();
        assert_eq!(d.runner, "sh");
        assert_eq!(d.program, "/bin/sh");
        assert_eq!(d.source, "extension_map");
        assert!(d.used_extension_map);
        assert!(!d.used_shebang);
    }

    #[test]
    fn shebang_is_preferred_over_default_extension_map() {
        if !Path::new("/bin/sh").exists() {
            return;
        }
        let d = detect_runner(
            ".aikit/temp/x.sh",
            "#!/bin/sh\necho hi\n",
            &cfg(),
            None,
            true,
        )
        .unwrap();
        assert_eq!(d.runner, "sh");
        assert_eq!(d.source, "shebang");
        assert!(d.used_shebang);
    }

    #[test]
    fn no_shebang_flag_disables_shebang() {
        if !Path::new("/bin/sh").exists() {
            return;
        }
        let d =
            detect_runner(".aikit/temp/x.sh", "#!/bin/sh\necho\n", &cfg(), None, false).unwrap();
        assert_eq!(d.source, "extension_map");
    }

    #[test]
    fn unknown_extension_is_blocked_unknown_type() {
        let err = detect_runner(".aikit/temp/x.xyz", "data\n", &cfg(), None, true).unwrap_err();
        match err {
            AikitError::Blocked { state, .. } => assert_eq!(state, blocked::UNKNOWN_SCRIPT_TYPE),
            _ => panic!("expected blocked"),
        }
    }

    #[test]
    fn explicit_unknown_runner_is_not_allowed() {
        let err = detect_runner(".aikit/temp/x.sh", "", &cfg(), Some("fish"), true).unwrap_err();
        match err {
            AikitError::Blocked { state, .. } => assert_eq!(state, blocked::RUNNER_NOT_ALLOWED),
            _ => panic!("expected blocked"),
        }
    }

    #[test]
    fn ps1_maps_to_powershell_family() {
        // The default map for .ps1 is [pwsh, powershell]; assert the candidate names
        // without requiring the interpreter to be installed on this host.
        assert_eq!(
            default_ext_map(".ps1").unwrap(),
            vec!["pwsh".to_string(), "powershell".to_string()]
        );
        assert_eq!(default_ext_map(".cmd").unwrap(), vec!["cmd".to_string()]);
        assert_eq!(default_ext_map(".bat").unwrap(), vec!["cmd".to_string()]);
        assert_eq!(
            default_ext_map(".py").unwrap(),
            vec!["python3".to_string(), "python".to_string()]
        );
        assert_eq!(default_ext_map(".js").unwrap(), vec!["node".to_string()]);
    }

    #[test]
    fn powershell_flags_use_file_form() {
        assert_eq!(
            runner_flags("pwsh"),
            vec!["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]
        );
        assert_eq!(runner_flags("cmd"), vec!["/C".to_string()]);
        assert!(runner_flags("node").is_empty());
    }

    #[test]
    fn preferred_runners_reorder_candidates() {
        let cands = vec!["sh".to_string(), "bash".to_string()];
        let pref = vec!["bash".to_string()];
        assert_eq!(reorder(&cands, &pref), vec!["bash", "sh"]);
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
