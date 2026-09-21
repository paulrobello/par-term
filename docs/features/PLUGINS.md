# Plugins

Plugins are local, user-installed subprocesses that can publish a status-bar
widget and contribute actions to the command palette. v1 ships two plugin
kinds: `status-bar-widget` and `action-contributor`.

## Table of Contents

- [What a plugin is](#what-a-plugin-is)
- [Installing a plugin](#installing-a-plugin)
- [The manifest](#the-manifest)
- [Security model](#security-model)
- [The SetWidget contract](#the-setwidget-contract)
- [Contributing palette actions](#contributing-palette-actions)
- [Event subscriptions](#event-subscriptions)
- [Settings](#settings)
- [Lifecycle and restarts](#lifecycle-and-restarts)
- [Diagnostics](#diagnostics)
- [v1 limits](#v1-limits)
- [The example clock plugin](#the-example-clock-plugin)
- [Agent ui-test recipe](#agent-ui-test-recipe)

## What a plugin is

A plugin is a subprocess, never in-process. From the design:

> No in-process plugins, ever — par-term is a GPU app; a panic in plugin code
> must never take the render loop down. The subprocess boundary is the point.

A plugin therefore is: a directory under `~/.config/par-term/plugins/`
containing a `manifest.json` and whatever files the manifest's entry command
names. The host scans that directory, and for each plugin the user has
enabled, spawns the entry command as a child process. The plugin writes JSON
lines to stdout; the host reads them. Nothing a plugin does can take the
terminal down — at worst its process dies, and the host restarts it.

## Installing a plugin

The host never fetches anything from the network. Installing is copying a
directory:

```bash
cp -r <plugin-dir> ~/.config/par-term/plugins/<plugin-id>
```

The directory name must equal the manifest's `id`. After copying, open
**Settings → Automation → Plugins** (search: "plugin"), read the trust
surface — author, version, license, and the exact command that will run —
and flip the **Enabled** toggle. The toggle is the consent step: a freshly
copied plugin does nothing until it is flipped.

Removing a plugin directory does not lose its configuration: the Settings
section shows a gray "not found — state kept" row, and putting the directory
back restores the plugin with its settings and placement intact.

## The manifest

`manifest.json` is the entire plugin declaration:

| Field | Type | Meaning |
|---|---|---|
| `schemaVersion` | number | Must be `1`. |
| `id` | string | Reverse-DNS-style id; must equal the directory name. |
| `name` | string | Human-readable name shown in Settings. |
| `version` | string | Plugin version, shown at the enable toggle. |
| `author` | string? | Author, shown at the enable toggle. |
| `license` | string? | License, shown at the enable toggle. |
| `description` | string? | What the plugin does, shown in Settings. |
| `kinds` | string[] | Kinds provided; each must be known to the host (v1: `status-bar-widget`, `action-contributor`). A manifest naming an unknown kind is not loaded. |
| `activation` | string? | `manual` (default; enabled in Settings) or `on_startup`. Either way nothing runs before the enable toggle. |
| `entryPoints` | object | Map of kind → executable: `statusBarWidget` and/or `actionContributor` → `{ "command": "...", "args": [...] }`. |
| `statusBarWidget` | object? | Required when `kinds` includes `status-bar-widget`; see below. |
| `actions` | array? | Required when `kinds` includes `action-contributor`; see [Contributing palette actions](#contributing-palette-actions). |
| `subscriptions` | string[]? | Terminal event kinds delivered to the plugin's stdin; empty or absent means none (self-scheduled). Each name must be a known event kind — see [Event subscriptions](#event-subscriptions). |

The `statusBarWidget` block:

| Field | Type | Meaning |
|---|---|---|
| `displayName` | string | Name shown in the status-bar Settings list. |
| `section` | string? | Section the widget lands in when first enabled: `left`, `center`, or `right` (default `right`). Applies when the widget row is first created; the user can move it afterwards. |
| `defaults` | object? | Settings values used when the user has configured nothing. |
| `schema` | array | Typed settings the Settings UI renders an editor for. |

Each `schema` entry: `key`, `type` (`string` | `integer` | `number` |
`boolean`), `label`, optional `min`/`max`/`step` for numerics, and
`defaultValue`. Keys must be non-empty and unique.

A `.py` entry command is run with the first Python interpreter found on
`PATH` (`python3`, `python` — plus `py` on Windows). Any other command must
be executable by that name relative to the plugin directory.

## Security model

- **Subprocess boundary** — plugin code never runs inside par-term.
- **Entry confinement** — `entryPoints.*.command` is resolved relative to
  the plugin directory only. The joined path is canonicalized and must stay
  inside the plugin directory; there is no `PATH` lookup, and a `../` escape
  fails validation and skips the plugin with a warning.
- **Land-disabled** — discovery never spawns anything. Only plugins whose
  `plugins:` state entry says `enabled: true` run, and only the Settings
  toggle writes that bit.
- **Display-only commands** — a v1 plugin may only send `SetWidget`.
  Anything else on its stdout is refused with an error line and ignored.
- **Local installs only** — the host never fetches plugin code; what runs is
  what the user copied onto the disk.

## The SetWidget contract

The plugin writes JSON lines to stdout:

```json
{"type": "SetWidget", "text": "🕒 14:32"}
```

Last write wins: the status bar shows the most recent text for that plugin.
An empty `text` hides the widget. The widget's *presence* in the bar is the
`plugin:<id>` row in the status-bar widget configuration — enabling the
plugin in Settings creates it, and it self-hides while the plugin publishes
nothing.

par-term closes the plugin's stdin when the plugin is stopped; a
well-behaved plugin treats stdin EOF as its shutdown signal and exits.

## Contributing palette actions

The second plugin kind, `action-contributor`, declares named actions that
appear in the command palette and can be bound to keybindings —
extensibility without recompiling par-term. A manifest declaring the kind
needs an `actionContributor` entry point and an `actions` block; the
repository's example greeter is the reference shape:

```json
{
  "schemaVersion": 1,
  "id": "com.example.greeter",
  "name": "Greeter",
  "version": "0.1.0",
  "author": "par-term example",
  "license": "MIT",
  "description": "Example action-contributor plugin. Contributes one palette action; each invocation appends a timestamped greeting to stamps.txt next to the script. Also subscribes to bell_rang to demonstrate event subscriptions.",
  "kinds": ["action-contributor"],
  "activation": "manual",
  "entryPoints": { "actionContributor": { "command": "greeter.py", "args": [] } },
  "subscriptions": ["bell_rang"],
  "actions": [
    { "id": "greet", "label": "Greet", "description": "Append a timestamped greeting to stamps.txt" }
  ]
}
```

Each `actions` entry has an `id`, a `label`, and an optional `description`.
The `id` must match `[a-zA-Z0-9_-]+` — colons are rejected because the wire
id is colon-delimited — be unique in the manifest, and carry a non-empty
`label`. As with every manifest fault, an invalid `actions` block skips the
whole plugin with a warning, never loads it half-valid.

A plugin may declare both kinds in one manifest; each kind's entry point
validates and runs independently (one supervised process per kind, sharing
the plugin's settings).

**The wire id.** Every contributed action is addressable as
`plugin-action:<plugin-id>:<action-id>` — for the greeter,
`plugin-action:com.example.greeter:greet`. Enabled plugins' actions appear
in the palette when it opens, labeled `<manifest label> · <plugin name>`
(e.g. `Greet · Greeter`). A config keybinding can target the wire id
directly:

```yaml
keybindings:
  - key: "Ctrl+Alt+G"
    action: "plugin-action:com.example.greeter:greet"
```

There is deliberately no action-name allowlist at config load: the binding
is inert until the plugin is installed and enabled. A binding to an absent
or disabled plugin (or an unknown action) warns once and does nothing —
the same contract as a binding to a deleted snippet.

**Dispatch semantics.** Activating the palette row or pressing the bound
chord writes one line to the plugin's running action process on stdin:

```json
{"kind": "plugin_action_invoked", "data": {"data_type": "PluginActionInvoked", "action": "greet"}}
```

`true` back to the keybinding layer means **delivered**, not executed:
par-term does not wait for, parse, or require a response. A plugin whose
process is between restarts or crash-capped simply does not get the
invocation (warned once in the log). And the plugin's effects are its own
process's, by design — an action that wants something done does it itself
from that event (the greeter appends to `stamps.txt` beside its script);
plugins cannot ask the host to run commands or write to the terminal in
v1. That is the whole capability model of an action: a user-enabled
process running as the user.

**v2 seams (explicitly not in v1).** Two extensions are deliberately
deferred and recorded here so later phases find them:

- **Action arguments.** v1 actions invoke argumentless. When arguments
  appear, a schema entry (the same typed schema widgets use) will define
  palette prompting per argument.
- **Host-mediated effect commands.** The restricted `ScriptCommand` tier
  tab scripts use (flags, confirm dialogs, rate limits, denylists) stays
  scripts-only in v1. Bridging a restricted command set to plugins would
  reuse that machinery behind per-plugin permission config in the
  `plugins:` state — deliberately not shipped before any plugin needs it.

The example greeter plugin (`scripts/examples/plugins/com.example.greeter/`)
is the reference for the kind: copy it under `~/.config/par-term/plugins/`
and enable it like any plugin. It writes nothing to stdout — its effect is
a line in its own `stamps.txt` per invocation, and stdin EOF exits cleanly.
It also declares `subscriptions: ["bell_rang"]`, making it the reference
for [event subscriptions](#event-subscriptions): every bell in the window's
tabs appends a `bell` line to the same `stamps.txt`.

## Event subscriptions

A plugin that wants to react to the terminal declares the events it wants
in its manifest:

```json
"subscriptions": ["bell_rang", "cwd_changed", "command_complete"]
```

Each listed kind is delivered to the plugin's stdin as one NDJSON line, in
the same event shape tab scripts receive (payload details per kind in
[AUTOMATION.md](AUTOMATION.md#events-stdin)):

```json
{"kind": "cwd_changed", "data": {"data_type": "CwdChanged", "cwd": "/home/user/project"}}
```

**The vocabulary** is the script event vocabulary, validated at discovery —
a subscription naming an unknown or duplicate kind skips the whole plugin
with a warning, so a typo'd subscription can never silently never-fire:

`bell_rang` · `title_changed` · `size_changed` · `mode_changed` ·
`graphics_added` · `hyperlink_added` · `dirty_region` · `cwd_changed` ·
`trigger_matched` · `user_var_changed` · `progress_bar_changed` ·
`badge_changed` · `command_complete` · `zone_opened` · `zone_closed` ·
`zone_scrolled_out` · `environment_changed` · `remote_host_transition` ·
`sub_shell_detected` · `file_transfer_started` · `file_transfer_progress` ·
`file_transfer_completed` · `file_transfer_failed` · `upload_requested` ·
`screen_cleared`

Semantics worth knowing before declaring one:

- **Empty or absent means self-scheduled.** A plugin with no subscriptions
  receives no terminal events — the clock's model. This is deliberately the
  opposite of tab scripts, where an empty subscription list means *all*
  events; the difference keeps every pre-subscriptions plugin behaving
  exactly as it did.
- **Delivery is window-wide.** Events from every tab of the plugin's window
  are delivered; each event originates at exactly one terminal, so nothing
  is duplicated, but events carry no tab attribution in v1 — a plugin
  cannot tell which tab rang the bell.
- **Both kind processes hear them.** Subscriptions are plugin-level: a
  both-kinds manifest delivers to the widget process and the action
  process alike.
- **Backpressure is the script policy.** Each subscribed plugin gets its
  own event forwarder carrying its own kind filter — the same machinery
  tab scripts use: at most 1024 events buffered per plugin, evicting the
  oldest when full, with one overflow warning per forwarder. The per-plugin
  filter is what keeps a subscription reliable under load (a chatty
  terminal's `dirty_region` flood never enters a forwarder filtered to
  `bell_rang`). A plugin that subscribes but never reads its stdin will
  eventually fill its pipe and lose events — read what you subscribe to.
- **No secrets, no gate.** The vocabulary carries no raw terminal output —
  there is no event kind for pane bytes — so subscriptions need no
  permission flag in v1. The kinds that do carry terminal-derived text
  (`command_complete`, `trigger_matched`) already reach tab scripts
  ungated, and a plugin remains strictly narrower than a script: outbound
  it may still only send `SetWidget`. Adding a raw-output event kind is
  the point where a permission gate becomes mandatory; that seam is
  deliberate and unforged.

## Settings

## Settings

Schema-declared settings are edited in **Settings → Automation → Plugins**
(checkbox for booleans, drag value with the manifest's min/max/step for
numerics, text field for strings). Values are persisted in
`config.yaml` under `plugins:` and validated against the schema at apply
time — invalid persisted values fall back to schema defaults with a warning
rather than blocking the plugin.

Settings are passed to the plugin as a single JSON object on argv at spawn:

```
<entry command> <entry args> --par-term-settings {"format24h":true}
```

argv is read once at spawn, which is exactly the settings' lifecycle:
changed settings apply when the plugin next starts. A running plugin keeps
its spawn-time settings until it is toggled off and on again (or the app
restarts).

## Lifecycle and restarts

A plugin that exits successfully stays stopped. A plugin that crashes is
restarted after 250 ms under an on-failure policy with a crash-loop cap
(5 attempts in 5 s); the cap exists so a broken plugin cannot spin the CPU.
Restarts re-use the spawn-time settings argv. A spawn that fails outright
(missing interpreter, exec format error) is backed off through the same
supervisor — retried after the 250 ms delay under the same cap, not
re-attempted every frame.

## Diagnostics

Plugin faults (a directory skipped by discovery, an enabled plugin that is
not discovered, a failed spawn, invalid persisted settings) are reported as
`log::warn!` records in the debug log — once per episode, not per frame, so
a steady fault does not flood the log. They are visible by default: the
config `log_level` defaults to `warn` (a `Config log level: …` line in the
debug log records the level actually applied).

If the level has been lowered, raise it to see plugin diagnostics:

```bash
par-term --log-level warn        # CLI flag beats the config setting
```

or restore `log_level: warn` in the config (Settings → Advanced → System).
Note the config setting overrides `RUST_LOG`; only the CLI flag beats it.
The Settings → Automation → Plugins section also shows not-discovered
plugins as rows regardless of the log level.

## v1 limits

- **Event subscriptions are kind-only.** A plugin may declare which event
  kinds it wants (see [Event subscriptions](#event-subscriptions)); there is
  no filter or regex form in v1. A plugin with no subscriptions stays
  self-scheduled (the clock sleeps one second), and an action-contributor's
  process hears its own invocations plus whatever it subscribes to.
- **Two kinds**: `status-bar-widget` and `action-contributor`. `panel` and
  `overlay` kinds are deliberately deferred.
- **Restart policy is fixed** (on-failure); manifests carry no restart field.
- **One widget per plugin.**

## The example clock plugin

A dependency-free Python clock ships in the repository:

```bash
cp -r scripts/examples/plugins/com.example.clock \
    ~/.config/par-term/plugins/
```

Then Settings → Automation → Plugins → enable **Clock**. A 🕒 widget with
the current time appears on the right of the status bar; the "24-hour clock"
checkbox in its settings editor switches the format. The plugin emits the
time once per second and exits cleanly when par-term stops it.

The source (`scripts/examples/plugins/com.example.clock/`) is the reference
for the manifest shape, the settings argv, and the `SetWidget` loop.

## Agent ui-test recipe

The `--ui-test` harness ([AGENT_UI_VERIFICATION.md](../guides/AGENT_UI_VERIFICATION.md))
exposes three plugin operands: `plugins_loaded` (the host's last discovery
scan found ≥1 valid plugin), `plugin_widget_set` (some plugin published
non-empty widget text), and `plugin_action_dispatched` (≥1 plugin action
invocation was successfully delivered to a running action process this
session). The clock recipe runs the first two end-to-end from a clean XDG
root — no user config is touched, and the final `file_empty` assert proves
nothing leaked to the PTY:

```bash
ROOT=/tmp/pt-plugin-ui-test
rm -rf "$ROOT"; mkdir -p "$ROOT/cfg/par-term/plugins"
cp -r scripts/examples/plugins/com.example.clock "$ROOT/cfg/par-term/plugins/"
cat > "$ROOT/cfg/par-term/config.yaml" <<'EOF'
custom_shell: /bin/sh
shell_args:
  - -c
  - cat > /tmp/pt-plugin-ui-test/pty-capture.txt
shader_install_prompt: never
shell_integration_state: never
plugins:
  - id: com.example.clock
    enabled: true
status_bar_widgets:
  - id: plugin:com.example.clock
    section: right
EOF
cat > "$ROOT/script.json" <<'EOF'
{
  "steps": [
    {"wait_ms": 2500, "assert": "plugins_loaded"},
    {"wait_ms": 2000, "assert": "plugin_widget_set"},
    {"assert_not": "modal_guard"},
    {"assert_eq": ["file_empty", "/tmp/pt-plugin-ui-test/pty-capture.txt"]}
  ]
}
EOF
make build
XDG_CONFIG_HOME="$ROOT/cfg" ./target/dev-release/par-term \
  --ui-test "$ROOT/script.json" --ui-test-report "$ROOT/report.json"
```

`all_passed: true` means the clock was discovered under the isolated root,
spawned, published its `🕒 …` text into the bar, no modal guard interfered,
and no keystroke reached the shell. The example plugin needs `python3` (or
`python`) on `PATH` — on legs with no interpreter the first two asserts fail
with a not-discovered / not-publishing reading; that is the documented skip,
not a host defect.

### Actions variant (greeter)

The same harness drives the action kind. This variant installs the greeter,
binds its wire id to a chord in the test config, presses the chord through
the real keybinding layer, and asserts the delivery operand:

```bash
ROOT=/tmp/pt-plugin-action-ui-test
rm -rf "$ROOT"; mkdir -p "$ROOT/cfg/par-term/plugins"
cp -r scripts/examples/plugins/com.example.greeter "$ROOT/cfg/par-term/plugins/"
cat > "$ROOT/cfg/par-term/config.yaml" <<'EOF'
custom_shell: /bin/sh
shell_args:
  - -c
  - cat > /tmp/pt-plugin-action-ui-test/pty-capture.txt
shader_install_prompt: never
shell_integration_state: never
plugins:
  - id: com.example.greeter
    enabled: true
keybindings:
  - key: "Ctrl+Alt+G"
    action: "plugin-action:com.example.greeter:greet"
EOF
cat > "$ROOT/script.json" <<'EOF'
{
  "steps": [
    {"wait_ms": 2500, "assert": "plugins_loaded"},
    {"wait_ms": 1500, "chord": "Ctrl+Alt+G"},
    {"wait_ms": 500, "assert": "plugin_action_dispatched"},
    {"assert_not": "modal_guard"},
    {"assert_eq": ["file_empty", "/tmp/pt-plugin-action-ui-test/pty-capture.txt"]}
  ]
}
EOF
make build
XDG_CONFIG_HOME="$ROOT/cfg" ./target/dev-release/par-term \
  --ui-test "$ROOT/script.json" --ui-test-report "$ROOT/report.json"
```

The harness has no file-not-empty assert (only `file_empty`), so the recipe
checks the greeter's side effect — the `stamps.txt` line the invocation
appended in the plugin's installed directory — from the shell after
par-term exits:

```bash
[ -s "$ROOT/cfg/par-term/plugins/com.example.greeter/stamps.txt" ] \
  && echo "greeter stamped" || echo "MISSING stamps.txt"
```

`all_passed: true` plus the shell check means the greeter was discovered
and spawned under the isolated root, the chord reached the keybinding
registry, dispatch was delivered to the running action process
(`plugin_action_dispatched = true`), the plugin observed the event and
wrote its stamp, and nothing leaked to the PTY. As with the clock, the
interpreter is the environmental dependency.

### Subscriptions variant (greeter bell)

The greeter's manifest declares `subscriptions: ["bell_rang"]`, and this
variant proves the delivery chain end to end: the test shell itself rings
the terminal bell — after a delay, so the plugin is spawned and subscribed
first — and the greeter's `stamps.txt` must carry the `bell` line:

```bash
ROOT=/tmp/pt-plugin-sub-ui-test
rm -rf "$ROOT"; mkdir -p "$ROOT/cfg/par-term/plugins"
cp -r scripts/examples/plugins/com.example.greeter "$ROOT/cfg/par-term/plugins/"
cat > "$ROOT/cfg/par-term/config.yaml" <<'EOF'
custom_shell: /bin/sh
shell_args:
  - -c
  - sleep 3; printf '\a' > /dev/tty; exec cat > /tmp/pt-plugin-sub-ui-test/pty-capture.txt
shader_install_prompt: never
shell_integration_state: never
plugins:
  - id: com.example.greeter
    enabled: true
EOF
cat > "$ROOT/script.json" <<'EOF'
{
  "steps": [
    {"wait_ms": 2500, "assert": "plugins_loaded"},
    {"wait_ms": 3000, "assert_not": "modal_guard"},
    {"assert_eq": ["file_empty", "/tmp/pt-plugin-sub-ui-test/pty-capture.txt"]}
  ]
}
EOF
make build
XDG_CONFIG_HOME="$ROOT/cfg" ./target/dev-release/par-term \
  --ui-test "$ROOT/script.json" --ui-test-report "$ROOT/report.json"
grep -q bell "$ROOT/cfg/par-term/plugins/com.example.greeter/stamps.txt" \
  && echo "subscription delivered" || echo "MISSING bell stamp"
```

The `sleep 3` rings the bell only after the plugin's process is up and
subscribed; `\a` is written to `/dev/tty` (the terminal's output side), so
it never reaches the capture file. `all_passed: true` plus
"subscription delivered" is the full chain: bell byte → terminal event →
the plugin's forwarder → plugin stdin → `stamps.txt`. The interpreter
remains the environmental dependency.

## See also

- [AUTOMATION.md](AUTOMATION.md) — triggers, coprocesses, and observer
  scripts
- [STATUS_BAR.md](STATUS_BAR.md) — the bar the widget renders into
- [CUSTOM_SHADERS.md](CUSTOM_SHADERS.md) — unrelated: custom *shaders* are
  not plugins
