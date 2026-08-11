//! `aikit mcp` — CLI surface and a full stdio protocol exchange.
//!
//! The protocol tests drive the real binary as an MCP client would: they write JSON-RPC on
//! stdin and read it back from stdout. That is the only way to catch the failures that
//! matter here — a result the model cannot see, or stray output corrupting the transport.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::TempDir;

fn bin() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin("aikit"))
}

/// Help output with all whitespace collapsed to single spaces.
///
/// clap wraps long paragraphs to the terminal width, so a phrase can be split across lines.
/// These assertions are about what the help *says*, not how it is laid out, and matching
/// raw text would couple them to the wrap position.
fn help(args: &[&str]) -> String {
    let out = bin().args(args).output().expect("run");
    assert!(out.status.success(), "help should exit 0");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// ------------------------------------------------------------------ help surface

#[test]
fn mcp_help_is_available() {
    let text = help(&["mcp", "--help"]);
    assert!(text.contains("Model Context Protocol"));
    assert!(text.contains("--quiet"));
}

#[test]
fn mcp_runs_the_server_with_no_subcommand() {
    // `aikit mcp` IS the server. There is exactly one thing to do, so requiring a verb
    // would add a token that never varies.
    let mut s = Server::start();
    let resp = s.request("tools/list", json!({}));
    assert!(resp["result"]["tools"].is_array());
}

#[test]
fn mcp_takes_no_subcommand() {
    // `aikit mcp` IS the server; there is no verb to add. A `serve` alias would be
    // compatibility with a surface that never shipped, and a second way to invoke the
    // same thing is a second thing to keep consistent.
    let out = bin().args(["mcp", "serve"]).output().expect("run");
    assert!(
        !out.status.success(),
        "`mcp serve` should not be accepted; there is exactly one invocation"
    );
}

#[test]
fn mcp_help_states_it_is_not_a_sandbox() {
    assert!(
        help(&["mcp", "--help"]).contains("NOT a security sandbox"),
        "the help must not let a reader assume containment"
    );
}

#[test]
fn mcp_help_describes_the_divergence_from_script_run() {
    let text = help(&["mcp", "--help"]);
    for expected in [
        "no run record",
        "never inferred",
        "no repository is required",
    ] {
        assert!(
            text.contains(expected),
            "mcp help should state {expected:?}; the MCP contract deliberately differs \
             from `script run` and a reader must not assume they match"
        );
    }
}

#[test]
fn mcp_help_warns_that_cancelling_is_not_a_reliable_stop() {
    let text = help(&["mcp", "--help"]);
    assert!(
        text.contains("not a reliable stop"),
        "a caller must be told that cancelling may leave the script running"
    );
    assert!(
        text.contains("not required to send it"),
        "the reason matters: the server kills the tree when it IS told, so the warning is \
         about clients that never tell it — stating only 'cancelling does nothing' would \
         be false"
    );
}

#[test]
fn mcp_help_does_not_claim_that_no_file_is_written() {
    // A temporary script does exist while it runs. The honest claim is that nothing lands
    // in the user's repository or working tree.
    assert!(
        help(&["mcp", "--help"]).contains("temporary script does exist"),
        "the help must not imply the script never touches disk"
    );
}

// ------------------------------------------------------------------ protocol harness

struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl Server {
    fn start() -> Self {
        let mut child = bin()
            .args(["mcp", "--quiet"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn aikit mcp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        let mut s = Server {
            child,
            stdin,
            stdout,
            next_id: 0,
        };
        s.handshake();
        s
    }

    fn send(&mut self, msg: Value) {
        writeln!(self.stdin, "{msg}").expect("write");
        self.stdin.flush().expect("flush");
    }

    /// Read messages until one carries `id`, so notifications never desynchronize a test.
    fn read_response(&mut self, id: i64) -> Value {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            assert!(Instant::now() < deadline, "timed out waiting for id {id}");
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line).expect("read");
            assert!(n > 0, "server closed stdout while waiting for id {id}");
            let Ok(msg) = serde_json::from_str::<Value>(line.trim()) else {
                panic!("stdout carried a non-JSON line, which corrupts the transport: {line:?}");
            };
            if msg.get("id").and_then(Value::as_i64) == Some(id) {
                return msg;
            }
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        self.read_response(id)
    }

    /// Send a request and return its id **without** waiting for the response.
    ///
    /// Needed for cancellation: the protocol forbids responding to a cancelled request, so a
    /// test that blocks on the response would hang forever waiting for something that must
    /// never arrive.
    fn send_request(&mut self, method: &str, params: Value) -> i64 {
        self.next_id += 1;
        let id = self.next_id;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        id
    }

    /// Read until `id` arrives, returning it along with every other response id seen on the
    /// way. Those ids are the evidence for "no response was sent for the cancelled call".
    fn read_response_collecting(&mut self, id: i64) -> (Value, Vec<i64>) {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut seen = Vec::new();
        loop {
            assert!(Instant::now() < deadline, "timed out waiting for id {id}");
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line).expect("read");
            assert!(n > 0, "server closed stdout while waiting for id {id}");
            let Ok(msg) = serde_json::from_str::<Value>(line.trim()) else {
                panic!("stdout carried a non-JSON line, which corrupts the transport: {line:?}");
            };
            match msg.get("id").and_then(Value::as_i64) {
                Some(got) if got == id => return (msg, seen),
                Some(got) => seen.push(got),
                None => {} // a notification; not a response to anything
            }
        }
    }

    fn handshake(&mut self) {
        let resp = self.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "aikit-test", "version": "1"}
            }),
        );
        assert_eq!(resp["result"]["protocolVersion"], "2025-11-25");
        self.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    }

    fn call_tool(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": args}))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A runner that actually resolves on this platform.
///
/// Tests about *validation* assert platform-independent behaviour and should run everywhere,
/// but they still have to name a runner — and `runner` is resolved before `cwd`, `script`
/// and `limits` are looked at. Hardcoding `sh` makes them fail on Windows with "not
/// available on this host" instead of the error they exist to assert, which reads as a real
/// failure. `cmd` ships with every Windows install, `sh` with every POSIX one.
fn any_runner() -> &'static str {
    if cfg!(windows) {
        "cmd"
    } else {
        "sh"
    }
}

