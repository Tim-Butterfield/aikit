//! Tool declarations: names, descriptions and hand-written JSON schemas.
//!
//! Schemas are written by hand rather than derived so the declared contract is exactly the
//! intended one — in particular which fields are required and which are nullable. A derived
//! schema would collapse "absent" and "null" for `env` values, and the difference is
//! load-bearing there.

use serde_json::{json, Map, Value};

use crate::policy::script::{all_runner_names, FORBIDDEN_PATTERNS};

pub const RUN: &str = "run";
pub const LIST_RUNNERS: &str = "list_runners";
pub const AGENTS_MD: &str = "agents_md";

/// The agent guide, compiled into the binary.
///
/// Embedded rather than read from a file beside the executable: `cargo install` copies only
/// the executable, so a sidecar would be absent on the install path the README leads with —
/// and a sidecar can be edited or left over from an older version, so it can disagree with
/// the binary answering the call. Compiled in, the two cannot disagree.
pub const EMBEDDED_AGENTS_MD: &str = include_str!("../../AGENTS.md");

/// Environment variable naming a file to serve instead of the embedded guide.
///
/// Unprefixed on purpose: the binary is already `aikit`, so `AIKIT_AGENTS_MD` would say it
/// twice.
pub const AGENTS_MD_OVERRIDE: &str = "AGENTS_MD";

pub fn agents_md_description() -> String {
    "Return aikit's agent guide: what it does, the rules that are expensive to get wrong, \
     every CLI command with a one-line contract, and its safety posture. Call this first if \
     you have not used aikit before — it is cheaper and more complete than probing `--help` \
     one command at a time. The text is compiled into the binary, so it always matches the \
     version answering the call."
        .to_string()
}

/// Description of `run`, including the facts a model needs in order to use it correctly and
/// to recover from its errors.
///
/// The forbidden-pattern list is generated from the same table `scan_forbidden` enforces, so
/// the advertised rules cannot drift from the enforced ones.
pub fn run_description() -> String {
    let runners = all_runner_names().join(", ");
    let forbidden = FORBIDDEN_PATTERNS
        .iter()
        .map(|p| format!("{p:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Run a script through a named interpreter and return its output.\n\
         \n\
         `runner` is REQUIRED and is never inferred: `pwsh` and `powershell` both execute \
         .ps1 but interpret it differently, so the caller must say which is intended. Call \
         `{LIST_RUNNERS}` first and pass an exact name from it. Known runners: {runners}. A \
         runner that is not installed is an error, never a silent substitution.\n\
         \n\
         `cwd` must be an absolute path to an existing directory. Create scratch \
         directories inside the script rather than expecting this tool to make them.\n\
         \n\
         Configuration is not consulted: extension maps and preferred-runner settings that \
         affect `aikit script run` have no effect here.\n\
         \n\
         A non-zero exit code is a SUCCESSFUL call, reported in `exit_code`. On Windows \
         treat `exit_code == 0` as weaker evidence: cmd batch files propagate ERRORLEVEL \
         lossily, and powershell/pwsh -File exit 0 when a native command failed unless the \
         script checks $LASTEXITCODE.\n\
         \n\
         Do not rely on cancelling a call to stop the script. If your client delivers the \
         cancellation to this server the process tree is killed, but clients are not \
         required to send it — many simply stop waiting, and the script then runs to \
         completion. The timeout is the only guaranteed stop, which is why it cannot be \
         disabled.\n\
         \n\
         These patterns are refused before execution (an accident guard, not a security \
         boundary): {forbidden}. The refusal names the pattern and line. If the operation \
         is genuinely intended, repeat the call with `acknowledge_forbidden` listing those \
         exact patterns — acknowledging one does not disable the others.\n\
         \n\
         Nothing is written into the caller's repository or working tree, and no run record \
         is kept. The generated script does exist under the system temp location while it \
         runs, in a directory created private to the user, and deletion afterwards is \
         best-effort — so do not put a secret in the script body on the assumption it \
         vanishes."
    )
}

pub fn list_runners_description() -> String {
    "List the script runners this host supports, whether each applies to this OS, and \
     whether its interpreter was found. Call this before `run` and pass one of these exact \
     names as `runner`."
        .to_string()
}

/// Input schema for `run`.
pub fn run_input_schema() -> Map<String, Value> {
    obj(json!({
        "type": "object",
        "properties": {
            "runner": {
                "type": "string",
                "description": "Exact runner name from list_runners. Required; never inferred.",
                "enum": all_runner_names(),
            },
            "script": {
                "type": "string",
                "description": "Script source to execute.",
            },
            "cwd": {
                "type": "string",
                "description": "Absolute path to an existing directory to run in.",
            },
            "stdin": {
                "type": "string",
                "description": "Text written to the script's stdin, which is then closed. \
                                Absent means stdin is closed immediately.",
            },
            "env": {
                "type": "object",
                "description": "Environment overlay. A string sets a variable; null removes \
                                it. Names in the minimal floor cannot be removed.",
                "additionalProperties": { "type": ["string", "null"] },
            },
            "env_base": {
                "type": "string",
                "enum": ["inherit", "minimal"],
                "description": "inherit (default) passes this process's environment through. \
                                minimal starts from a curated floor under which a normal \
                                shell works.",
            },
            "acknowledge_forbidden": {
                "type": "array",
                // Same constant `scan_forbidden_all` enforces, so the accepted values and
                // the refused ones cannot drift apart.
                "items": { "type": "string", "enum": FORBIDDEN_PATTERNS },
                "description": "Forbidden patterns you are deliberately accepting. Each entry \
                                must be one of the advertised patterns exactly, so \
                                acknowledging one never disables the rest. Use this only when \
                                the operation is intended — the guard exists because an agent \
                                usually did not mean it.",
            },
            "limits": {
                "type": "object",
                "properties": {
                    "timeout_ms": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 3600000,
                        "description": "Wall-clock limit, default 120000, max 3600000. It \
                                        cannot be disabled: it is the only thing that stops \
                                        a runaway script.",
                    },
                    "max_bytes": {
                        "type": "integer",
                        "minimum": 1,
                        // The cap is enforced in RunSpec::parse; declaring it here too means
                        // a client generating requests from the schema cannot produce one the
                        // server will reject.
                        "maximum": 134217728,
                        "description": "Combined stdout+stderr capture budget, default \
                                        33554432, max 134217728. The budget can be adjusted \
                                        but not removed: uncapped capture across concurrent \
                                        calls is an unrecoverable failure rather than a slow \
                                        one.",
                    },
                    "on_output_limit": {
                        "type": "string",
                        "enum": ["truncate", "kill"],
                        "description": "truncate (default) keeps running but stops \
                                        recording; kill terminates the process tree.",
                    }
                },
                "additionalProperties": false,
            }
        },
        "required": ["runner", "script", "cwd"],
        "additionalProperties": false,
    }))
}

