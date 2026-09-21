# Plugins

Plugins are local, user-installed subprocesses that can publish a status-bar
widget. v1 ships exactly one plugin kind: `status-bar-widget`.

## Table of Contents

- [What a plugin is](#what-a-plugin-is)
- [Installing a plugin](#installing-a-plugin)
- [The manifest](#the-manifest)
- [Security model](#security-model)
- [The SetWidget contract](#the-setwidget-contract)
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
| `kinds` | string[] | Kinds provided; each must be known to the host (v1: `status-bar-widget`). A manifest naming an unknown kind is not loaded. |
| `activation` | string? | `manual` (default; enabled in Settings) or `on_startup`. Either way nothing runs before the enable toggle. |
| `entryPoints` | object | Map of kind → executable. For v1: `statusBarWidget` → `{ "command": "...", "args": [...] }`. |
| `statusBarWidget` | object? | Required when `kinds` includes `status-bar-widget`; see below. |

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
Restarts re-use the spawn-time settings argv.

## Diagnostics

Plugin faults (a directory skipped by discovery, an enabled plugin that is
not discovered, a failed spawn, invalid persisted settings) are reported as
`log::warn!` records in the debug log — once per episode, not per frame, so
a steady fault does not flood the log. They are **not visible by default**:
the config `log_level` defaults to `off` and is applied right after startup,
silencing all `log::` records from that point (a `Config log level: …` line
in the debug log records the level actually applied).

To see plugin diagnostics, run with the level raised:

```bash
par-term --log-level warn        # CLI flag beats the config setting
```

or set `log_level: warn` in the config (Settings → Advanced → System). Note
the config setting overrides `RUST_LOG`; only the CLI flag beats it. The
Settings → Automation → Plugins section also shows not-discovered plugins as
rows regardless of the log level.

## v1 limits

- **Self-scheduled only.** Plugins receive no terminal events in v1; a
  widget decides for itself when to refresh (the clock sleeps one second).
  The manifest `subscriptions` field is reserved for a future phase.
- **One kind**: `status-bar-widget`. `action-contributor`, `panel`, and
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
exposes two plugin operands: `plugins_loaded` (the host's last discovery scan
found ≥1 valid plugin) and `plugin_widget_set` (some plugin published
non-empty widget text). This recipe runs both end-to-end from a clean XDG
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

## See also

- [AUTOMATION.md](AUTOMATION.md) — triggers, coprocesses, and observer
  scripts
- [STATUS_BAR.md](STATUS_BAR.md) — the bar the widget renders into
- [CUSTOM_SHADERS.md](CUSTOM_SHADERS.md) — unrelated: custom *shaders* are
  not plugins
