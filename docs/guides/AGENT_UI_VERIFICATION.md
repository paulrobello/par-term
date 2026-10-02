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
| `{"chord": "Ctrl+Alt+Cmd+P"}` (or `"Unidentified+Ctrl+Alt+S"`, see below) | Inject a chord through the **real key path**: first the overlay-stack routing that `handle_window_event` runs for every key (UX.md OV2 — the same `route_overlay_key` call, not a mirror), then, if no overlay owns the key, the registry lookup → `execute_keybinding_action`. So while an overlay is open the chord does what a real key does: an overlay's own toggle chord closes it, another popup's chord replaces the top popup, Escape closes only the top overlay, and a dialog consumes everything else. The chord fires the action literally — `open_settings` opens (it is not a toggle), so closing the window again needs the window's own close path, not a second chord. |
| `{"type_text": "fullscr"}` | Deliver text to the focused egui widget (the same synthetic-input channel macOS menu accelerators use), then render one frame synchronously so the next step reads post-input state. |
| `{"press": "Enter"}` | Press a named key on the egui side, then render one frame synchronously (the redraw round-trip is neither immediate nor guaranteed — an occluded window or a gate-rejected redraw left presses undelivered run-to-run before this). Names: `Enter`, `Escape`, `Tab`, `Backspace`, `Delete`, arrows, `Home`, `End`, `PageUp`, `PageDown`, `F1`–`F12`, and single letters `a`–`z` (for overlays with letter-driven keys, e.g. the agent-usage panel's `r`). `Shift+`, `Cmd+`, and `Ctrl+` prefixes, in any order (e.g. `Shift+Enter`, `Cmd+Shift+d`), carry the modifiers on the egui event — panels that distinguish modified keys read them via `consume_key` (`Cmd+` is egui's platform command modifier). |
| PTY-delivery asserts | A `file_bytes` proof that a keypress reached the shell needs a **primer and a flush chord** around it (see `tests/ui/b64_enter_safe_choice.json`, `b70_panel_nav.json`): a `{"chord": "Enter"}` before the interaction proves the sink is live, and one after flushes the read — without the trailing chord an async paste can sit in the PTY buffer unread when the script ends, and the sink asserts empty even though the app wrote the bytes. The sink path is reset at script load and may reference `$PAR_TERM_UI_TEST_SINK` (see [PTY-leak capture](#pty-leak-capture)). |
| `{"assert": "X"}` / `{"assert_not": "X"}` | Boolean conditions, below. |
| `{"assert_eq": ["what", "expected"]}` | Keyed values, below. |
| `{"capture": "what"}` | Stash a capture-capable operand's current value. |
| `{"assert_eq_captured": "what"}` | Assert the operand's current value equals the stashed one — for values a script cannot know up front, like a spawned shell's PID. |
| `{"open_modal": "D"}` | Seed dialog `D` open through its real entry point (`close_running_job`, `mux_last_tab`, `trigger_confirm`, `agent_command_confirm`, `update_dialog`, `tab_context_menu`, `profile_launcher`, `demote_chooser`, `profile_drawer`, `quit_confirmation`, `tmux_picker`, `command_history`, `clipboard_history`) — the seam standing in for the user interaction that opens it, so a script can prove typed keys stay off the PTY while it is open (worked example: `tests/ui/b61_modal_guard.json`). |
| `{"close_modal": "D"}` | Clear the state `open_modal` seeded. Buttons and Escape are the dialog's own egui handling (`press` steps); this only arms/disarms the modal the key guard sums over. |
| `{"seed_clipboard": ["s1", "s2"]}` | Seed clipboard-history entries into the focused pane's terminal, newest last (B70: in production only selection copies feed that history, which a script cannot drive; the pinned core's OSC 52 parser sets `clipboard_content` but records no history). |

### Boolean operands

- `palette_open` / `search_open` — overlay visible
- `tmux_picker_open` — tmux session picker visible
- `command_history_open` / `clipboard_history_open` / `paste_special_open` — the respective panel is visible
- `palette_selected_visible` — the palette's selected row falls inside the drawn 12-row window (the B62 scroll invariant)
- `overlay_open:<name>` — any overlay-stack member is open, by its stack name (`tree_picker_ui`, `help_ui`, `quit_confirmation_ui`, …; the names `OverlayId::name` lists). Worked example: `tests/ui/mp1_overlay_stack.json`
- `agent_usage_panel_open` — the agent-usage popup panel is visible
- `agent_usage_ready` — the usage store has ≥1 displayable record
- `plugins_loaded` — the plugin host's last discovery scan found ≥1 valid plugin
- `plugin_widget_set` — some plugin has published a non-empty widget text (runnable recipe in [PLUGINS.md](../features/PLUGINS.md))
- `plugin_panel_set` — some panel plugin has pushed a `SetPanel` (runnable recipe in [PLUGINS.md](../features/PLUGINS.md))
- `plugin_overlay_set` — some overlay plugin has pushed a `SetOverlay` (runnable recipe in [PLUGINS.md](../features/PLUGINS.md))
- `settings_window_open` — the standalone settings window is open
- `modal_guard` — `any_modal_ui_visible()`: the guard that blocks keys from the PTY. Covers in-window overlays only: the standalone settings window is a separate OS window with its own focus, so `settings_window_open` with `modal_guard=false` is the expected reading (terminal-window keys keep flowing while it floats), not a leak.
- `pane_hint_mode_active` — the built-in pane-hint selection mode is armed (`select_pane_hint` chord on a multi-pane tab)
- `leader_armed` / `which_key_open` — the leader is armed / its which-key overlay is showing ([Leader Key](../features/LEADER_KEY.md))
- `pane_zoomed` — the active tab's focused pane is zoomed
- `egui_keyboard` — egui owns keyboard focus (a text field is focused)
- `fullscreen` — window is fullscreen

### Keyed operands

- `["top_action", "toggle_fullscreen"]` — top-ranked palette action for the current query
- `["palette_selected", "N"]` — the palette's selected row index into the filtered list (the B62 scroll proof pairs it with `palette_selected_visible`)
- `["command_history_selected", "echo foo"]` / `["clipboard_history_selected", "text"]` — the selected row's text (B70 navigation proofs; `<none>` when no selection)
- `["font_size", "13.5"]` — live config font size (the B68 reset proof)
- `["file_empty", "/path"]` — file is absent or zero bytes (a missing file counts as empty)
- `["window_count", "N"]` — the app's open-window count (manager-level; works with zero terminal windows)
- `["tab_count", "N"]` / `["pane_count", "N"]` — the first terminal window's visible tab count / its active tab's pane count
- `["tab_profile", "Name"]` — the active tab's profile name (`Default` for a plain tab)
- `["tab_count", "N"]` / `["active_tab", "N"]` — visible tabs and the 1-based active tab (the leader proof)

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

### Windows Ctrl+Alt chords: the `Unidentified+` prefix

A plain `chord` step always injects `Key::Character` with no physical key.
That is not what Windows delivers for Ctrl+Alt+letter: Ctrl+Alt is AltGr
there, a US layout has no AltGr character, and winit reports the key as
`Key::Unidentified` with only the physical key set. The matcher has a
separate path for that case (fixed in `b76ab8be`), and a plain
`Ctrl+Alt+S` chord step never reaches it. **A plain Ctrl+Alt chord passing
under `--ui-test` therefore proves nothing about Windows.**

Prefix the chord with `Unidentified+` to inject it the Windows way:

```json
{"chord": "Unidentified+Ctrl+Alt+S", "wait_ms": 400}
```

The step injects `Key::Unidentified` plus the physical key for the letter
(US layout, letters and digits only), so the registry lookup exercises the
physical-key fallback. Use it for every Ctrl+Alt chord in a script that must
hold on Windows. Other key kinds (named keys, punctuation) are rejected with
a failed step. The step is injection through the seam, not a real keyboard
path: the final confirmation on Windows is still a real keystroke on the
Windows VM (for example `prlctl send-key-event`) with `DEBUG_LEVEL=3`.

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
HOME=/tmp/pt-ui-test/home XDG_CONFIG_HOME=/tmp/pt-ui-test/cfg PAR_TERM_NO_MIGRATE=1 par-term --ui-test script.json --ui-test-report report.json
```

Every recipe below sets `PAR_TERM_NO_MIGRATE=1` — this is **required**, not
advisory: with only `XDG_CONFIG_HOME` redirected, the startup config
migration treats the real `~/.config/par-term` as a legacy location and
**moves its contents into the throwaway dir** (observed 2026-09-28 — 11
entries relocated before the run was stopped and manually recovered; again
2026-09-30 — 12 entries moved over a scratch config and the scratch was then
deleted). Isolating `HOME` removes the legacy-source candidates, and the
migration now also skips a target that already has its own `config.yaml`,
but a scratch dir without one is still fair game, so the env var is the
guarantee (documented in the
[Environment Variables Reference](ENVIRONMENT_VARIABLES.md)).

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

### Per-run sink guarantees

Two harness behaviors keep a sink assert from passing on stale state:

1. **Reset at load.** Before the first window opens, `--ui-test` deletes
   every file a `file_bytes`/`file_empty` operand names. The app's own shell
   recreates the sink on first delivery, so a leftover file from an earlier
   run can never answer a delivery proof — observed 2026-09-29, b64 step 14
   passed on a file a prior day's run left while that run's config captured
   elsewhere. A sink the harness cannot delete (permissions, wrong type)
   fails the load loudly.
2. **Per-run stamping.** A file operand may name its sink as
   `$PAR_TERM_UI_TEST_SINK` or `${PAR_TERM_UI_TEST_SINK}`; the variable
   resolves once at script load and the report shows the resolved path.
   Export the same value before launching: the PTY child inherits it, so the
   config's `cat > "$PAR_TERM_UI_TEST_SINK"` lands on the identical path. A
   referenced but unset variable fails the load loudly.
   `tests/ui/b64_enter_safe_choice.json` uses the stamped form; the other
   checked-in sink scripts keep literal paths and are covered by the reset.

## Worked example

`/tmp/pt-ui-test/script.json` from the harness's inaugural run (2026-09-20)
covers the full palette loop — open via chord, guard/focus asserts, type +
rank assert, Enter dispatch to fullscreen, fullscreen off via chord, reopen,
type, Escape close, settings window open, PTY empty — and caught a live
keystroke-leak bug in its pre-fix run (`modal_guard=false` with the palette
open; fixed the same day). Pre/post-fix reports for that run:
`report-prefix4.json` vs `report-postfix.json` (10/3 fail → 13/0 pass).

## Checked-in script: reopen closed tab keeps the shell running

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
HOME=/tmp/pt-ui-test/home XDG_CONFIG_HOME=/tmp/pt-ui-test/cfg PAR_TERM_NO_MIGRATE=1 \
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
HOME=/tmp/pt-ui-test/home XDG_CONFIG_HOME=/tmp/pt-ui-test/cfg PAR_TERM_NO_MIGRATE=1 \
  target/dev-release/par-term \
  --ui-test tests/ui/tw2_quit_saves_every_window.json \
  --ui-test-report /tmp/pt-ui-test/tw2-quit-report.json
HOME=/tmp/pt-ui-test/home XDG_CONFIG_HOME=/tmp/pt-ui-test/cfg PAR_TERM_NO_MIGRATE=1 \
  target/dev-release/par-term \
  --ui-test tests/ui/tw2_restore_brings_back_three.json \
  --ui-test-report /tmp/pt-ui-test/tw2-restore-report.json
```

`restore_session` sits at the TOP level (the `SessionRestoreConfig` sub-config is
`#[serde(flatten)]`ed) — nesting it under a `session_restore:` key silently
defaults it off and run B restores nothing. Before the TW2 fix, run B reports
`window_count` 1 ≠ 3 — the negative control.

## Checked-in script: Open Profiles opens a tab, a window, and splits (UX MP3)

`tests/ui/mp3_open_profiles.json` proves the profile launcher end to end
from a config with a legacy per-profile `keyboard_shortcut`: the startup
migration turns it into an `open_profile:<id>` binding and the chord opens
the profile in a new tab (`tab_count` 2, `tab_profile` Work); Open Profiles
(Enter) opens another tab; `Cmd+d` and `Cmd+Shift+d` split the active tab
(`pane_count` 2 then 3); `Shift+Enter` opens a second window
(`window_count` 2); Escape closes the launcher. The chords ride the
injector, which bypasses the native menu — on macOS the menu's Split
accelerators reach the launcher through `execute_keybinding_action` instead
(unit test `a_native_menu_split_while_the_launcher_is_open_splits_with_the_profile`).

```bash
mkdir -p /tmp/pt-mp3/cfg/par-term /tmp/pt-mp3/home
cat > /tmp/pt-mp3/cfg/par-term/config.yaml <<'EOF'
custom_shell: /bin/sh
shader_install_prompt: never
shell_integration_state: never
agent_skill_state: never
keybindings:
  - key: "Ctrl+Alt+Cmd+O"
    action: "toggle_profile_drawer"
EOF
cat > /tmp/pt-mp3/cfg/par-term/profiles.yaml <<'EOF'
- id: 11111111-1111-4111-8111-111111111111
  name: Work
  order: 0
  keyboard_shortcut: "Ctrl+Alt+Cmd+W"
  tags: [dev]
EOF
HOME=/tmp/pt-mp3/home XDG_CONFIG_HOME=/tmp/pt-mp3/cfg PAR_TERM_NO_MIGRATE=1 \
  par-term --ui-test tests/ui/mp3_open_profiles.json --ui-test-report /tmp/pt-mp3/report.json
```

## Checked-in script: pass_to_terminal delivers to the shell (K27)

`tests/ui/k27_pass_to_terminal_delivers.json` proves a user-set
`pass_to_terminal` row end to end: the bound chords are encoded to the PTY,
the shell receives exactly the expected bytes, and the binding still claims
its chords in a second, fresh process (the restart-survival half). The chord
injector's PTY tail mirrors the real key handler: a `pass_to_terminal`
match, or a chord no binding matches, is encoded through
`InputHandler::handle_key_input_with_mode` (`KeyInput` — the sanctioned
path; a winit `KeyEvent` cannot be fabricated) and written to the focused
terminal. The script binds BOTH `CmdOrCtrl+1` (claimed by the macOS
`switch_to_tab_1` default) and `Alt+1` (claimed by the Windows/Linux
default), so the byte-exact `file_bytes` assert has teeth on every
platform: if the rows were lost, the platform's tab-switch default consumes
its chord and the sink is missing that byte run. The trailing `Enter`
flushes the canonical-mode line (ICRNL turns the CR into LF, hence the
final `0a`); the `esc` option mode is pinned in the config so `Alt+1`
encodes as `ESC 1` (`1b31`). The assert uses the ends-with form of
`file_bytes` (`path*hex`) because the app window can steal desktop focus
mid-run on a live machine and catch real keystrokes into the PTY ahead of
the scripted ones — observed as stray `cc-` and TAB runs contaminating the
sink — and the long final settle (4 s) absorbs the first-run tail of the
async PTY write under startup load.

```bash
mkdir -p /tmp/pt-k27/cfg/par-term /tmp/pt-k27/home
cat > /tmp/pt-k27/cfg/par-term/config.yaml <<'EOF'
custom_shell: /bin/sh
shell_args:
  - "-c"
  - "cat > /tmp/pt-k27-sink.bin"
login_shell: false
shader_install_prompt: never
shell_integration_state: never
agent_skill_state: never
left_option_key_mode: esc
right_option_key_mode: esc
keybindings:
  - key: "Alt+1"
    action: "pass_to_terminal"
  - key: "CmdOrCtrl+1"
    action: "pass_to_terminal"
EOF
cat > /tmp/pt-k27/run.sh <<'EOF'
#!/bin/bash
rm -f /tmp/pt-k27-sink.bin
export HOME=/tmp/pt-k27/home
export XDG_CONFIG_HOME=/tmp/pt-k27/cfg
export PAR_TERM_NO_MIGRATE=1
exec "$1" --ui-test tests/ui/k27_pass_to_terminal_delivers.json --ui-test-report "$2"
EOF
chmod +x /tmp/pt-k27/run.sh
# Run A (deliver) and run B (survives restart: same config dir, fresh process).
/tmp/pt-k27/run.sh target/dev-release/par-term /tmp/pt-k27/a.json
/tmp/pt-k27/run.sh target/dev-release/par-term /tmp/pt-k27/b.json
```

Both reports' `all_passed` must be true, with step records
`-> pass_to_terminal -> PTY (…)` naming the branch each chord took. Negative
control: delete the `keybindings:` rows from the config and rerun — the
report now shows `CmdOrCtrl+1 -> keybinding 'switch_to_tab_1'` (macOS;
`Alt+1` symmetrically on Windows/Linux) and the `file_bytes` assert fails on
the missing byte run, proving the row (not some fallthrough) is what frees
the chord for the shell.

## Checked-in script: panel keyboard navigation (B70)

`tests/ui/b70_panel_nav.json` drives clipboard history, command history,
and paste special from the keyboard (arrows move the selection, Enter
pastes byte-exact to the capture sink, Shift+Enter opens paste special,
Escape closes). Clipboard entries come from the script's `seed_clipboard`
step, but **command history is read from disk**: the run needs a seeded
`command_history.yaml` in the isolated config directory
(`$XDG_CONFIG_HOME/par-term/`) holding exactly three entries, newest first
— `echo b70-gamma`, then `echo b70-beta`, then `echo b70-alpha`. The script
asserts the first row is gamma and that two `ArrowDown` presses land on
alpha.

```bash
mkdir -p /tmp/pt-b70/cfg/par-term /tmp/pt-b70/home
cat > /tmp/pt-b70/cfg/par-term/config.yaml <<'EOF'
custom_shell: /bin/sh
shell_args:
  - "-c"
  - "cat > /tmp/pt-b70-sink.bin"
login_shell: false
shader_install_prompt: never
shell_integration_state: never
agent_skill_state: never
EOF
cat > /tmp/pt-b70/cfg/par-term/command_history.yaml <<'EOF'
commands:
  - command: echo b70-gamma
    timestamp_ms: 3000
    exit_code: 0
    duration_ms: null
  - command: echo b70-beta
    timestamp_ms: 2000
    exit_code: 0
    duration_ms: null
  - command: echo b70-alpha
    timestamp_ms: 1000
    exit_code: 0
    duration_ms: null
EOF
rm -f /tmp/pt-b70-sink.bin
HOME=/tmp/pt-b70/home XDG_CONFIG_HOME=/tmp/pt-b70/cfg PAR_TERM_NO_MIGRATE=1 \
  target/dev-release/par-term \
  --ui-test tests/ui/b70_panel_nav.json \
  --ui-test-report /tmp/pt-b70/report.json
```

The report's `all_passed` must be true (17 asserts). Without the seed file
the command-history half fails (measured 2026-09-30: 12 pass, 5 fail —
`command_history_selected = "<none>"`, the panel never closes on Enter, the
sink misses `echo b70-alpha`, and the trailing paste-special assert trips
on the panel left open), so a failing run there is the missing seed, not a
navigation regression. The seed is written before launch because the
history loads once at startup.

## Checked-in script: leader key (K4)

`tests/ui/k4_leader_key.json` drives the leader through the real key path: a `chord` step reaches `handle_leader_press`, the same entry point `handle_key_event` calls, after the overlay-stack routing. The script proves the which-key overlay appears only after `leader_overlay_delay_ms` and that `Escape` cancels. It then runs leader `c` (tab count 1 → 2), `p` and `n` (active tab 2 → 1 → 2), `%` then `z` twice (zoom on and off), and `x` (pane count 2 → 1). Leader twice sends the chord to the pane, and the byte-exact sink assert expects `0a020a`: the primer, then `^B` (Ctrl+Shift+B, 0x02), then the flushing Enter. A final arm with no key proves the timeout cancels. The config pins `leader_key: "Ctrl+Shift+B"` so one script runs on every platform. The sink appends (`cat >>`) because leader `c` starts a second shell that would otherwise truncate it.

```bash
mkdir -p /tmp/pt-k4/cfg/par-term /tmp/pt-k4/home
rm -f /tmp/pt-k4-sink.bin
cat > /tmp/pt-k4/cfg/par-term/config.yaml <<'EOF'
custom_shell: /bin/sh
shell_args:
  - "-c"
  - "stty -echo; cat >> /tmp/pt-k4-sink.bin"
login_shell: false
shader_install_prompt: never
shell_integration_state: never
agent_skill_state: never
leader_key: "Ctrl+Shift+B"
leader_overlay_delay_ms: 400
leader_timeout_ms: 2000
EOF
HOME=/tmp/pt-k4/home XDG_CONFIG_HOME=/tmp/pt-k4/cfg PAR_TERM_NO_MIGRATE=1 \
  target/dev-release/par-term --ui-test tests/ui/k4_leader_key.json \
  --ui-test-report /tmp/pt-k4/report.json
```

The report must show `all_passed: true` (25 asserts), with each leader step recorded as `-> leader Run { action: "new_tab", .. }` and so on. Negative control (2026-09-30): the same run with `leader_key: ""` fails 10 asserts, because every follow-up key types into the shell instead.

## Checked-in script: Enter is the safe choice in destructive dialogs (B64/MD5)

`tests/ui/b64_enter_safe_choice.json` proves the MD5 dialog-Enter rule end to
end: `press Enter` (the egui-event seam — `chord` mirrors the keybinding path
and never reaches dialog input, so it cannot test this) in the quit, close-job,
and par-mux last-tab dialogs cancels each one — `assert_not modal_guard` after
every press. The final `chord Enter` flushes `0a` to the capture sink, proving
no other key reached the shell. The quit half is self-checking: if Enter ever
maps back to Quit, the app exits mid-script and the report is never written.
Negative control (2026-09-29): pre-fix binary, same script — report missing,
the quit dialog's Enter killed the app mid-run.

```bash
mkdir -p /tmp/pt-b64/cfg/par-term /tmp/pt-b64/home
cat > /tmp/pt-b64/cfg/par-term/config.yaml <<'EOF'
custom_shell: /bin/sh
shell_args:
  - "-c"
  - "cat > \"$PAR_TERM_UI_TEST_SINK\""
login_shell: false
shader_install_prompt: never
shell_integration_state: never
agent_skill_state: never
EOF
HOME=/tmp/pt-b64/home XDG_CONFIG_HOME=/tmp/pt-b64/cfg PAR_TERM_NO_MIGRATE=1 \
  PAR_TERM_UI_TEST_SINK=/tmp/pt-b64/sink.bin \
  target/dev-release/par-term \
  --ui-test tests/ui/b64_enter_safe_choice.json \
  --ui-test-report /tmp/pt-b64/report.json
```

The report's `all_passed` must be true. The sink path is stamped per run via
`PAR_TERM_UI_TEST_SINK` — the script's `file_bytes` operand and the config's
`cat` resolve to the same file through it, and the harness deletes the sink
at load, so a leftover from an earlier run can never answer the assert.
