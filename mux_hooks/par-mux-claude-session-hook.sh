#!/bin/sh
# par-mux claude session hook — reports session identity to the par-mux
# daemon on claude SessionStart events (no action argument, or `session`)
# and releases the pane's claim on SessionEnd (`release`). Same reporter
# shape as the kimi port (core tests/assets/par-mux-agent-state.sh): one
# JSON line over the control socket, one reply read, close. Inert outside
# a par-mux pane (env guards) and silent on every failure.
# PAR_MUX_INTEGRATION_ID=claude
# PAR_MUX_INTEGRATION_VERSION=2

action="${1:-}"

[ "${PAR_MUX_ENV:-}" = "1" ] || exit 0
[ -n "${PAR_MUX_SOCKET:-}" ] || exit 0
[ -n "${PAR_MUX_PANE_ID:-}" ] || exit 0
command -v python3 >/dev/null 2>&1 || exit 0

PAR_MUX_HOOK_ACTION="$action" python3 -c '
import json
import os
import socket
import sys
import time

try:
    payload = json.load(sys.stdin)
except Exception:
    payload = {}

action = os.environ.get("PAR_MUX_HOOK_ACTION", "")
seq = time.time_ns()

if action == "release":
    # pane.release_agent clears the pane claim (core aed41c2). The guards
    # are on the daemon side; a later time_ns() always advances past the
    # session report seq from the same source.
    request = json.dumps(
        {
            "id": f"par-mux:claude:release:{seq}",
            "method": "pane.release_agent",
            "params": {
                "pane_id": os.environ["PAR_MUX_PANE_ID"],
                "source": "par-mux:claude",
                "agent": "claude",
                "seq": seq,
            },
        }
    )
else:
    session_id = payload.get("session_id")
    if not isinstance(session_id, str) or not session_id:
        raise SystemExit(0)

    source = payload.get("source")
    if not isinstance(source, str) or not source:
        source = "startup"

    params = {
        "pane_id": os.environ["PAR_MUX_PANE_ID"],
        "source": "par-mux:claude",
        "agent": "claude",
        "seq": seq,
        "session_start_source": source,
        "agent_session_id": session_id,
        "session_resume_argv": ["claude", "--resume", session_id],
    }
    transcript = payload.get("transcript_path")
    if isinstance(transcript, str) and transcript:
        params["agent_session_path"] = transcript

    request = json.dumps(
        {"id": f"par-mux:claude:{seq}", "method": "pane.report_agent_session", "params": params}
    )

try:
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
        client.settimeout(0.5)
        client.connect(os.environ["PAR_MUX_SOCKET"])
        client.sendall((request + "\n").encode())
        client.recv(4096)
except Exception:
    pass
' 2>/dev/null || true
