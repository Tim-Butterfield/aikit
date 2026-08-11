#!/usr/bin/env python3
"""Verify probe_server.py works, with no MCP client involved.

Run this first. If it fails, the probe is broken; if it passes, any difference you see
through a real client is the CLIENT's behaviour, which is the whole point.

    python3 tools/mcp-probe/selftest.py

No paths are assumed: the probe is located relative to this file and run with the same
interpreter. The probe logs to $AIKIT_PROBE_LOG or the OS temp directory, never the repo.
"""
import json
import pathlib
import subprocess
import sys
import threading
import time

PROBE = pathlib.Path(__file__).resolve().parent / "probe_server.py"

p = subprocess.Popen([sys.executable, str(PROBE)], stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
inbox, lock, fails = [], threading.Lock(), []


def reader():
    for line in p.stdout:
        line = line.strip()
        if line:
            with lock:
                inbox.append(json.loads(line))


threading.Thread(target=reader, daemon=True).start()


def send(o):
    p.stdin.write(json.dumps(o) + "\n")
    p.stdin.flush()


def wait_id(want, timeout=10):
    end = time.time() + timeout
    while time.time() < end:
        with lock:
            for m in inbox:
                if m.get("id") == want:
                    return m
        time.sleep(0.05)
    return None


def check(label, cond, detail=""):
    print(f"  [{'ok  ' if cond else 'FAIL'}] {label}" + (f" — {detail}" if detail else ""))
    if not cond:
        fails.append(label)


print("1. handshake")
send({"jsonrpc": "2.0", "id": 1, "method": "initialize",
      "params": {"protocolVersion": "2025-11-25", "capabilities": {},
                 "clientInfo": {"name": "selftest", "version": "1"}}})
r = wait_id(1)
check("initialize responds", r is not None)
if r:
    res = r["result"]
    check("negotiates the requested version", res.get("protocolVersion") == "2025-11-25")
    check("declares tools capability", "tools" in res.get("capabilities", {}))
send({"jsonrpc": "2.0", "method": "notifications/initialized"})

print("2. tools/list")
send({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})
tools = (wait_id(2) or {}).get("result", {}).get("tools", [])
check("lists tools", len(tools) == 7, f"{len(tools)} tools")
check("every tool declares inputSchema", all("inputSchema" in t for t in tools))
check("probe_structured declares outputSchema",
      any(t["name"] == "probe_structured" and "outputSchema" in t for t in tools))

print("3. result shapes")
send({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
      "params": {"name": "probe_structured", "arguments": {}}})
res = (wait_id(3) or {}).get("result", {})
check("probe_structured: structuredContent set, content empty",
      "structuredContent" in res and res.get("content") == [])

send({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
      "params": {"name": "probe_both", "arguments": {}}})
res = (wait_id(4) or {}).get("result", {})
check("probe_both: both channels populated",
      "structuredContent" in res and len(res.get("content") or []) == 1)

send({"jsonrpc": "2.0", "id": 5, "method": "tools/call",
      "params": {"name": "probe_error_empty", "arguments": {}}})
res = (wait_id(5) or {}).get("result", {})
check("probe_error_empty: isError with empty content",
      res.get("isError") is True and res.get("content") == [])

print("4. progress, cancellation, and a responsive read loop")
send({"jsonrpc": "2.0", "id": 6, "method": "tools/call",
      "params": {"name": "probe_slow", "arguments": {"seconds": 10},
                 "_meta": {"progressToken": "t1"}}})
time.sleep(2.5)
with lock:
    progs = [m for m in inbox if m.get("method") == "notifications/progress"]
check("emits progress when given a token", len(progs) >= 1, f"{len(progs)} received")

send({"jsonrpc": "2.0", "method": "notifications/cancelled",
      "params": {"requestId": 6, "reason": "selftest"}})
time.sleep(2)
check("sends no response after cancellation", wait_id(6, timeout=2) is None)

send({"jsonrpc": "2.0", "id": 7, "method": "tools/call",
      "params": {"name": "probe_echo", "arguments": {"text": "alive"}}})
check("read loop stayed responsive during a blocking call", wait_id(7, 5) is not None)

print("5. environment report")
send({"jsonrpc": "2.0", "id": 8, "method": "tools/call",
      "params": {"name": "probe_env", "arguments": {}}})
txt = (wait_id(8) or {}).get("result", {}).get("content", [{}])[0].get("text", "")
check("probe_env reports", "windows_in_job_object" in txt)
for line in txt.splitlines():
    print("        " + line)

p.kill()
print(f"\n{'ALL CHECKS PASSED' if not fails else 'FAILURES: ' + ', '.join(fails)}")
sys.exit(1 if fails else 0)