/// An absolute directory that exists on this platform. `/tmp` does not exist on Windows.
fn any_dir() -> String {
    std::env::temp_dir().to_string_lossy().into_owned()
}

fn result_text(resp: &Value) -> String {
    resp["result"]["content"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

// ------------------------------------------------------------------ protocol behaviour

#[test]
fn the_server_identifies_itself_as_aikit() {
    // `Implementation::from_build_env()` reports the SDK's own crate ("rmcp"), which would
    // leave this server unidentifiable in a client's list of configured servers.
    let mut s = Server::start();
    let resp = s.request(
        "initialize",
        json!({"protocolVersion": "2025-11-25", "capabilities": {},
               "clientInfo": {"name": "t", "version": "1"}}),
    );
    assert_eq!(resp["result"]["serverInfo"]["name"], "aikit");
    assert_eq!(
        resp["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn an_unrecognised_protocol_version_falls_back_to_the_implemented_one() {
    // The SDK echoes any revision it knows, so this is a fallback rather than a ceiling —
    // which is exactly why the docs must not claim the server "only speaks 2025-11-25".
    let mut s = Server::start();
    let resp = s.request(
        "initialize",
        json!({"protocolVersion": "9999-99-99", "capabilities": {},
               "clientInfo": {"name": "t", "version": "1"}}),
    );
    assert_eq!(resp["result"]["protocolVersion"], "2025-11-25");
}

#[test]
fn server_discover_answers_without_a_handshake() {
    // The stateless discovery flow works: no `initialize`, one request, full answer.
    //
    // A discover request uses the inline lifecycle, so it must carry self-contained
    // `_meta`. Omitting it fails the lifecycle contract — which looks like "unimplemented"
    // but is a malformed request, so this test sends a well-formed one.
    let mut child = bin()
        .args(["mcp", "--quiet"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));

    let req = json!({
        "jsonrpc": "2.0", "id": 1, "method": "server/discover",
        "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientInfo": {"name": "aikit-test", "version": "1"},
            "io.modelcontextprotocol/clientCapabilities": {},
        }}
    });
    writeln!(stdin, "{req}").expect("write");
    stdin.flush().expect("flush");

    let mut line = String::new();
    stdout.read_line(&mut line).expect("read");
    let _ = child.kill();
    let _ = child.wait();
    let resp: Value = serde_json::from_str(line.trim()).expect("json");

    assert!(resp.get("error").is_none(), "discover failed: {resp}");
    let result = &resp["result"];
    assert!(
        result["supportedVersions"]
            .as_array()
            .is_some_and(|v| v.iter().any(|x| x == "2025-11-25")),
        "discovery must advertise the versions this server negotiates: {result}"
    );
    assert!(result["capabilities"]["tools"].is_object());
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "aikit"
    );
}

#[test]
fn tools_list_declares_both_tools_with_schemas() {
    let mut s = Server::start();
    let resp = s.request("tools/list", json!({}));
    let tools = resp["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"run"));
    assert!(names.contains(&"list_runners"));

    for tool in tools {
        assert!(
            tool["inputSchema"].is_object(),
            "inputSchema is mandatory under the tools capability; {} lacks one",
            tool["name"]
        );
        assert!(tool["outputSchema"].is_object());
    }

    let run = tools.iter().find(|t| t["name"] == "run").unwrap();
    let required = run["inputSchema"]["required"].as_array().unwrap();
    for field in ["runner", "script", "cwd"] {
        assert!(
            required.iter().any(|v| v == field),
            "{field} must be required"
        );
    }
    assert_eq!(run["annotations"]["destructiveHint"], json!(true));
}

/// Assert `value` conforms to the parts of `schema` this needs to police: every `required`
/// property is present, and — when the schema closes the object with
/// `additionalProperties: false` — no property appears that the schema does not declare.
/// Object items inside declared arrays are checked the same way.
///
/// Deliberately not a full JSON Schema implementation. It catches exactly the failure that
/// motivated it: a result grows a field while its declared schema stays closed, so a client
/// that validates responses rejects one aikit considers correct — and nothing in the suite
/// notices, because the field is present and the test asserting on it passes.
fn assert_matches_schema(value: &Value, schema: &Value, label: &str) {
    let props = match schema["properties"].as_object() {
        Some(p) => p,
        None => return,
    };

    if let Some(required) = schema["required"].as_array() {
        for field in required {
            let name = field.as_str().unwrap();
            assert!(
                value.get(name).is_some(),
                "{label}: schema requires {name:?} but the result omits it: {value}"
            );
        }
    }

    if schema["additionalProperties"] == json!(false) {
        for key in value.as_object().expect("an object result").keys() {
            assert!(
                props.contains_key(key),
                "{label}: result carries {key:?}, which the declared outputSchema does not \
                 allow (additionalProperties is false). Either declare it or stop emitting it."
            );
        }
    }

    // Recurse into declared arrays of objects (e.g. list_runners' `runners`).
    for (name, prop_schema) in props {
        let Some(items) = prop_schema.get("items") else {
            continue;
        };
        let Some(array) = value.get(name).and_then(|v| v.as_array()) else {
            continue;
        };
        for (i, element) in array.iter().enumerate() {
            if element.is_object() {
                assert_matches_schema(element, items, &format!("{label}.{name}[{i}]"));
            }
        }
    }
}

#[test]
fn results_conform_to_their_declared_output_schemas() {
    let mut s = Server::start();

    let listed = s.request("tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().expect("tools").clone();
    let schema_for = |name: &str| -> Value {
        tools
            .iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("tool {name} not listed"))["outputSchema"]
            .clone()
    };

    let runners = s.call_tool("list_runners", json!({}));
    assert_matches_schema(
        &runners["result"]["structuredContent"],
        &schema_for("list_runners"),
        "list_runners",
    );

    let script = if cfg!(windows) {
        "@echo hi\r\n"
    } else {
        "echo hi\n"
    };
    let runner = if cfg!(windows) { "cmd" } else { "sh" };
    let run = s.call_tool(
        "run",
        json!({
            "runner": runner,
            "script": script,
            "cwd": std::env::temp_dir().to_string_lossy(),
        }),
    );
    assert_matches_schema(
        &run["result"]["structuredContent"],
        &schema_for("run"),
        "run",
    );
}

/// A script that records that it started, waits, then records that it finished.
///
/// The two markers are what make cancellation observable from outside the process: a killed
/// script leaves the first and never writes the second.
fn two_phase_script(
    started: &std::path::Path,
    finished: &std::path::Path,
    secs: u32,
) -> (String, String) {
    if cfg!(windows) {
        // `timeout` refuses to run with stdin redirected (which it is here), so use ping,
        // the conventional cmd sleep. n+1 pings ≈ n seconds.
        (
            "cmd".to_string(),
            format!(
                "@type nul > \"{}\"\r\n@ping -n {} 127.0.0.1 > nul\r\n@type nul > \"{}\"\r\n",
                started.display(),
                secs + 1,
                finished.display()
            ),
        )
    } else {
        (
            "sh".to_string(),
            format!(
                "touch '{}'\nsleep {}\ntouch '{}'\n",
                started.display(),
                secs,
                finished.display()
            ),
        )
    }
}

/// Block until `path` exists, or panic. Proves the script really started before we cancel.
fn wait_for_marker(path: &std::path::Path, within: Duration) {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!(
        "marker {} never appeared — the script did not start",
        path.display()
    );
}

#[test]
fn cancelling_a_call_kills_the_script_and_sends_no_response() {
    // The two halves of the documented contract, both observable from outside:
    //   1. the process tree is killed — the second marker never appears;
    //   2. no response is sent — the protocol forbids answering a cancelled request, which
    //      is also why `stop_reason` has no `cancelled` variant to report.
    let dir = TempDir::new().unwrap();
    let started = dir.path().join("started");
    let finished = dir.path().join("finished");
    let (runner, script) = two_phase_script(&started, &finished, 6);

    let mut s = Server::start();
    let call_id = s.send_request(
        "tools/call",
        json!({
            "name": "run",
            "arguments": {
                "runner": runner,
                "script": script,
                "cwd": dir.path().to_string_lossy(),
                // Far longer than the script needs, so the timeout cannot be what stops it —
                // otherwise this would silently retest the timeout path instead.
                "limits": {"timeout_ms": 120000}
            }
        }),
    );

    wait_for_marker(&started, Duration::from_secs(20));

    s.send(json!({
        "jsonrpc": "2.0",
        "method": "notifications/cancelled",
        "params": {"requestId": call_id, "reason": "cancelled by test"}
    }));

    // Wait past the point where an un-cancelled script would have finished and its response
    // would already be queued ahead of anything sent later. That ordering is what makes the
    // "no response" assertion below meaningful rather than a race.
    std::thread::sleep(Duration::from_secs(9));

    // The server must still be healthy and answering other work.
    let probe_id = s.send_request(
        "tools/call",
        json!({"name": "list_runners", "arguments": {}}),
    );
    let (probe, seen_before_probe) = s.read_response_collecting(probe_id);
    assert!(
        probe["result"].is_object(),
        "the server should still serve other calls after a cancellation: {probe}"
    );
    assert!(
        !seen_before_probe.contains(&call_id),
        "a cancelled request must receive no response, but id {call_id} was answered"
    );

    assert!(
        started.exists(),
        "sanity: the script should have started before being cancelled"
    );
    assert!(
        !finished.exists(),
        "the script survived cancellation — the process tree was not killed"
    );
}

#[test]
fn results_report_exit_code_trust_and_containment() {
    // `exit_code: 0` is not equally meaningful across runners, and "kills the process tree"
    // is not unconditionally true. Both are reported so a caller gating on an exit code, or
    // relying on cleanup, knows which guarantee it actually has.
    let mut s = Server::start();

    let (runner, script, expected_source) = if cfg!(windows) {
        // cmd needs an appended `exit /b %ERRORLEVEL%`; aikit writes the file, so it adds one.
        ("cmd", "@echo hi\r\n", "epilogue")
    } else {
        // A POSIX shell already exits with the last command's status.
        ("sh", "echo hi\n", "interpreter")
    };

    let resp = s.call_tool(
        "run",
        json!({
            "runner": runner,
            "script": script,
            "cwd": std::env::temp_dir().to_string_lossy(),
        }),
    );
    let sc = &resp["result"]["structuredContent"];
    assert_eq!(sc["exit_code_source"], expected_source);
    assert_eq!(
        sc["containment"],
        if cfg!(windows) {
            "job_object"
        } else {
            "process_group"
        }
    );
}

#[test]
fn successful_results_carry_a_text_block_as_well_as_structured_content() {
    // At least one widely used client never forwards structuredContent into model context,
    // so a structured-only result reaches the model as an empty response.
    let mut s = Server::start();
    let resp = s.call_tool("list_runners", json!({}));
    assert!(
        resp["result"]["structuredContent"].is_object(),
        "structured content should still be present for clients that read it"
    );
    let text = result_text(&resp);
    assert!(
        text.contains("runners"),
        "a result with no text block is invisible to some clients; got {text:?}"
    );
}

#[test]
fn errors_carry_a_text_message() {
    // outputSchema governs non-error results only, so an error with an empty content array
    // gives the model nothing to correct from.
    let mut s = Server::start();
    let resp = s.call_tool("run", json!({"script": "echo hi", "cwd": any_dir()}));
    assert_eq!(resp["result"]["isError"], json!(true));
    let text = result_text(&resp);
    assert!(
        text.contains("runner"),
        "the error must name the missing field; got {text:?}"
    );
}

#[test]
fn unknown_runner_is_a_correctable_error_naming_the_alternatives() {
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({"runner": "perl", "script": "print 1", "cwd": any_dir()}),
    );
    assert_eq!(resp["result"]["isError"], json!(true));
    let text = result_text(&resp);
    assert!(text.contains("perl"));
    assert!(
        // The known-runner list is the same table on every platform, so naming `sh` here is
        // about the message's content, not about this host.
        text.contains("sh"),
        "listing the known runners is what makes this self-correctable: {text:?}"
    );
}

#[test]
fn relative_cwd_is_rejected() {
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({"runner": any_runner(), "script": "true", "cwd": "relative/path"}),
    );
    assert_eq!(resp["result"]["isError"], json!(true));
    assert!(result_text(&resp).contains("absolute"));
}

