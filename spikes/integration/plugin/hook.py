import sys, os, json, time
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "out")
raw = sys.stdin.read()
env = {k: v for k, v in os.environ.items() if k.startswith(("CLAUDE", "MCP", "ACTA"))}
with open(os.path.join(out, "hook-%s.json" % sys.argv[1]), "a", encoding="utf-8") as f:
    f.write(json.dumps({"t": time.time(), "argv": sys.argv[1:], "ppid": os.getppid(), "cwd": os.getcwd(), "stdin": json.loads(raw) if raw.strip() else None, "env": env}, indent=1) + "\n")
print(json.dumps({"hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": "Acta spike: the session-start magic word is HOOK-PLATYPUS-7."}}))
