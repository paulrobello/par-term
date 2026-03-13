# Keyboard Shortcuts

Complete reference for all par-term keyboard shortcuts.

> **📝 Note:** On macOS, keybindings use `Cmd` as the primary modifier. On Linux and Windows, keybindings use `Ctrl+Shift` combinations to avoid conflicts with standard terminal control codes (Ctrl+C for SIGINT, Ctrl+D for EOF, etc.). This follows conventions from WezTerm, Kitty, GNOME Terminal, and Windows Terminal.

## Table of Contents
- [Window & Tab Management](#window--tab-management)
- [Navigation & Scrolling](#navigation--scrolling)
- [Copy, Paste & Selection](#copy-paste--selection)
- [Copy Mode](#copy-mode)
- [Search](#search)
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
| New window | `Cmd + N` | `Ctrl + Shift + N` |
| New tab | `Cmd + T` | `Ctrl + Shift + T` |
| Close tab/window | `Cmd + W` | `Ctrl + Shift + W` |
| Quit application | `Cmd + Q` | `Ctrl + Q` |
| Next tab | `Cmd + Shift + ]` | `Ctrl + Shift + ]` |
| Previous tab | `Cmd + Shift + [` | `Ctrl + Shift + [` |
| Next tab (alt) | `Ctrl + Tab` | `Ctrl + Tab` |
| Previous tab (alt) | `Ctrl + Shift + Tab` | `Ctrl + Shift + Tab` |
| Switch to tab 1-9 | `Cmd + 1-9` | `Alt + 1-9` |
| Move tab left | `Cmd + Shift + Left` | `Ctrl + Shift + Left` |
| Move tab right | `Cmd + Shift + Right` | `Ctrl + Shift + Right` |
| Reopen closed tab | `Cmd + Z` | `Ctrl + Shift + Z` |
| Save window arrangement | View menu: "Save Window Arrangement..." | View menu: "Save Window Arrangement..." |

## Navigation & Scrolling

| Shortcut | Action |
|----------|--------|
| `PageUp` | Forward `\x1b[5~` to terminal application |
| `PageDown` | Forward `\x1b[6~` to terminal application |
| `Shift + PageUp` | Scroll up one page in scrollback |
| `Shift + PageDown` | Scroll down one page in scrollback |
| `Shift + Home` | Jump to top of scrollback |
| `Shift + End` | Jump to bottom |
| `Mouse Wheel` | Scroll up/down |

## Copy, Paste & Selection

| Action | macOS | Linux/Windows |
|--------|-------|---------------|
| Copy selection | `Cmd + C` | `Ctrl + Shift + C` |
| Paste | `Cmd + V` | `Ctrl + Shift + V` |
| Paste (X11 fallback) | `Shift + Insert` | `Shift + Insert` |
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

Vi-style keyboard-driven text selection. See [Copy Mode](COPY_MODE.md) for complete reference.

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
| Find next (global) | `Cmd + G` | `Ctrl + G` |
| Find previous (global) | `Cmd + Shift + G` | `Ctrl + Shift + G` |
| Open command history | `Cmd + R` | `Ctrl + Alt + R` |

> **📝 Note:** Command history uses `toggle_command_history` action (fuzzy search). This is separate from shell's built-in reverse search (Ctrl+R).

## Terminal Operations

| Shortcut | Action |
|----------|--------|
| `Ctrl + L` | Clear visible screen |
| `Ctrl + Shift + K` | Clear scrollback buffer |
| `Ctrl + Shift + S` | Take screenshot (via MCP) |
| `Cmd/Ctrl + Shift + R` | Toggle session recording |
| `Cmd + Shift + T` (macOS) / `Ctrl + Shift + M` (Linux/Win) | Toggle maximize throughput mode |
| `Ctrl + Shift + F5` | Fix rendering (after monitor change) |

> **📝 Note:** Throughput mode uses `toggle_throughput_mode` action. Screenshot is captured via MCP server tool.

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
| `F11` | Toggle fullscreen |
| `F12` | Open Settings |
| `Cmd + ,` (macOS) | Open Settings |
| `Ctrl + ,` (Windows/Linux) | Open Settings |
| `Escape` | Close current UI panel |

## Pane Management

| Action | macOS | Linux/Windows |
|--------|-------|---------------|
| Split horizontally | `Cmd + D` | `Ctrl + Shift + D` |
| Split vertically | `Cmd + Shift + D` | `Ctrl + Shift + E` |
| Close focused pane | `Cmd + Shift + W` | `Ctrl + Shift + X` |
| Navigate pane left | `Cmd + Alt + Left` | `Ctrl + Alt + Left` |
| Navigate pane right | `Cmd + Alt + Right` | `Ctrl + Alt + Right` |
| Navigate pane up | `Cmd + Alt + Up` | `Ctrl + Alt + Up` |
| Navigate pane down | `Cmd + Alt + Down` | `Ctrl + Alt + Down` |
| Resize pane left | `Cmd + Alt + Shift + Left` | `Ctrl + Alt + Shift + Left` |
| Resize pane right | `Cmd + Alt + Shift + Right` | `Ctrl + Alt + Shift + Right` |
| Resize pane up | `Cmd + Alt + Shift + Up` | `Ctrl + Alt + Shift + Up` |
| Resize pane down | `Cmd + Alt + Shift + Down` | `Ctrl + Alt + Shift + Down` |

## Advanced Features

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + Shift + B` | Toggle background shader |
| `Cmd/Ctrl + Shift + U` | Toggle cursor shader |
| `Cmd/Ctrl + Shift + P` | Toggle profile drawer |
| `Cmd + Shift + S` (macOS) / `Ctrl + Shift + S` (Linux/Win) | SSH Quick Connect |
| `Cmd/Ctrl + Alt + I` | Toggle broadcast input |
| `Cmd/Ctrl + Alt + T` | Toggle tmux session picker |
| `Ctrl + ,` | Cycle cursor style |

> **📝 Note:** The Assistant panel (`toggle_ai_inspector`) can be bound via custom keybindings. See [Assistant Panel](ASSISTANT_PANEL.md) for details.

## Customizing Keybindings

Keybindings can be customized in `~/.config/par-term/config.yaml`:

```yaml
keybindings:
  - key: "CmdOrCtrl+Shift+B"
    action: "toggle_background_shader"
  - key: "CmdOrCtrl+Shift+V"
    action: "paste_special"
  - key: "CmdOrCtrl+D"
    action: "split_horizontal"
```

Custom actions also support an optional two-stroke trigger. Set `custom_action_prefix_key`
globally, then assign a single-character `prefix_char` on each action. Press the prefix key,
release it, then press the action character to execute it. While prefix mode is armed, a toast
remains visible; press `Esc` to cancel it. See [Snippets](SNIPPETS.md) for the full action format.

### Available Modifiers

| Modifier | Description |
|----------|-------------|
| `Ctrl` | Control key |
| `Alt` | Alt/Option key |
| `Shift` | Shift key |
| `Super` | Windows/Command key |
| `CmdOrCtrl` | Cmd (macOS) or Ctrl (Windows/Linux) |

### Available Actions

**Tab Management:**
- `new_tab`, `close_tab`, `next_tab`, `prev_tab`
- `move_tab_left`, `move_tab_right`
- `switch_to_tab_1` through `switch_to_tab_9`
- `reopen_closed_tab`

**Window Arrangements:**
- `save_arrangement` - Save current window layout as a named arrangement
- `restore_arrangement:<name>` - Restore a previously saved arrangement by name

**Pane Management:**
- `split_horizontal`, `split_vertical`, `close_pane`
- `navigate_pane_left`, `navigate_pane_right`
- `navigate_pane_up`, `navigate_pane_down`
- `resize_pane_left`, `resize_pane_right`
- `resize_pane_up`, `resize_pane_down`

**Display:**
- `toggle_fullscreen`, `maximize_vertically`
- `toggle_fps_overlay`, `toggle_help`
- `toggle_search`, `open_settings`

**Features:**
- `paste_special`, `toggle_clipboard_history`
- `toggle_copy_mode`, `toggle_session_logging`, `toggle_throughput_mode`
- `toggle_background_shader`, `toggle_cursor_shader`, `toggle_prettifier`
- `toggle_broadcast_input`, `toggle_profile_drawer`
- `toggle_tmux_session_picker`, `ssh_quick_connect`
- `toggle_ai_inspector`, `toggle_command_history`
- `reload_dynamic_profiles`

**Terminal:**
- `clear_scrollback`, `reload_config`
- `increase_font_size`, `decrease_font_size`, `reset_font_size`
- `cycle_cursor_style`

## Related Documentation

- [README.md](../README.md) - Project overview
- [Mouse Features](MOUSE_FEATURES.md) - Mouse interactions and semantic history
- [Copy Mode](COPY_MODE.md) - Vi-style keyboard-driven text selection
- [Tabs](TABS.md) - Tab and split pane management
- [Profiles](PROFILES.md) - Profile keyboard shortcuts
- [Search](SEARCH.md) - Search keyboard shortcuts
- [Command History](COMMAND_HISTORY.md) - Fuzzy command history search
- [SSH Host Management](SSH.md) - SSH Quick Connect shortcuts
- [Session Management](SESSION_MANAGEMENT.md) - Session undo and restore
- [Window Management](WINDOW_MANAGEMENT.md) - Window arrangements and layout management
- [Snippets](SNIPPETS.md) - Custom snippet and action keybindings
