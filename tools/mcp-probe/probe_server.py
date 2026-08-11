#!/usr/bin/env python3
"""A diagnostic MCP server that answers "what does MY client actually do?".

Register it with any MCP client, call its tools, and compare what the client shows you
against what this server recorded on the wire. It is client-agnostic: it makes no
assumptions about which agent, CLI or IDE is driving it.

WHY IT EXISTS
  MCP leaves several behaviours to the client, and they differ. Before relying on any MCP
  server, it is worth knowing whether YOUR client:
    * forwards `structuredContent` into the model's context, or only `content`
    * shows the model anything at all for an `isError` result with an empty content array
    * delivers `notifications/cancelled` to the server when a call is abandoned
    * imposes its own per-request timeout
    * honours `notifications/progress`
  Each tool below isolates one of those, with a control alongside it.

TOOLS
  probe_echo          sanity check
  probe_structured    structuredContent ONLY (empty content array)
  probe_both          control: the same payload plus a TextContent block
  probe_error_empty   isError with an EMPTY content array
  probe_error_text    control: isError WITH a TextContent message
  probe_slow          blocks for N seconds; optional progress; logs any cancellation
  probe_env           reports this process's environment, including (on Windows)
                      whether it is already inside a Job Object

LOG
  Every message in both directions is appended to a log file. Default location is
  `aikit-probe.log` in the OS temp directory — deliberately NOT next to this file, so
  running it never dirties a checkout. Override with $AIKIT_PROBE_LOG. The path is printed
  to stderr at startup and returned by probe_env.

NOTES
  * stdout carries JSON-RPC and nothing else; diagnostics go to the log and stderr.
  * A reader loop dispatches while slow work runs on a worker thread, so a cancellation
    notification can still arrive while a call is in flight. A server that blocks its read
    loop in the handler can never observe one.
"""
import ctypes
import datetime
import json
import os
import pathlib
import platform
import sys
import tempfile
import threading
import time

LOG_PATH = pathlib.Path(
    os.environ.get("AIKIT_PROBE_LOG")
    or (pathlib.Path(tempfile.gettempdir()) / "aikit-probe.log")
)
_logf = LOG_PATH.open("a", encoding="utf-8", buffering=1)
_log_lock = threading.Lock()
_out_lock = threading.Lock()

_inflight = {}
_inflight_lock = threading.Lock()

SUPPORTED_PROTOCOLS = {"2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25", "2026-07-28"}
DEFAULT_PROTOCOL = "2025-11-25"


def log(msg):
    with _log_lock:
        _logf.write(f"{msg}\n")


def logev(direction, payload):
    log(f"[{datetime.datetime.now().isoformat(timespec='milliseconds')}] {direction} {payload}")


def send(obj):
    """The ONLY writer to stdout."""
    data = json.dumps(obj, separators=(",", ":"))
    with _out_lock:
        sys.stdout.write(data + "\n")
        sys.stdout.flush()
    logev("-->", data)


def reply(req_id, result):
    send({"jsonrpc": "2.0", "id": req_id, "result": result})


def reply_err(req_id, code, message):
    send({"jsonrpc": "2.0", "id": req_id, "error": {"code": code, "message": message}})


STRUCTURED_SCHEMA = {
    "type": "object",
    "properties": {"marker": {"type": "string"},
                   "note": {"type": "string"},
                   "nonce": {"type": "string"}},
    "required": ["marker", "note", "nonce"],
}

