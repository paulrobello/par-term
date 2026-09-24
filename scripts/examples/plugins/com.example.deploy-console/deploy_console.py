#!/usr/bin/env python3
"""Example par-term **interactive** overlay plugin: a deploy console.

Pushes one interactive overlay — a filter ``text_input``, an environment
``list``, and a ``Deploy`` ``button`` — then serves semantic
``overlay_event`` lines from stdin: ``text_changed`` updates the filter,
``select`` picks an environment, ``click`` acknowledges the deploy by
re-rendering the scene with a timestamped status line.

The plugin owns all state (full-scene replace on every event; the host
owns pixels and input). Delete the marker file (default
``/tmp/par-term-deploy-console``) to see ``ClearOverlay`` land.

Install (see docs/features/PLUGINS.md):
    cp -r scripts/examples/plugins/com.example.deploy-console \\
        ~/.config/par-term/plugins/

then enable it in Settings > Automation > Plugins. Click the overlay to
focus it; Escape returns focus to the terminal.
"""

import datetime
import json
import pathlib
import sys


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


ENVIRONMENTS = ["production", "staging", "integration", "sandbox"]


def scene(filter_text: str, selected: int, status: str) -> dict:
    """Build the full scene for the current state (full-scene replace)."""
    items = [e for e in ENVIRONMENTS if filter_text in e]
    # Keep the selection valid for the filtered list.
    if selected >= len(items):
        selected = max(0, len(items) - 1) if items else -1
    return {
        "type": "row",
        "children": [
            {
                "type": "text_input",
                "id": "filter",
                "value": filter_text,
                "placeholder": "filter envs…",
            },
            {
                "type": "list",
                "id": "env",
                "items": items,
                "selected": selected if selected >= 0 else None,
            },
            {
                "type": "button",
                "id": "deploy",
                "label": "Deploy" if not status else "Deploy again",
            },
            {"type": "text", "text": status},
        ],
    }


def main() -> None:
    settings = settings_from_argv()
    marker = settings.get("marker", "/tmp/par-term-deploy-console")

    marker_armed = pathlib.Path(marker).exists()
    filter_text = ""
    selected = 0
    status = ""

    def push() -> None:
        emit(
            {
                "type": "SetOverlay",
                "id": "console",
                "position": "top-right",
                "size": {"w": 0.3, "h": 0.25},
                "interactive": True,
                "content": scene(filter_text, selected, status),
            }
        )

    push()

    # Event loop: one NDJSON line per stdin read — the single reader (no
    # watcher thread; the for-loop over stdin ends on EOF, which is
    # par-term closing stdin at stop). Events are the semantic widget
    # interactions the focused overlay produced.
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if event.get("kind") != "overlay_event":
            continue
        data = event.get("data", {})
        widget = data.get("widget", "")
        payload = data.get("event", {})
        etype = payload.get("type")
        if widget == "filter" and etype == "TextChanged":
            filter_text = payload.get("value", "")
            status = ""
        elif widget == "env" and etype == "Select":
            selected = payload.get("index", 0)
            status = ""
        elif widget == "deploy" and etype == "Click":
            items = [e for e in ENVIRONMENTS if filter_text in e]
            env = items[selected] if 0 <= selected < len(items) else "?"
            stamp = datetime.datetime.now().strftime("%H:%M:%S")
            status = f"deployed {env} at {stamp}"
        if marker_armed and not pathlib.Path(marker).exists():
            emit({"type": "ClearOverlay", "id": "console"})
            return
        push()


if __name__ == "__main__":
    main()