#[test]
fn cwd_must_be_a_directory_not_merely_an_existing_path() {
    let file = std::env::temp_dir().join("aikit-mcp-cwd-test-file");
    std::fs::write(&file, b"x").expect("write");
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({"runner": any_runner(), "script": "true", "cwd": file.to_string_lossy()}),
    );
    let _ = std::fs::remove_file(&file);
    assert_eq!(resp["result"]["isError"], json!(true));
    assert!(result_text(&resp).contains("directory"));
}

#[test]
fn forbidden_patterns_are_refused_before_execution() {
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({"runner": any_runner(), "script": "git push origin main", "cwd": any_dir()}),
    );
    assert_eq!(resp["result"]["isError"], json!(true));
    assert!(result_text(&resp).contains("git push"));
}

#[test]
fn null_timeout_is_rejected_rather_than_disabling_the_only_stop_mechanism() {
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": any_runner(), "script": "true", "cwd": any_dir(),
            "limits": {"timeout_ms": null}
        }),
    );
    assert_eq!(resp["result"]["isError"], json!(true));
    let text = result_text(&resp);
    assert!(
        text.contains("cannot be null"),
        "cancellation is not delivered by every client, so the timeout must not be \
         disablable: {text:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_script_runs_and_reports_its_exit_code_as_success() {
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": "sh",
            "script": "echo out; echo err 1>&2; exit 3",
            "cwd": std::env::temp_dir().to_string_lossy(),
        }),
    );
    assert!(
        resp["result"]["isError"] != json!(true),
        "a non-zero exit code is a successful call, not a tool error"
    );
    let sc = &resp["result"]["structuredContent"];
    assert_eq!(sc["stop_reason"], "exited");
    assert_eq!(sc["exit_code"], json!(3));
    assert!(sc["stdout"].as_str().unwrap().contains("out"));
    assert!(sc["stderr"].as_str().unwrap().contains("err"));
    assert_eq!(sc["runner"], "sh");
}

