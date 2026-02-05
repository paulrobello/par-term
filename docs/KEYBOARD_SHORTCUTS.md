# Keyboard Shortcuts

Complete reference for all par-term keyboard shortcuts.

## Table of Contents
- [Window & Tab Management](#window--tab-management)
- [Navigation & Scrolling](#navigation--scrolling)
- [Copy, Paste & Selection](#copy-paste--selection)
- [Search](#search)
- [Terminal Operations](#terminal-operations)
- [Font & Text Sizing](#font--text-sizing)
- [UI Toggles & Display](#ui-toggles--display)
- [Pane Management](#pane-management)
- [Advanced Features](#advanced-features)
- [Customizing Keybindings](#customizing-keybindings)
- [Related Documentation](#related-documentation)

## Window & Tab Management

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + N` | New window |
| `Cmd/Ctrl + T` | New tab |
| `Cmd/Ctrl + W` | Close tab (or window if single tab) |
| `Cmd/Ctrl + Q` | Quit application (Windows/Linux) |
| `Cmd/Ctrl + Shift + ]` | Next tab |
| `Cmd/Ctrl + Shift + [` | Previous tab |
| `Ctrl + Tab` | Next tab (alternative) |
| `Ctrl + Shift + Tab` | Previous tab (alternative) |
| `Cmd/Ctrl + 1-9` | Switch to tab 1-9 |
| `Cmd/Ctrl + Shift + Left` | Move tab left |
| `Cmd/Ctrl + Shift + Right` | Move tab right |

## Navigation & Scrolling

| Shortcut | Action |
|----------|--------|
| `PageUp` | Scroll up one page |
| `PageDown` | Scroll down one page |
| `Shift + Home` | Jump to top of scrollback |
| `Shift + End` | Jump to bottom |
| `Mouse Wheel` | Scroll up/down |

## Copy, Paste & Selection

| Shortcut | Action |
|----------|--------|
| `Cmd + C` (macOS) | Copy selection |
| `Ctrl + Shift + C` (Windows/Linux) | Copy selection |
| `Cmd + V` (macOS) | Paste |
| `Ctrl + V` (Windows/Linux) | Paste |
| `Shift + Insert` | Paste (X11 fallback) |
| `Cmd/Ctrl + Shift + V` | Paste Special (transform clipboard) |
| `Cmd/Ctrl + Shift + H` | Clipboard history |

**Mouse Selection:**

| Action | Effect |
|--------|--------|
| Click + Drag | Normal selection |
| Double-Click | Select word |
| Triple-Click | Select line |
| Cmd/Ctrl + Click | Open URL |
| Alt/Option + Click | Move cursor to position |
| Alt + Cmd/Ctrl + Drag | Rectangular selection |
| Middle-Click | Paste primary selection |

## Search

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + F` | Open search |
| `Enter` | Find next match |
| `Shift + Enter` | Find previous match |
| `Escape` | Close search |
| `Cmd/Ctrl + G` | Find next (global) |
| `Cmd/Ctrl + Shift + G` | Find previous (global) |

## Terminal Operations

| Shortcut | Action |
|----------|--------|
| `Ctrl + L` | Clear visible screen |
| `Ctrl + Shift + K` | Clear scrollback buffer |
| `Ctrl + Shift + S` | Take screenshot |
| `Cmd/Ctrl + Shift + R` | Toggle session recording |
| `Cmd/Ctrl + Shift + T` | Toggle maximize throughput mode |
| `Ctrl + Shift + F5` | Fix rendering (after monitor change) |

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

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + D` | Split horizontally |
| `Cmd/Ctrl + Shift + D` | Split vertically |
| `Cmd/Ctrl + Shift + W` | Close focused pane |
| `Cmd/Ctrl + Alt + Left` | Navigate to pane left |
| `Cmd/Ctrl + Alt + Right` | Navigate to pane right |
| `Cmd/Ctrl + Alt + Up` | Navigate to pane above |
| `Cmd/Ctrl + Alt + Down` | Navigate to pane below |
| `Cmd/Ctrl + Alt + Shift + Left` | Resize pane left |
| `Cmd/Ctrl + Alt + Shift + Right` | Resize pane right |
| `Cmd/Ctrl + Alt + Shift + Up` | Resize pane up |
| `Cmd/Ctrl + Alt + Shift + Down` | Resize pane down |

## Advanced Features

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + Shift + B` | Toggle background shader |
| `Cmd/Ctrl + Shift + U` | Toggle cursor shader |
| `Cmd/Ctrl + Shift + P` | Toggle profile drawer |
| `Cmd/Ctrl + Alt + I` | Toggle broadcast input |
| `Cmd/Ctrl + Alt + T` | Toggle tmux session picker |
| `Ctrl + ,` | Cycle cursor style |

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
- `toggle_session_logging`, `toggle_maximize_throughput`
- `toggle_background_shader`, `toggle_cursor_shader`
- `toggle_broadcast_input`, `toggle_profile_drawer`
- `toggle_tmux_session_picker`

**Terminal:**
- `clear_scrollback`, `reload_config`
- `increase_font_size`, `decrease_font_size`, `reset_font_size`
- `cycle_cursor_style`

## Related Documentation

- [README.md](../README.md) - Project overview
- [PROFILES.md](PROFILES.md) - Profile keyboard shortcuts
- [SEARCH.md](SEARCH.md) - Search keyboard shortcuts
