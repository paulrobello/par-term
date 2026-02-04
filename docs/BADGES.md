# Badges

par-term provides an iTerm2-style badge system for displaying dynamic session information as semi-transparent text overlays in the terminal corner.

## Table of Contents
- [Overview](#overview)
- [Enabling Badges](#enabling-badges)
- [Badge Format](#badge-format)
  - [Static Text](#static-text)
  - [Dynamic Variables](#dynamic-variables)
- [Appearance](#appearance)
- [Position](#position)
- [OSC 1337 Support](#osc-1337-support)
- [Configuration](#configuration)
- [Related Documentation](#related-documentation)

## Overview

Badges display contextual information about your terminal session without interfering with terminal content:

```mermaid
graph TD
    Badge[Badge System]
    Format[Badge Format]
    Variables[Dynamic Variables]
    Appearance[Appearance Settings]
    Position[Position Settings]
    OSC[OSC 1337 Protocol]

    Badge --> Format
    Badge --> Appearance
    Badge --> Position
    Badge --> OSC

    Format --> Variables
    Variables --> Session[Session Info]
    Variables --> Command[Command Info]
    Variables --> Terminal[Terminal Info]

    style Badge fill:#e65100,stroke:#ff9800,stroke-width:3px,color:#ffffff
    style Format fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
    style Variables fill:#0d47a1,stroke:#2196f3,stroke-width:2px,color:#ffffff
    style Appearance fill:#4a148c,stroke:#9c27b0,stroke-width:2px,color:#ffffff
    style Position fill:#880e4f,stroke:#c2185b,stroke-width:2px,color:#ffffff
    style OSC fill:#ff6f00,stroke:#ffa726,stroke-width:2px,color:#ffffff
    style Session fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Command fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Terminal fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
```

## Enabling Badges

**Method 1: Settings UI**

1. Press `F12` to open Settings
2. Navigate to the **Badge** tab
3. Enable "Show badge"
4. Configure format, appearance, and position

**Method 2: Configuration File**

Add to `~/.config/par-term/config.yaml`:

```yaml
badge_enabled: true
badge_format: "\\(session.username)@\\(session.hostname)"
```

## Badge Format

### Static Text

Use any static text in the badge format:

```yaml
badge_format: "Production Server"
```

### Dynamic Variables

Insert dynamic values using the `\(session.*)` syntax:

| Variable | Description | Example Output |
|----------|-------------|----------------|
| `\(session.hostname)` | System hostname | `macbook-pro` |
| `\(session.username)` | Current username | `alice` |
| `\(session.path)` | Current working directory | `/Users/alice/project` |
| `\(session.job)` | Current foreground job | `vim` |
| `\(session.last_command)` | Last executed command | `git status` |
| `\(session.profile_name)` | Active profile name | `Development` |
| `\(session.tty)` | TTY device name | `/dev/ttys001` |
| `\(session.columns)` | Terminal width in columns | `120` |
| `\(session.rows)` | Terminal height in rows | `40` |
| `\(session.bell_count)` | Number of bells received | `3` |
| `\(session.selection)` | Currently selected text | `hello world` |
| `\(session.tmux_pane_title)` | tmux pane title (if connected) | `main:0` |

**Example Formats:**

```yaml
# User and host
badge_format: "\\(session.username)@\\(session.hostname)"

# Current directory
badge_format: "📁 \\(session.path)"

# Multiple lines
badge_format: "\\(session.hostname)\n\\(session.path)"

# With static text
badge_format: "🖥️ \\(session.columns)x\\(session.rows)"
```

## Appearance

Configure the visual style of badges:

| Setting | Description | Default |
|---------|-------------|---------|
| `badge_color` | RGB color array | `[255, 0, 0]` (red) |
| `badge_color_alpha` | Opacity (0.0-1.0) | `0.2` |
| `badge_font` | Font family | System default |
| `badge_font_bold` | Use bold weight | `true` |

**Example Configuration:**

```yaml
badge_color: [100, 200, 255]  # Light blue
badge_color_alpha: 0.15        # 15% opacity
badge_font: "JetBrains Mono"
badge_font_bold: true
```

## Position

Control badge placement within the terminal:

| Setting | Description | Default |
|---------|-------------|---------|
| `badge_top_margin` | Distance from top (0.0-1.0) | `0.0` |
| `badge_right_margin` | Distance from right (0.0-1.0) | `0.02` |
| `badge_max_width` | Maximum width fraction | `0.5` |
| `badge_max_height` | Maximum height fraction | `0.2` |

```mermaid
graph TD
    subgraph Terminal Window
        TopMargin[Top Margin]
        Badge[Badge Area]
        RightMargin[Right Margin]
        Content[Terminal Content]
    end

    style Badge fill:#e65100,stroke:#ff9800,stroke-width:2px,color:#ffffff
    style TopMargin fill:#37474f,stroke:#78909c,stroke-width:1px,color:#ffffff
    style RightMargin fill:#37474f,stroke:#78909c,stroke-width:1px,color:#ffffff
    style Content fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
```

**Example:**

```yaml
badge_top_margin: 0.02    # 2% from top
badge_right_margin: 0.05  # 5% from right
badge_max_width: 0.4      # Max 40% of terminal width
badge_max_height: 0.15    # Max 15% of terminal height
```

## OSC 1337 Support

par-term supports iTerm2's OSC 1337 escape sequence for programmatically setting the badge format:

```bash
# Set badge format from command line
printf "\033]1337;SetBadgeFormat=%s\007" "$(echo -n "My Badge" | base64)"
```

**Security:** Badge format changes via OSC 1337 are validated to prevent injection attacks. Only safe variable references are allowed.

## Configuration

Complete configuration reference:

```yaml
# Enable/disable badges
badge_enabled: true

# Badge format with variables
badge_format: "\\(session.username)@\\(session.hostname)"

# Appearance
badge_color: [255, 100, 100]
badge_color_alpha: 0.2
badge_font: "SF Mono"
badge_font_bold: true

# Position (values are fractions of terminal size)
badge_top_margin: 0.0
badge_right_margin: 0.02
badge_max_width: 0.5
badge_max_height: 0.2
```

## Related Documentation

- [Profiles](PROFILES.md) - Terminal profiles with per-profile settings
- [Window Management](WINDOW_MANAGEMENT.md) - Window display options
- [Keyboard Shortcuts](KEYBOARD_SHORTCUTS.md) - Access Settings UI with F12
