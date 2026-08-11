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
        // An explicit `--runner` names one runner, so there is no cross-name preference to
        // apply: whatever that name resolves to, alias or not, is what the caller asked for.
        let (program, _real) = resolve_runner(&name).ok_or_else(|| {
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
    // A proposal backed by a real executable wins over an earlier proposal backed only by an
    // App Execution Alias. Windows ships a zero-byte `python3.exe` alias with no Python
    // behind it, so `.py` would otherwise select `python3`, report the runner as available,
    // and then fail at exec with 9009 — a "command not found" surfacing as the script's own
    // exit code. An alias is still accepted when nothing real is proposed, because a
    // Store-installed PowerShell 7 has no other PATH entry.
    let mut alias_backed: Option<Detection> = None;
    for (name, source, used_shebang, used_ext) in &proposals {
        if !is_known_runner(name) {
            continue;
        }
        if !attempted.contains(name) {
            attempted.push(name.clone());
        }
        if let Some((program, real)) = resolve_runner(name) {
            let detection = Detection {
                runner: name.clone(),
                program,
                argv_flags: runner_flags(name),
                source: source.to_string(),
                used_shebang: *used_shebang,
                used_extension_map: *used_ext,
            };
            if real {
                return Ok(detection);
            }
            if alias_backed.is_none() {
                alias_backed = Some(detection);
            }
        }
    }
    if let Some(detection) = alias_backed {
        return Ok(detection);
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
    /// Absolute path of the resolved interpreter, when one was found.
    ///
    /// `available` alone cannot be acted on: two hosts both reporting `powershell:
    /// available` may be running 5.1 and 7, which differ in output encoding and exit-code
    /// behaviour. The path is what makes the report reproducible.
    pub path: Option<String>,
    /// Best-effort version string, when the interpreter reports one.
    ///
    /// Best-effort by nature: a POSIX `sh` that is really `dash` rejects `--version`, and no
    /// probe is attempted for a runner that is not available. `None` means "not determined",
    /// never "old" or "broken".
    pub version: Option<String>,
    /// Stable machine-readable reason for `available`, so a caller can tell "this OS never
    /// has it" from "install it and retry" without parsing prose.
    ///
    /// One of `available`, `available_via_alias`, `not_found_on_path`,
    /// `not_applicable_on_this_os`.
    ///
    /// `available_via_alias` means the only thing on PATH is a Windows App Execution Alias.
    /// That is not the same as broken — a Store-installed PowerShell 7 resolves this way and
    /// works — but it is also how Windows ships a `python3.exe` with no Python behind it,
    /// which opens the Store and exits 9009. Reporting it as plain `available` would state
    /// more than aikit knows.
    pub reason: &'static str,
}

/// Report availability for every supported runner, in the built-in table order. Used by
/// `aikit doctor` to model runner readiness instead of hard-coding `/bin/sh` + `/bin/zsh`,
/// and by the MCP `list_runners` tool.
///
/// Version probing spawns one short-lived process per **available** runner. Unavailable and
/// inapplicable runners are never probed, which bounds the cost to interpreters that are
/// actually installed.
pub fn runner_availability() -> Vec<RunnerAvailability> {
    RUNNERS
        .iter()
        .map(|r| {
            let applicable = (!r.windows_only || cfg!(windows)) && (!r.unix_only || !cfg!(windows));
            let resolved = resolve_runner(r.name);
            let available = resolved.is_some();
            let real = resolved.as_ref().map(|(_, real)| *real).unwrap_or(false);
            let path = resolved.map(|(program, _real)| program);
            let version = path.as_deref().and_then(|p| probe_version(r.name, p));
            let reason = match (available, real, applicable) {
                (true, true, _) => "available",
                (true, false, _) => "available_via_alias",
                (false, _, true) => "not_found_on_path",
                (false, _, false) => "not_applicable_on_this_os",
            };
            RunnerAvailability {
                name: r.name,
                available,
                applicable,
                path,
                version,
                reason,
            }
        })
        .collect()
}

/// Ask an interpreter for its version, using that interpreter's own idiom.
///
/// There is no universal flag: `powershell --version` is not a thing (5.1 predates it), and
/// `cmd` reports via `ver`. Guessing `--version` everywhere would return an error message as
/// if it were a version, which is worse than reporting nothing. Failures are swallowed
/// deliberately — this is a diagnostic field, and a host that cannot answer should still get
/// a report.
fn probe_version(name: &str, program: &str) -> Option<String> {
    let args: &[&str] = match name {
        // `$PSVersionTable` is the only form both 5.1 and 7 answer. `-NoProfile` for the
        // same reason runs use it: a profile can print banners that would become the
        // "version".
        "pwsh" | "powershell" => &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$PSVersionTable.PSVersion.ToString()",
        ],
        "cmd" => &["/d", "/c", "ver"],
        _ => &["--version"],
    };

    let out = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    // Some report on stderr (older node, some shells); take whichever spoke.
    let text = if out.stdout.iter().any(|b| !b.is_ascii_whitespace()) {
        out.stdout
    } else {
        out.stderr
    };
    let text = String::from_utf8_lossy(&text);
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    Some(line.to_string())
}

// ---------------------------------------------------------------------------------------
// Public surface for the MCP server (`aikit mcp`).
//
// The MCP path never infers a runner: the caller names it. So it needs the runner table,
// program resolution and per-runner file conventions, but none of the CLI's detection
// tiers. These are additive; CLI behaviour is unchanged.
// ---------------------------------------------------------------------------------------

/// Whether `name` is a recognized runner.
pub fn is_known_runner_name(name: &str) -> bool {
    is_known_runner(name)
}

/// Every recognized runner name, in table order.
pub fn all_runner_names() -> Vec<&'static str> {
    known_runner_names()
}

