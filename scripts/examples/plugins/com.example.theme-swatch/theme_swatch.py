#!/usr/bin/env python3
"""Example par-term status-bar widget plugin: a theme swatch.

Event-driven: subscribes to ``theme_changed`` (see manifest.json), so it
receives the current theme immediately on startup and again on every switch
— including the light/dark auto-switch. Each event carries the theme name
and every theme color as ``#rrggbb`` tokens keyed by field name
(``background``, ``foreground``, the 16 ANSI names, ...).

Install (see docs/features/PLUGINS.md):
    cp -r scripts/examples/plugins/com.example.theme-swatch \
        ~/.config/par-term/plugins/

then enable it in Settings > Automation > Plugins.

par-term closes the plugin's stdin on stop; EOF ends the read loop.
"""

import json
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


def main() -> None:
    show_tokens = bool(settings_from_argv().get("showTokens", True))

    for line in iter(sys.stdin.readline, ""):
        line = line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if event.get("kind") != "theme_changed":
            continue
        data = event.get("data", {})
        tokens = data.get("tokens", {})
        text = f"\U0001f3a8 {data.get('theme', '?')}"
        if show_tokens:
            text += f" · bg {tokens.get('background', '?')} · fg {tokens.get('foreground', '?')}"
        print(json.dumps({"type": "SetWidget", "text": text}), flush=True)


if __name__ == "__main__":
    main()