#[cfg(windows)]
#[test]
fn a_script_runs_and_reports_its_exit_code_as_success() {
    // The Windows counterpart of the test above. Windows is the deployment target, so the
    // execute path needs coverage that actually runs there rather than only typechecking:
    // this is what exercises the Job Object, the `.cmd` extension, CRLF and the BOM rules.
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": "cmd",
            "script": "@echo off\r\necho out\r\necho err 1>&2\r\nexit /b 3",
            "cwd": std::env::temp_dir().to_string_lossy(),
        }),
    );
    assert!(
        resp["result"]["isError"] != json!(true),
        "a non-zero exit code is a successful call, not a tool error"
    );
    let sc = &resp["result"]["structuredContent"];
    assert_eq!(sc["stop_reason"], "exited");
    assert_eq!(sc["exit_code"], json!(3));
    assert!(sc["stdout"].as_str().unwrap().contains("out"));
    assert!(sc["stderr"].as_str().unwrap().contains("err"));
    assert_eq!(sc["runner"], "cmd");
}

#[cfg(unix)]
#[test]
fn stdin_is_delivered_and_then_closed() {
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": "sh",
            "script": "cat",
            "stdin": "hello from stdin",
            "cwd": std::env::temp_dir().to_string_lossy(),
        }),
    );
    let sc = &resp["result"]["structuredContent"];
    assert_eq!(sc["stop_reason"], "exited");
    assert!(sc["stdout"].as_str().unwrap().contains("hello from stdin"));
}

