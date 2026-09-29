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
| `{"capture": "what"}` | Stash a capture-capable operand's current value. |
| `{"assert_eq_captured": "what"}` | Assert the operand's current value equals the stashed one — for values a script cannot know up front, like a spawned shell's PID. |

### Boolean operands

- `palette_open` / `search_open` — overlay visible
- `agent_usage_panel_open` — the agent-usage popup panel is visible
- `agent_usage_ready` — the usage store has ≥1 displayable record
- `plugins_loaded` — the plugin host's last discovery scan found ≥1 valid plugin
- `plugin_widget_set` — some plugin has published a non-empty widget text (runnable recipe in [PLUGINS.md](../features/PLUGINS.md))
- `plugin_panel_set` — some panel plugin has pushed a `SetPanel` (runnable recipe in [PLUGINS.md](../features/PLUGINS.md))
- `plugin_overlay_set` — some overlay plugin has pushed a `SetOverlay` (runnable recipe in [PLUGINS.md](../features/PLUGINS.md))
- `settings_window_open` — the standalone settings window is open
- `modal_guard` — `any_modal_ui_visible()`: the guard that blocks keys from the PTY. Covers in-window overlays only: the standalone settings window is a separate OS window with its own focus, so `settings_window_open` with `modal_guard=false` is the expected reading (terminal-window keys keep flowing while it floats), not a leak.
- `pane_hint_mode_active` — the built-in pane-hint selection mode is armed (`select_pane_hint` chord on a multi-pane tab)
- `egui_keyboard` — egui owns keyboard focus (a text field is focused)
- `fullscreen` — window is fullscreen

### Keyed operands

- `["top_action", "toggle_fullscreen"]` — top-ranked palette action for the current query
- `["file_empty", "/path"]` — file is absent or zero bytes (a missing file counts as empty)
- `["window_count", "N"]` — the app's open-window count (manager-level; works with zero terminal windows)

Capture-capable operands (usable with `capture`/`assert_eq_captured`):

- `tab_shell_pid` — the focused terminal's PTY child PID

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
mkdir -p /tmp/pt-ui-test/cfg/par-term /tmp/pt-ui-test/home
cat > /tmp/pt-ui-test/cfg/par-term/config.yaml <<'EOF'
custom_shell: /bin/sh
shell_args:
  - -c
  - cat > /tmp/pt-ui-test/pty-capture.txt
shader_install_prompt: never
shell_integration_state: never
agent_skill_state: never
keybindings:
  - key: "Ctrl+Alt+Cmd+P"
    action: "toggle_command_palette"
EOF
HOME=/tmp/pt-ui-test/home XDG_CONFIG_HOME=/tmp/pt-ui-test/cfg par-term --ui-test script.json --ui-test-report report.json
```

`HOME` must be isolated too: with only `XDG_CONFIG_HOME` redirected, the
startup config migration treats the real `~/.config/par-term` as a legacy
location and **moves its contents into the throwaway dir** (observed
2026-09-28 — 11 entries relocated before the run was stopped and manually
recovered). A scratch `HOME` removes both legacy-source candidates;
`PAR_TERM_NO_MIGRATE=1` skips the migration outright and is the
belt-and-braces fallback when `HOME` cannot be redirected (documented in
the [Environment Variables Reference](ENVIRONMENT_VARIABLES.md)).

Three first-run prompts defeat a clean run otherwise — `integrations_ui`
opens at startup and holds the modal guard — so all three suppression keys go
in the config. They are **top-level** keys (the integrations sub-config is
`#[serde(flatten)]`-ed) and their enum values are **lowercase** (`never`,
not `Never`). `shader_install_prompt` and `shell_integration_state` alone are
not enough: `agent_skill_state` defaults to `ask` and opens the same dialog
(observed 2026-09-28).

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

## Checked-in script: session-undo preserves the shell

`tests/ui/d6_reopen_preserves_shell.json` proves the D6 close-safety default
`session_undo_preserve_shell: true` end to end: open a second tab, capture its
`tab_shell_pid`, close it, reopen it, and assert the PID is unchanged — the
restored tab is the same live process, not a fresh shell. On macOS new tab /
close tab are menu accelerators, not registry keybindings, so the config binds
them to chord-injectable keys; reopen rides its default `CmdOrCtrl+Z`. Run it
with the shell replaced by a long-lived process:

```bash
mkdir -p /tmp/pt-ui-test/cfg/par-term /tmp/pt-ui-test/home
cat > /tmp/pt-ui-test/cfg/par-term/config.yaml <<'EOF'
custom_shell: /bin/sleep
shell_args:
  - "100"
login_shell: false
shader_install_prompt: never
shell_integration_state: never
agent_skill_state: never
keybindings:
  - key: "CmdOrCtrl+Alt+N"
    action: "new_tab"
  - key: "CmdOrCtrl+Alt+W"
    action: "close_tab"
EOF
HOME=/tmp/pt-ui-test/home XDG_CONFIG_HOME=/tmp/pt-ui-test/cfg \
  target/dev-release/par-term \
  --ui-test tests/ui/d6_reopen_preserves_shell.json \
  --ui-test-report /tmp/pt-ui-test/d6-report.json
```

`login_shell: false` keeps the spawn `/bin/sleep 100` (a login shell appends
`-l`, which sleep rejects). The report's `all_passed` must be true. Setting
`session_undo_preserve_shell: false` in the same config flips the assert to a
failure (the reopen spawns a fresh shell with a new PID) — the negative
control proving the assertion has teeth.

## Checked-in script: quit saves every window (TW2)

`tests/ui/tw2_quit_saves_every_window.json` + `tests/ui/tw2_restore_brings_
back_three.json` are a two-run pair: run A opens two more windows, asserts
`window_count == 3`, then quits through the real `MenuAction::Quit` path; run
B starts a fresh app against the same config dir with `restore_session: true`
and asserts all three windows came back. The quit chord is the script's last
step, so run A's report usually loses the race with the event-loop exit —
run B's report is the criterion evidence (`all_passed` must be true).

```bash
mkdir -p /tmp/pt-ui-test/cfg/par-term /tmp/pt-ui-test/home
cat > /tmp/pt-ui-test/cfg/par-term/config.yaml <<'EOF'
custom_shell: /bin/sleep
shell_args:
  - "100"
login_shell: false
shader_install_prompt: never
shell_integration_state: never
agent_skill_state: never
restore_session: true
keybindings:
  - key: "CmdOrCtrl+Alt+Shift+N"
    action: "new_window"
  - key: "CmdOrCtrl+Alt+Shift+Q"
    action: "quit"
EOF
HOME=/tmp/pt-ui-test/home XDG_CONFIG_HOME=/tmp/pt-ui-test/cfg \
  target/dev-release/par-term \
  --ui-test tests/ui/tw2_quit_saves_every_window.json \
  --ui-test-report /tmp/pt-ui-test/tw2-quit-report.json
HOME=/tmp/pt-ui-test/home XDG_CONFIG_HOME=/tmp/pt-ui-test/cfg \
  target/dev-release/par-term \
  --ui-test tests/ui/tw2_restore_brings_back_three.json \
  --ui-test-report /tmp/pt-ui-test/tw2-restore-report.json
```

`restore_session` sits at the TOP level (the session-restore sub-config is
`#[serde(flatten)]`ed) — nesting it under a `session_restore:` key silently
defaults it off and run B restores nothing. Before the TW2 fix, run B reports
`window_count` 1 ≠ 3 — the negative control.