/// Resolve a runner to the program that invokes it, honouring OS constraints. `None` when
/// the runner does not apply to this OS or no candidate is installed.
pub fn resolve_runner_program(name: &str) -> Option<String> {
    resolve_runner(name).map(|(program, _real)| program)
}

/// Flags preceding the script path for the MCP path.
///
/// This deliberately diverges from the CLI's [`runner_flags`]: PowerShell additionally gets
/// `-NonInteractive`. The CLI runs scripts a user approved from a repo-local location and
/// can tolerate a prompt; an MCP call closes stdin, so any prompt would block silently for
/// the whole timeout instead of failing.
pub fn mcp_runner_flags(name: &str) -> Vec<String> {
    match name {
        "pwsh" | "powershell" => [
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        // `/d` before `/c`: it suppresses the AutoRun command registered under
        // `Command Processor\AutoRun` in HKLM/HKCU, which cmd otherwise executes before the
        // script and which can alter PATH and the environment. Same class of ambient
        // mutation as PowerShell's `$PROFILE`, and the same reason to refuse it.
        "cmd" => vec!["/d".to_string(), "/c".to_string()],
        _ => Vec::new(),
    }
}

/// File extension for a generated script, derived from the runner rather than guessed.
pub fn runner_script_extension(name: &str) -> &'static str {
    match name {
        "pwsh" | "powershell" => ".ps1",
        "cmd" => ".cmd",
        "python" | "python3" => ".py",
        "node" => ".js",
        "zsh" => ".zsh",
        _ => ".sh",
    }
}

/// Whether a generated script for this runner uses CRLF line endings.
pub fn runner_wants_crlf(name: &str) -> bool {
    matches!(name, "cmd" | "powershell")
}