#[cfg(unix)]
#[test]
fn a_script_reading_stdin_with_none_supplied_does_not_hang() {
    // Absent stdin means an immediately-closed pipe. Inheriting instead would block for the
    // whole timeout against a client that has no stdin to give.
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": "sh", "script": "cat",
            "cwd": std::env::temp_dir().to_string_lossy(),
            "limits": {"timeout_ms": 10000}
        }),
    );
    assert_eq!(resp["result"]["structuredContent"]["stop_reason"], "exited");
}

#[cfg(unix)]
#[test]
fn a_hanging_script_is_stopped_by_its_timeout() {
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": "sh", "script": "sleep 30",
            "cwd": std::env::temp_dir().to_string_lossy(),
            "limits": {"timeout_ms": 1500}
        }),
    );
    let sc = &resp["result"]["structuredContent"];
    assert_eq!(sc["stop_reason"], "timeout");
    assert_eq!(
        sc["exit_code"],
        json!(null),
        "a timed-out process has no exit code; a synthetic one would be a lie"
    );
}

#[cfg(unix)]
#[test]
fn output_over_the_budget_is_truncated_and_flagged() {
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": "sh",
            "script": "head -c 100000 /dev/zero | tr '\\0' 'a'",
            "cwd": std::env::temp_dir().to_string_lossy(),
            "limits": {"max_bytes": 1024}
        }),
    );
    let sc = &resp["result"]["structuredContent"];
    assert_eq!(sc["stop_reason"], "output_limit");
    assert_eq!(sc["stdout_truncated"], json!(true));
    assert!(sc["stdout"].as_str().unwrap().len() <= 1024);
}

