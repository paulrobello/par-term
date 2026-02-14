#!/usr/bin/env python3
"""Example par-term observer script.

Reads terminal events as JSON from stdin, responds with commands on stdout.
Demonstrates the par-term scripting protocol.

Usage in config.yaml:
  scripts:
    - name: "Hello Observer"
      script_path: "scripts/examples/hello_observer.py"
      auto_start: true
      subscriptions: ["bell_rang", "cwd_changed", "command_complete"]
"""

import json
import sys


def send_command(cmd: dict) -> None:
    """Send a JSON command to par-term."""
    print(json.dumps(cmd), flush=True)


def log(level: str, message: str) -> None:
    """Log a message through par-term's logging system."""
    send_command({"type": "Log", "level": level, "message": message})


def set_panel(title: str, content: str) -> None:
    """Set a markdown panel in the terminal UI."""
    send_command({"type": "SetPanel", "title": title, "content": content})


def notify(title: str, body: str) -> None:
    """Show a desktop notification."""
    send_command({"type": "Notify", "title": title, "body": body})


def main() -> None:
    log("info", "Hello Observer script started")
    set_panel("Observer", "## Hello Observer\n- Status: Running\n- Events: 0")

    event_count = 0
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as e:
            log("error", f"Invalid JSON: {e}")
            continue

        event_count += 1
        kind = event.get("kind", "unknown")
        data = event.get("data", {})
        log("info", f"Received event: {kind} (#{event_count})")

        set_panel(
            "Observer",
            f"## Hello Observer\n- Status: Running\n- Events: {event_count}\n- Last: {kind}",
        )

        if kind == "bell_rang":
            notify("Bell!", "Terminal bell was triggered")

        elif kind == "cwd_changed":
            cwd = data.get("cwd", "unknown")
            log("info", f"Directory changed to: {cwd}")

        elif kind == "command_complete":
            cmd_text = data.get("command", "")
            exit_code = data.get("exit_code")
            if exit_code is not None and exit_code != 0:
                notify(
                    "Command Failed",
                    f"{cmd_text} exited with code {exit_code}",
                )

    log("info", "Hello Observer script shutting down")


if __name__ == "__main__":
    main()