/// Text appended to a **generated** script so the interpreter's own exit code reflects the
/// last native command's failure.
///
/// Measured on Windows PowerShell 5.1 (aarch64, build 26200): a script whose last statement
/// is a failing native command exits **0** through `-File`, masking the failure entirely.
/// Appending `exit $LASTEXITCODE` recovers the real code — 7 in the probe.
///
/// This applies only where aikit *generates* the file from call arguments (the MCP `run`
/// tool). `aikit script run` executes a file the caller authored and must never rewrite it,
/// so it gets no epilogue: the fix is available exactly where the bytes are aikit's to own.
///
/// Limits worth knowing: `$LASTEXITCODE` tracks native commands only, so a failing *cmdlet*
/// is still not reflected; and a script that calls `exit` itself terminates before the
/// epilogue runs, which is correct — an explicit exit is the caller's stated intent.
pub fn runner_exit_epilogue(name: &str) -> Option<&'static str> {
    match name {
        // `$LASTEXITCODE` is `$null` when no native command ran; PowerShell exits 0 for that.
        "pwsh" | "powershell" => Some("exit $LASTEXITCODE"),
        // Batch files propagate ERRORLEVEL lossily; `exit /b` makes the value explicit.
        //
        // The leading `@` is not cosmetic. cmd echoes each command it runs unless the line is
        // prefixed, so without it the capture gains a line aikit wrote and the script never
        // produced — `C:\path>exit /b 0` — which then reaches the model as if it were output
        // and is stored in the audit record as if it were the script's.
        "cmd" => Some("@exit /b %ERRORLEVEL%"),
        // POSIX shells and interpreters already exit with the last command's status.
        _ => None,
    }
}

/// How faithfully the recorded exit code reflects what the script actually did.
///
/// `exit_code: 0` is not equally meaningful across runners, and a caller gating on it
/// deserves to know which it got. A POSIX shell, python or node exits with the last
/// command's status, so 0 means the script succeeded. PowerShell and cmd do not: PowerShell
/// exits 0 when the last *native* command failed, so 0 can mean "ran" rather than
/// "succeeded". Where aikit generates the script file it appends an explicit propagation
/// (see [`runner_exit_epilogue`]); where it runs a file the caller owns, it cannot.
///
/// Returns one of:
/// - `interpreter` — the interpreter propagates the status itself. Trustworthy.
/// - `epilogue` — aikit appended an explicit propagation. Trustworthy for native commands;
///   a failing *cmdlet*, or a script that calls `exit` before the epilogue runs, bypasses it.
/// - `unpropagated` — the runner needs help and none was applied, because the script is the
///   caller's own file. **Treat 0 as "it ran", not "it succeeded."**
pub fn exit_code_source(name: &str, epilogue_applied: bool) -> &'static str {
    if runner_exit_epilogue(name).is_none() {
        "interpreter"
    } else if epilogue_applied {
        "epilogue"
    } else {
        "unpropagated"
    }
}

/// Whether a generated script for this runner is written with a UTF-8 BOM.
///
/// Windows PowerShell 5.1 assumes the ANSI code page for a BOM-less file. `pwsh` (7)
/// assumes UTF-8 and needs none. **Never for `cmd`**: cmd.exe parses the BOM bytes as part
/// of the first command.
pub fn runner_wants_bom(name: &str) -> bool {
    name == "powershell"
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

/// One occurrence of a forbidden pattern, located precisely enough to act on.
///
/// The location matters because the guard is acknowledgeable: a caller deciding whether a
/// match is the accident it exists to catch, or a deliberate operation, needs to see the
/// line rather than be told only that the script "contains" something somewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForbiddenMatch {
    pub pattern: &'static str,
    /// 1-based line number.
    pub line: usize,
    /// 1-based column within the line, in bytes.
    pub column: usize,
    /// Byte offset from the start of the content.
    pub offset: usize,
}

/// Every forbidden pattern occurrence in the content, first match per pattern, ordered by
/// position. Best-effort substring scanning — not a security check.
pub fn scan_forbidden_all(content: &str) -> Vec<ForbiddenMatch> {
    let mut found: Vec<ForbiddenMatch> = FORBIDDEN_PATTERNS
        .iter()
        .filter_map(|p| content.find(p).map(|offset| (p, offset)))
        .map(|(pattern, offset)| {
            // Line/column from the byte offset. Counting once per match keeps this O(n·k)
            // for k patterns, which is irrelevant at script sizes.
            let before = &content[..offset];
            let line = before.matches('\n').count() + 1;
            let column = before.rsplit('\n').next().map(str::len).unwrap_or(0) + 1;
            ForbiddenMatch {
                pattern,
                line,
                column,
                offset,
            }
        })
        .collect();
    found.sort_by_key(|m| m.offset);
    found
}

