//! The MCP server: tool listing, argument validation and dispatch.
//!
//! Two shapes here are not incidental and should not be "simplified":
//!
//! * **Every result carries a text block.** `outputSchema` governs non-error results only,
//!   so an error whose content array is empty gives the model nothing to correct from. And
//!   at least one widely used agent does not forward `structuredContent` into model context
//!   at all, which would make every successful run look empty. `structured_content` is set
//!   as well, for clients that do consume it.
//! * **Over-cap calls are rejected, not queued.** A client that abandons a call does not
//!   tell the server, so an abandoned call holds its slot for the whole timeout. Queueing
//!   would stack live callers behind work nobody is waiting for.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerInfo, Tool,
    ToolAnnotations,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler, ServiceExt};
use serde_json::{json, Map, Value};
use tokio::sync::Semaphore;

use super::exec::{self, ExecRequest, OnOutputLimit};
use super::tools;
use super::workdir::InstanceDir;
use crate::cli::McpServeArgs;
use crate::errors::AikitError;
use crate::policy::script as policy;

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 3_600_000;
const DEFAULT_MAX_BYTES: usize = 32 * 1024 * 1024;
/// An upper bound is needed for the same reason `null` is refused: the budget exists so
/// concurrent captures cannot exhaust memory, and a budget the caller can set arbitrarily
/// high does not bound anything. At the cap, `MAX_CONCURRENCY` saturated calls hold ~1 GiB.
const MAX_MAX_BYTES: usize = 128 * 1024 * 1024;
const MAX_CONCURRENCY: usize = 8;

/// Accepted `run` argument names, and accepted keys inside `limits`.
///
/// Both schemas declare `additionalProperties: false`, but validation here is hand-written,
/// so nothing would otherwise enforce it. Silently ignoring an unknown key is the worst
/// outcome available: `{"limits": {"timeoutMs": 5000}}` would run for the 120 s default
/// while the caller believes it bounded the call to five seconds.
const RUN_KEYS: &[&str] = &[
    "runner",
    "script",
    "cwd",
    "stdin",
    "env",
    "env_base",
    "limits",
    "acknowledge_forbidden",
];
const LIMIT_KEYS: &[&str] = &["timeout_ms", "max_bytes", "on_output_limit"];

/// Reject unknown keys, naming the accepted ones so a typo is self-correctable.
fn reject_unknown_keys(
    map: &Map<String, Value>,
    allowed: &[&str],
    what: &str,
) -> Result<(), String> {
    for key in map.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!(
                "unknown {what} key {key:?}. Accepted keys: {}.",
                allowed.join(", ")
            ));
        }
    }
    Ok(())
}
/// `list_runners` resolves interpreters, which touches PATH. A stale network entry can
/// stall that for tens of seconds, and this is often the first call a client makes.
const LIST_RUNNERS_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn run(args: McpServeArgs) -> Result<(), AikitError> {
    // Reclaim directories left by servers that died without cleaning up. Liveness is
    // decided by taking the lock, never by a PID, which would be reused.
    super::workdir::sweep_orphans();

    let handler = AikitMcp {
        slots: Arc::new(Semaphore::new(MAX_CONCURRENCY)),
        quiet: args.quiet,
    };

    let service = handler
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| AikitError::other(format!("MCP server failed to start: {e}")))?;

    service
        .waiting()
        .await
        .map_err(|e| AikitError::other(format!("MCP server stopped: {e}")))?;
    Ok(())
}

#[derive(Clone)]
struct AikitMcp {
    slots: Arc<Semaphore>,
    quiet: bool,
}

impl AikitMcp {
    fn log(&self, msg: &str) {
        if !self.quiet {
            eprintln!("aikit mcp: {msg}");
        }
    }
}

