# Agent-Operable UI Verification (`--ui-test`)

How an agent (or CI) drives par-term's UI and observes overlay state without
any host permission. This exists because the alternatives do not work on this
platform:

| Route | Why it fails |
|---|---|
| `osascript ... keystroke` | TCC assistive access is granted per code signature; an ungranted host process gets error 1002 |
| `screencapture -x` | TCC screen recording: "could not create image from display" |
| `--screenshot` | Captures the offscreen pane composite only — the egui layer (palette, settings, tab bar) is excluded by design |
| Automation triggers | Eight action types, none can invoke a keybinding action |

`--ui-test` runs inside the live app, where none of those walls apply.

## Usage

```bash
par-term --ui-test script.json [--ui-test-report report.json]
```

The app starts normally (real window, real PTY, real config), executes the
script, writes a JSON report, and exits. The report's `all_passed` field is
the verdict; every step also carries an `observation` snapshot of overlay
state regardless of assertions, so a failing run shows the UI's actual state
at each point. A step that cannot execute at all — an unknown press/chord
name, or an action before any terminal window exists — counts as a failure
and flips `all_passed`, the same as a false assert; a chord deliberately
blocked by the modal guard is recorded (`chord … blocked by modal guard`) but
stays verdict-neutral, since the block mirrors the real key path.

## Script format

A JSON object with a `steps` array. Each step is one object with an optional
`wait_ms` (delay before the step runs; default 250):

```json
{
  "steps": [
    {"wait_ms": 2500, "assert_not": "palette_open"},
    {"chord": "Ctrl+Alt+Cmd+P", "wait_ms": 400},
    {"assert": "palette_open"},
    {"type_text": "fullscr", "wait_ms": 400},
    {"assert_eq": ["top_action", "toggle_fullscreen"]},
    {"press": "Enter", "wait_ms": 700},
    {"assert_eq": ["file_empty", "/tmp/pty-capture.txt"]}
  ]
}
```

### Steps

| Step | Meaning |
|---|---|
| `{"chord": "Ctrl+Alt+Cmd+P"}` | Inject a chord through the **real keybinding layer**: config registry lookup → `execute_keybinding_action`. Mirrors `handle_key_event`'s modal guard, so chords are blocked while a modal overlay is open, exactly as real keys are. The chord fires the action literally — `open_settings` opens (it is not a toggle), so closing the window again needs the window's own close path (e.g. its Escape handling), not a second chord. |
| `{"type_text": "fullscr"}` | Deliver text to the focused egui widget (the same synthetic-input channel macOS menu accelerators use). |
| `{"press": "Enter"}` | Press a named key on the egui side. Names: `Enter`, `Escape`, `Tab`, `Backspace`, `Delete`, arrows, `Home`, `End`, `PageUp`, `PageDown`, `F1`–`F12`, and single letters `a`–`z` (for overlays with letter-driven keys, e.g. the agent-usage panel's `r`). |
| `{"assert": "X"}` / `{"assert_not": "X"}` | Boolean conditions, below. |
| `{"assert_eq": ["what", "expected"]}` | Keyed values, below. |

### Boolean operands

- `palette_open` / `search_open` — overlay visible
- `agent_usage_panel_open` — the agent-usage popup panel is visible
- `agent_usage_ready` — the usage store has ≥1 displayable record
- `plugins_loaded` — the plugin host's last discovery scan found ≥1 valid plugin
- `plugin_widget_set` — some plugin has published a non-empty widget text (runnable recipe in [PLUGINS.md](../features/PLUGINS.md))
- `plugin_panel_set` — some panel plugin has pushed a `SetPanel` (runnable recipe in [PLUGINS.md](../features/PLUGINS.md))
- `settings_window_open` — the standalone settings window is open
- `modal_guard` — `any_modal_ui_visible()`: the guard that blocks keys from the PTY. Covers in-window overlays only: the standalone settings window is a separate OS window with its own focus, so `settings_window_open` with `modal_guard=false` is the expected reading (terminal-window keys keep flowing while it floats), not a leak.
- `egui_keyboard` — egui owns keyboard focus (a text field is focused)
- `fullscreen` — window is fullscreen

### Keyed operands

- `["top_action", "toggle_fullscreen"]` — top-ranked palette action for the current query
- `["file_empty", "/path"]` — file is absent or zero bytes (a missing file counts as empty)

Unknown operands fail the step (and the run) loudly — a typo never passes
silently.

### Why chords ride a seam instead of a key event

winit's `KeyEvent` has a private platform field and no public constructor, so
nothing outside winit can build one (fabricating it is UB — a past Linux
SIGSEGV). Chord injection therefore enters through
`KeybindingRegistry::lookup_with_key_fields`, which takes the public key
fields a real event would carry. Character chords match logically (physical
preference is irrelevant unless `input.use_physical_keys` is enabled).

### Timing

`press`/`type_text` events are consumed on the **next** frame, and actions
like fullscreen apply asynchronously — always leave `wait_ms` (300–700ms)
between an action and the assertion that reads its effect.

## Isolated config (no user-config mutation)

par-term honors `XDG_CONFIG_HOME`, so a script's keybindings and shell never
touch the real config:

```bash
mkdir -p /tmp/pt-ui-test/cfg/par-term
cat > /tmp/pt-ui-test/cfg/par-term/config.yaml <<'EOF'
custom_shell: /bin/sh
shell_args:
  - -c
  - cat > /tmp/pt-ui-test/pty-capture.txt
shader_install_prompt: never
shell_integration_state: never
keybindings:
  - key: "Ctrl+Alt+Cmd+P"
    action: "toggle_command_palette"
EOF
XDG_CONFIG_HOME=/tmp/pt-ui-test/cfg par-term --ui-test script.json --ui-test-report report.json
```

Two first-run prompts defeat a clean run otherwise — `integrations_ui` opens
at startup and holds the modal guard — so both suppression keys go in the
config. They are **top-level** keys (the integrations sub-config is
`#[serde(flatten)]`-ed) and their enum values are **lowercase** (`never`,
not `Never`).

## PTY-leak capture

With the shell replaced by `cat > file` (above), anything that leaks past the
overlay guards lands in the file. `["file_empty", ...]` at the end of a
script is the sink assertion: no keystroke, chord, or escape byte reached the
shell during the run. This is how "typing in the palette must not reach the
prompt" is verified mechanically.

## Worked example

`/tmp/pt-ui-test/script.json` from the harness's inaugural run (2026-09-20)
covers the full palette loop — open via chord, guard/focus asserts, type +
rank assert, Enter dispatch to fullscreen, fullscreen off via chord, reopen,
type, Escape close, settings window open, PTY empty — and caught a live
keystroke-leak bug in its pre-fix run (`modal_guard=false` with the palette
open; fixed the same day). Pre/post-fix reports for that run:
`report-prefix4.json` vs `report-postfix.json` (10/3 fail → 13/0 pass).