TOOLS = [
    {"name": "probe_echo",
     "description": "Sanity check. Returns a text block containing the string you pass.",
     "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}},
                     "required": ["text"]}},
    {"name": "probe_structured",
     "description": ("Returns structuredContent ONLY, with an EMPTY content array. If you "
                     "can read the marker/note/nonce values, structuredContent reached you; "
                     "if the result looks empty, it did not. Report exactly what you got."),
     "inputSchema": {"type": "object", "properties": {}},
     "outputSchema": STRUCTURED_SCHEMA},
    {"name": "probe_both",
     "description": ("Control for probe_structured: the SAME structuredContent plus a "
                     "duplicate TextContent block. Report exactly what you got."),
     "inputSchema": {"type": "object", "properties": {}},
     "outputSchema": STRUCTURED_SCHEMA},
    {"name": "probe_error_empty",
     "description": ("Returns isError:true with an EMPTY content array — the shape a "
                     "structuredContent-only server produces on failure. Report whether you "
                     "saw any error message at all."),
     "inputSchema": {"type": "object", "properties": {}}},
    {"name": "probe_error_text",
     "description": "Control: isError:true WITH a TextContent message. Report what you saw.",
     "inputSchema": {"type": "object", "properties": {}}},
    {"name": "probe_slow",
     "description": ("Blocks for `seconds` (default 30). Emits progress notifications when "
                     "the call supplies a progressToken, unless emit_progress is false. Use "
                     "it to measure how long your client waits, whether progress keeps the "
                     "call alive, and whether cancelling reaches the server."),
     "inputSchema": {"type": "object", "properties": {
         "seconds": {"type": "integer", "minimum": 1, "maximum": 900},
         "emit_progress": {"type": "boolean",
                           "description": "false suppresses progress notifications"}}}},
    {"name": "probe_env",
     "description": ("Reports this server process's environment: OS, PID, whether stdout is "
                     "a TTY, whether a console is attached, whether the process is already "
                     "inside a Job Object (Windows), and where the probe log is."),
     "inputSchema": {"type": "object", "properties": {}}},
]


def _payload():
    return {"marker": "STRUCTURED_CONTENT_REACHED_THE_MODEL",
            "note": "If you can read this, structuredContent was delivered into your context.",
            "nonce": f"{os.getpid()}-{int(time.time())}"}


def _in_job_object():
    if platform.system() != "Windows":
        return "n/a (not Windows)"
    try:
        k32 = ctypes.windll.kernel32  # type: ignore[attr-defined]
        res = ctypes.c_int(0)
        if not k32.IsProcessInJob(k32.GetCurrentProcess(), None, ctypes.byref(res)):
            return f"IsProcessInJob failed (GetLastError={k32.GetLastError()})"
        return bool(res.value)
    except Exception as e:
        return f"error: {e}"


def _console_attached():
    if platform.system() != "Windows":
        return "n/a (not Windows)"
    try:
        return bool(ctypes.windll.kernel32.GetConsoleWindow())  # type: ignore[attr-defined]
    except Exception as e:
        return f"error: {e}"


def run_slow(req_id, seconds, token):
    cancelled = threading.Event()
    with _inflight_lock:
        _inflight[req_id] = cancelled
    log(f"    probe_slow id={req_id} start seconds={seconds} progressToken={token!r}")
    started = time.time()
    try:
        for i in range(seconds):
            if cancelled.wait(timeout=1.0):
                log(f"    probe_slow id={req_id} CANCELLED after {time.time()-started:.1f}s "
                    f"— sending NO response, per spec")
                return
            if token is not None:
                send({"jsonrpc": "2.0", "method": "notifications/progress",
                      "params": {"progressToken": token, "progress": i + 1,
                                 "total": seconds, "message": f"{i+1}/{seconds}s"}})
        waited = time.time() - started
        log(f"    probe_slow id={req_id} completed after {waited:.1f}s "
            f"(client still connected when we replied)")
        reply(req_id, {"content": [{"type": "text", "text":
              f"probe_slow finished after {waited:.1f}s. If you are reading this, the "
              f"client waited at least that long."}], "isError": False})
    finally:
        with _inflight_lock:
            _inflight.pop(req_id, None)


