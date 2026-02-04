# Tabs

par-term provides a multi-tab interface for managing multiple terminal sessions within a single window.

## Table of Contents
- [Overview](#overview)
- [Creating and Closing Tabs](#creating-and-closing-tabs)
- [Switching Tabs](#switching-tabs)
- [Tab Bar](#tab-bar)
  - [Visibility Modes](#visibility-modes)
  - [Tab Stretch](#tab-stretch)
  - [HTML Titles](#html-titles)
- [Tab Appearance](#tab-appearance)
- [Configuration](#configuration)
- [Related Documentation](#related-documentation)

## Overview

The tab system manages multiple terminal sessions:

```mermaid
graph TD
    TabManager[Tab Manager]
    TabBar[Tab Bar UI]
    Tabs[Terminal Tabs]
    Sessions[Shell Sessions]

    TabManager --> TabBar
    TabManager --> Tabs
    Tabs --> Sessions

    TabBar --> Visibility[Visibility Mode]
    TabBar --> Stretch[Tab Stretch]
    TabBar --> Colors[Tab Colors]
    TabBar --> HTML[HTML Titles]

    style TabManager fill:#e65100,stroke:#ff9800,stroke-width:3px,color:#ffffff
    style TabBar fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
    style Tabs fill:#0d47a1,stroke:#2196f3,stroke-width:2px,color:#ffffff
    style Sessions fill:#4a148c,stroke:#9c27b0,stroke-width:2px,color:#ffffff
    style Visibility fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Stretch fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Colors fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style HTML fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
```

## Creating and Closing Tabs

| Action | Shortcut |
|--------|----------|
| New tab | `Cmd+T` (macOS) / `Ctrl+T` |
| Close tab | `Cmd+W` (macOS) / `Ctrl+W` |
| Close window | `Cmd+Shift+W` (macOS) / `Ctrl+Shift+W` |

New tabs inherit the working directory from the current tab (if shell integration is installed) or start in the configured startup directory.

## Switching Tabs

| Action | Shortcut |
|--------|----------|
| Next tab | `Cmd+Shift+]` or `Ctrl+Tab` |
| Previous tab | `Cmd+Shift+[` or `Ctrl+Shift+Tab` |
| Go to tab 1-9 | `Cmd+1` through `Cmd+9` (macOS) / `Ctrl+1` through `Ctrl+9` |
| Go to last tab | `Cmd+9` (macOS) / `Ctrl+9` |

## Tab Bar

### Visibility Modes

Control when the tab bar appears:

| Mode | Description |
|------|-------------|
| `always` | Tab bar always visible |
| `when_multiple` | Show only when 2+ tabs exist (default) |
| `never` | Tab bar never shown |

```yaml
tab_bar_visibility: "when_multiple"
```

### Tab Stretch

By default, tabs stretch to fill the available tab bar width while respecting minimum width constraints.

**Behavior:**
- Tabs expand equally to fill the bar
- Each tab respects the `tab_min_width` setting
- When too many tabs exist, they compress to minimum width

```yaml
# Enable/disable tab stretching (default: true)
tab_stretch_to_fill: true

# Minimum tab width in pixels
tab_min_width: 100.0
```

### HTML Titles

Tab titles support limited HTML markup for styling:

**Supported Tags:**
- `<b>` - Bold text
- `<i>` - Italic text
- `<u>` - Underlined text
- `<span style="color:...">` - Colored text

**Examples:**

```bash
# Set tab title with bold text (OSC 0 or OSC 2)
printf "\033]0;<b>Important</b> Server\007"

# Colored text
printf "\033]0;<span style=\"color:red\">Production</span>\007"

# Combined formatting
printf "\033]0;<b><span style=\"color:green\">✓</span></b> Tests Passing\007"
```

**Enabling HTML Titles:**

```yaml
tab_html_titles: true
```

> **⚠️ Note:** When `tab_html_titles` is disabled, HTML tags are stripped from titles.

## Tab Appearance

Customize the visual style of tabs:

| Setting | Description | Default |
|---------|-------------|---------|
| `tab_bar_height` | Height in pixels | `28.0` |
| `tab_bar_background` | Background RGBA | `[30, 30, 30, 255]` |
| `tab_active_color` | Active tab color | `"blue"` |
| `tab_inactive_color` | Inactive tab color | `"gray"` |
| `tab_min_width` | Minimum tab width | `100.0` |
| `tab_max_tabs` | Maximum tabs allowed | `20` |

**Available Tab Colors:**

`red`, `orange`, `yellow`, `green`, `blue`, `purple`, `pink`, `teal`, `gray`, `white`, `none`

**Example Configuration:**

```yaml
tab_bar_height: 32.0
tab_bar_background: [25, 25, 25, 255]
tab_active_color: "teal"
tab_inactive_color: "gray"
tab_stretch_to_fill: true
tab_min_width: 120.0
tab_html_titles: true
```

## Configuration

Complete tab configuration reference:

```yaml
# Tab bar visibility: "always", "when_multiple", "never"
tab_bar_visibility: "when_multiple"

# Tab bar appearance
tab_bar_height: 28.0
tab_bar_background: [30, 30, 30, 255]

# Tab colors
tab_active_color: "blue"
tab_inactive_color: "gray"

# Tab sizing
tab_stretch_to_fill: true
tab_min_width: 100.0
tab_max_tabs: 20

# Tab titles
tab_html_titles: true
```

## Related Documentation

- [Keyboard Shortcuts](KEYBOARD_SHORTCUTS.md) - Tab navigation shortcuts
- [Profiles](PROFILES.md) - Open profiles in new tabs
- [Window Management](WINDOW_MANAGEMENT.md) - Window and tab interaction
- [Integrations](INTEGRATIONS.md) - Shell integration for directory inheritance
