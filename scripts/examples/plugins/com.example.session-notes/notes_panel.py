#!/usr/bin/env python3
"""Example par-term panel plugin: a session notes viewer.

Reads ``notes.md`` from the par-term config directory and pushes it as a
panel (``SetPanel``) that renders in Settings > Automation > Plugins. The
panel kind is push-based: the plugin decides what to show and when, exactly
like a tab script calling ``SetPanel``. It refreshes once a minute and on
every ``bell_rang`` event (demonstrating event subscriptions); an empty or
missing notes file clears the panel (``ClearPanel``).

Install (see docs/features/PLUGINS.md):
    cp -r scripts/examples/plugins/com.example.session-notes \\
        ~/.config/par-term/plugins/
    echo '# remember the milk' > ~/.config/par-term/notes.md

then enable it in Settings > Automation > Plugins.
"""

import json
import os
import sys
import threading


def notes_path() -> str:
    config = os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config"))
    return os.path.join(config, "par-term", "notes.md")


def read_notes() -> str:
    try:
        with open(notes_path(), "r", encoding="utf-8") as f:
            return f.read().strip()
    except OSError:
        return ""


def main() -> None:
    # par-term closes stdin when the plugin is stopped; EOF there is the
    # shutdown signal. Subscribed events (bell_rang) arrive as NDJSON lines
    # on stdin — a wake-up to re-read the notes, nothing to parse further.
    wake = threading.Event()

    def watch_stdin() -> None:
        while sys.stdin.readline() != "":
            wake.set()
        wake.set()

    threading.Thread(target=watch_stdin, daemon=True).start()

    while True:
        notes = read_notes()
        if notes:
            cmd = {
                "type": "SetPanel",
                "title": "Session notes",
                "content": notes,
            }
        else:
            cmd = {"type": "ClearPanel"}
        print(json.dumps(cmd), flush=True)
        wake.wait(60.0)
        wake.clear()


if __name__ == "__main__":
    main()