impl ServerHandler for AikitMcp {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        // The version the SDK falls back to when a client proposes one it does not know.
        // It is NOT a ceiling: the SDK echoes any revision in `supported_protocol_versions`
        // (defaulted to every revision it knows), so a client asking for a newer one gets
        // it. `server/discover` is served from this same info, so a client may skip the
        // handshake entirely — both lifecycles work.
        info.protocol_version = ProtocolVersion::V_2025_11_25;
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        // Identify as aikit. `Implementation::from_build_env()` reads the *SDK's* build
        // environment and reports "rmcp", which would make this server unidentifiable in a
        // client's server list.
        let mut me = Implementation::default();
        me.name = "aikit".into();
        me.version = env!("CARGO_PKG_VERSION").into();
        info.server_info = me;
        info.instructions = Some(tools::server_instructions());
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        // Log the NEGOTIATED version, not the one the client asked for.
        self.log(&format!(
            "negotiated protocol {:?}, client {:?}",
            context.protocol_version(),
            context.client_info()
        ));

        let mut run = Tool::new(
            tools::RUN,
            tools::run_description(),
            Arc::new(tools::run_input_schema()),
        );
        run.output_schema = Some(Arc::new(tools::run_output_schema()));
        run.annotations = Some(ToolAnnotations::from_raw(
            None,
            Some(false), // read_only
            Some(true),  // destructive
            Some(false), // idempotent
            Some(true),  // open_world
        ));

        let mut list = Tool::new(
            tools::LIST_RUNNERS,
            tools::list_runners_description(),
            Arc::new(tools::list_runners_input_schema()),
        );
        list.output_schema = Some(Arc::new(tools::list_runners_output_schema()));
        list.annotations = Some(ToolAnnotations::from_raw(
            None,
            Some(true),
            Some(false),
            Some(true),
            Some(false),
        ));

        let mut guide = Tool::new(
            tools::AGENTS_MD,
            tools::agents_md_description(),
            Arc::new(tools::agents_md_input_schema()),
        );
        guide.output_schema = Some(Arc::new(tools::agents_md_output_schema()));
        guide.annotations = Some(ToolAnnotations::from_raw(
            None,
            Some(true),  // read_only
            Some(false), // destructive
            Some(true),  // idempotent
            Some(false), // open_world
        ));