/// Output schema for `run`.
pub fn run_output_schema() -> Map<String, Value> {
    obj(json!({
        "type": "object",
        "properties": {
            "stop_reason": {
                "type": "string",
                "enum": ["exited", "timeout", "output_limit", "server_limit",
                         "spawn_failed", "setup_failed"],
            },
            "exit_code": {
                "type": ["integer", "null"],
                "description": "Null when the process did not exit on its own.",
            },
            "stdout": { "type": "string" },
            "stderr": { "type": "string" },
            "stdout_truncated": { "type": "boolean" },
            "stderr_truncated": { "type": "boolean" },
            "duration_ms": { "type": "integer" },
            "runner": { "type": "string" },
            "program": { "type": "string" },
            "argv_flags": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Flags aikit placed before the script path. These are chosen \
                                by aikit, not the caller — PowerShell gets -NoProfile \
                                -NonInteractive -ExecutionPolicy Bypass, cmd gets /d — so \
                                they are reported rather than left implicit.",
            },
            "exit_code_epilogue": {
                "type": ["string", "null"],
                "description": "A line aikit appended to the script so the interpreter's \
                                exit code reflects the last native command, or null when \
                                the runner needs none. PowerShell exits 0 through -File \
                                even when the last native command failed, so `exit \
                                $LASTEXITCODE` is added; POSIX shells already propagate. \
                                Reported because aikit modified what you sent.",
            },
            "exit_code_source": {
                "type": "string",
                "enum": ["interpreter", "epilogue", "unpropagated"],
                "description": "How far exit_code can be trusted. interpreter: the \
                                interpreter propagates the last command's status itself, so \
                                0 means the script succeeded. epilogue: aikit appended an \
                                explicit propagation — trustworthy for native commands, but \
                                a failing cmdlet or a script that calls exit first bypasses \
                                it. unpropagated: the runner needs help and none was \
                                applied, so treat 0 as 'it ran', NOT 'it succeeded'.",
            },
            "containment": {
                "type": "string",
                "enum": ["job_object", "process_group"],
                "description": "The mechanism that contains and kills the script's \
                                descendants — reported rather than claimed, because 'kills \
                                the process tree' is not unconditionally true. job_object \
                                (Windows): descendants cannot be spawned outside the job, \
                                though a process created with explicit breakaway escapes. \
                                process_group (Unix): the group is signalled, but a \
                                descendant calling setsid/setpgid leaves it and survives.",
            }
        },
        "required": ["stop_reason", "exit_code", "stdout", "stderr",
                     "stdout_truncated", "stderr_truncated", "duration_ms",
                     "runner", "program", "argv_flags", "exit_code_epilogue",
                     "exit_code_source", "containment"],
        "additionalProperties": false,
    }))
}