#[cfg(unix)]
#[test]
fn env_values_set_and_null_removes() {
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": "sh",
            "script": "echo \"[$AIKIT_TEST_SET][$PATH_SHOULD_BE_GONE]\"",
            "cwd": std::env::temp_dir().to_string_lossy(),
            "env": {"AIKIT_TEST_SET": "yes", "PATH_SHOULD_BE_GONE": null},
        }),
    );
    let out = resp["result"]["structuredContent"]["stdout"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(out.contains("[yes][]"), "got {out:?}");
}

#[cfg(unix)]
#[test]
fn minimal_env_still_provides_a_working_shell() {
    // The floor exists so a script can find programs: a floor without PATH produces
    // "not recognized" errors that look nothing like their cause.
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": "sh",
            "script": "echo \"path=${PATH:+set}\"",
            "cwd": std::env::temp_dir().to_string_lossy(),
            "env_base": "minimal",
        }),
    );
    let sc = &resp["result"]["structuredContent"];
    assert_eq!(sc["stop_reason"], "exited");
    assert!(sc["stdout"].as_str().unwrap().contains("path=set"));
}

#[test]
fn null_max_bytes_is_rejected_like_a_null_timeout() {
    // Uncapped capture across concurrent calls is an unrecoverable OOM, so the budget can
    // be raised but not removed — the same reasoning as the timeout.
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": any_runner(), "script": "true", "cwd": any_dir(),
            "limits": {"max_bytes": null}
        }),
    );
    assert_eq!(resp["result"]["isError"], json!(true));
    assert!(result_text(&resp).contains("cannot be null"));
}

#[test]
fn a_malformed_limits_object_is_rejected_rather_than_ignored() {
    // Silently falling back to defaults would tell a caller their bound was applied when
    // it was discarded.
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({"runner": any_runner(), "script": "true", "cwd": any_dir(),
               "limits": "120000"}),
    );
    assert_eq!(resp["result"]["isError"], json!(true));
    assert!(result_text(&resp).contains("must be an object"));
}

#[test]
fn a_misspelled_limits_key_is_rejected_rather_than_silently_defaulted() {
    // The input schema declares `additionalProperties: false`, but validation here is
    // hand-written, so nothing else enforces it. Accepting `timeoutMs` and then running for
    // the 120 s default is the worst available outcome: the caller believes the call is
    // bounded to five seconds and nothing says otherwise.
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": any_runner(), "script": "true", "cwd": any_dir(),
            "limits": {"timeoutMs": 5000}
        }),
    );
    assert_eq!(resp["result"]["isError"], json!(true));
    let text = result_text(&resp);
    assert!(
        text.contains("timeoutMs"),
        "name the offending key: {text:?}"
    );
    assert!(
        text.contains("timeout_ms"),
        "listing the accepted keys is what makes a typo self-correctable: {text:?}"
    );
}