        Ok(ListToolsResult::with_all_items(vec![run, list, guide]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let result = match request.name.as_ref() {
            tools::LIST_RUNNERS => list_runners().await,
            tools::AGENTS_MD => agents_md(),
            tools::RUN => {
                self.call_run(request.arguments.unwrap_or_default(), context)
                    .await?
            }
            other => tool_error(format!(
                "unknown tool {other:?}; this server provides {:?}, {:?} and {:?}",
                tools::RUN,
                tools::LIST_RUNNERS,
                tools::AGENTS_MD
            )),
        };
        // `Complete` is the only variant this server produces: the Tasks extension is not
        // implemented, and the target client advertises no capabilities for it.
        Ok(CallToolResponse::Complete(result))
    }
}

impl AikitMcp {
    async fn call_run(
        &self,
        args: Map<String, Value>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let spec = match RunSpec::parse(&args) {
            Ok(s) => s,
            Err(msg) => return Ok(tool_error(msg)),
        };

        // Resolved before any early return, so every result carries it. aikit chooses these
        // flags, not the caller — PowerShell gets `-NoProfile -NonInteractive
        // -ExecutionPolicy Bypass`, cmd gets `/d` — and a caller cannot otherwise tell which
        // invocation produced an outcome, so the choice is reported rather than implied.
        let argv_flags = policy::mcp_runner_flags(&spec.runner);

        // Measured on PowerShell 5.1: a script ending in a failing native command exits 0
        // through `-File`, masking the failure. aikit generates this file, so it fixes that
        // here rather than documenting it as a hazard for the caller to work around.
        // Resolved alongside argv so every result reports it, including the early returns.
        let epilogue = policy::runner_exit_epilogue(&spec.runner);
        // aikit generates this script file, so the epilogue is always applied where the
        // runner needs one — which is what makes this `epilogue` rather than `unpropagated`
        // here, unlike `script run`, which executes a file the caller owns.
        let exit_code_source = policy::exit_code_source(&spec.runner, epilogue.is_some());

        // Reject rather than queue: an abandoned call holds its slot for the full timeout.
        let Ok(_slot) = self.slots.clone().try_acquire_owned() else {
            return Ok(structured_result(
                json!({
                    "stop_reason": "server_limit",
                    "exit_code": Value::Null,
                    "stdout": "",
                    "stderr": format!(
                        "server is at its concurrency limit of {MAX_CONCURRENCY}; retry shortly"
                    ),
                    "stdout_truncated": false,
                    "stderr_truncated": false,
                    "duration_ms": 0,
                    "runner": spec.runner,
                    "program": "",
                    "argv_flags": argv_flags,
                    "exit_code_epilogue": epilogue,
                    "exit_code_source": exit_code_source,
                    "containment": exec::containment(),
                }),
                false,
            ));
        };

        let dir = match InstanceDir::create() {
            Ok(d) => d,
            Err(e) => {
                return Ok(run_failure(
                    "setup_failed",
                    format!("setup_failed: temp directory: {e}"),
                    &spec,
                    &argv_flags,
                    epilogue,
                ))
            }
        };
        let script_path = match dir.write_script(
            &spec.runner,
            &spec.script,
            policy::runner_script_extension(&spec.runner),
            policy::runner_wants_crlf(&spec.runner),
            policy::runner_wants_bom(&spec.runner),
            epilogue,
        ) {
            Ok(p) => p,
            Err(e) => {
                return Ok(run_failure(
                    "setup_failed",
                    format!("setup_failed: writing the script: {e}"),
                    &spec,
                    &argv_flags,
                    epilogue,
                ))
            }
        };

        // The request's own cancellation token goes straight to the executor: no bridging
        // task to outlive the call, and no second signal that could miss a wakeup.
        let cancel = context.ct.clone();

        let req = ExecRequest {
            program: spec.program.clone(),
            argv_flags: argv_flags.clone(),
            script_path,
            cwd: spec.cwd.clone(),
            stdin: spec.stdin.clone(),
            env: spec.env.clone(),
            env_minimal: spec.env_minimal,
            timeout: Duration::from_millis(spec.timeout_ms),
            max_bytes: spec.max_bytes,
            on_output_limit: spec.on_output_limit,
        };

        match exec::execute(req, cancel).await {
            Err(e) => Ok(run_failure(
                "spawn_failed",
                format!("spawn_failed: {e}"),
                &spec,
                &argv_flags,
                epilogue,
            )),
            // Cancelled: per the protocol the receiver sends no response for a cancelled
            // request, so the error here is never delivered — it just closes the call out.
            Ok(None) => Err(ErrorData::internal_error("request cancelled", None)),
            Ok(Some(out)) => Ok(structured_result(
                json!({
                    "stop_reason": out.stop_reason.as_str(),
                    "exit_code": match out.exit_code {
                        Some(c) => Value::from(c),
                        None => Value::Null,
                    },
                    "stdout": out.stdout,
                    "stderr": out.stderr,
                    "stdout_truncated": out.stdout_truncated,
                    "stderr_truncated": out.stderr_truncated,
                    "duration_ms": out.duration_ms,
                    "runner": spec.runner,
                    "program": spec.program,
                    "argv_flags": argv_flags,
                    "exit_code_epilogue": epilogue,
                    "exit_code_source": exit_code_source,
                    "containment": out.containment,
                }),
                false,
            )),
        }
    }
}

/// Validated `run` arguments.
struct RunSpec {
    runner: String,
    program: String,
    script: String,
    cwd: std::path::PathBuf,
    stdin: Option<String>,
    env: BTreeMap<String, Option<String>>,
    env_minimal: bool,
    timeout_ms: u64,
    max_bytes: Option<usize>,
    on_output_limit: OnOutputLimit,
}

impl RunSpec {
    fn parse(args: &Map<String, Value>) -> Result<Self, String> {
        reject_unknown_keys(args, RUN_KEYS, "`run` argument")?;

        let runner = args
            .get("runner")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "`runner` is required and is never inferred. Call {:?} and pass an exact \
                     name from it.",
                    tools::LIST_RUNNERS
                )
            })?
            .to_string();

        if !policy::is_known_runner_name(&runner) {
            return Err(format!(
                "unknown runner {runner:?}. Known runners: {}.",
                policy::all_runner_names().join(", ")
            ));
        }
        let program = policy::resolve_runner_program(&runner).ok_or_else(|| {
            format!(
                "runner {runner:?} is not available on this host. Call {:?} to see which \
                 runners are installed; this server never substitutes a different one.",
                tools::LIST_RUNNERS
            )
        })?;

        let script = args
            .get("script")
            .and_then(Value::as_str)
            .ok_or("`script` is required")?
            .to_string();

        // Acknowledgement is parsed before the scan so the scan can consult it.
        let acknowledged: Vec<String> = match args.get("acknowledge_forbidden") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        Value::String(s) => out.push(s.clone()),
                        other => {
                            return Err(format!(
                                "`acknowledge_forbidden` entries must be strings, got {other}"
                            ))
                        }
                    }
                }
                out
            }
            Some(other) => {
                return Err(format!(
                    "`acknowledge_forbidden` must be an array of pattern strings, got {other}"
                ))
            }
        };

        let matches = policy::scan_forbidden_all(&script);
        let unacked = policy::unacknowledged(&matches, &acknowledged);
        if !unacked.is_empty() {
            // Name the location and the exact escape. A refusal with no compliant route
            // pushes a model toward whatever unaudited shell it can otherwise reach.
            let where_ = matches
                .iter()
                .filter(|m| unacked.contains(&m.pattern))
                .map(|m| format!("{:?} at line {}", m.pattern, m.line))
                .collect::<Vec<_>>()
                .join("; ");
            let list = unacked
                .iter()
                .map(|p| format!("{p:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "script contains a forbidden pattern: {where_}. This is an accident guard, \
                 not a security boundary. Rewrite the script without it, or if the operation \
                 is intended, repeat the call with \"acknowledge_forbidden\": [{list}]."
            ));
        }

        let cwd_raw = args
            .get("cwd")
            .and_then(Value::as_str)
            .ok_or("`cwd` is required and must be an absolute path")?;
        let cwd = std::path::PathBuf::from(cwd_raw);
        if !cwd.is_absolute() {
            return Err(format!("`cwd` must be absolute, got {cwd_raw:?}"));
        }
        if !cwd.is_dir() {
            return Err(format!(
                "`cwd` must be an existing directory, and {cwd_raw:?} is not one"
            ));
        }
        let cwd = normalize_cwd(&cwd);

        // Wrong-typed optional arguments are rejected rather than ignored. Falling back to
        // the default would run something other than what was asked for without saying so.
        let stdin = match args.get("stdin") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.clone()),
            Some(other) => return Err(format!("`stdin` must be a string, got {other}")),
        };

        // Three states, deliberately distinguished: absent, null (remove), string (set).
        let mut env = BTreeMap::new();
        match args.get("env") {
            None | Some(Value::Null) => {}
            Some(Value::Object(map)) => {
                for (k, v) in map {
                    match v {
                        Value::Null => env.insert(k.clone(), None),
                        Value::String(s) => env.insert(k.clone(), Some(s.clone())),
                        other => {
                            return Err(format!("env[{k:?}] must be a string or null, got {other}"))
                        }
                    };
                }
            }
            Some(other) => {
                return Err(format!(
                    "`env` must be an object mapping names to strings or null, got {other}"
                ))
            }
        }

        let env_minimal = match args.get("env_base") {
            None | Some(Value::Null) => false,
            Some(Value::String(s)) if s == "inherit" => false,
            Some(Value::String(s)) if s == "minimal" => true,
            Some(other) => {
                return Err(format!(
                    "`env_base` must be \"inherit\" or \"minimal\", got {other}"
                ))
            }
        };

        // A malformed `limits` must not degrade silently to defaults: the caller asked for
        // a bound and would never learn it was ignored.
        let limits = match args.get("limits") {
            None | Some(Value::Null) => None,
            Some(Value::Object(map)) => {
                reject_unknown_keys(map, LIMIT_KEYS, "`limits`")?;
                Some(map)
            }
            Some(other) => {
                return Err(format!("`limits` must be an object, got {other}"));
            }
        };
        let timeout_ms = match limits.and_then(|l| l.get("timeout_ms")) {
            None => DEFAULT_TIMEOUT_MS,
            Some(Value::Null) => {
                return Err(format!(
                    "`limits.timeout_ms` cannot be null. Cancelling a call does not stop the \
                     script, so the timeout is the only thing that can — raise it instead \
                     (maximum {MAX_TIMEOUT_MS} ms)."
                ))
            }
            Some(v) => {
                let ms = v
                    .as_u64()
                    .ok_or("`limits.timeout_ms` must be a positive integer")?;
                if ms == 0 || ms > MAX_TIMEOUT_MS {
                    return Err(format!(
                        "`limits.timeout_ms` must be between 1 and {MAX_TIMEOUT_MS}"
                    ));
                }
                ms
            }
        };
        // Null is rejected for the same reason as the timeout: uncapped capture across
        // eight concurrent calls is an unrecoverable OOM on the default path, and a caller
        // who needs more can raise the budget rather than remove it.
        let max_bytes = match limits.and_then(|l| l.get("max_bytes")) {
            None => Some(DEFAULT_MAX_BYTES),
            Some(Value::Null) => {
                return Err(
                    "`limits.max_bytes` cannot be null; raise it instead — uncapped capture \
                     can exhaust memory and take every in-flight call down with it"
                        .to_string(),
                )
            }
            Some(v) => {
                let n = v
                    .as_u64()
                    .filter(|n| *n > 0)
                    .ok_or("`limits.max_bytes` must be a positive integer")?;
                if n > MAX_MAX_BYTES as u64 {
                    return Err(format!(
                        "`limits.max_bytes` must be between 1 and {MAX_MAX_BYTES}. A budget \
                         above that bounds nothing: it is the cap that keeps concurrent \
                         captures from exhausting memory."
                    ));
                }
                Some(n as usize)
            }
        };
        let on_output_limit = match limits.and_then(|l| l.get("on_output_limit")) {
            None => OnOutputLimit::Truncate,
            Some(Value::String(s)) if s == "truncate" => OnOutputLimit::Truncate,
            Some(Value::String(s)) if s == "kill" => OnOutputLimit::Kill,
            Some(other) => {
                return Err(format!(
                    "`limits.on_output_limit` must be \"truncate\" or \"kill\", got \
                     {other:?}"
                ))
            }
        };

        Ok(RunSpec {
            runner,
            program,
            script,
            cwd,
            stdin,
            env,
            env_minimal,
            timeout_ms,
            max_bytes,
            on_output_limit,
        })
    }
}

