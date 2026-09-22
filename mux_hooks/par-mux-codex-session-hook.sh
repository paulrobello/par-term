#!/bin/sh
# par-mux codex session hook — reports session identity to the par-mux
# daemon on codex SessionStart events. Same reporter shape as the kimi port
# (core tests/assets/par-mux-agent-state.sh): one JSON line over the control
# socket, one reply read, close. Inert outside a par-mux pane (env guards)
# and silent on every failure.
# PAR_MUX_INTEGRATION_ID=codex
# PAR_MUX_INTEGRATION_VERSION=1

set -eu

action="${1:-}"
hook_input_file="$(mktemp "${TMPDIR:-/tmp}/par-mux-codex-hook.XXXXXX")" || exit 0
trap 'rm -f "$hook_input_file"' EXIT HUP INT TERM
cat >"$hook_input_file" 2>/dev/null || true

case "$action" in
  session) ;;
  *) exit 0 ;;
esac

[ "${PAR_MUX_ENV:-}" = "1" ] || exit 0
[ -n "${PAR_MUX_SOCKET:-}" ] || exit 0
[ -n "${PAR_MUX_PANE_ID:-}" ] || exit 0
command -v python3 >/dev/null 2>&1 || exit 0

PAR_MUX_ACTION="$action" PAR_MUX_HOOK_INPUT_FILE="$hook_input_file" python3 - <<'PY'
import json
import os
import random
import socket
import time

source = "par-mux:codex"
pane_id = os.environ.get("PAR_MUX_PANE_ID")
socket_path = os.environ.get("PAR_MUX_SOCKET")
hook_input_file = os.environ.get("PAR_MUX_HOOK_INPUT_FILE")

if not pane_id or not socket_path:
    raise SystemExit(0)

hook_input = {}
if hook_input_file:
    try:
        with open(hook_input_file, encoding="utf-8") as handle:
            content = handle.read()
        if content.strip():
            hook_input = json.loads(content)
    except Exception:
        hook_input = {}

# codex names its events claude-style; only session starts are ours to report.
hook_event_name = hook_input.get("hook_event_name")
if hook_event_name != "SessionStart":
    raise SystemExit(0)

session_id = hook_input.get("session_id")
if not isinstance(session_id, str) or not session_id:
    raise SystemExit(0)

# codex exports the pane's thread id into hook processes. When a fork or
# subagent fires SessionStart under a different id, that session is not this
# pane's session — do not overwrite the roster with it.
inherited = os.environ.get("CODEX_THREAD_ID")
if inherited and inherited != session_id:
    raise SystemExit(0)

session_start_source = hook_input.get("source")
if not isinstance(session_start_source, str) or not session_start_source:
    session_start_source = "startup"

request_id = f"{source}:{int(time.time() * 1000)}:{random.randrange(1_000_000):06d}"
report_seq = time.time_ns()
params = {
    "pane_id": pane_id,
    "source": source,
    "agent": "codex",
    "seq": report_seq,
    "session_start_source": session_start_source,
    "agent_session_id": session_id,
    "session_resume_argv": ["codex", "resume", session_id],
}
transcript = hook_input.get("transcript_path")
if isinstance(transcript, str) and transcript:
    params["agent_session_path"] = transcript

request = {
    "id": request_id,
    "method": "pane.report_agent_session",
    "params": params,
}

try:
    client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    client.settimeout(0.5)
    client.connect(socket_path)
    client.sendall((json.dumps(request) + "\n").encode())
    try:
        client.recv(4096)
    except Exception:
        pass
    client.close()
except Exception:
    pass
PY