#[test]
fn an_unknown_top_level_argument_is_rejected() {
    // `timeout_ms` at the top level rather than inside `limits` is the same trap as a typo.
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": any_runner(), "script": "true", "cwd": any_dir(),
            "timeout_ms": 5000
        }),
    );
    assert_eq!(resp["result"]["isError"], json!(true));
    assert!(result_text(&resp).contains("limits"));
}

#[test]
fn max_bytes_has_an_upper_bound() {
    // Refusing `null` while accepting any u64 would leave the OOM that refusing null exists
    // to prevent perfectly reachable.
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": any_runner(), "script": "true", "cwd": any_dir(),
            "limits": {"max_bytes": 64_u64 * 1024 * 1024 * 1024}
        }),
    );
    assert_eq!(resp["result"]["isError"], json!(true));
    assert!(result_text(&resp).contains("must be between 1 and"));
}

#[test]
fn an_unknown_on_output_limit_is_rejected() {
    let mut s = Server::start();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": any_runner(), "script": "true", "cwd": any_dir(),
            "limits": {"on_output_limit": "explode"}
        }),
    );
    assert_eq!(resp["result"]["isError"], json!(true));
    assert!(result_text(&resp).contains("truncate"));
}

#[cfg(unix)]
#[test]
fn a_child_that_closes_its_pipes_but_keeps_running_still_times_out() {
    // Both pipes closing does not mean the child exited. Reaping it outside the deadline
    // would let such a child outlive the timeout that exists to stop it.
    let mut s = Server::start();
    let started = std::time::Instant::now();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": "sh",
            "script": "exec 1>&- 2>&-; sleep 30",
            "cwd": std::env::temp_dir().to_string_lossy(),
            "limits": {"timeout_ms": 2000}
        }),
    );
    assert_eq!(
        resp["result"]["structuredContent"]["stop_reason"],
        "timeout"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "the call must end at its timeout, not when the child finally exits"
    );
}

// ------------------------------------------------------------------ agents_md