pub fn list_runners_input_schema() -> Map<String, Value> {
    obj(json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false,
    }))
}

pub fn list_runners_output_schema() -> Map<String, Value> {
    obj(json!({
        "type": "object",
        "properties": {
            "runners": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "applicable": { "type": "boolean" },
                        "available": { "type": "boolean" },
                        "path": {
                            "type": ["string", "null"],
                            "description": "Absolute path of the resolved interpreter, or \
                                            null when none was found. Two hosts both \
                                            reporting powershell available may be running \
                                            5.1 and 7.",
                        },
                        "version": {
                            "type": ["string", "null"],
                            "description": "Best-effort version string. Null means NOT \
                                            DETERMINED, never old or broken — a POSIX sh \
                                            that is really dash rejects --version. Use it \
                                            to tell powershell 5.1 from pwsh 7, which \
                                            differ in output encoding and exit-code \
                                            behaviour.",
                        },
                        "reason": {
                            "type": "string",
                            "enum": ["available", "available_via_alias",
                                     "not_found_on_path", "not_applicable_on_this_os"],
                            "description": "Why this runner is or is not usable. \
                                            available_via_alias means only a Windows App \
                                            Execution Alias was found: sometimes a working \
                                            Store install, sometimes a stub that opens the \
                                            Store and exits 9009.",
                        }
                    },
                    "required": ["name", "applicable", "available", "path", "version", "reason"],
                    "additionalProperties": false,
                }
            }
        },
        "required": ["runners"],
        "additionalProperties": false,
    }))
}

pub fn agents_md_input_schema() -> Map<String, Value> {
    obj(json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false,
    }))
}

pub fn agents_md_output_schema() -> Map<String, Value> {
    obj(json!({
        "type": "object",
        "properties": {
            "content": { "type": "string" },
            "source": {
                "type": "string",
                "description": "`embedded`, or `override:<path>` when AGENTS_MD names a file. \
                                Guidance that was customised should be identifiable as such.",
            },
            "aikit_version": { "type": "string" }
        },
        "required": ["content", "source", "aikit_version"],
        "additionalProperties": false,
    }))
}

/// Guidance surfaced through the `initialize` result; some clients inject it as system
/// context, which makes it the highest-leverage place for the rules a caller must know.
///
/// Deliberately short: this is paid for on every session whether or not it is used, so it
/// carries only the rules that prevent a wrong first call, plus a pointer to `agents_md`
/// for everything else.
pub fn server_instructions() -> String {
    format!(
        "aikit runs shell scripts locally. Call `{LIST_RUNNERS}` first and pass an exact \
         runner name to `{RUN}` — the runner is required and never inferred. `cwd` must be \
         an absolute existing directory. A non-zero exit code is a successful call. \
         Cancelling a run does not stop the script; only its timeout does. Call \
         `{AGENTS_MD}` for the full agent guide, including the `aikit` CLI reachable from \
         inside a `{RUN}` call."
    )
}

fn obj(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => unreachable!("schema literals are objects"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertised_forbidden_patterns_match_the_enforced_ones() {
        // The description is generated from FORBIDDEN_PATTERNS precisely so the advertised
        // list cannot drift from what scan_forbidden refuses.
        let desc = run_description();
        for p in FORBIDDEN_PATTERNS {
            assert!(
                desc.contains(p),
                "run description omits enforced pattern {p:?}"
            );
        }
    }

    #[test]
    fn run_requires_runner_script_and_cwd() {
        let schema = run_input_schema();
        let required = schema["required"].as_array().unwrap();
        for field in ["runner", "script", "cwd"] {
            assert!(
                required.iter().any(|v| v == field),
                "{field} must be required"
            );
        }
    }

    #[test]
    fn env_values_are_nullable_in_the_schema() {
        // "absent", "null" and "a string" are three distinct states; a schema that types
        // values as plain strings would make null-removal unexpressible.
        let schema = run_input_schema();
        let types = &schema["properties"]["env"]["additionalProperties"]["type"];
        assert!(types.as_array().unwrap().iter().any(|v| v == "null"));
    }
}
