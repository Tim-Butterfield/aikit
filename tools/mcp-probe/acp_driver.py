#!/usr/bin/env python3
"""Drive any ACP-capable agent CLI through the probe suites, non-interactively.

This is one convenience driver, not a requirement: if your client is an IDE or a chat UI,
register the probe there and ask it to call the tools by hand — the probe records the same
evidence either way.

NO PATHS ARE ASSUMED. You supply the binary and the flags that put it into ACP mode:

    python3 tools/mcp-probe/acp_driver.py --command /path/to/agent --arg acp --suite delivery
    python3 tools/mcp-probe/acp_driver.py --command /path/to/other --arg --acp --suite timeout

Register probe_server.py with your client FIRST. Most clients ignore any MCP server passed
per-session and use their own configuration; see README.md.

SUITES
  delivery  Does structuredContent reach the model? Do empty-content errors show anything?
  env       Environment report, including Windows Job Object membership.
  timeout   Blocking call of --seconds; use --no-progress to test without progress.
  cancel    Start a blocking call, then abandon it, and see whether the server is told.
"""
import argparse
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import threading
import time

SUITES = {
    "delivery": ("Call these MCP tools from the probe server, in order: probe_structured, "
                 "probe_both, probe_error_empty, probe_error_text. For EACH, report VERBATIM "
                 "what you received — quote the raw content. For probe_structured and "
                 "probe_error_empty, state explicitly whether you received any readable "
                 "content at all or whether the result appeared EMPTY. Do not summarise and "
                 "do not infer from the tool description; report only what reached you."),
    "env": ("Call the MCP tool probe_env from the probe server and report its output "
            "verbatim."),
    "timeout": ("Call the MCP tool probe_slow from the probe server with arguments {args}. "
                "It is expected to take about {seconds} seconds. Wait for it, then report "
                "what it returned, or state plainly that it failed, timed out or was "
                "cancelled — and how long you waited."),
    "cancel": ("Call the MCP tool probe_slow from the probe server with arguments "
               "{{\"seconds\": {seconds}}}. Wait for it."),
}


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--command", required=True,
                    help="agent binary that speaks ACP over stdio (full path or on PATH)")
    ap.add_argument("--arg", action="append", default=[], dest="args",
                    help="argument putting it into ACP mode; repeatable (e.g. --arg acp)")
    ap.add_argument("--suite", choices=sorted(SUITES), default="delivery")
    ap.add_argument("--cwd", default=None,
                    help="working directory for the session (default: current directory). "
                         "Some clients refuse directories they have not been told to trust.")
    ap.add_argument("--seconds", type=int, default=240, help="for timeout/cancel suites")
    ap.add_argument("--cancel-after", type=int, default=15,
                    help="seconds before abandoning the call in the cancel suite")
    ap.add_argument("--no-progress", action="store_true",
                    help="timeout suite: ask the probe to suppress progress notifications")
    ap.add_argument("--log", default=os.environ.get("AIKIT_PROBE_LOG")
                    or str(pathlib.Path(tempfile.gettempdir()) / "aikit-probe.log"),
                    help="probe log to read afterwards (default: $AIKIT_PROBE_LOG or temp)")
    ap.add_argument("--deadline", type=int, default=0,
                    help="give up after N seconds (default: suite-dependent)")
    opt = ap.parse_args()

    cwd = opt.cwd or os.getcwd()
    deadline = opt.deadline or (opt.seconds + 90 if opt.suite in ("timeout", "cancel") else 120)
    logp = pathlib.Path(opt.log)
    mark = sum(1 for _ in logp.open()) if logp.exists() else 0

    if opt.suite == "timeout":
        a = {"seconds": opt.seconds}
        if opt.no_progress:
            a["emit_progress"] = False
        prompt = SUITES["timeout"].format(args=json.dumps(a), seconds=opt.seconds)
    elif opt.suite == "cancel":
        prompt = SUITES["cancel"].format(seconds=opt.seconds)
    else:
        prompt = SUITES[opt.suite]

    proc = subprocess.Popen([opt.command] + opt.args, stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                            text=True, bufsize=1, cwd=cwd)
    threading.Thread(target=lambda: (time.sleep(deadline), proc.kill()), daemon=True).start()

    lock, responses, chunks, events = threading.Lock(), {}, [], []
    t0 = time.time()

    def send(o):
        with lock:
            try:
                proc.stdin.write(json.dumps(o) + "\n")
                proc.stdin.flush()
            except Exception:
                pass

    def reader():
        for line in proc.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                m = json.loads(line)
            except ValueError:
                continue
            if "method" in m and "id" in m:
                if m["method"] == "session/request_permission":
                    params = m.get("params") or {}
                    opts = params.get("options") or []
                    pick = next((o for o in opts if o.get("kind") == "allow_once"), None) \
                        or (opts[0] if opts else None)
                    # Least privilege: only ever approve probe-related calls.
                    ok = "probe" in json.dumps(params).lower() and pick
                    events.append(f"[{time.time()-t0:6.1f}s] permission "
                                  f"{'granted' if ok else 'REFUSED'}")
                    send({"jsonrpc": "2.0", "id": m["id"],
                          "result": {"outcome": {"outcome": "selected",
                                                 "optionId": pick["optionId"]}} if ok
                          else {"outcome": {"outcome": "cancelled"}}})
                else:
                    send({"jsonrpc": "2.0", "id": m["id"],
                          "error": {"code": -32601, "message": "not supported"}})
            elif "method" in m:
                if m["method"] == "session/update":
                    u = (m.get("params") or {}).get("update") or {}
                    k = u.get("sessionUpdate")
                    if k == "agent_message_chunk":
                        c = u.get("content") or {}
                        if c.get("type") == "text":
                            chunks.append(c["text"])
                    elif k in ("tool_call", "tool_call_update"):
                        events.append(f"[{time.time()-t0:6.1f}s] {k}: "
                                      f"{u.get('title') or u.get('toolCallId')} "
                                      f"status={u.get('status')}")
            elif "id" in m:
                responses[m["id"]] = m

    threading.Thread(target=reader, daemon=True).start()

    def call(rid, method, params, timeout):
        send({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
        end = time.time() + timeout
        while time.time() < end:
            if rid in responses:
                return responses[rid]
            time.sleep(0.05)
        return None

    print(f"driving: {opt.command} {' '.join(opt.args)}   suite={opt.suite}   cwd={cwd}")
    if not call(1, "initialize", {"protocolVersion": 1,
                                  "clientCapabilities": {"fs": {"readTextFile": False,
                                                                "writeTextFile": False}}}, 60):
        print("initialize failed — is that the right ACP flag for this binary?")
        return 1
    r = call(2, "session/new", {"cwd": cwd, "mcpServers": []}, 90)
    if not r or "result" not in r:
        print(f"session/new failed: {json.dumps(r)[:400] if r else 'no response'}")
        return 1
    sid = r["result"].get("sessionId")
    print(f"session: {sid}")

    send({"jsonrpc": "2.0", "id": 3, "method": "session/prompt",
          "params": {"sessionId": sid, "prompt": [{"type": "text", "text": prompt}]}})

    if opt.suite == "cancel":
        def later():
            time.sleep(opt.cancel_after)
            print(f"[{time.time()-t0:6.1f}s] abandoning the call (session/cancel)")
            send({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": sid}})
        threading.Thread(target=later, daemon=True).start()

    end = time.time() + deadline - 20
    resp = None
    while time.time() < end:
        if 3 in responses:
            resp = responses[3]
            break
        time.sleep(0.1)

    print(f"\nturn ended after {time.time()-t0:.1f}s: "
          f"{json.dumps((resp or {}).get('result'))[:200] if resp else 'no response (deadline)'}")
    for e in events:
        print("  " + e)
    print("\n--- what the agent reported ---")
    print("".join(chunks).strip() or "(nothing)")

    time.sleep(1)
    new = (logp.read_text(encoding="utf-8", errors="replace").splitlines()[mark:]
           if logp.exists() else [])
    calls = [l for l in new if "tools/call name=" in l]
    prog = [l for l in new if "notifications/progress" in l and "-->" in l]
    cancels = [l for l in new if "notifications/cancelled RECEIVED" in l]
    slow = [l for l in new if "probe_slow id=" in l]
    print(f"\n--- what the server recorded ({logp}) ---")
    for l in calls + slow + cancels:
        print("  " + l[:220])
    print(f"  progress notifications sent: {len(prog)}")
    if opt.suite == "cancel":
        print("\nVERDICT: " + ("this client DELIVERS notifications/cancelled to the server"
                               if cancels else
                               "NO cancellation reached the server — it abandons calls "
                               "silently, so only a server-side timeout can stop work"))
    if not new:
        print("  (nothing — the probe was never reached. Is it registered with this client?)")
    proc.kill()
    return 0


if __name__ == "__main__":
    sys.exit(main())
