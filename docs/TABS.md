# Tabs

par-term provides a multi-tab interface for managing multiple terminal sessions within a single window.

## Table of Contents
- [Overview](#overview)
- [Creating and Closing Tabs](#creating-and-closing-tabs)
  - [Profile Selection on New Tab](#profile-selection-on-new-tab)
- [Reopening Closed Tabs](#reopening-closed-tabs)
- [Switching Tabs](#switching-tabs)
- [Reordering Tabs](#reordering-tabs)
  - [Drag-and-Drop Reordering](#drag-and-drop-reordering)
  - [Keyboard Reordering](#keyboard-reordering)
- [Duplicating Tabs](#duplicating-tabs)
- [Moving tabs between windows](#moving-tabs-between-windows)
- [Tab Icons](#tab-icons)
- [Tab Bar](#tab-bar)
  - [Tab Bar Position](#tab-bar-position)
  - [Visibility Modes](#visibility-modes)
  - [Tab Style Variants](#tab-style-variants)
  - [Tab Stretch](#tab-stretch)
  - [HTML Titles](#html-titles)
  - [Inactive Tab Outline-Only Mode](#inactive-tab-outline-only-mode)
- [Tab Title Mode](#tab-title-mode)
  - [Per-Pane Title Tracking](#per-pane-title-tracking)
  - [Remote Tab Title Format](#remote-tab-title-format)
  - [Renaming Tabs](#renaming-tabs)
  - [Session Persistence for Tab Names and Colors](#session-persistence-for-tab-names-and-colors)
- [Tab Appearance](#tab-appearance)
- [Known Fixes](#known-fixes)
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
    TabBar --> Icons[Tab Icons]
    TabBar --> HTML[HTML Titles]

    style TabManager fill:#e65100,stroke:#ff9800,stroke-width:3px,color:#ffffff
    style TabBar fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
    style Tabs fill:#0d47a1,stroke:#2196f3,stroke-width:2px,color:#ffffff
    style Sessions fill:#4a148c,stroke:#9c27b0,stroke-width:2px,color:#ffffff
    style Visibility fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Stretch fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Colors fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Icons fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style HTML fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
```

## Creating and Closing Tabs

| Action | macOS | Linux/Windows |
|--------|-------|---------------|
| New tab | `Cmd+T` | `Ctrl+Shift+T` |
| Close (smart) | `Cmd+W` | `Ctrl+Shift+W` |

`Close` is a smart action: it closes the active split pane when the tab has splits, or closes the tab itself when there is only one pane. On macOS, press `Cmd+Shift+W` to close the entire window. On Linux/Windows, the window close button or window manager shortcut closes the window.

New tabs inherit the working directory from the current tab (if shell integration is installed) or start in the configured startup directory.

### Profile Selection on New Tab

The new tab button (`+`) on the tab bar is a split button:

- **Left portion** (`+`): Creates a default tab (existing behavior)
- **Right portion** (`▾`): Opens a profile dropdown menu

The dropdown appears anchored to the top-right corner of the window and shows:
1. **Default** — creates a tab using global terminal config
2. All user profiles listed in order with their icons

Click a profile to open a new tab with that profile's settings (working directory, shell, command, tab name, etc.). Press `Escape` or click outside the dropdown to dismiss it.

> **📝 Note:** The chevron only appears when one or more profiles exist. Works in both horizontal and vertical tab bar layouts.

**Configuration:**

To make the new tab shortcut (`Cmd+T` / `Ctrl+Shift+T`) show the profile picker instead of immediately creating a default tab:

```yaml
new_tab_shortcut_shows_profiles: true  # default: false
```

**Settings UI:** Settings > Window > Tab Behavior > "New Tab Shortcut Shows Profiles"

### New Tab Position

Control where new tabs are inserted in the tab bar:

```yaml
new_tab_position: end          # default — append to the end of the tab bar
new_tab_position: after_active # insert immediately to the right of the active tab
```

`after_active` applies to all user-initiated new-tab actions: `Cmd+T` / `+` button / profile picker / custom `NewTab` actions. Session undo and arrangement restore always restore tabs to their original positions regardless of this setting.

**Settings UI:** Settings > Window > Tab Behavior > "New Tab Position"

## Reopening Closed Tabs

Accidentally closed tabs can be recovered using session undo:

| Action | macOS | Linux/Windows |
|--------|-------|---------------|
| Reopen closed tab | `Cmd + Z` | `Ctrl + Shift + Z` |

A toast notification appears after closing a tab, showing the undo keybinding and a countdown timer. Undo restores the tab at its original position with its title, custom color, and split pane layout.

For full details on session undo configuration and shell session preservation, see [Session Management](SESSION_MANAGEMENT.md).

## Switching Tabs

| Action | Shortcut |
|--------|----------|
| Next tab | `Cmd+Shift+]` or `Ctrl+Tab` |
| Previous tab | `Cmd+Shift+[` or `Ctrl+Shift+Tab` |
| Go to tab 1-9 | `Cmd+1` through `Cmd+9` (macOS) / `Alt+1` through `Alt+9` |
| Go to last tab | `Cmd+9` (macOS) / `Alt+9` |

## Reordering Tabs

Tabs can be reordered using drag-and-drop or keyboard shortcuts. Tab numbers update automatically after reordering to reflect the new positions.

### Drag-and-Drop Reordering

Click and drag any tab in the tab bar to move it to a new position:

1. **Press and hold** the mouse button on a tab to begin dragging
2. **Drag the tab** left or right to the desired position
3. **Release the mouse button** to drop the tab into place

**Visual Feedback:**
- A floating ghost tab follows the cursor during the drag with a semi-transparent preview of the tab being moved
- A blue insertion indicator line with a glow effect marks the drop target between tabs
- The dragged tab dims in its original position to indicate it is being moved

**Behavior Notes:**
- Press `Escape` to cancel a drag operation and return the tab to its original position
- Drag initiation is suppressed when only one tab exists in the window
- Dropping a tab on its original position has no effect

### Keyboard Reordering

| Action | Shortcut |
|--------|----------|
| Move tab left | `Cmd+Shift+Left` (macOS) / `Ctrl+Shift+Left` |
| Move tab right | `Cmd+Shift+Right` (macOS) / `Ctrl+Shift+Right` |

## Duplicating Tabs

Any tab can be duplicated via the context menu:

1. **Right-click** on any tab in the tab bar to open the context menu and select **Duplicate Tab**

To assign a keyboard shortcut, bind the `duplicate_tab` action in Settings > Keybindings. There is no default shortcut.

**Behavior:**
- The duplicated tab inherits the working directory of the source tab
- Any custom tab color set on the source tab carries over to the new tab
- Any custom tab icon set on the source tab carries over to the new tab
- Duplication works on any tab via context menu, not just the currently active tab
- The new tab starts a fresh shell session in the inherited directory

> **📝 Note:** The duplicated tab launches a new shell process. Running commands or session state from the original tab are not carried over.

## Moving tabs between windows

A tab (including its PTY, scrollback, running processes, split panes, session
logger, and prettifier state) can be moved to a different window without being
restarted.

**Context menu:**

- Right-click a tab → **Move Tab to New Window** spawns a new par-term window
  with the tab as its only occupant. The new window matches the source window's
  size and is placed 30 pixels down-and-right from the source, clamped to the
  source monitor's bounds. Disabled when the source window has only one tab
  (the operation would be a no-op) or when the source window is hosting a tmux
  gateway.
- Right-click a tab → **Move Tab to Window →** opens a submenu listing every
  other par-term window, labeled `Window N — <active tab title>`. Selecting a
  window transfers the tab there. If the source window becomes empty as a
  result, it closes. Disabled for tmux-gateway windows; hidden entirely when no
  other par-term windows exist.

**Keybinding:** Bind the `move_tab_to_new_window` action in
Settings → Keybindings. There is no default chord. The keybinding only covers
the "new window" case; the "move to existing window" case is menu-only because
keybindings cannot parameterize on a specific target window.

**Limitations:**

- Tmux gateway tabs and tmux display tabs cannot be moved — both would break
  the gateway/display link inside the source window.
- Per-window state (custom shader, assistant panel, window-level settings)
  does not travel with the tab. The moved tab adopts the destination window's
  settings.

## Tab Icons

Custom icons can be assigned to individual tabs for quick visual identification.

**Setting an Icon:**

1. **Right-click** on any tab in the tab bar to open the context menu
2. Select **Set Icon**
3. Choose an icon from the Nerd Font grid, or type any character or emoji into the text field

**Clearing an Icon:**

Right-click the tab and select **Clear Icon** to remove the custom icon and revert to the profile-assigned icon (if any) or no icon.

**Icon Precedence:**

```mermaid
graph LR
    Custom[Custom Icon<br/>via context menu]
    Profile[Profile Icon<br/>from profile config]
    None[No Icon]

    Custom -->|if cleared| Profile
    Profile -->|if none set| None

    style Custom fill:#e65100,stroke:#ff9800,stroke-width:3px,color:#ffffff
    style Profile fill:#0d47a1,stroke:#2196f3,stroke-width:2px,color:#ffffff
    style None fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
```

A custom icon set via the context menu takes precedence over any profile-assigned icon. When the custom icon is cleared, the tab falls back to the profile icon or displays no icon.

**Persistence:**
- Custom icons persist across session save/restore
- Custom icons are preserved in saved window arrangements (layouts)
- Custom icons carry over when duplicating a tab

## Tab Bar

### Tab Bar Position

The tab bar can be placed in three positions:

| Position | Description |
|----------|-------------|
| **Top** | Horizontal tab bar at the top of the window (default) |
| **Bottom** | Horizontal tab bar below terminal content |
| **Left** | Vertical sidebar with scrollable tab list |

The **Left** position renders a vertical sidebar with:
- Scrollable tab list with active indicator
- Drag-and-drop reordering support
- Configurable sidebar width (default 160px, range 100–300px)

```yaml
# Tab bar position: "top", "bottom", or "left"
tab_bar_position: top

# Sidebar width for left position (pixels)
tab_bar_width: 160.0
```

**Settings UI:** Settings > Window > Tab Bar > "Tab Bar Position"

Switching between positions takes effect immediately without restart.

### Visibility Modes

Control when the tab bar appears:

| Mode | Description |
|------|-------------|
| `always` | Tab bar always visible (default) |
| `when_multiple` | Show only when 2+ visible tabs exist (hidden tabs such as the tmux gateway tab do not count) |
| `never` | Tab bar never shown |

```yaml
tab_bar_mode: "always"
```

### Tab Style Variants

par-term includes 6 built-in tab style presets that apply coordinated color, size, and spacing adjustments:

| Style | Description |
|-------|-------------|
| **Dark** | Default dark theme with subtle contrast (default) |
| **Light** | Light background with darker text |
| **Compact** | Reduced height and spacing for minimal footprint |
| **Minimal** | Understated design with muted colors |
| **High Contrast** | Bold colors for maximum readability |
| **Automatic** | Switches between light/dark based on system theme |

```yaml
# Tab style preset
tab_style: dark  # dark, light, compact, minimal, high_contrast, automatic
```

**Settings UI:** Settings > Window > Tab Bar > "Tab Style"

Each preset adjusts the tab bar background, active/inactive colors, height, and spacing as a coordinated set. Individual settings can still be overridden after selecting a preset.

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
tab_min_width: 120.0
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

### Inactive Tab Outline-Only Mode

The `tab_inactive_outline_only` option renders inactive tabs with just a border stroke and no background fill. This produces a cleaner, more minimal look where only the active tab has a solid background. Hovered inactive tabs brighten the outline for visual feedback.

```yaml
tab_inactive_outline_only: true  # default: true
```

**Settings UI:** Settings > Window > Tab Bar > "Inactive tabs outline only"

```mermaid
graph LR
    Active["Active Tab<br/>(solid fill)"]
    Inactive1["Inactive Tab<br/>(outline only)"]
    Inactive2["Inactive Tab<br/>(outline only)"]
    Hovered["Hovered Tab<br/>(bright outline)"]

    style Active fill:#0d47a1,stroke:#2196f3,stroke-width:3px,color:#ffffff
    style Inactive1 fill:#1E1E1E,stroke:#78909c,stroke-width:2px,color:#B0B0B0
    style Inactive2 fill:#1E1E1E,stroke:#78909c,stroke-width:2px,color:#B0B0B0
    style Hovered fill:#1E1E1E,stroke:#E0E0E0,stroke-width:3px,color:#ffffff
```

When enabled (the default), inactive tabs show only the border stroke, and hovered inactive tabs increase the outline brightness instead of adding a fill. When disabled, inactive tabs render with a dimmed background fill matching their assigned tab color.

## Tab Title Mode

Control how tab titles are automatically updated:

| Mode | Description |
|------|-------------|
| `auto` | OSC title first, then working directory from shell integration, then keep default "Tab N" (default) |
| `osc_only` | Only update from explicit OSC escape sequences; never auto-set from CWD |

```yaml
tab_title_mode: auto
```

**Settings UI:** Settings > Window > Tab Bar > "Tab title mode"

### Per-Pane Title Tracking

When a tab contains split panes, each pane tracks its own title independently. The tab bar displays the title of the currently focused pane.

**How it works:**

- Each pane stores its own last-known title from OSC sequences and shell-integration CWD
- `Tab::update_title()` iterates all panes each frame and updates each pane's title from its terminal
- Switching focus between split panes instantly reflects the correct title in the tab bar without waiting for new terminal output
- Local hostname and home-directory lookups are performed once per frame (not once per pane) to avoid redundant syscalls

### Remote Tab Title Format

When shell integration detects that the terminal is connected to a remote host (via SSH or similar), the tab title can automatically reflect the remote context. Control the format with `remote_tab_title_format`:

| Value | Format | Example |
|-------|--------|---------|
| `user_at_host` (default) | `user@hostname` | `deploy@prod.example.com` |
| `host` | hostname only | `prod.example.com` |
| `host_and_cwd` | `hostname:~/cwd` | `prod.example.com:~/app` |

```yaml
remote_tab_title_format: user_at_host  # user_at_host | host | host_and_cwd

# When true (default), explicit OSC title sequences override this format.
remote_tab_title_osc_priority: true
```

**Settings UI:** Settings > Window > Tab Bar > "Remote Tab Title Format"

> **📝 Note:** Requires shell integration on the remote host to emit OSC 7 or OSC 1337 RemoteHost sequences. See [Integrations](INTEGRATIONS.md).

### Renaming Tabs

Right-click any tab and select **Rename Tab** to set a custom name. Manually named tabs are static — they are never auto-updated regardless of the title mode setting.

To revert a renamed tab to automatic title updates, right-click and rename with a blank name.

### Session Persistence for Tab Names and Colors

User-set tab names and custom tab colors are preserved across:

- **Session save/restore** -- closing and reopening par-term restores custom names and colors
- **Window arrangements** -- saved layouts retain per-tab names and colors
- **Tab duplication** -- duplicated tabs inherit the source tab's custom name and color

This persistence also applies to custom tab icons (see [Tab Icons](#tab-icons)).

## Tab Appearance

Customize the visual style of tabs:

### Layout

| Setting | Description | Default |
|---------|-------------|---------|
| `tab_bar_height` | Height in pixels | `28.0` |
| `tab_bar_width` | Sidebar width for left position (pixels) | `160.0` |
| `tab_min_width` | Minimum tab width | `120.0` |
| `max_tabs` | Maximum tabs per window (0 = unlimited) | `0` |
| `tab_stretch_to_fill` | Stretch tabs to fill bar width | `true` |
| `tab_show_close_button` | Show close button on tabs | `true` |
| `tab_show_index` | Show tab index numbers | `false` |
| `tab_border_width` | Tab border width in pixels | varies |

### Colors (RGB arrays)

| Setting | Description |
|---------|-------------|
| `tab_bar_background` | Tab bar background |
| `tab_active_background` | Active tab background |
| `tab_inactive_background` | Inactive tab background |
| `tab_hover_background` | Hovered tab background |
| `tab_active_text` | Active tab text color |
| `tab_inactive_text` | Inactive tab text color |
| `tab_active_indicator` | Active tab underline/indicator |
| `tab_activity_indicator` | Activity dot color |
| `tab_bell_indicator` | Bell indicator color |
| `tab_close_button` | Close button color |
| `tab_close_button_hover` | Close button hover color |
| `tab_border_color` | Tab border color |

### Inactive Tab Styling

| Setting | Description | Default |
|---------|-------------|---------|
| `tab_inactive_outline_only` | Outline-only mode (no fill) | `true` |
| `dim_inactive_tabs` | Dim inactive tabs with reduced opacity | `true` |
| `inactive_tab_opacity` | Opacity for dimmed inactive tabs (0.0-1.0) | varies |

### Automatic Style Sub-Styles

When `tab_style` is set to `automatic`, the light and dark sub-styles can be overridden:

```yaml
tab_style: automatic
light_tab_style: light     # Style used in light mode
dark_tab_style: dark       # Style used in dark mode
```

### Behavior

| Setting | Description | Default |
|---------|-------------|---------|
| `tab_inherit_cwd` | New tab inherits working directory from active tab | `true` |
| `tab_html_titles` | Render limited HTML in tab titles | `false` |

**Example Configuration:**

```yaml
tab_bar_height: 32.0
tab_bar_background: [40, 40, 40]
tab_active_background: [60, 60, 60]
tab_inactive_background: [40, 40, 40]
tab_stretch_to_fill: true
tab_min_width: 120.0
tab_html_titles: true
tab_inactive_outline_only: true
dim_inactive_tabs: true
tab_border_color: [80, 80, 80]
tab_border_width: 1.0
```

## Known Fixes

The following tab bar rendering issues have been resolved:

- **Rounded Corner Stroke Thickness** -- Tab bar rounded corners now render with consistent stroke thickness, eliminating visual artifacts at corner boundaries
- **First Tab Border Clipping** -- The leftmost tab no longer clips its left border against the tab bar edge
- **Progress Bar Overlapping Tab Bar** -- Terminal progress bars no longer visually overlap or bleed into the tab bar region
- **New Tab Button Clipped Off Right Edge** -- A width budget miscalculation caused the new-tab button (and chevron) to be clipped past the right edge of the tab bar; the layout now correctly accounts for padding in its width budget
- **Assistant Panel Overlapping Tab Bar** -- When the assistant panel was open, tab bar content could render underneath the overlay; the tab bar now reserves the assistant panel width so tabs and buttons shrink to fit
- **Rounded Tab Border Consistency** -- Tab pill draw rects are snapped to pixel boundaries with a centered stroke so rounded and straight border segments render with uniform thickness

## Configuration

Complete tab configuration reference:

```yaml
# Tab bar position: "top", "bottom", "left"
tab_bar_position: top

# Tab bar visibility: "always", "when_multiple", "never"
tab_bar_mode: "always"

# Tab title mode: "auto", "osc_only"
tab_title_mode: auto

# Remote tab title format: "user_at_host", "host", "host_and_cwd"
remote_tab_title_format: user_at_host
# When true (default), explicit OSC titles override remote_tab_title_format
remote_tab_title_osc_priority: true

# Tab style preset: "dark", "light", "compact", "minimal", "high_contrast", "automatic"
tab_style: dark
# Sub-styles for automatic mode
light_tab_style: light
dark_tab_style: dark

# Tab bar dimensions
tab_bar_height: 28.0
tab_bar_background: [40, 40, 40]
tab_bar_width: 160.0  # Sidebar width for left position

# Tab colors (RGB arrays)
tab_active_background: [60, 60, 60]
tab_inactive_background: [40, 40, 40]
tab_hover_background: [50, 50, 50]
tab_active_text: [220, 220, 220]
tab_inactive_text: [160, 160, 160]
tab_active_indicator: [66, 165, 245]
tab_activity_indicator: [255, 193, 7]
tab_bell_indicator: [244, 67, 54]
tab_close_button: [150, 150, 150]
tab_close_button_hover: [255, 255, 255]
tab_border_color: [80, 80, 80]
tab_border_width: 1.0

# Inactive tab rendering
tab_inactive_outline_only: true
dim_inactive_tabs: true
inactive_tab_opacity: 0.6

# Tab sizing and layout
tab_stretch_to_fill: true
tab_min_width: 120.0
tab_show_close_button: true
tab_show_index: false
max_tabs: 0  # 0 = unlimited

# Tab titles
tab_html_titles: false
tab_inherit_cwd: true

# Profile selection
show_profile_drawer_button: false
new_tab_shortcut_shows_profiles: false  # Show profile picker on Cmd+T
new_tab_position: end  # "end" or "after_active"
```

## Related Documentation

- [Keyboard Shortcuts](KEYBOARD_SHORTCUTS.md) - Tab navigation shortcuts
- [Session Management](SESSION_MANAGEMENT.md) - Reopen closed tabs and session restore
- [Profiles](PROFILES.md) - Open profiles in new tabs
- [SSH Host Management](SSH.md) - SSH profile-based tab creation
- [Window Management](WINDOW_MANAGEMENT.md) - Window and tab interaction
- [Integrations](INTEGRATIONS.md) - Shell integration for directory inheritance
