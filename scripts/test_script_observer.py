#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# ///
"""Test script for par-term's observer scripting feature.

Run with: uv run scripts/test_script_observer.py [--mode MODE]

Modes:
  monitor    - Log all events and show a live status panel (default)
  command    - React to events by sending commands back to the terminal
  stress     - High-throughput event processing with timing metrics
  validate   - Validate JSON protocol by echoing parsed event structure
  demo       - Interactive demo exercising all 9 command types

When used as a par-term script with observer event forwarding, the script
receives terminal events as JSON on stdin (one object per line) and sends
commands as JSON on stdout (one object per line).

Examples:
  # Monitor mode: logs events and updates a live panel
  uv run scripts/test_script_observer.py --mode monitor

  # Command mode: reacts to events with terminal commands
  uv run scripts/test_script_observer.py --mode command

  # Stress mode: measures event processing throughput
  uv run scripts/test_script_observer.py --mode stress

  # Validate mode: echoes parsed events for protocol debugging
  uv run scripts/test_script_observer.py --mode validate

  # Demo mode: sends all command types on first event received
  uv run scripts/test_script_observer.py --mode demo

Config example:
  scripts:
    - name: "Test Observer"
      script_path: "scripts/test_script_observer.py"
      args: ["--mode", "monitor"]
      auto_start: true
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from datetime import datetime, timezone


# ---------------------------------------------------------------------------
# Protocol helpers
# ---------------------------------------------------------------------------

def send(cmd: dict) -> None:
    """Send a JSON command to par-term (one line on stdout)."""
    print(json.dumps(cmd), flush=True)


def log(level: str, message: str) -> None:
    """Send a Log command."""
    send({"type": "Log", "level": level, "message": message})


def notify(title: str, body: str) -> None:
    """Send a Notify command (desktop notification)."""
    send({"type": "Notify", "title": title, "body": body})


def set_panel(title: str, content: str) -> None:
    """Send a SetPanel command (markdown UI panel)."""
    send({"type": "SetPanel", "title": title, "content": content})


def clear_panel() -> None:
    """Send a ClearPanel command."""
    send({"type": "ClearPanel"})


def write_text(text: str) -> None:
    """Send a WriteText command (write to PTY)."""
    send({"type": "WriteText", "text": text})


def set_badge(text: str) -> None:
    """Send a SetBadge command."""
    send({"type": "SetBadge", "text": text})


def set_variable(name: str, value: str) -> None:
    """Send a SetVariable command."""
    send({"type": "SetVariable", "name": name, "value": value})


def run_command(command: str) -> None:
    """Send a RunCommand command."""
    send({"type": "RunCommand", "command": command})


def change_config(key: str, value: object) -> None:
    """Send a ChangeConfig command."""
    send({"type": "ChangeConfig", "key": key, "value": value})


def read_event() -> dict | None:
    """Read one JSON event from stdin. Returns None on EOF."""
    line = sys.stdin.readline()
    if not line:
        return None
    line = line.strip()
    if not line:
        return None
    return json.loads(line)


def event_stream():
    """Yield events from stdin until EOF."""
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            yield json.loads(line)
        except json.JSONDecodeError as e:
            log("error", f"Invalid JSON from terminal: {e}")


# ---------------------------------------------------------------------------
# Modes
# ---------------------------------------------------------------------------

def mode_monitor(args: argparse.Namespace) -> None:
    """Log all events and maintain a live status panel."""
    log("info", "Monitor mode started")

    event_count = 0
    event_kinds: dict[str, int] = {}
    last_cwd = "?"
    last_title = "?"
    errors = 0
    start_time = time.monotonic()

    def update_panel() -> None:
        elapsed = time.monotonic() - start_time
        rate = event_count / elapsed if elapsed > 0 else 0.0
        top_kinds = sorted(event_kinds.items(), key=lambda x: -x[1])[:5]
        kind_lines = "\n".join(f"  - `{k}`: {c}" for k, c in top_kinds)
        content = (
            f"## Monitor\n"
            f"- **Events**: {event_count} ({rate:.1f}/s)\n"
            f"- **Errors**: {errors}\n"
            f"- **CWD**: `{last_cwd}`\n"
            f"- **Title**: {last_title}\n"
            f"- **Top events**:\n{kind_lines}\n"
        )
        set_panel("Monitor", content)

    for event in event_stream():
        event_count += 1
        kind = event.get("kind", "unknown")
        data = event.get("data", {})
        event_kinds[kind] = event_kinds.get(kind, 0) + 1

        log("debug", f"[{event_count}] {kind}: {json.dumps(data)[:120]}")

        match kind:
            case "cwd_changed":
                last_cwd = data.get("cwd", "?")
                log("info", f"CWD -> {last_cwd}")
            case "title_changed":
                last_title = data.get("title", "?")
            case "command_complete":
                cmd = data.get("command", "")
                code = data.get("exit_code")
                if code is not None and code != 0:
                    errors += 1
                    log("warn", f"Command failed: {cmd} (exit {code})")
            case "bell_rang":
                log("info", "Bell rang")
            case _:
                pass

        # Update panel every 5 events or immediately for important ones
        if event_count % 5 == 0 or kind in ("cwd_changed", "command_complete", "bell_rang"):
            update_panel()

    update_panel()
    log("info", f"Monitor ended after {event_count} events")


def mode_command(_args: argparse.Namespace) -> None:
    """React to events with terminal commands."""
    log("info", "Command mode started - reacting to terminal events")
    set_badge("CMD")

    failed_commands: list[str] = []

    for event in event_stream():
        kind = event.get("kind", "unknown")
        data = event.get("data", {})

        match kind:
            case "bell_rang":
                notify("Bell", "Terminal bell was triggered")
                log("info", "Sent notification for bell event")

            case "cwd_changed":
                cwd = data.get("cwd", "")
                set_variable("last_cwd", cwd)
                set_badge(cwd.split("/")[-1] or "/")
                log("info", f"Updated badge and variable for CWD: {cwd}")

            case "command_complete":
                cmd = data.get("command", "")
                code = data.get("exit_code")
                if code is not None and code != 0:
                    failed_commands.append(f"{cmd} (exit {code})")
                    notify("Command Failed", f"`{cmd}` exited with code {code}")
                    set_badge(f"FAIL:{code}")
                    log("error", f"Command failed: {cmd} exit={code}")
                elif cmd:
                    set_badge("OK")
                    log("info", f"Command succeeded: {cmd}")

            case "title_changed":
                title = data.get("title", "")
                set_variable("last_title", title)

            case "environment_changed":
                key = data.get("key", "")
                value = data.get("value", "")
                log("info", f"Env changed: {key}={value[:50]}")

            case "user_var_changed":
                name = data.get("name", "")
                value = data.get("value", "")
                log("info", f"User var: {name}={value}")

            case _:
                log("debug", f"Unhandled event: {kind}")

        # Update panel with failure history
        if failed_commands:
            lines = "\n".join(f"  - {f}" for f in failed_commands[-10:])
            set_panel("Failed Commands", f"## Recent Failures\n{lines}")

    log("info", "Command mode ended")
    clear_panel()


def mode_stress(args: argparse.Namespace) -> None:
    """Process events as fast as possible and report throughput."""
    log("info", "Stress mode started - measuring throughput")

    count = 0
    start = time.monotonic()
    report_interval = args.interval
    last_report = start

    for event in event_stream():
        count += 1
        now = time.monotonic()

        if now - last_report >= report_interval:
            elapsed = now - start
            rate = count / elapsed if elapsed > 0 else 0
            log("info", f"Processed {count} events in {elapsed:.1f}s ({rate:.0f} events/s)")
            set_panel(
                "Stress Test",
                f"## Throughput\n- Events: {count}\n- Rate: {rate:.0f}/s\n- Elapsed: {elapsed:.1f}s",
            )
            last_report = now

    elapsed = time.monotonic() - start
    rate = count / elapsed if elapsed > 0 else 0
    log("info", f"Stress test complete: {count} events in {elapsed:.1f}s ({rate:.0f}/s)")


def mode_validate(_args: argparse.Namespace) -> None:
    """Echo parsed event structure for protocol debugging."""
    log("info", "Validate mode started - echoing event structure")

    for event in event_stream():
        kind = event.get("kind", "?")
        data = event.get("data", {})
        data_type = data.get("data_type", "?")

        fields = {k: type(v).__name__ for k, v in data.items() if k != "data_type"}
        log(
            "info",
            f"Event kind={kind!r} data_type={data_type!r} fields={fields}",
        )

        # Validate expected structure
        issues: list[str] = []
        if not kind:
            issues.append("missing 'kind'")
        if "data" not in event:
            issues.append("missing 'data'")
        if "data_type" not in data:
            issues.append("missing 'data_type' in data")

        match data_type:
            case "CwdChanged":
                if "cwd" not in data:
                    issues.append("CwdChanged missing 'cwd' field")
            case "CommandComplete":
                if "command" not in data:
                    issues.append("CommandComplete missing 'command' field")
            case "TitleChanged":
                if "title" not in data:
                    issues.append("TitleChanged missing 'title' field")
            case "SizeChanged":
                if "cols" not in data or "rows" not in data:
                    issues.append("SizeChanged missing 'cols' or 'rows'")
            case "VariableChanged":
                if "name" not in data or "value" not in data:
                    issues.append("VariableChanged missing 'name' or 'value'")
            case "EnvironmentChanged":
                if "key" not in data or "value" not in data:
                    issues.append("EnvironmentChanged missing 'key' or 'value'")

        if issues:
            log("warn", f"Validation issues: {', '.join(issues)}")
        else:
            log("debug", f"Event {kind} validated OK")

    log("info", "Validate mode ended")


def mode_demo(_args: argparse.Namespace) -> None:
    """Exercise all 9 command types on the first event received."""
    log("info", "Demo mode started - waiting for first event to demo all commands")
    set_panel("Demo", "## Script Demo\nWaiting for first terminal event...")

    event = read_event()
    if event is None:
        log("warn", "No event received - stdin closed immediately")
        return

    kind = event.get("kind", "unknown")
    ts = datetime.now(timezone.utc).strftime("%H:%M:%S")
    log("info", f"Received trigger event: {kind} at {ts}")

    # 1. Log - already demonstrated above
    log("info", "Demo: testing Log command (you're seeing this)")

    # 2. Notify
    notify("Script Demo", f"Triggered by {kind} event at {ts}")
    log("info", "Demo: sent Notify command")

    # 3. SetBadge
    set_badge("DEMO")
    log("info", "Demo: sent SetBadge command")

    # 4. SetVariable
    set_variable("demo_timestamp", ts)
    set_variable("demo_trigger_event", kind)
    log("info", "Demo: sent SetVariable commands")

    # 5. SetPanel
    set_panel(
        "Demo Results",
        (
            f"## Script Demo Complete\n"
            f"- **Trigger**: `{kind}` at {ts}\n"
            f"- **Commands sent**: 9 (all types)\n"
            f"- **Log**: working\n"
            f"- **Notify**: sent\n"
            f"- **SetBadge**: DEMO\n"
            f"- **SetVariable**: demo_timestamp, demo_trigger_event\n"
            f"- **WriteText**: echo command\n"
            f"- **RunCommand**: date\n"
            f"- **ChangeConfig**: (logged only)\n"
            f"- **ClearPanel**: will fire on exit\n"
        ),
    )
    log("info", "Demo: sent SetPanel command")

    # 6. WriteText - write a harmless echo to the terminal
    write_text("echo 'par-term script demo: WriteText command works!'\n")
    log("info", "Demo: sent WriteText command")

    # 7. RunCommand - run a background command
    run_command("echo 'par-term script demo: RunCommand works' > /tmp/par_term_script_demo.txt")
    log("info", "Demo: sent RunCommand command")

    # 8. ChangeConfig - send a config change (may not be implemented yet)
    change_config("font_size", 14.0)
    log("info", "Demo: sent ChangeConfig command (may be no-op)")

    log("info", "Demo complete! All 9 command types sent. Draining remaining events...")

    # Continue draining events to keep the script alive for panel viewing
    for event in event_stream():
        kind = event.get("kind", "unknown")
        log("debug", f"Post-demo event: {kind}")

    # 9. ClearPanel - on exit
    clear_panel()
    log("info", "Demo: sent ClearPanel on exit")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Test script for par-term observer scripting feature",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--mode",
        choices=["monitor", "command", "stress", "validate", "demo"],
        default="monitor",
        help="Operating mode (default: monitor)",
    )
    parser.add_argument(
        "--interval",
        type=float,
        default=2.0,
        help="Report interval in seconds for stress mode (default: 2.0)",
    )

    args = parser.parse_args()

    modes = {
        "monitor": mode_monitor,
        "command": mode_command,
        "stress": mode_stress,
        "validate": mode_validate,
        "demo": mode_demo,
    }

    log("info", f"test_script_observer started in '{args.mode}' mode")

    try:
        modes[args.mode](args)
    except (BrokenPipeError, KeyboardInterrupt):
        pass
    except json.JSONDecodeError as e:
        log("error", f"JSON decode error: {e}")
    finally:
        log("info", "test_script_observer exiting")


if __name__ == "__main__":
    main()
