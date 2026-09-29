# Keyboard Shortcuts

Complete reference for all par-term keyboard shortcuts.

> **📝 Note:** On macOS, the defaults follow iTerm2 wherever par-term has the matching action, with `Cmd` as the primary modifier. On Linux and Windows, the same letters use the K1 family: `Ctrl+Shift` where macOS uses `Cmd` (window and tab), and `Ctrl+Alt` where macOS uses `Cmd+Opt` (panes). Plain `Ctrl+letter` stays with the shell (Ctrl+C for SIGINT, Ctrl+D for EOF, etc.). The **F1** help panel always shows your *current* bindings.

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
| `Cmd/Ctrl + Shift + R` | Toggle session logging |
| Command palette (macOS) / `Ctrl + Shift + M` (Linux/Win) | Toggle maximize throughput mode |

> **📝 Note:** Session logging uses the `toggle_session_logging` action (see [Session Logging](../features/SESSION_LOGGING.md)). Throughput mode uses `toggle_throughput_mode`; on macOS it has no default chord since `Cmd + Shift + T` became Reopen Closed Tab, so run it from the command palette or bind it. Screenshots are taken via the `--screenshot` CLI option or MCP server tool, not a keyboard shortcut.

## Font & Text Sizing

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + +` or `Cmd/Ctrl + =` | Increase font size |
| `Cmd/Ctrl + -` | Decrease font size |
| `Cmd/Ctrl + 0` | Reset font size to default |

## UI Toggles & Display

| Shortcut | Action |
|----------|--------|
| `F1` | Toggle Help panel |
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
| Swap pane left / right / up / down | `Cmd + Opt + Shift + Arrow` | *(unbound)* |
| Promote pane to tab | *(unbound)* | *(unbound)* |
| Demote tab to pane | *(unbound)* | *(unbound)* |
| Select pane by letter | `Cmd + Opt + P` | `Ctrl + Alt + P` |
| Toggle broadcast input | `Cmd + Opt + I` | `Ctrl + Alt + I` |

> **📝 Note:** Pane swap ships unbound on Linux and Windows: editors use `Alt + Shift + Arrow`, and Windows switches the input language on `Alt + Shift`. Bind `swap_pane_*` yourself if you want it.

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
| `Cmd + Ctrl + S` (macOS) / `Ctrl + Alt + T` or `Ctrl + Alt + S` (Linux/Win) | Toggle tmux session picker |
| `Cmd + ,` (macOS) / `F12` | Open the Settings window |
| `Ctrl + ,` (all platforms) | Cycle cursor style (Block → Beam → Underline) |

> **📝 Note:** The command palette owns `Cmd/Ctrl + Shift + P` (the VS Code convention). The profile drawer moved to iTerm2's **Open Profiles** chord, `Cmd + O`, on macOS. On Linux and Windows it ships unbound, because the matching `Ctrl + Shift + O` is Split Down there; open it from the Profiles menu or the palette. Reach **Manage Profiles...** from the Profiles menu, or from **Settings ▸ Profiles**.

> **📝 Note:** The tmux session picker moved off `Cmd + Opt + T` on macOS, which iTerm2 uses for New Tab Next to Current. On Linux, Ubuntu opens a terminal on `Ctrl + Alt + T`, so `Ctrl + Alt + S` is offered as well.

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
- `close_window` - Smart close: the active tab when the window has several, otherwise the window. No default chord since `Cmd + W` became the pane-cascading `close_pane`.
- `select_all` - Select the whole terminal buffer (scrollback plus visible screen), or the focused text field when Settings or an overlay has focus
- `toggle_menu` - Open the in-app application menu

`new_window`, `quit` and `select_all` ship with default `keybindings` entries
mirroring the menu accelerators (`Cmd + N` / `Ctrl + Shift + N`, `+ Q`,
`+ A`). `toggle_menu` has no default. Bind it if you run with
`tab_bar_mode: never` — the tab bar is what normally holds the `☰` button, so
without a binding there is no way to open the in-app menu.

**Command Palette:**
- `toggle_command_palette` - Open the fuzzy action launcher over every bindable action (`Cmd/Ctrl + Shift + P`)

**Tab Management:**
- `new_tab`, `close_tab`, `duplicate_tab`, `next_tab`, `prev_tab`
- `move_tab_left`, `move_tab_right`
- `switch_to_tab_1` through `switch_to_tab_9`
- `reopen_closed_tab`
- `move_tab_to_new_window` — transfer the active tab to a new window while preserving PTY, scrollback, and split panes

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
- `promote_pane_to_tab`, `demote_tab_to_pane`

**Display:**
- `toggle_fullscreen`, `maximize_vertically`
- `toggle_fps_overlay`, `toggle_help`
- `toggle_search`, `open_settings`

**Features:**
- `paste_special`, `toggle_clipboard_history`
- `toggle_copy_mode`, `enter_copy_mode`, `toggle_session_logging`, `toggle_throughput_mode`
- `toggle_background_shader`, `toggle_cursor_shader`
- `cycle_background_shader`, `toggle_shader_animation`, `toggle_shader_readability_mode`
- `toggle_broadcast_input`, `toggle_profile_drawer`
- `toggle_tmux_session_picker`, `ssh_quick_connect`
- `toggle_ai_inspector`, `toggle_command_history`
- `reload_dynamic_profiles`

**Terminal:**
- `clear_scrollback`, `reload_config`
- `increase_font_size`, `decrease_font_size`, `reset_font_size`
- `cycle_cursor_style`

## Related Documentation

- [README.md](../../README.md) - Project overview
- [Mouse Features](../features/MOUSE_FEATURES.md) - Mouse interactions and semantic history
- [Copy Mode](../features/COPY_MODE.md) - Vi-style keyboard-driven text selection
- [Tabs](../features/TABS.md) - Tab and split pane management
- [Profiles](../features/PROFILES.md) - Profile keyboard shortcuts
- [Search](../features/SEARCH.md) - Search keyboard shortcuts
- [Command History](../features/COMMAND_HISTORY.md) - Fuzzy command history search
- [SSH Host Management](../features/SSH.md) - SSH Quick Connect shortcuts
- [Session Management](../features/SESSION_MANAGEMENT.md) - Session undo and restore
- [Window Management](../features/WINDOW_MANAGEMENT.md) - Window arrangements and layout management
- [Snippets](../features/SNIPPETS.md) - Custom snippet and action keybindings