def call_tool(req_id, name, args):
    if name == "probe_echo":
        reply(req_id, {"content": [{"type": "text", "text": str(args.get("text", ""))}],
                       "isError": False})
    elif name == "probe_structured":
        reply(req_id, {"content": [], "structuredContent": _payload(), "isError": False})
    elif name == "probe_both":
        pl = _payload()
        reply(req_id, {"content": [{"type": "text", "text": json.dumps(pl)}],
                       "structuredContent": pl, "isError": False})
    elif name == "probe_error_empty":
        reply(req_id, {"content": [], "isError": True})
    elif name == "probe_error_text":
        reply(req_id, {"content": [{"type": "text", "text":
              "PROBE_ERROR_TEXT: this error carried a TextContent block."}], "isError": True})
    elif name == "probe_env":
        info = {"os": platform.platform(), "python": sys.version.split()[0],
                "pid": os.getpid(), "stdout_isatty": sys.stdout.isatty(),
                "windows_console_attached": _console_attached(),
                "windows_in_job_object": _in_job_object(),
                "cwd": os.getcwd(), "probe_log": str(LOG_PATH)}
        log(f"    probe_env -> {json.dumps(info)}")
        reply(req_id, {"content": [{"type": "text", "text": json.dumps(info, indent=2)}],
                       "isError": False})
    elif name == "probe_slow":
        token = (args.get("_meta") or {}).get("progressToken")
        if args.get("emit_progress") is False:
            token = None
        threading.Thread(target=run_slow,
                         args=(req_id, int(args.get("seconds", 30)), token),
                         daemon=True).start()
    else:
        reply_err(req_id, -32602, f"unknown tool: {name}")


def handle(msg):
    method, req_id = msg.get("method"), msg.get("id")
    if method == "initialize":
        params = msg.get("params") or {}
        requested = params.get("protocolVersion")
        negotiated = requested if requested in SUPPORTED_PROTOCOLS else DEFAULT_PROTOCOL
        log(f"    initialize: requested={requested!r} negotiated={negotiated!r} "
            f"client={params.get('clientInfo')} capabilities={params.get('capabilities')}")
        reply(req_id, {"protocolVersion": negotiated,
                       "capabilities": {"tools": {"listChanged": False}},
                       "serverInfo": {"name": "aikit-mcp-probe", "version": "1.0.0"},
                       "instructions": ("Diagnostic server. Call probe_structured and "
                                        "probe_error_empty, then report VERBATIM what you "
                                        "received — including whether it looked empty.")})
    elif method == "notifications/initialized":
        log("    handshake complete")
    elif method == "tools/list":
        log(f"    tools/list ({len(TOOLS)} tools)")
        reply(req_id, {"tools": TOOLS})
    elif method == "tools/call":
        params = msg.get("params") or {}
        args = dict(params.get("arguments") or {})
        if params.get("_meta"):
            args["_meta"] = params["_meta"]
        log(f"    tools/call name={params.get('name')!r} args={json.dumps(args)}")
        call_tool(req_id, params.get("name"), args)
    elif method == "notifications/cancelled":
        params = msg.get("params") or {}
        target = params.get("requestId")
        log(f"*** notifications/cancelled RECEIVED requestId={target!r} "
            f"reason={params.get('reason')!r} — this client DOES deliver cancellations")
        with _inflight_lock:
            ev = _inflight.get(target)
        if ev:
            ev.set()
        else:
            log(f"    (no in-flight request {target!r}; ignoring, per spec)")
    elif method == "ping":
        reply(req_id, {})
    elif req_id is not None:
        log(f"    unhandled request {method!r}")
        reply_err(req_id, -32601, f"method not found: {method}")
    else:
        log(f"    unhandled notification {method!r}")


def main():
    log(f"--- started {datetime.datetime.now().isoformat()} py={sys.version.split()[0]} "
        f"pid={os.getpid()} ---")
    print(f"aikit-mcp-probe: started, logging to {LOG_PATH}", file=sys.stderr, flush=True)
    try:
        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue
            logev("<--", line)
            try:
                msg = json.loads(line)
            except ValueError:
                log("    !! not JSON, ignoring")
                continue
            try:
                handle(msg)
            except Exception as e:
                log(f"    !! handler error: {e!r}")
    except KeyboardInterrupt:
        log("--- interrupted ---")
    finally:
        log("--- stdin closed (EOF) ---")


if __name__ == "__main__":
    main()
