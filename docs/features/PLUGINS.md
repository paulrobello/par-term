# Plugins

Plugins are local, user-installed subprocesses that can publish a status-bar
widget, contribute actions to the command palette, or push panel content.
v1 ships three plugin kinds: `status-bar-widget`, `action-contributor`, and
`panel`.

## Table of Contents

- [What a plugin is](#what-a-plugin-is)
- [Installing a plugin](#installing-a-plugin)
- [The manifest](#the-manifest)
- [Security model](#security-model)
- [The SetWidget contract](#the-setwidget-contract)
- [Contributing palette actions](#contributing-palette-actions)
- [Pushing panel content](#pushing-panel-content)
- [Drawing an overlay](#drawing-an-overlay)
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

There are two install paths, managed and unmanaged, and the difference is
deliberate: the git operations only ever touch plugins they installed.

### From a git URL (managed)

```bash
par-term plugin add https://github.com/owner/par-term-plugin-xyz.git
```

`add` warns before it clones and refuses if a plugin with the same id is
already installed, naming the directory in its way. The plugin lands
**disabled** — nothing runs until you enable it (see below).

Updates show a diff before anything is applied, and fast-forward only:

```bash
par-term plugin update              # every git-installed plugin
par-term plugin update <plugin-id>  # just one
```

If upstream rewrote history (a fast-forward is impossible), `update`
refuses and leaves the installed code untouched; remove and re-add the
plugin to take the rewritten version. par-term never resets plugin code,
because the trust model is "read the code before you enable it" — a
reset would silently discard exactly the code you were invited to read.

Removal only ever deletes plugins that `plugin add` installed — a
directory with a `.git` folder and an `origin` remote:

```bash
par-term plugin remove <plugin-id>
```

All three operations are also in **Settings → Automation → Plugins**: an
"Add from git URL" field at the top of the section, and Check for
updates / Remove buttons on git-installed plugin rows.

### By copying a directory (unmanaged)

```bash
cp -r <plugin-dir> ~/.config/par-term/plugins/<plugin-id>
```

The directory name must equal the manifest's `id`. This path involves no
network and leaves no git metadata, so `plugin update` and `plugin
remove` refuse to touch it — copy it in, delete it out. `add` takes git
URLs only for the same reason: a hand-extracted archive is
indistinguishable from a plugin you wrote yourself, so the URL check is
a statement about provenance, not a sandbox.

### After installing, either way

Open **Settings → Automation → Plugins** (search: "plugin"), read the
trust surface — author, version, license, and the exact command that
will run — and flip the **Enabled** toggle. The toggle is the consent
step: a freshly installed plugin does nothing until it is flipped.

Removing a plugin directory does not lose its configuration: the Settings
section shows a gray "not found — state kept" row, and putting the directory
back restores the plugin with its settings and placement intact.

### Why the git operations are shaped this way

- **Warn, then land disabled.** Cloning arbitrary code should never be a
  silent act: `add` says what it is about to fetch, and the plugin still
  does nothing until the enable toggle is flipped.
- **Refuse, never overwrite.** An existing target directory is named and
  refused; nothing merges into it. (Two plugins cannot collide on an id:
  the manifest id must equal the directory name, and directory names are
  unique by filesystem.)
- **Fast-forward only, never reset.** A rewritten upstream is refused
  with the remedy spelled out. The user's path to rewritten code is
  remove-and-re-add, in the open.
- **No prompt can hang the terminal.** Every git invocation runs with
  `GIT_TERMINAL_PROMPT=0` under a hard deadline, so a private-repo URL
  or a passphrase-protected key fails as a clean error instead of
  blocking the GPU process on an auth prompt that has no terminal.

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
| `kinds` | string[] | Kinds provided; each must be known to the host (v1: `status-bar-widget`, `action-contributor`, `panel`). A manifest naming an unknown kind is not loaded. |
| `activation` | string? | `manual` (default; enabled in Settings) or `on_startup`. Either way nothing runs before the enable toggle. |
| `entryPoints` | object | Map of kind → executable: `statusBarWidget`, `actionContributor`, and/or `panel` → `{ "command": "...", "args": [...] }`. |
| `statusBarWidget` | object? | Required when `kinds` includes `status-bar-widget`; see below. |
| `actions` | array? | Required when `kinds` includes `action-contributor`; see [Contributing palette actions](#contributing-palette-actions). |
| `subscriptions` | string[]? | Terminal event kinds delivered to the plugin's stdin; empty or absent means none (self-scheduled). Each name must be a known event kind — see [Event subscriptions](#event-subscriptions). |
| `restart` | string? | When the host restarts an exited process: `on_failure` (default), `never`, or `always`. Mode-only — backoff stays host-owned; see [Lifecycle and restarts](#lifecycle-and-restarts). |

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
- **Installs are explicit** — plugin code reaches the disk only through an
  action the user took: `plugin add` cloning a URL they named, or a
  directory they copied in. Discovery and the running host never fetch
  anything; what runs is what is on the disk.

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

## Pushing panel content

The third plugin kind, `panel`, pushes markdown content that renders in the
plugin's own row in Settings > Automation > Plugins — the same surface a
tab script's `SetPanel` command drives. The model is **push-based**: the
panel kind does not own a persistent window surface (that model belongs to
the `overlay` kind, next section); the plugin's process decides what to
show and when, with two commands:

- `{"type": "SetPanel", "title": "...", "content": "..."}` — show (or
  replace) the plugin's panel. Last write wins; one panel per plugin.
- `{"type": "ClearPanel"}` — dismiss the plugin's panel.

A manifest declaring the kind needs a `panel` entry point and nothing else
— there is no panel block in the manifest, because the pushed content is
entirely the process's runtime decision:

```json
{
  "schemaVersion": 1,
  "id": "com.example.session-notes",
  "name": "Session Notes",
  "version": "0.1.0",
  "kinds": ["panel"],
  "entryPoints": { "panel": { "command": "notes_panel.py", "args": [] } }
}
```

Lifecycle is identical to the other kinds: the process spawns on enable,
supervises under the manifest's `restart` policy, and stops on disable —
which also drops its panel, so a disabled plugin leaves no orphaned
surface. Command dispatch is kind-pure: a panel process's `SetWidget` is
refused just as a widget process's `SetPanel` is.

**Summoning in v1 needs no new mechanism.** The panel is visible wherever
its surface renders (open Settings > Automation > Plugins). A plugin that
wants palette-driven behavior declares *both* `panel` and
`action-contributor`: the palette action's invocation event reaches the
action process through the normal delivery, and the panel process's pushed
content renders regardless.

The example notes plugin (`scripts/examples/plugins/com.example.session-notes/`)
is the reference for the kind: it reads `~/.config/par-term/notes.md`,
pushes it as a panel, refreshes once a minute and on every `bell_rang`
(subscriptions work for the panel kind exactly as for the others), and
clears the panel when the notes file is empty.

## Drawing an overlay

The fourth plugin kind, `overlay`, owns a persistent surface drawn **over
the terminal** — a HUD, dashboard, sticky note, or timer (design:
[overlay plugin design](../plans/2026-09-24-overlay-plugin-design.md)). Two
commands, kind-pure to the overlay kind:

```json
{"type": "SetOverlay", "id": "hud", "position": "top-right", "size": {"w": 0.2, "h": 0.1}, "opacity": 0.9, "content": {"type": "markdown", "text": "## Build\nPASSING"}}
{"type": "ClearOverlay", "id": "hud"}
```

- **Position** is a named anchor (`top-left`, `top`, `top-right`, `left`,
  `center`, `right`, `bottom-left`, `bottom`, `bottom-right`) or an edge
  strip (`top-strip`, `bottom-strip`, `left-strip`, `right-strip`), or a
  free rect `{"x": 0.1, "y": 0.2}` in window fractions. The host clamps
  every overlay on-screen and caps its size at half the window per axis.
- **Size** is `{"w": ..., "h": ...}` in window fractions.
- **Opacity** (0.0–1.0, default 1.0).
- **Content** is a declarative scene tree — `text`, `row` (horizontal
  layout of children), `markdown`, and (in interactive overlays) `button`,
  `text_input`, `list`. Full-scene replace on every upsert; the host
  renders through egui. Updates are clamped to ~30 per second per plugin
  (excess dropped with a warning) so a runaway plugin cannot spin the
  renderer. Images are deferred.
- **Clearing is guaranteed**: plugin stop or disable drops its overlay —
  a disabled plugin leaves no orphaned surface. Overlay commands sent by
  any other kind (or by a tab script) are refused like any other
  kind-impure command.

A manifest declaring the kind needs an `overlay` entry point and nothing
else:

```json
{
  "schemaVersion": 1,
  "id": "com.example.build-hud",
  "name": "Build HUD",
  "version": "0.1.0",
  "kinds": ["overlay"],
  "entryPoints": { "overlay": { "command": "build_hud.py", "args": [] } }
}
```

The example HUD plugin
(`scripts/examples/plugins/com.example.build-hud/`) pushes a ticking
top-right overlay once per second; delete its marker file
(`/tmp/par-term-build-hud`) to see `ClearOverlay` land.

### Interactive overlays

Overlays are non-interactive by default — they render, pointer events pass
through to the terminal beneath, and an `interactive: true` request is
forced off at ingest. Interactivity needs the manifest capability:

```json
"overlay": { "interactive": true }
```

With the capability, a `SetOverlay` may set `"interactive": true` and use
the interactive scene vocabulary:

```json
{"type": "SetOverlay", "id": "hud", "position": "top-right",
 "size": {"w": 0.25, "h": 0.3}, "interactive": true,
 "content": {"type": "row", "children": [
    {"type": "button", "id": "deploy", "label": "Deploy"},
    {"type": "text_input", "id": "filter", "value": "", "placeholder": "filter…"},
    {"type": "list", "id": "jobs", "items": ["build", "test"], "selected": 0}
 ]}}
```

Focus and input follow the mode stack:

- **Never on appear** — an overlay gains focus only when you click one of
  its widgets; at most one overlay is focused at a time (the focused
  overlay draws a brighter border).
- **Escape** returns focus to the terminal.
- **While focused, the plugin receives semantic events only** over its
  stdin — one NDJSON line per widget interaction, never raw keys:

```json
{"kind": "overlay_event", "data": {"data_type": "OverlayEvent", "overlay": "hud", "widget": "deploy", "event": {"type": "Click"}}}
{"kind": "overlay_event", "data": {"data_type": "OverlayEvent", "overlay": "hud", "widget": "filter", "event": {"type": "TextChanged", "value": "par"}}}
{"kind": "overlay_event", "data": {"data_type": "OverlayEvent", "overlay": "hud", "widget": "jobs", "event": {"type": "Select", "index": 1}}}
```

The plugin owns state: react to events by pushing a new full scene (the
`text_input` reports `text_changed` and the host renders whatever value
the scene carries). Focused overlays sit below the built-in pane-hint
selection mode — that modal mode always owns the keys and draws above
every overlay. A plugin without the capability, or an upsert that drops
`interactive`, loses focus immediately.

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

**The vocabulary** is the script event vocabulary — the terminal-sourced
kinds below plus the app-sourced `theme_changed` — validated at discovery:
a subscription naming an unknown or duplicate kind skips the whole plugin
with a warning, so a typo'd subscription can never silently never-fire:

`bell_rang` · `title_changed` · `size_changed` · `mode_changed` ·
`graphics_added` · `hyperlink_added` · `dirty_region` · `cwd_changed` ·
`trigger_matched` · `user_var_changed` · `progress_bar_changed` ·
`badge_changed` · `command_complete` · `zone_opened` · `zone_closed` ·
`zone_scrolled_out` · `environment_changed` · `remote_host_transition` ·
`sub_shell_detected` · `file_transfer_started` · `file_transfer_progress` ·
`file_transfer_completed` · `file_transfer_failed` · `upload_requested` ·
`screen_cleared` · `theme_changed`

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
- **Every kind's process hears them.** Subscriptions are plugin-level: a
  multi-kind manifest delivers to the widget, action, and panel processes
  alike.
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
  it may only send its kind's display commands — `SetWidget` for the
  widget and action kinds, `SetPanel`/`ClearPanel` for the panel kind.
  Adding a raw-output event kind is
  the point where a permission gate becomes mandatory; that seam is
  deliberate and unforged.
- **The manifest declaration is the disclosure.** A plugin's declared
  subscriptions render as a bracketed list in its Settings row (the trust
  surface above the Enable checkbox), the same treatment tab scripts get.
  A plugin with no subscriptions renders no subscription line at all — the
  no-gate decision above rests on the user reading this before enabling.

### theme_changed (app-sourced)

`theme_changed` is the one app-sourced kind: it comes from par-term itself,
not from a terminal, so it never depends on tab activity. A subscribed
plugin receives the current theme immediately on spawn (and again after a
supervised respawn or a disable/enable cycle — a fresh process has never
seen a theme), and one event per actual switch, including the light/dark
auto-switch. Delivery is deduped by theme name: re-applying the same theme
sends nothing.

```json
{"kind": "theme_changed", "data": {"data_type": "ThemeChanged", "theme": "Dracula", "tokens": {"background": "#282a36", "cursor": "#f8f8f0", "foreground": "#f8f8f2", "red": "#ff5555", ...}}}
```

`tokens` carries every theme color — `foreground`, `background`, `cursor`,
`selection_bg`, `selection_fg`, and the 16 ANSI names (`black` …
`bright_white`) — each a lowercase `#rrggbb` string keyed by the theme field
name. See the [theme-swatch example](#the-example-theme-swatch-plugin) for
a working consumer.

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

A plugin declares when the host should restart its exited process with the
manifest's `restart` field:

| Mode | Behaviour |
|---|---|
| `on_failure` (default) | Restart after a crash (non-zero exit) — a plugin that exits successfully stays stopped. |
| `never` | Never restart: a one-shot plugin that does its work and exits is done, and a failing one is not retried. |
| `always` | Keep the process up across even clean exits — for a plugin whose job is to be running. |

All modes share the host-owned backoff: 250 ms restart delay and a
crash-loop cap of 5 attempts in 5 s (a process that survives past the grace
window resets the counter). The cap exists so a broken plugin cannot spin
the CPU, which is exactly why the mode is the only thing the manifest can
declare — backoff parameters stay host-owned, and a manifest that could
tune them could configure itself out of the protection. Restarts re-use the
spawn-time settings argv. A spawn that fails outright (missing interpreter,
exec format error) is backed off through the same supervisor — retried
after the 250 ms delay under the same cap, not re-attempted every frame.

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
- **Four kinds**: `status-bar-widget`, `action-contributor`, `panel`, and
  `overlay` (interactive overlays need the manifest `overlay.interactive`
  capability — see [Drawing an overlay](#drawing-an-overlay)).
- **Restart policy is manifest-declared but mode-only** (`on_failure`
  default, `never`, `always`); backoff parameters (delay, crash-loop cap)
  stay host-owned.
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

## The example theme-swatch plugin

A dependency-free Python theme follower ships in the repository:

```bash
cp -r scripts/examples/plugins/com.example.theme-swatch \
    ~/.config/par-term/plugins/
```

Then Settings → Automation → Plugins → enable **Theme Swatch**. A 🎨 widget
shows the active theme's name and (with "Show bg/fg hex tokens" on, the
default) its background/foreground tokens, and re-renders the moment the
theme changes — try switching themes in Settings, or let the light/dark
auto-switch fire. It is the reference for the app-sourced
[`theme_changed`](#theme_changed-app-sourced) subscription: pure
event-driven, no self-scheduling loop, initial greet on startup.

## Agent ui-test recipe

The `--ui-test` harness ([AGENT_UI_VERIFICATION.md](../guides/AGENT_UI_VERIFICATION.md))
exposes eight plugin operands: `plugins_loaded` (the host's last discovery
scan found ≥1 valid plugin), `plugin_widget_set` (some plugin published
non-empty widget text), `plugin_panel_set` (some panel plugin pushed a
`SetPanel`), `plugin_overlay_set` (some overlay plugin pushed a
`SetOverlay`), `plugin_overlay_interactive` (some overlay is interactive),
`plugin_overlay_focused` (an overlay currently holds focus),
`plugin_overlay_event` (≥1 overlay event was delivered to a running
overlay process this session), and `plugin_action_dispatched` (≥1 plugin
action invocation was successfully delivered to a running action process
this session). The clock recipe runs the first two end-to-end from a clean
XDG root — no user config is touched, and the final `file_empty` assert
proves nothing leaked to the PTY:

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

### Panel variant (session notes)

The panel kind's recipe installs the session-notes example with a notes
file and asserts the `plugin_panel_set` operand — the pushed `SetPanel`
landed in the host (the same map the Settings plugins section mirrors):

```bash
ROOT=/tmp/pt-plugin-panel-ui-test
rm -rf "$ROOT"; mkdir -p "$ROOT/cfg/par-term/plugins"
cp -r scripts/examples/plugins/com.example.session-notes "$ROOT/cfg/par-term/plugins/"
echo '# remember the milk' > "$ROOT/cfg/par-term/notes.md"
cat > "$ROOT/cfg/par-term/config.yaml" <<'EOF'
custom_shell: /bin/sh
shell_args:
  - -c
  - cat > /tmp/pt-plugin-panel-ui-test/pty-capture.txt
shader_install_prompt: never
shell_integration_state: never
plugins:
  - id: com.example.session-notes
    enabled: true
EOF
cat > "$ROOT/script.json" <<'EOF'
{
  "steps": [
    {"wait_ms": 2500, "assert": "plugins_loaded"},
    {"wait_ms": 2000, "assert": "plugin_panel_set"},
    {"assert_not": "modal_guard"},
    {"assert_eq": ["file_empty", "/tmp/pt-plugin-panel-ui-test/pty-capture.txt"]}
  ]
}
EOF
make build
XDG_CONFIG_HOME="$ROOT/cfg" ./target/dev-release/par-term \
  --ui-test "$ROOT/script.json" --ui-test-report "$ROOT/report.json"
```

`all_passed: true` means the notes plugin was discovered, spawned, read
the notes file, and pushed `SetPanel` into the host's panel map — the
exact state the Settings > Automation > Plugins viewer renders. As with
the clock, `python3` on `PATH` is the environmental dependency.

### Overlay variant (build HUD)

The overlay kind's recipe installs the build-hud example and asserts the
`plugin_overlay_set` operand — the pushed `SetOverlay` landed in the
host's overlay map (the same map the render layer draws):

```bash
ROOT=/tmp/pt-plugin-overlay-ui-test
rm -rf "$ROOT"; mkdir -p "$ROOT/cfg/par-term/plugins"
cp -r scripts/examples/plugins/com.example.build-hud "$ROOT/cfg/par-term/plugins/"
cat > "$ROOT/cfg/par-term/config.yaml" <<'EOF'
custom_shell: /bin/sh
shell_args:
  - -c
  - cat > /tmp/pt-plugin-overlay-ui-test/pty-capture.txt
shader_install_prompt: never
shell_integration_state: never
plugins:
  - id: com.example.build-hud
    enabled: true
EOF
cat > "$ROOT/script.json" <<'EOF'
{
  "steps": [
    {"wait_ms": 2500, "assert": "plugins_loaded"},
    {"wait_ms": 2000, "assert": "plugin_overlay_set"},
    {"assert_not": "modal_guard"},
    {"assert_eq": ["file_empty", "/tmp/pt-plugin-overlay-ui-test/pty-capture.txt"]}
  ]
}
EOF
make build
XDG_CONFIG_HOME="$ROOT/cfg" ./target/dev-release/par-term \
  --ui-test "$ROOT/script.json" --ui-test-report "$ROOT/report.json"
```

### Interactive variant (deploy console + pane-hint mode stack)

The interactive overlay's recipe installs the deploy-console example and
exercises the phase 2 invariants end to end: the overlay lands with its
interactive flag intact (manifest capability accepted), focus is never
stolen on appear, and the built-in pane-hint mode both arms over a pushed
overlay and resolves without disturbing it (mode-stack contract — modal
chrome trumps plugin surfaces):

```bash
ROOT=/tmp/pt-panehint-overlay-ui-test
rm -rf "$ROOT"; mkdir -p "$ROOT/cfg/par-term/plugins"
cp -r scripts/examples/plugins/com.example.deploy-console "$ROOT/cfg/par-term/plugins/"
cat > "$ROOT/cfg/par-term/config.yaml" <<'EOF'
custom_shell: /bin/sh
shell_args:
  - -c
  - cat > /tmp/pt-panehint-overlay-ui-test/pty-capture.txt
shader_install_prompt: never
shell_integration_state: never
plugins:
  - id: com.example.deploy-console
    enabled: true
EOF
cat > "$ROOT/script.json" <<'EOF'
{
  "steps": [
    {"wait_ms": 2500, "assert": "plugins_loaded"},
    {"wait_ms": 2000, "assert": "plugin_overlay_interactive"},
    {"assert_not": "plugin_overlay_focused"},
    {"chord": "CmdOrCtrl+D"},
    {"wait_ms": 500, "chord": "CmdOrCtrl+Alt+P"},
    {"wait_ms": 500, "assert": "pane_hint_mode_active"},
    {"assert": "plugin_overlay_set"},
    {"chord": "CmdOrCtrl+Alt+P"},
    {"wait_ms": 500, "assert_not": "pane_hint_mode_active"},
    {"assert": "plugin_overlay_set"},
    {"assert_eq": ["file_empty", "/tmp/pt-panehint-overlay-ui-test/pty-capture.txt"]}
  ]
}
EOF
make build
XDG_CONFIG_HOME="$ROOT/cfg" ./target/dev-release/par-term \
  --ui-test "$ROOT/script.json" --ui-test-report "$ROOT/report.json"
```

Every step must report `ok` in the report (chord steps report the action
they performed; assert steps report the boolean). While the pane-hint mode
is armed, an injected chord resolves the mode instead of reaching the
keybinding layer — the injector mirrors the real event path's mode capture.

## See also

- [AUTOMATION.md](AUTOMATION.md) — triggers, coprocesses, and observer
  scripts
- [STATUS_BAR.md](STATUS_BAR.md) — the bar the widget renders into
- [CUSTOM_SHADERS.md](CUSTOM_SHADERS.md) — unrelated: custom *shaders* are
  not plugins
