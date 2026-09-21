#!/usr/bin/env python3
"""Example par-term action-contributor plugin: a greeter.

Declares one palette action, ``greet``. par-term delivers an invocation as
one NDJSON line on stdin — ``{"kind": "plugin_action_invoked", "data":
{"data_type": "PluginActionInvoked", "action": "greet"}}`` — and each
``greet`` invocation appends ``<iso-timestamp> greeting`` to ``stamps.txt``
next to this script; events naming any other action id are ignored (the
routing pattern a multi-action plugin copies). par-term closes the plugin's
stdin on stop; EOF exits cleanly (PLUGINS.md's shutdown contract).

The plugin writes nothing to stdout: its effects are its own process's, by
design — an action that wants something done does it itself rather than
asking the host to (design D4).

Install (see docs/features/PLUGINS.md):
    cp -r scripts/examples/plugins/com.example.greeter \
        ~/.config/par-term/plugins/

then enable it in Settings > Automation > Plugins. The action appears in
the command palette as "Greet · Greeter" and can be bound in config.yaml:

    keybindings:
      - key: "Ctrl+Alt+G"
        action: "plugin-action:com.example.greeter:greet"
"""

import datetime
import json
import pathlib
import sys

STAMPS = pathlib.Path(__file__).resolve().parent / "stamps.txt"


def main() -> None:
    for line in iter(sys.stdin.readline, ""):
        line = line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except ValueError:
            continue
        data = event.get("data", {})
        if (
            data.get("data_type") == "PluginActionInvoked"
            and data.get("action") == "greet"
        ):
            stamp = datetime.datetime.now(datetime.timezone.utc).isoformat()
            with STAMPS.open("a", encoding="utf-8") as stamps:
                stamps.write(f"{stamp} greeting\n")
                stamps.flush()


if __name__ == "__main__":
    main()
