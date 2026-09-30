# Keyboard Shortcuts

Complete reference for all par-term keyboard shortcuts.

> **📝 Note:** On macOS, the defaults follow iTerm2 wherever par-term has the matching action, with `Cmd` as the primary modifier. On Linux and Windows, the same letters use the K1 family: `Ctrl+Shift` where macOS uses `Cmd` (window and tab), and `Ctrl+Alt` where macOS uses `Cmd+Opt` (panes). Plain `Ctrl+letter` stays with the shell (Ctrl+C for SIGINT, Ctrl+D for EOF, etc.). The **F1** help panel always shows your *current* bindings: its Window, Tab, Pane, Session & Profiles, and other sections are generated from the live keybinding registry and the menu model, so a rebind shows there and an unbound action is left out.

> **📝 Linux:** par-term cannot attach a *native* menu bar on Linux — `muda`
> requires a `gtk::Window` that winit does not create — so Linux gets an in-app
> menu instead, opened from the `☰` button in the tab bar or by binding
> `toggle_menu`. It offers the same commands as the macOS and Windows menus,
> because all three are built from one shared model. Every chord in the tables
> below is a registry default, so it works on Linux without the native menu.
> Binding `toggle_menu` matters if you run with `tab_bar_mode: never`, since
> there is then no tab bar to hold the button.
>
> Set `PAR_TERM_IN_APP_MENU=1` to force the in-app menu on any platform, or `0`
> to disable it.

