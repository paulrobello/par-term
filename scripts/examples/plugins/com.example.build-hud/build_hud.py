#!/usr/bin/env python3
"""Example par-term overlay plugin: a ticking build HUD.

Pushes a ``SetOverlay`` once per second — a small panel anchored
top-right showing a status line and an elapsed counter. When the
marker file named by ``--par-term-hud-marker`` (default
``/tmp/par-term-build-hud``) disappears, the plugin sends
``ClearOverlay`` and exits, so a scripted run can verify the
clearing-on-disable invariant by deleting the file.

par-term closes the plugin's stdin on stop, which is the shutdown
signal for the stdin watcher thread.

Install (see docs/features/PLUGINS.md):
    cp -r scripts/examples/plugins/com.example.build-hud \\
        ~/.config/par-term/plugins/

then enable it in Settings > Automation > Plugins.
"""

import datetime
import json
import sys
import threading
import time


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


def emit(command: dict) -> None:
    """Write one NDJSON protocol line to stdout and flush."""
    sys.stdout.write(json.dumps(command) + "\n")
    sys.stdout.flush()


def main() -> None:
    settings = settings_from_argv()
    marker = settings.get("marker", "/tmp/par-term-build-hud")

    # The stdin watcher thread: EOF (par-term closed stdin on stop) is the
    # exit signal, so the main loop below can block on sleep indefinitely.
    stop = threading.Event()

    def watch_stdin() -> None:
        try:
            while sys.stdin.buffer.read(4096):
                pass
        except Exception:
            pass
        stop.set()

    threading.Thread(target=watch_stdin, daemon=True).start()

    started = time.monotonic()
    try:
        while not stop.is_set():
            elapsed = int(time.monotonic() - started)
            stamp = datetime.datetime.now().strftime("%H:%M:%S")
            try:
                marker_alive = __import__("pathlib").Path(marker).exists()
            except OSError:
                marker_alive = True
            if not marker_alive:
                emit({"type": "ClearOverlay", "id": "hud"})
                return
            emit(
                {
                    "type": "SetOverlay",
                    "id": "hud",
                    "position": "top-right",
                    "size": {"w": 0.18, "h": 0.12},
                    "opacity": 0.9,
                    "content": {
                        "type": "markdown",
                        "text": f"## BUILD\n\nPASSING · {elapsed}s\n\n_{stamp}_",
                    },
                }
            )
            stop.wait(1.0)
    except BrokenPipeError:
        # stdout closed under us — par-term is tearing down; just exit.
        pass


if __name__ == "__main__":
    main()
