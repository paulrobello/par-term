#!/usr/bin/env python3
"""Example par-term status-bar widget plugin: a clock.

Self-scheduled (v1 plugins receive no events): reads its settings from
``--par-term-settings <json>`` on argv, then emits a ``SetWidget`` line
once per second. par-term closes the plugin's stdin on stop, which ends
the stdin watcher thread and lets the loop exit cleanly.

Install (see docs/features/PLUGINS.md):
    cp -r scripts/examples/plugins/com.example.clock \
        ~/.config/par-term/plugins/

then enable it in Settings > Automation > Plugins.
"""

import datetime
import json
import sys
import threading


def settings_from_argv() -> dict:
    """Parse the ``--par-term-settings`` JSON object par-term appends to argv."""
    argv = sys.argv[1:]
    for i, arg in enumerate(argv):
        if arg == "--par-term-settings" and i + 1 < len(argv):
            try:
                value = json.loads(argv[i + 1])
            except json.JSONDecodeError:
                return {}
            return value if isinstance(value, dict) else {}
    return {}


def main() -> None:
    settings = settings_from_argv()
    use_24h = bool(settings.get("format24h", True))

    # par-term closes stdin when the plugin is stopped; EOF there is the
    # shutdown signal.
    stop = threading.Event()

    def watch_stdin() -> None:
        while sys.stdin.readline() != "":
            pass
        stop.set()

    threading.Thread(target=watch_stdin, daemon=True).start()

    while not stop.is_set():
        now = datetime.datetime.now()
        text = now.strftime("%H:%M" if use_24h else "%I:%M %p")
        print(json.dumps({"type": "SetWidget", "text": f"\U0001f552 {text}"}), flush=True)
        stop.wait(1.0)


if __name__ == "__main__":
    main()