/// Canonicalize for validation, then hand the child a path it can actually use.
///
/// On Windows `canonicalize` yields an extended-length `\\?\` path, which cmd.exe cannot
/// parse — and cmd cannot use a UNC path as a working directory at all, falling back
/// silently to somewhere else. Validating with the canonical form while passing the plain
/// one keeps both properties.
fn normalize_cwd(path: &std::path::Path) -> std::path::PathBuf {
    // The verbatim-prefix stripping lives in one place; this differs only in falling back
    // to the caller's path rather than propagating, since an unresolvable `cwd` is already
    // rejected during argument validation.
    crate::formats::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

async fn list_runners() -> CallToolResult {
    // The timeout must wrap the *resolution*, not a loop over its results: availability
    // probes PATH for every runner up front, so a stale network entry stalls inside that
    // call. Checking the clock afterwards would bound nothing — and this is typically the
    // first call a client makes.
    let probe = tokio::task::spawn_blocking(|| {
        policy::runner_availability()
            .into_iter()
            .map(|r| {
                json!({
                    "name": r.name,
                    "applicable": r.applicable,
                    "available": r.available,
                    // Same shape `aikit doctor` reports: a client that can only reach aikit
                    // over MCP should not get a thinner answer than one with a shell.
                    "path": r.path,
                    "version": r.version,
                    "reason": r.reason,
                })
            })
            .collect::<Vec<Value>>()
    });

    match tokio::time::timeout(LIST_RUNNERS_TIMEOUT, probe).await {
        Ok(Ok(runners)) => structured_result(json!({ "runners": runners }), false),
        Ok(Err(e)) => tool_error(format!("runner discovery failed: {e}")),
        Err(_) => tool_error(format!(
            "runner discovery did not finish within {}s. This usually means an entry on \
             PATH is unreachable — a disconnected network drive stalls the probe. Fix PATH \
             and retry.",
            LIST_RUNNERS_TIMEOUT.as_secs()
        )),
    }
}

/// Build a result carrying BOTH a text block and structured content.
///
/// The text block is not optional: at least one major client never forwards
/// `structuredContent` into model context, so a structured-only result reaches the model as
/// an empty response.
fn structured_result(value: Value, is_error: bool) -> CallToolResult {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    let mut result = if is_error {
        CallToolResult::error(vec![ContentBlock::text(text)])
    } else {
        CallToolResult::success(vec![ContentBlock::text(text)])
    };
    result.structured_content = Some(value);
    result
}

/// Resolve the agent guide: the embedded text, or the file named by `AGENTS_MD`.
///
/// An override that cannot be read is an **error**, not a silent fall back to the embedded
/// text. Someone who pointed the variable at a file wants that file; quietly serving
/// different guidance would be the kind of failure nobody notices.
pub fn resolve_agents_md() -> Result<(String, String), String> {
    match std::env::var_os(tools::AGENTS_MD_OVERRIDE) {
        None => Ok((
            tools::EMBEDDED_AGENTS_MD.to_string(),
            "embedded".to_string(),
        )),
        Some(path) => {
            let path = std::path::PathBuf::from(path);
            match std::fs::read_to_string(&path) {
                Ok(text) => Ok((text, format!("override:{}", path.display()))),
                Err(e) => Err(format!(
                    "{} names {} but it could not be read: {e}. Unset {} to use the guide \
                     embedded in this binary.",
                    tools::AGENTS_MD_OVERRIDE,
                    path.display(),
                    tools::AGENTS_MD_OVERRIDE
                )),
            }
        }
    }
}

fn agents_md() -> CallToolResult {
    let (content, source) = match resolve_agents_md() {
        Ok(pair) => pair,
        Err(msg) => return tool_error(msg),
    };

    // The text block is the markdown itself, not a JSON rendering of it. Every other tool
    // returns a small record where pretty-printed JSON reads fine; here that would escape
    // every newline and roughly double the payload for a document meant to be read.
    let mut result = CallToolResult::success(vec![ContentBlock::text(content.clone())]);
    result.structured_content = Some(json!({
        "content": content,
        "source": source,
        "aikit_version": env!("CARGO_PKG_VERSION"),
    }));
    result
}

/// A tool-level error the model can correct from. Always carries text: `outputSchema`
/// governs non-error results only, so an empty content array here would say nothing.
fn tool_error(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

/// A run that failed before or during spawn, reported in the same shape as a run that
/// finished. `setup_failed` and `spawn_failed` are declared `stop_reason` values, so they
/// must actually appear in that field — reporting them only as a text prefix would leave a
/// model that reads `stop_reason` unable to tell what happened. The result is still flagged
/// as an error, because a client that surfaces `isError` is how the user sees it went wrong.
fn run_failure(
    stop_reason: &str,
    message: String,
    spec: &RunSpec,
    argv_flags: &[String],
    epilogue: Option<&str>,
) -> CallToolResult {
    structured_result(
        json!({
            "stop_reason": stop_reason,
            "exit_code": Value::Null,
            "stdout": "",
            "stderr": message,
            "stdout_truncated": false,
            "stderr_truncated": false,
            "duration_ms": 0,
            "runner": spec.runner,
            "program": spec.program,
            "argv_flags": argv_flags,
            "exit_code_epilogue": epilogue,
            // Nothing ran, so no exit code exists to trust — but the shape stays uniform so
            // a client validating against the declared schema never sees a missing field.
            "exit_code_source": policy::exit_code_source(&spec.runner, epilogue.is_some()),
            "containment": exec::containment(),
        }),
        true,
    )
}
