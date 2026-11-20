import sys, os, json, time
out = os.environ.get("ACTA_SPIKE_OUT") or os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "out")
log = open(os.path.join(out, "server.jsonl"), "a", encoding="utf-8")
def w(kind, data):
    log.write(json.dumps({"t": time.time(), "kind": kind, "data": data}) + "\n"); log.flush()
w("start", {"ppid": os.getppid(), "pid": os.getpid(), "cwd": os.getcwd(), "argv": sys.argv,
            "env": {k: v for k, v in os.environ.items() if k.startswith(("CLAUDE", "MCP", "ACTA"))}})
TOOLS = [
 {"name": "brief", "description": "Spike: get bearings for the active topic.",
  "inputSchema": {"type": "object", "properties": {"topic": {"type": "string"}}},
  "outputSchema": {"type": "object", "properties": {"marker": {"type": "string"}}},
  "annotations": {"readOnlyHint": True, "title": "Brief"}, "_meta": {"anthropic/alwaysLoad": True}},
 {"name": "log", "description": "Spike: append a timeline entry to the active topic.",
  "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]},
  "annotations": {"readOnlyHint": False, "destructiveHint": False}},
 {"name": "hook", "description": "Internal: called by Claude Code hooks only.",
  "inputSchema": {"type": "object", "properties": {"event": {"type": "string"}, "session": {"type": "string"}, "cwd": {"type": "string"}}},
  "annotations": {"readOnlyHint": True}},
]
def send(o):
    sys.stdout.write(json.dumps(o) + "\n"); sys.stdout.flush()
for line in sys.stdin:
    line = line.strip()
    if not line: continue
    m = json.loads(line); w("recv", m)
    mid, method, p = m.get("id"), m.get("method"), m.get("params") or {}
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": mid, "result": {"protocolVersion": p.get("protocolVersion"), "capabilities": {"tools": {}},
              "serverInfo": {"name": "acta-spike", "version": "0.0.1"},
              "instructions": "Acta spike server. The server-instructions magic word is INSTR-OTTER-5."}})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": mid, "result": {"tools": TOOLS}})
    elif method == "tools/call":
        name = p.get("name"); a = p.get("arguments") or {}
        if name == "brief":
            res = {"content": [{"type": "text", "text": "TEXT-MARKER-ALPHA (plain text content block)"}], "structuredContent": {"marker": "STRUCT-MARKER-BETA"}}
        elif name == "hook":
            res = {"content": [{"type": "text", "text": json.dumps({"hookSpecificOutput": {"hookEventName": a.get("event"), "additionalContext": "Acta spike: the mcp_tool-hook magic word is MCPHOOK-LYNX-9."}})}]}
        else:
            res = {"content": [{"type": "text", "text": "ok T-42#15"}]}
        send({"jsonrpc": "2.0", "id": mid, "result": res})
    elif mid is not None:
        send({"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": "method not found"}})
w("eof", None)
