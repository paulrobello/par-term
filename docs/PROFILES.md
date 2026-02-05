# Profiles

par-term provides a profile system for saving and quickly launching terminal sessions with custom configurations, similar to iTerm2's profile system.

## Table of Contents
- [Overview](#overview)
- [Profile Settings](#profile-settings)
- [Managing Profiles](#managing-profiles)
  - [Profile Drawer](#profile-drawer)
  - [Profile Modal](#profile-modal)
- [Creating Profiles](#creating-profiles)
- [Using Profiles](#using-profiles)
- [Default Startup Directory](#default-startup-directory)
- [Storage](#storage)
- [Related Documentation](#related-documentation)

## Overview

Profiles allow you to save terminal configurations for quick access:

```mermaid
graph TD
    Profiles[Profile System]
    Manager[ProfileManager]
    Drawer[Profile Drawer]
    Modal[Profile Modal]
    Storage[profiles.yaml]
    Session[Terminal Session]

    Profiles --> Manager
    Manager --> Drawer
    Manager --> Modal
    Manager --> Storage

    Drawer -->|Open Profile| Session
    Modal -->|Create/Edit| Manager

    style Profiles fill:#e65100,stroke:#ff9800,stroke-width:3px,color:#ffffff
    style Manager fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
    style Drawer fill:#0d47a1,stroke:#2196f3,stroke-width:2px,color:#ffffff
    style Modal fill:#4a148c,stroke:#9c27b0,stroke-width:2px,color:#ffffff
    style Storage fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Session fill:#880e4f,stroke:#c2185b,stroke-width:2px,color:#ffffff
```

## Profile Settings

Each profile can customize the following:

| Setting | Description | Required |
|---------|-------------|----------|
| **Name** | Display name for the profile | Yes |
| **Icon** | Emoji or icon identifier | No |
| **Working Directory** | Initial directory for the session | No |
| **Command** | Custom command (instead of default shell) | No |
| **Command Arguments** | Arguments for the custom command | No |
| **Tab Name** | Custom name for the terminal tab | No |
| **Tmux Session Patterns** | Glob patterns for auto-switching (e.g., `work-*`) | No |
| **Badge Text** | Custom badge format for this profile | No |
| **Badge Appearance** | Override badge color, font, position, size | No |

## Managing Profiles

### Profile Drawer

The profile drawer provides quick access to your profiles from the right side of the window.

**Opening the Drawer:**
- Press `Cmd+Shift+P` (macOS) or `Ctrl+Shift+P` (Windows/Linux)
- Or click the toggle button on the right edge of the window

**Drawer Features:**
- Collapsible panel (220px wide when expanded, 12px when collapsed)
- Scrollable profile list with icons
- Single-click to select, double-click to open
- Indicator dots (`...`) for profiles with custom settings
- Quick action buttons: **Open** and **Manage**

```mermaid
flowchart LR
    Toggle[Toggle Button]
    Drawer[Profile Drawer]
    List[Profile List]
    Actions[Action Buttons]

    Toggle -->|Click| Drawer
    Drawer --> List
    Drawer --> Actions
    Actions -->|Open| Launch[Launch Session]
    Actions -->|Manage| Modal[Profile Modal]

    style Toggle fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Drawer fill:#0d47a1,stroke:#2196f3,stroke-width:2px,color:#ffffff
    style List fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Actions fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
    style Launch fill:#880e4f,stroke:#c2185b,stroke-width:2px,color:#ffffff
    style Modal fill:#4a148c,stroke:#9c27b0,stroke-width:2px,color:#ffffff
```

### Profile Modal

The profile modal provides full CRUD (Create, Read, Update, Delete) operations.

**Opening the Modal:**
- Click **Manage** in the profile drawer
- Or use the Settings UI

**Modal Views:**

1. **List View** - Shows all profiles with:
   - Up/Down reorder buttons
   - Edit (pencil) button
   - Delete (trash) button
   - Unsaved changes indicator

2. **Edit/Create Form** - Fields for all profile settings with:
   - Name validation (required)
   - Browse button for working directory
   - Help text for optional fields

3. **Delete Confirmation** - Safety dialog before deletion

## Creating Profiles

**Step-by-step:**

1. Open the profile drawer (`Cmd/Ctrl+Shift+P`)
2. Click **Manage**
3. Click **+ New Profile**
4. Fill in the profile settings:
   - **Name** (required): Give your profile a descriptive name
   - **Icon**: Add an emoji for visual identification
   - **Working Directory**: Set the starting directory
   - **Command**: Override the default shell (optional)
   - **Arguments**: Space-separated command arguments
   - **Tab Name**: Custom tab title (optional)
5. Click **Save Profile**
6. Click **Save** to persist changes

**Example Profiles:**

| Profile | Command | Working Dir | Use Case |
|---------|---------|-------------|----------|
| Development | - | `~/projects` | General development |
| SSH Server | `ssh user@server` | - | Remote connection |
| Docker Shell | `docker exec -it container bash` | - | Container access |
| Python REPL | `python3` | `~/scripts` | Interactive Python |

## Using Profiles

**Launch a Profile:**

1. Open the profile drawer (`Cmd/Ctrl+Shift+P`)
2. Double-click a profile, or
3. Select a profile and click **Open**

**What Happens:**
- A new tab opens with the profile's configuration
- Working directory is set if specified
- Custom command runs (or default shell if not specified)
- Tab name updates if specified

## Default Startup Directory

When opening a new terminal without a profile, par-term uses the configured startup directory mode.

### Startup Modes

| Mode | Description |
|------|-------------|
| `home` | Start in home directory (default) |
| `previous` | Start in last session's working directory |
| `custom` | Start in a user-specified directory |

### Configuration

```yaml
# Startup mode: "home", "previous", or "custom"
startup_directory_mode: "home"

# Custom directory (only used when mode is "custom")
startup_directory: "/path/to/directory"
```

### Settings UI

1. Press `F12` to open Settings
2. Navigate to **Terminal** → **Shell**
3. Find the **Startup Directory** section
4. Select mode and configure path if needed

### Priority

Directory selection follows this priority:

1. **Profile working directory** - If launching a profile with a directory set
2. **Legacy `working_directory`** - If set in config (for backwards compatibility)
3. **Startup directory mode** - Based on `startup_directory_mode` setting
4. **Home directory** - Fallback if configured path doesn't exist

> **📝 Note:** The `previous` mode requires shell integration to track directory changes during a session.

## Tmux Profile Auto-Switching

Profiles can automatically apply when connecting to tmux sessions with matching names.

### Configuration

Add `tmux_session_patterns` to a profile with glob patterns:

```yaml
- id: 550e8400-e29b-41d4-a716-446655440000
  name: Production
  tmux_session_patterns:
    - "*-prod"
    - "*-production"
    - "prod-*"
  badge_text: "🔴 PROD"
  badge_color: [255, 0, 0]
```

### Pattern Matching

| Pattern | Matches |
|---------|---------|
| `dev-*` | `dev-api`, `dev-frontend`, etc. |
| `*-prod` | `api-prod`, `web-prod`, etc. |
| `*server*` | `webserver`, `api-server-1`, etc. |
| `main` | Exact match only |

- Patterns are case-insensitive
- First matching profile wins (check profile order)
- Profile is cleared when tmux session ends

### Settings UI

1. Open profile editor
2. Find "Auto-Switch Tmux" field
3. Enter comma-separated patterns: `work-*, *-production`

## Per-Profile Badge Configuration

Profiles can override global badge settings for visual differentiation per environment.

### Available Overrides

| Setting | Description |
|---------|-------------|
| `badge_text` | Custom badge format string |
| `badge_color` | RGB color override |
| `badge_color_alpha` | Opacity override (0.0-1.0) |
| `badge_font` | Font family override |
| `badge_font_bold` | Bold toggle override |
| `badge_top_margin` | Position override |
| `badge_right_margin` | Position override |
| `badge_max_width` | Size constraint override |
| `badge_max_height` | Size constraint override |

### Example: Environment Indicators

```yaml
# Production profile - red badge
- name: Production
  badge_text: "🔴 PROD"
  badge_color: [255, 0, 0]
  badge_color_alpha: 0.3

# Development profile - green badge
- name: Development
  badge_text: "🟢 DEV"
  badge_color: [0, 255, 0]
  badge_color_alpha: 0.2

# Staging profile - yellow badge
- name: Staging
  badge_text: "🟡 STAGING"
  badge_color: [255, 200, 0]
```

### Settings UI

1. Open profile editor (double-click profile or click edit)
2. Expand "Badge Appearance" section
3. Check boxes to enable individual overrides
4. Configure color, font, margins, and size as needed

## Storage

Profiles are stored in YAML format:

**Location:** `~/.config/par-term/profiles.yaml`

**Format:**
```yaml
- id: 550e8400-e29b-41d4-a716-446655440000
  name: Development
  working_directory: ~/projects
  icon: "\U0001F4BB"
  order: 0
- id: 6fa459ea-ee8a-3ca4-894e-db77e160355e
  name: SSH Server
  command: ssh
  command_args:
    - user@server
  icon: "\U0001F310"
  order: 1
```

**Key Points:**
- UUIDs uniquely identify each profile
- Order field controls display sequence
- Changes save immediately when clicking **Save** in the modal

## Related Documentation

- [Keyboard Shortcuts](KEYBOARD_SHORTCUTS.md) - Profile keyboard shortcuts
- [Tabs](TABS.md) - Tab management and directory inheritance
- [Integrations](INTEGRATIONS.md) - Shell integration for directory tracking