/// The distinct patterns in `matches` that the caller has not acknowledged.
///
/// Comparison is exact against the advertised pattern text: acknowledging `"git push"`
/// permits that pattern and nothing else, so an acknowledgement cannot become a blanket
/// opt-out of the whole scan.
pub fn unacknowledged<'a>(matches: &'a [ForbiddenMatch], acknowledged: &[String]) -> Vec<&'a str> {
    let mut out: Vec<&str> = matches
        .iter()
        .map(|m| m.pattern)
        .filter(|p| !acknowledged.iter().any(|a| a == p))
        .collect();
    out.dedup();
    out
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
        // See `mcp_runner_flags`: `/d` suppresses the registry AutoRun command, which is
        // ambient environment mutation of the same kind `-NoProfile` refuses.
        "cmd" => vec!["/d".to_string(), "/c".to_string()],
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
/// `None` when the runner is not applicable to this OS or no candidate is installed, and
/// otherwise the path plus whether it is a real executable rather than an App Execution
/// Alias (see [`which_kind`]).
fn resolve_runner(name: &str) -> Option<(String, bool)> {
    let spec = RUNNERS.iter().find(|r| r.name == name)?;
    if spec.windows_only && !cfg!(windows) {
        return None;
    }
    if spec.unix_only && cfg!(windows) {
        return None;
    }
    // Two passes over the candidate list: a real executable *anywhere* in it beats an App
    // Execution Alias *earlier* in it. The same preference is applied again across runner
    // names in `detect_runner`, which is where the case that matters actually arises.
    spec.candidates
        .iter()
        .find_map(|c| which_kind(c).filter(|(_, real)| *real))
        .or_else(|| spec.candidates.iter().find_map(|c| which_kind(c)))
}