#[test]
fn agents_md_returns_the_guide_as_readable_markdown() {
    // The text block must be the document, not a JSON rendering of it: escaping every
    // newline would roughly double the payload for something meant to be read.
    let mut s = Server::start();
    let resp = s.call_tool("agents_md", json!({}));
    assert!(resp["result"]["isError"] != json!(true));

    let text = result_text(&resp);
    assert!(
        text.starts_with("# AGENTS.md"),
        "the text block should be the markdown itself; got {:?}",
        &text[..text.len().min(80)]
    );
    assert!(text.contains("non-zero exit code is a SUCCESSFUL call"));

    let sc = &resp["result"]["structuredContent"];
    assert_eq!(sc["source"], "embedded");
    assert_eq!(sc["aikit_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(sc["content"], json!(text));
}

#[test]
fn agents_md_is_advertised_with_read_only_annotations() {
    let mut s = Server::start();
    let resp = s.request("tools/list", json!({}));
    let tools = resp["result"]["tools"].as_array().unwrap();
    let guide = tools
        .iter()
        .find(|t| t["name"] == "agents_md")
        .expect("agents_md must be advertised");
    assert_eq!(guide["annotations"]["readOnlyHint"], json!(true));
    assert_eq!(guide["annotations"]["destructiveHint"], json!(false));
}

#[test]
fn instructions_point_at_the_guide() {
    // `instructions` is paid for every session, so it stays short — but it must at least
    // say the guide exists, or nothing prompts a model to ask for it.
    let mut s = Server::start();
    let resp = s.request(
        "initialize",
        json!({"protocolVersion": "2025-11-25", "capabilities": {},
               "clientInfo": {"name": "t", "version": "1"}}),
    );
    let instructions = resp["result"]["instructions"].as_str().unwrap_or_default();
    assert!(
        instructions.contains("agents_md"),
        "instructions must name the guide tool: {instructions:?}"
    );
}

#[test]
fn an_unreadable_agents_md_override_is_an_error_not_a_silent_fallback() {
    // Someone who set the variable wants that file. Quietly serving different guidance is
    // the kind of failure nobody notices.
    let mut child = bin()
        .args(["mcp", "--quiet"])
        .env("AGENTS_MD", "/definitely/not/a/real/path/AGENTS.md")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));

    for msg in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-11-25","capabilities":{},
            "clientInfo":{"name":"t","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
               "params":{"name":"agents_md","arguments":{}}}),
    ] {
        writeln!(stdin, "{msg}").expect("write");
        stdin.flush().expect("flush");
    }

    let mut found = None;
    for _ in 0..10 {
        let mut line = String::new();
        if stdout.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let Ok(m) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        if m.get("id").and_then(Value::as_i64) == Some(2) {
            found = Some(m);
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait(); // reap it; killing alone leaves a zombie

    let resp = found.expect("a response to the agents_md call");
    assert_eq!(resp["result"]["isError"], json!(true));
    let text = result_text(&resp);
    assert!(text.contains("AGENTS_MD"), "name the variable: {text:?}");
    assert!(
        text.contains("could not be read"),
        "say what went wrong: {text:?}"
    );
}

#[test]
fn unknown_tool_names_are_reported_as_correctable_errors() {
    let mut s = Server::start();
    let resp = s.call_tool("not_a_tool", json!({}));
    assert_eq!(resp["result"]["isError"], json!(true));
    assert!(result_text(&resp).contains("run"));
}

#[test]
fn wrong_typed_optional_arguments_are_rejected_rather_than_ignored() {
    // A wrong-typed optional argument must be an error, never a silent fall back to the
    // default: a caller who asked for a minimal environment and got an inherited one would
    // have no way to find out. Reading these with `.and_then(Value::as_str)` would collapse
    // "wrong type" into "absent" and do exactly that.
    let mut s = Server::start();
    for (field, args) in [
        (
            "stdin",
            json!({"runner": any_runner(), "script": "true", "cwd": any_dir(), "stdin": 5}),
        ),
        (
            "env",
            json!({"runner": any_runner(), "script": "true", "cwd": any_dir(),
                   "env": "PATH=/x"}),
        ),
        (
            "env_base",
            json!({"runner": any_runner(), "script": "true", "cwd": any_dir(),
                   "env_base": 1}),
        ),
    ] {
        let resp = s.call_tool("run", args);
        assert_eq!(
            resp["result"]["isError"],
            json!(true),
            "a wrong-typed `{field}` must be an error, not a silent default"
        );
        assert!(
            result_text(&resp).contains(field),
            "the error must name `{field}`"
        );
    }
}

#[cfg(unix)]
#[test]
fn over_capacity_calls_are_rejected_rather_than_queued() {
    // An abandoned call holds its slot for its whole timeout, so queueing would stack live
    // callers behind work nobody is waiting for. The cap is 8: this fills it with sleepers
    // and checks the next call comes straight back as `server_limit` instead of waiting.
    //
    // Requests go out before any response is read — `request()` is synchronous, so sending
    // all eight first is what puts them in flight together.
    let mut s = Server::start();
    let mut pending: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for _ in 0..8 {
        s.next_id += 1;
        let id = s.next_id;
        s.send(
            json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {
                "name": "run",
                "arguments": {
                    "runner": "sh", "script": "sleep 6",
                    "cwd": std::env::temp_dir().to_string_lossy(),
                    "limits": {"timeout_ms": 8000}
                }
            }}),
        );
        pending.insert(id);
    }

    // Let the eight acquire their slots before the ninth asks for one.
    std::thread::sleep(Duration::from_millis(1500));

    let started = Instant::now();
    let resp = s.call_tool(
        "run",
        json!({
            "runner": "sh", "script": "true",
            "cwd": std::env::temp_dir().to_string_lossy(),
        }),
    );
    assert_eq!(
        resp["result"]["structuredContent"]["stop_reason"], "server_limit",
        "the ninth concurrent call must be refused, not queued"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "refusal must be immediate; queueing would have made this wait for a sleeper"
    );
    assert!(
        result_text(&resp).contains("concurrency limit"),
        "the message must say why, so the caller knows to retry rather than change the call"
    );

    // Drain by id-set, never in send order. The eight run concurrently and finish in
    // whatever order they like, so reading them sequentially would discard an out-of-order
    // response and then block forever waiting for one already consumed.
    let deadline = Instant::now() + Duration::from_secs(30);
    while !pending.is_empty() {
        assert!(
            Instant::now() < deadline,
            "sleepers did not all report back; still waiting on {pending:?}"
        );
        let mut line = String::new();
        if s.stdout.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if let Ok(msg) = serde_json::from_str::<Value>(line.trim()) {
            if let Some(id) = msg.get("id").and_then(Value::as_i64) {
                pending.remove(&id);
            }
        }
    }
}