> **📝 Upgrading:** these defaults changed in the iTerm2 alignment (see
> [Migration](MIGRATION.md#unreleased--default-shortcuts-aligned-with-iterm2)).
> A chord you had already bound keeps its old action; a moved default reaches
> an existing config only once its chord is free (delete the saved row).

## Table of Contents
- [Window & Tab Management](#window--tab-management)
- [Navigation & Scrolling](#navigation--scrolling)
- [Copy, Paste & Selection](#copy-paste--selection)
- [Copy Mode](#copy-mode)
- [Search & History](#search--history)
- [Terminal Operations](#terminal-operations)
- [Font & Text Sizing](#font--text-sizing)
- [UI Toggles & Display](#ui-toggles--display)
- [Pane Management](#pane-management)
- [Advanced Features](#advanced-features)
- [Customizing Keybindings](#customizing-keybindings)
- [Platform Shortcut Conflicts](#platform-shortcut-conflicts)
- [Related Documentation](#related-documentation)

## Window & Tab Management

| Action | macOS | Linux/Windows |
|--------|-------|---------------|
| New window | `Cmd + N` (alias `Cmd + Ctrl + Shift + N`) | `Ctrl + Shift + N` |
| New tab | `Cmd + T` (alias `Cmd + Ctrl + Shift + T`) | `Ctrl + Shift + T` |
| Duplicate tab | `Cmd + Shift + J` | `Ctrl + Shift + J` |
| Close (pane, then tab, then window) | `Cmd + W` | `Ctrl + Shift + W` (alias `Ctrl + Shift + X`) |
| Close tab | `Cmd + Opt + W` | `Ctrl + Alt + W` |
| Minimize window | `Cmd + M` *(menu)* | — |
| Next tab | `Cmd + Shift + ]` | `Ctrl + Shift + ]` |
| Previous tab | `Cmd + Shift + [` | `Ctrl + Shift + [` |
| Next tab (alt) | `Ctrl + Tab` | `Ctrl + Tab` |
| Previous tab (alt) | `Ctrl + Shift + Tab` | `Ctrl + Shift + Tab` |
| Switch to tab 1-9 | `Cmd + 1-9` | `Alt + 1-9` |
| Move tab left | `Cmd + Opt + Shift + [` (alias `Cmd + Shift + Left`) | `Ctrl + Shift + Left` |
| Move tab right | `Cmd + Opt + Shift + ]` (alias `Cmd + Shift + Right`) | `Ctrl + Shift + Right` |
| Reopen closed tab | `Cmd + Shift + T` (alias `Cmd + Z`) | `Ctrl + Shift + Z` |
| Quit | `Cmd + Q` | `Ctrl + Shift + Q` |
| Move tab to new window | *(unbound)* | *(unbound)* |
| Save window arrangement | View menu: "Save Window Arrangement..." | View menu: "Save Window Arrangement..." |

> **📝 Note:** **Close** follows iTerm2: it closes the focused pane; closing a tab's last pane closes the tab, and closing the last tab closes the window. The running-job confirmation and the par-mux last-tab dialog apply before the cascade. **Close tab** closes every pane in the tab at once. The *(menu)* Minimize chord is dispatched by the **native** menu bar, which Linux does not have.

## Navigation & Scrolling

| Shortcut | Action |
|----------|--------|
| `PageUp` | Forward `\x1b[5~` to terminal application |
| `PageDown` | Forward `\x1b[6~` to terminal application |
| `Shift + PageUp` | Scroll up one page in scrollback |
| `Shift + PageDown` | Scroll down one page in scrollback |
| `Shift + Home` | Jump to top of scrollback |
| `Shift + End` | Jump to bottom |
| `Cmd + Shift + Up` (macOS, alias `Cmd + Up`) / `Super + Up` | Jump to previous command mark |
| `Cmd + Shift + Down` (macOS, alias `Cmd + Down`) / `Super + Down` | Jump to next command mark |
| `Mouse Wheel` | Scroll up/down |

### Modifier Keys With Special Keys

Modifier keys (`Shift`, `Ctrl`, `Alt`, and combinations) work with special keys such as arrows, `Home`, `End`, `Insert`, `Delete`, `PageUp`, `PageDown`, and `F1`--`F12`. par-term emits xterm-standard modifier-parameterized escape sequences (e.g., `CSI 1;2A` for `Shift+Up`) so that terminal applications such as vim, tmux, and readline correctly interpret modified special keys.

> **📝 Note:** Inside a tmux session, tmux handles modifier encoding independently. The sequences described here apply to sessions running outside tmux.

## Copy, Paste & Selection

| Action | macOS | Linux/Windows |
|--------|-------|---------------|
| Copy selection | `Cmd + C` | `Ctrl + Shift + C` |
| Paste | `Cmd + V` | `Ctrl + Shift + V` |
| Paste (X11 fallback) | — | `Shift + Insert` |
| Paste Special | `Cmd + Shift + V` | `Ctrl + Alt + V` |
| Clipboard history | `Cmd + Shift + H` | `Ctrl + Shift + H` |
| Select all | `Cmd + A` | `Ctrl + Shift + A` |

**Mouse Selection:**

| Action | Effect |
|--------|--------|
| Click + Drag | Normal selection |
| Double-Click | Select word |
| Triple-Click | Select line |
| Cmd/Ctrl + Click | Open URL or file path |
| Alt/Option + Click | Move cursor to position |
| Alt + Cmd/Ctrl + Drag | Rectangular selection |
| Middle-Click | Paste primary selection |

## Copy Mode

Vi-style keyboard-driven text selection. See [Copy Mode](../features/COPY_MODE.md) for complete reference.

| Action | macOS | Linux/Windows |
|--------|-------|---------------|
| Toggle copy mode | `Cmd + Shift + C` | `Ctrl + Shift + Space` |

**In copy mode:**

| Key | Action |
|-----|--------|
| `h/j/k/l` | Navigate left/down/up/right |
| `w/b/e` | Word forward/backward/end |
| `0/$` | Line start/end |
| `gg/G` | Top/bottom of buffer |
| `Ctrl+U/D` | Half page up/down |
| `v/V/Ctrl+V` | Character/Line/Block selection |
| `y` | Yank (copy) selection |
| `/` / `?` | Search forward/backward |
| `n/N` | Next/previous match |
| `m{a-z}` | Set mark |
| `'{a-z}` | Jump to mark |
| `q` / `Escape` | Exit copy mode |

## Search & History

| Action | macOS | Linux/Windows |
|--------|-------|---------------|
| Open search | `Cmd + F` | `Ctrl + Shift + F` |
| Find next match | `Enter` | `Enter` |
| Find previous match | `Shift + Enter` | `Shift + Enter` |
| Close search | `Escape` | `Escape` |
| Open command history | `Cmd + Shift + ;` | `Ctrl + Alt + R` |

> **📝 Note:** Command history uses the `toggle_command_history` action (fuzzy search), on iTerm2's `Cmd + Shift + ;`. `Cmd + R` is no longer bound, so it reaches the shell. In `config.yaml` the macOS chord is written `CmdOrCtrl+Shift+:`, the character that key press produces.

## Terminal Operations

| Shortcut | Action |
|----------|--------|
| `Ctrl + L` | Clear visible screen |
| `Cmd/Ctrl + Shift + K` | Clear scrollback buffer |
| `Cmd/Ctrl + Shift + R` | Toggle output recording |
| Command palette (macOS) / `Ctrl + Shift + M` (Linux/Win) | Toggle maximize throughput mode |

> **📝 Note:** Output recording uses the `toggle_session_logging` action (see [Output Recording](../features/SESSION_LOGGING.md)). Throughput mode uses `toggle_throughput_mode`; on macOS it has no default chord since `Cmd + Shift + T` became Reopen Closed Tab, so run it from the command palette or bind it. Screenshots are taken via the `--screenshot` CLI option or MCP server tool, not a keyboard shortcut.

## Font & Text Sizing

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + +` or `Cmd/Ctrl + =` | Increase font size |
| `Cmd/Ctrl + -` | Decrease font size |
| `Cmd/Ctrl + 0` | Reset font size to default |

## UI Toggles & Display

| Shortcut | Action |
|----------|--------|
| `F1` | Toggle Help panel (shortcut sections generated from your current bindings) |
| `F3` | Toggle FPS overlay |
| `F5` | Reload configuration |
| `F11` (macOS also `Cmd + Ctrl + F`) | Toggle fullscreen |
| `Shift + F11` | Maximize vertically. Unlike `F11`, this is an exact-chord registry binding: modifier variants of `F11` itself no longer toggle fullscreen — rebind or free the chord in Settings. |
| `F12` | Open Settings |
| `Cmd + ,` (macOS) | Open Settings |
| `Escape` | Close current UI panel |

## Pane Management

| Action | macOS | Linux/Windows |
|--------|-------|---------------|
| Split right (new pane beside) | `Cmd + D` | `Ctrl + Shift + E` |
| Split down (new pane below) | `Cmd + Shift + D` | `Ctrl + Shift + O` (alias `Ctrl + Shift + D`) |
| Close focused pane | `Cmd + W` | `Ctrl + Shift + W` (alias `Ctrl + Shift + X`) |
| Navigate pane left | `Cmd + Opt + Left` | `Ctrl + Alt + Left` |
| Navigate pane right | `Cmd + Opt + Right` | `Ctrl + Alt + Right` |
| Navigate pane up | `Cmd + Opt + Up` | `Ctrl + Alt + Up` |
| Navigate pane down | `Cmd + Opt + Down` | `Ctrl + Alt + Down` |
| Resize pane left | `Cmd + Ctrl + Left` | `Ctrl + Alt + Shift + Left` |
| Resize pane right | `Cmd + Ctrl + Right` | `Ctrl + Alt + Shift + Right` |
| Resize pane up | `Cmd + Ctrl + Up` | `Ctrl + Alt + Shift + Up` |
| Resize pane down | `Cmd + Ctrl + Down` | `Ctrl + Alt + Shift + Down` |
| Zoom pane (Maximize Active Pane) | `Cmd + Shift + Enter` | `Ctrl + Shift + Enter` |
| Next / previous pane | `Cmd + ]` / `Cmd + [` | `Ctrl + Alt + ]` / `Ctrl + Alt + [` |
| Equalize panes | `Cmd + Opt + =` | `Ctrl + Alt + =` |
| Last-focused pane | *(unbound)* | *(unbound)* |
| Restart pane process | *(unbound)* | *(unbound)* |
| Cycle layout presets | *(unbound)* | *(unbound)* |
| Split left / split up | *(unbound)* | *(unbound)* |
| Resize mode (arrows resize until `Esc`) | *(unbound)* | *(unbound)* |
| Swap pane left / right / up / down | `Cmd + Opt + Shift + Arrow` | *(unbound)* |
| Promote pane to tab | *(unbound)* | *(unbound)* |
| Demote tab to pane | *(unbound)* | *(unbound)* |
| Select pane by letter | `Cmd + Opt + P` | `Ctrl + Alt + P` |
| Toggle broadcast input (this tab) | `Cmd + Opt + I` | `Ctrl + Alt + I` |
| Toggle broadcast for the current pane | `Cmd + Ctrl + Opt + I` | *(unbound)* |

> **📝 Note:** Pane swap ships unbound on Linux and Windows: editors use `Alt + Shift + Arrow`, and Windows switches the input language on `Alt + Shift`. Bind `swap_pane_*` yourself if you want it.

> **📝 Note:** A resize arrow moves the divider on that side of the focused pane in the arrow's direction (tmux semantics); a pane with no divider on that side moves the one on its other side. Each press moves it `pane_resize_step` percent of its split (default 5). **Resize mode** (`enter_resize_mode`) keeps the arrows resizing until `Esc` or `Enter`: plain arrows move by `pane_resize_step`, `Shift` + arrows by one cell, and every other key is ignored so nothing reaches the shell. No resize, split, drag, equalize, or layout preset takes a pane below `pane_min_size` cells. `split_balance: siblings` or `all` rebalances after each keyboard or menu split, so repeated splits share the space evenly instead of halving. Double-click a divider to equalize its split.

> **📝 Note:** In a par-mux tab, equalize works: par-term computes equal sizes and sends them to the daemon, so other clients see the change. Layout presets (`cycle_layout`, `layout:<name>`) are refused there with a message: the daemon has no `select-layout`, and every preset except an equalize of the existing arrangement needs panes moved between splits daemon-side. The same limit applies to `split_balance`, which the daemon's layout overrides. tmux gateway tabs refuse both, because tmux owns their layout.

> **📝 Note:** Zoom fills the tab with the focused pane and shows ⛶ on the tab and the pane title; splitting, closing, or moving focus unzooms. In a par-mux tab the zoom is daemon-side (`resize-pane -Z`), so other clients see it.

> **📝 Note:** Broadcast input is per tab: every pane that receives it gets an amber outline, the tab shows 📡, the status bar (when enabled) shows how many panes receive it, and pastes are broadcast along with keystrokes.

> **📝 Note:** Promote and demote actions have no default keybinding. Bind them in Settings → Input → Keybindings or via config YAML using the `promote_pane_to_tab` and `demote_tab_to_pane` action names. See [Tabs](../features/TABS.md#promoting-and-demoting-panes) for details.

> **📝 Note:** **Select pane by letter** (`select_pane_hint`, tmux `display-panes` style) draws a letter badge centered in every pane of the focused tab; type a pane's letter to focus it. Any other key, including `Escape`, cancels without changing focus. See [Tabs](../features/TABS.md#selecting-a-pane-by-letter) for details.

## Advanced Features

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + Shift + P` | Open the command palette (`toggle_command_palette`) |
| `Cmd + O` (macOS) / Profiles menu (Linux/Win) | Toggle the profile drawer (`toggle_profile_drawer`) |
| `Cmd/Ctrl + Shift + B` | Toggle background shader |
| `Cmd/Ctrl + Shift + U` | Toggle cursor shader |
| `Cmd + Shift + S` (macOS) / `Ctrl + Shift + S` (Linux/Win) | SSH Quick Connect |
| `Cmd + Ctrl + S` (macOS) / `Ctrl + Alt + T` or `Ctrl + Alt + S` (Linux/Win) | Toggle session picker (par-mux and tmux) |
| `Cmd + ,` (macOS) / `F12` | Open the Settings window |
| `Ctrl + ,` (all platforms) | Cycle cursor style (Block → Beam → Underline) |

> **📝 Note:** The command palette owns `Cmd/Ctrl + Shift + P` (the VS Code convention). The profile drawer moved to iTerm2's **Open Profiles** chord, `Cmd + O`, on macOS. On Linux and Windows it ships unbound, because the matching `Ctrl + Shift + O` is Split Down there; open it from the Profiles menu or the palette. Reach **Manage Profiles...** from the Profiles menu, or from **Settings ▸ Profiles**.

> **📝 Note:** The session picker moved off `Cmd + Opt + T` on macOS, which iTerm2 uses for New Tab Next to Current. On Linux, Ubuntu opens a terminal on `Ctrl + Alt + T`, so `Ctrl + Alt + S` is offered as well.

> **📝 Note:** Cursor style cycles on `Ctrl + ,` on **every** platform, macOS included — the cycler accepts either Ctrl or Cmd, and nothing higher in the dispatch chain claims `Ctrl + ,`. On macOS it is specifically **not** `Cmd + ,`: that is the `Settings...` key equivalent on the application menu and opens the Settings window instead.

> **📝 Note:** The Assistant panel is toggled with `Cmd + I` (macOS) or `Ctrl + Shift + I` (Linux/Windows) when `ai_inspector_enabled` is `true`. It can also be bound via custom keybindings. See [Assistant Panel](../ASSISTANT_PANEL.md) for details.

## Customizing Keybindings

Keybindings can be customized in `~/.config/par-term/config.yaml`:

```yaml
keybindings:
  - key: "CmdOrCtrl+Shift+B"
    action: "toggle_background_shader"
  - key: "CmdOrCtrl+Shift+V"
    action: "paste_special"
  - key: "CmdOrCtrl+D"
    action: "split_right"
```

A binding saved under a renamed action id keeps working: `split_horizontal` is
migrated to `split_down` and `split_vertical` to `split_right` when the config
loads, with the chord unchanged.

#### Passing a chord through to the shell

Binding a chord to the `pass_to_terminal` action gives it back to the shell:
no shortcut layer, menu accelerator fallback, or paste/copy interception may
take it, and the default merge never re-binds that chord on load, so the row
survives restarts. This is how to free chords the terminal steals by default,
e.g. `Alt+1` (readline `digit-argument`, irssi/weechat window numbers) or
`Cmd+1` tab switching:

```yaml
keybindings:
  - key: "Alt+1"
    action: "pass_to_terminal"
```

Custom actions also support an optional two-stroke trigger. Set `custom_action_prefix_key`
globally, then assign a single-character `prefix_char` on each action (or a single-character
`keybinding` with no modifiers). Press the prefix key, release it, then press the action
character to execute it. While prefix mode is armed, a toast remains visible; press `Esc`
to cancel it. When a prefix key is set, a single-character `keybinding` is the prefix
follow-up only — it is not also a global shortcut. All action types — including the
workflow types `sequence`, `condition`, and `repeat` — accept keybindings and prefix chars.
See [Snippets](../features/SNIPPETS.md) for the full action format and workflow action
reference.



### Available Modifiers

| Modifier | Aliases | Description |
|----------|---------|-------------|
| `Ctrl` | `Control` | Control key |
| `Alt` | `Option` | Alt/Option key |
| `Shift` | — | Shift key |
| `Super` | `Cmd`, `Command`, `Meta`, `Win` | Windows/Command key |
| `CmdOrCtrl` | — | Cmd (macOS) or Ctrl (Windows/Linux) |

### Available Actions

**Application & Windows:**
- `new_window`, `quit`
- `close_window` - Close the whole window with all its tabs, asking first when it holds more than one tab (the same confirmation as the title-bar close)
- `close_tab_or_window` - Smart close: the active tab when the window has several, otherwise the window. This was `close_window` before; neither has a default chord since `Cmd + W` became the pane-cascading `close_pane`.
- `next_window`, `prev_window` - Focus the next or previous window in window-number order, wrapping
- `toggle_tree_picker` - Open Quickly: every window, tab, and pane in one fuzzy list (hidden par-mux tabs included; a query also matches a tab's session and a pane's directory); Enter jumps there, re-showing a hidden tab (`Cmd + Shift + O` / `Ctrl + Alt + O`)
- `switch_to_window_1`, `switch_to_window_2`, `switch_to_window_3`, `switch_to_window_4`, `switch_to_window_5`, `switch_to_window_6`, `switch_to_window_7`, `switch_to_window_8`, `switch_to_window_9` - Focus the window holding that number (shown in the title with `show_window_number`). No default chord (iTerm2 uses `Cmd + Opt + digit`); bind one yourself
- `select_all` - Select the whole terminal buffer (scrollback plus visible screen), or the focused text field when Settings or an overlay has focus
- `toggle_menu` - Open the in-app application menu

`new_window`, `quit` and `select_all` ship with default `keybindings` entries
mirroring the menu accelerators (`Cmd + N` / `Ctrl + Shift + N`, `+ Q`,
`+ A`). `toggle_menu` has no default. Bind it if you run with
`tab_bar_mode: never` — the tab bar is what normally holds the `☰` button, so
without a binding there is no way to open the in-app menu.

**Command Palette:**
- `toggle_command_palette` - Open the fuzzy action launcher over every bindable action (`Cmd/Ctrl + Shift + P`). Each row shows its live chord and a line naming its category (Window, Tab, Pane, Session, Profiles, View, Agents, Edit, Terminal) and what it does. With nothing typed, agents waiting on you come first, then the actions you last ran from the palette in this window, then everything grouped by category

**Tab Management:**
- `new_tab`, `close_tab`, `duplicate_tab`, `next_tab`, `prev_tab`
- `move_tab_left`, `move_tab_right`
- `switch_to_tab_1`, `switch_to_tab_2`, `switch_to_tab_3`, `switch_to_tab_4`, `switch_to_tab_5`, `switch_to_tab_6`, `switch_to_tab_7`, `switch_to_tab_8`, `switch_to_tab_9`
- `reopen_closed_tab`
- `last_tab` - Toggle to the previously active tab (tmux `last-window`)
- `go_to_last_tab` - Switch to the rightmost tab
- `rename_tab` - Open the inline rename field on the active tab (the same field a double-click opens)
- `close_other_tabs`, `close_tabs_to_right` - Close every other tab, or every tab right of the active one; each tab keeps its own running-job confirmation, and a par-mux tab closes its daemon window
- `move_tab_to_new_window` — transfer the active tab to a new window while preserving PTY, scrollback, and split panes
- `move_tab_to_window_picker` — open the command palette on **Move Tab to Window N** rows, one per other window; `move_tab_to_window:<n>` moves the active tab to window N directly. A par-mux tab cannot move (detach first)

**Window Arrangements:**
- `save_arrangement` - Save current window layout as a named arrangement
- `restore_arrangement:<name>` - Restore a previously saved arrangement by name

**Pane Management:**
- `split_right`, `split_down` (the old ids `split_vertical` and `split_horizontal` still work as aliases)
- `close_pane` - Close the focused pane, cascading to the tab and window
- `navigate_pane_left`, `navigate_pane_right`
- `navigate_pane_up`, `navigate_pane_down`
- `select_pane_hint`
- `resize_pane_left`, `resize_pane_right`
- `resize_pane_up`, `resize_pane_down`
- `swap_pane_left`, `swap_pane_right`, `swap_pane_up`, `swap_pane_down` — swap the focused pane with its neighbor in that direction
- `toggle_pane_zoom` — fill the tab with the focused pane (again to unzoom)
- `next_pane`, `prev_pane`, `last_pane` — cycle panes in tree order, or return to the previously focused pane
- `equalize_panes`, `cycle_layout`, `layout:<name>` — equal pane sizes; step through or pick a layout (`even-horizontal`, `even-vertical`, `main-left`, `main-top`, `tiled`)
- `split_left`, `split_up` — split with the new pane before the focused one
- `enter_resize_mode` — resize panes with the arrow keys until `Esc` or `Enter`
- `restart_pane` — restart the focused pane's process in place: the program it was started with (a profile's command or SSH connection), else the default shell
- `toggle_pane_broadcast` — exclude or include the focused pane in its tab's broadcast
- `promote_pane_to_tab`, `demote_tab_to_pane`
- `rename_pane` - Open the inline rename field on the focused pane's title bar

**Display:**
- `toggle_fullscreen`, `maximize_vertically`
- `toggle_always_on_top` - Keep the focused window above other windows (no default chord)
- `toggle_fps_overlay`, `toggle_help`
- `toggle_search`, `open_settings`

**Features:**
- `paste_special`, `toggle_clipboard_history`
- `toggle_copy_mode`, `enter_copy_mode`, `toggle_session_logging`, `toggle_throughput_mode`
- `toggle_background_shader`, `toggle_cursor_shader`
- `cycle_background_shader`, `toggle_shader_animation`, `toggle_shader_readability_mode`
- `toggle_broadcast_input`, `toggle_profile_drawer`
- `toggle_session_picker` - The session picker: par-mux sessions (attach, switch, create, rename, end, detach) and, with tmux integration on, tmux sessions. The old id `toggle_tmux_session_picker` still works
- `ssh_quick_connect`
- `toggle_ai_inspector`, `toggle_command_history`
- `reload_dynamic_profiles`
- `toggle_agent_usage_panel` - Show or hide the agent usage panel ([Agent Usage](../features/AGENT_USAGE.md))

**Scrolling:**
- `scroll_up_page`, `scroll_down_page`, `scroll_to_top`, `scroll_to_bottom`
- `scroll_to_previous_mark`, `scroll_to_next_mark` - Jump between shell-integration command marks

**Terminal:**
- `clear_screen`, `clear_scrollback`, `reload_config`
- `increase_font_size`, `decrease_font_size`, `reset_font_size`
- `cycle_cursor_style`

**Runtime actions** (ids built from your config and live state; the command palette lists them, and each can be bound like any other action):
- `snippet:<id>`, `action:<id>` - Run a snippet or custom action ([Snippets](../features/SNIPPETS.md))
- `restore_arrangement:<name>` - Restore a saved window arrangement
- `launch-agent:<id>`, `launch-agent-autonomous:<id>`, `launch-default-agent` - Launch a configured coding agent
- `agent-cmd:<id>` - Run an agent-authored command ([Agent Commands](../features/AGENT_COMMANDS.md))
- `agent-roster-focus:<pane>` - Focus the pane of a par-mux roster agent ([par-mux](../features/MUX.md))
- `detach` - Detach this window from its par-mux session, leaving the session running (`mux-detach` still works as an alias)
- `focus_next_attention_agent` - Jump to the next par-mux agent that is blocked, then to agents that finished and were not looked at, cycling across tabs (`Cmd + Opt + A` / `Ctrl + Alt + A`)
- `new_mux_session` - Create a par-mux session (named `session-N`) and attach this window to it, no profile needed; an attached window switches to it
- `attach_mux_session:<daemon>/<name>` - Attach this window to a running par-mux session (`attach_mux_session:<name>` finds it by name among the running daemons); the palette lists one row per running session. A name no daemon lists is reported and nothing is created
- `mux-restart-pane` - Restart the focused par-mux pane's process
- `plugin-action:<plugin_id>:<action_id>` - Run a plugin-contributed action ([Plugins](../features/PLUGINS.md))
- `triage-crash:<id>` - Open a crash-triage offer ([Crash Triage](../features/CRASH_TRIAGE.md))

## Platform Shortcut Conflicts

A static audit (UX.md RT2 and RT3, 2026-09-29) of par-term's shipped default chords against the shortcuts each desktop reserves out of the box. An OS-level shortcut is taken by the desktop before par-term sees the key, so an entry marked **conflict** will not reach par-term on that platform unless the desktop shortcut is removed. Rebind the par-term action in **Settings ▸ Input ▸ Keybindings** if you keep the desktop shortcut. This is a table-level check; whether each key actually reaches par-term on a running system is tracked separately as a runtime check.

| par-term default | Action | Platform | Desktop default | Result |
|------------------|--------|----------|-----------------|--------|
| `Ctrl+Alt+Left` / `Right` / `Up` / `Down` | Focus pane | Linux (GNOME) | Switch workspace ([GNOME schema][gnome-wm]) | **Conflict** |
| `Ctrl+Alt+Shift+Left` / `Right` / `Up` / `Down` | Resize pane | Linux (GNOME) | Move window to workspace ([GNOME schema][gnome-wm]) | **Conflict** |
| `Ctrl+Alt+T` | Session picker | Linux (Ubuntu, many KDE distributions) | Open a terminal (distribution default, not in upstream GNOME schemas) | **Conflict where configured**; use the alias `Ctrl+Alt+S` |
| `Ctrl+Shift+U` | Toggle cursor shader | Linux with IBus | Unicode code-point entry ([IBus schema][ibus]) | **Conflict** when IBus is the input method |
| `Ctrl+Alt+W`, `Ctrl+Alt+P`, `Ctrl+Alt+I`, `Ctrl+Alt+V`, `Ctrl+Alt+R`, `Ctrl+Alt+S` | Close tab, select pane, broadcast, paste special, command history, session picker | Linux (GNOME) | None found in the GNOME window-manager or media-key schemas ([wm][gnome-wm], [media keys][gnome-media]) | No conflict found |
| `Ctrl+Shift+E`, `Ctrl+Shift+O` | Split right, split down | Linux with IBus | IBus reserves `Ctrl+Shift+U` only; emoji is `Super+.` ([IBus schema][ibus]) | No conflict found |
| `Ctrl+Shift+<key>` chords | Window and tab actions | Windows | `Ctrl+Shift` alone switches keyboard layout when several are installed ([Microsoft][ms-keys]); chords that add a letter are not listed | No conflict found |
| `Ctrl+Alt+<key>` chords | Pane actions | Windows | `Ctrl+Alt+Del`, `Ctrl+Alt+Tab` only ([Microsoft][ms-keys]) | No conflict found |
| `Alt+Shift+Arrow` | (none shipped; the old Linux/Windows swap default was removed) | Windows, Linux with IBus | Windows lists `Win+Space` for input switching ([Microsoft][ms-keys]); IBus binds `Alt+Shift_L` to next engine ([IBus schema][ibus]) | Not applicable |
| `F11` | Toggle fullscreen | macOS | Show desktop is `Fn+F11` or `Cmd+Mission Control` ([Apple][apple-keys]); on keyboards where F11 is a media key, press `Fn+F11` | Use `Cmd+Ctrl+F` (also shipped) |
| `Cmd+Ctrl+F` | Toggle fullscreen | macOS | Full screen, the standard app shortcut ([Apple][apple-keys]) | Aligned |
| `Cmd+Opt+<key>` pane chords | Focus, swap, select, broadcast, close tab | macOS | `Cmd+Opt+Esc`, `Cmd+Opt+H`, `Cmd+Opt+D` reserved ([Apple][apple-keys]); none of par-term's keys | No conflict found |
| `Cmd+Ctrl+Arrow` | Resize pane | macOS | Not a system default; window-manager apps such as Rectangle may claim it | No conflict with the OS |
| `Cmd+\`` | (none shipped) | macOS | Cycles the front app's windows ([Apple][apple-keys]) | Not applicable: par-term binds nothing to it. Whether winit delivers it to par-term (RT3) is a runtime check, unverified |

`Cmd+Opt+A` / `Ctrl+Alt+A` (next agent needing attention) and `Ctrl+Alt+=` (equalize panes) ship as defaults; none of them appears in the vendor lists above. KDE Plasma's upstream defaults were not checked against a primary source.

[gnome-wm]: https://gitlab.gnome.org/GNOME/gsettings-desktop-schemas/-/blob/master/schemas/org.gnome.desktop.wm.keybindings.gschema.xml.in
[gnome-media]: https://gitlab.gnome.org/GNOME/gnome-settings-daemon/-/blob/master/data/org.gnome.settings-daemon.plugins.media-keys.gschema.xml.in
[ibus]: https://github.com/ibus/ibus/blob/main/data/dconf/org.freedesktop.ibus.gschema.xml
[ms-keys]: https://support.microsoft.com/en-us/windows/keyboard-shortcuts-in-windows-dcc61a57-8ff0-cffe-9796-cb9706c75eec
[apple-keys]: https://support.apple.com/en-us/102650

## Related Documentation

- [README.md](../../README.md) - Project overview
- [Mouse Features](../features/MOUSE_FEATURES.md) - Mouse interactions and semantic history
- [Copy Mode](../features/COPY_MODE.md) - Vi-style keyboard-driven text selection
- [Tabs](../features/TABS.md) - Tab and split pane management
- [Profiles](../features/PROFILES.md) - Profile keyboard shortcuts
- [Search](../features/SEARCH.md) - Search keyboard shortcuts
- [Command History](../features/COMMAND_HISTORY.md) - Fuzzy command history search
- [SSH Host Management](../features/SSH.md) - SSH Quick Connect shortcuts
- [Restoring Windows and Reopening Tabs](../features/SESSION_MANAGEMENT.md) - Reopen closed tab and restore windows on launch
- [Window Management](../features/WINDOW_MANAGEMENT.md) - Window arrangements and layout management
- [Snippets](../features/SNIPPETS.md) - Custom snippet and action keybindings