/// Locate a program and report what was found: an absolute/relative path is checked
/// directly; a bare name is searched on PATH (with PATHEXT extensions on Windows).
///
/// Returns the resolved path plus whether it is a real executable (`true`) or an App
/// Execution Alias standing in for one (`false`). The caller needs that distinction because
/// the choice between candidate *names* depends on it — see `resolve_runner`.
fn which_kind(program: &str) -> Option<(String, bool)> {
    let p = Path::new(program);
    if program.contains('/') || program.contains('\\') || p.is_absolute() {
        return p
            .is_file()
            .then(|| (program.to_string(), !is_app_execution_alias(p)));
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
    // An App Execution Alias is kept only as a fallback. When the app behind it is
    // installed the alias launches it correctly (Store-installed PowerShell 7 has no other
    // PATH entry), but when it is not, the alias opens the Microsoft Store and the caller
    // sees exit 9009. Preferring a real executable resolves both: a genuine interpreter
    // wins when one exists, and the alias is still reachable when it is all there is.
    let mut alias_fallback: Option<String> = None;
    let mut consider = |candidate: &Path| -> Option<String> {
        if !candidate.is_file() || is_wsl_launcher(candidate) {
            return None;
        }
        if is_app_execution_alias(candidate) {
            if alias_fallback.is_none() {
                alias_fallback = Some(candidate.to_string_lossy().to_string());
            }
            return None;
        }
        Some(candidate.to_string_lossy().to_string())
    };

    for dir in std::env::split_paths(&path_var) {
        if let Some(found) = consider(&dir.join(program)) {
            return Some((found, true));
        }
        for e in &exts {
            if let Some(found) = consider(&dir.join(format!("{program}{e}"))) {
                return Some((found, true));
            }
        }
    }
    alias_fallback.map(|p| (p, false))
}

/// Whether `path` is a Windows App Execution Alias: a zero-byte reparse point standing in
/// for a Store-delivered program rather than being one.
///
/// Not disqualifying on its own — see `which`. It only means "prefer something real".
fn is_app_execution_alias(path: &Path) -> bool {
    cfg!(windows)
        && std::fs::metadata(path)
            .map(|m| m.len() == 0)
            .unwrap_or(false)
}

/// Whether `path` is Windows' WSL launcher rather than a local interpreter.
///
/// `%SystemRoot%\System32\bash.exe` (and `wsl.exe`) do not run a script on Windows — they
/// run it **inside a Linux distribution**, with a different filesystem view, different
/// tools, and Windows paths that mostly do not resolve. A caller asking for `bash` and
/// silently getting another operating system is not a substitution aikit is entitled to
/// make for them, so this is a hard exclusion rather than a preference.
///
/// Contrast [`is_app_execution_alias`], which only deprioritises: an alias stands in for
/// the *same* program, whereas WSL's bash is a different program entirely.
///
/// Deliberately narrow: it matches only the System32 launcher, so a genuine
/// `C:\Program Files\Git\bin\bash.exe` from Git for Windows still resolves normally.
fn is_wsl_launcher(path: &Path) -> bool {
    if !cfg!(windows) {
        return false;
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if name != "bash.exe" && name != "wsl.exe" {
        return false;
    }
    // Compare against the real system directory rather than a hardcoded C:\Windows.
    let sysroot = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"));
    let system32 = sysroot.join("System32");
    let parent = path
        .parent()
        .map(|p| p.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    if parent == system32.to_string_lossy().to_ascii_lowercase() {
        return true;
    }
    // WSL also ships as a Store App Execution Alias under `WindowsApps`, which the System32
    // check alone misses — observed on a Windows ARM64 host where `WindowsApps\bash.EXE` was
    // the only `bash` on PATH. Same program, same objection: it would run the script inside a
    // Linux distribution rather than on Windows. Matching the directory (not a full path)
    // covers both the per-user and machine-wide alias locations, and stays narrow because it
    // still applies only to the two launcher names.
    parent.contains(r"\windowsapps")
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
    fn runner_flags_refuse_ambient_environment_mutation() {
        // The property under test is not the exact argv but that neither shell is allowed to
        // run user-configured startup code first. `$PROFILE` and cmd's registry AutoRun can
        // both alter PATH and the environment, which would make "deterministic" false.
        assert_eq!(
            runner_flags("pwsh"),
            vec!["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]
        );
        assert_eq!(
            runner_flags("cmd"),
            vec!["/d".to_string(), "/c".to_string()]
        );
        assert!(runner_flags("node").is_empty());

        // The MCP path adds -NonInteractive on top: it closes stdin, so a prompt would block
        // silently for the whole timeout instead of failing.
        let mcp_ps = mcp_runner_flags("powershell");
        assert!(mcp_ps.contains(&"-NoProfile".to_string()));
        assert!(mcp_ps.contains(&"-NonInteractive".to_string()));
        assert_eq!(
            mcp_runner_flags("cmd"),
            vec!["/d".to_string(), "/c".to_string()]
        );
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
    fn forbidden_scan_locates_the_match() {
        // The location is the point: a caller deciding whether a match is the accident the
        // guard exists for, or a deliberate operation, needs to see where it is.
        let hits = scan_forbidden_all("echo hi\ngit push\n");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].pattern, "git push");
        assert_eq!(hits[0].line, 2);
        assert_eq!(hits[0].column, 1);

        assert!(scan_forbidden_all("echo ok\n").is_empty());
    }

    #[test]
    fn acknowledging_one_pattern_does_not_disable_the_others() {
        // The whole safety of an overridable guard rests on this: an acknowledgement is a
        // statement about one specific pattern, never a blanket opt-out of the scan.
        let hits = scan_forbidden_all("sudo apt install x\ngit push\n");
        assert_eq!(hits.len(), 2);

        let acked = vec!["git push".to_string()];
        assert_eq!(unacknowledged(&hits, &acked), vec!["sudo"]);

        let both = vec!["git push".to_string(), "sudo".to_string()];
        assert!(unacknowledged(&hits, &both).is_empty());

        // An unrelated value acknowledges nothing.
        let irrelevant = vec!["rm -rf /".to_string()];
        assert_eq!(unacknowledged(&hits, &irrelevant).len(), 2);
    }
}
