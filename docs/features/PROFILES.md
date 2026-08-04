# Profiles

par-term provides a profile system for saving and quickly launching terminal sessions with custom configurations, similar to iTerm2's profile system.

## Table of Contents
- [Overview](#overview)
- [Profile Settings](#profile-settings)
- [Managing Profiles](#managing-profiles)
  - [Settings UI](#settings-ui)
  - [Profile Drawer](#profile-drawer)
- [Creating Profiles](#creating-profiles)
  - [Profile Icon Picker](#profile-icon-picker)
- [Using Profiles](#using-profiles)
- [Auto-Switching](#auto-switching)
  - [Directory-Based Profile Switching](#directory-based-profile-switching)
  - [Tmux Profile Auto-Switching](#tmux-profile-auto-switching)
  - [Hostname-Based Switching](#hostname-based-switching)
  - [Auto-Switch Priority](#auto-switch-priority)
  - [Auto-Switch Visual Application](#auto-switch-visual-application)
  - [Profile Commands and Confirmation](#profile-commands-and-confirmation)
- [Tmux Auto-Connect](#tmux-auto-connect)
- [Default Startup Directory](#default-startup-directory)
- [Per-Profile Badge Configuration](#per-profile-badge-configuration)
- [Per-Profile Shader Settings](#per-profile-shader-settings)
- [Per-Pane Background Settings](#per-pane-background-settings)
  - [Available Controls](#available-controls)
  - [Darken Control](#darken-control)
  - [Real-Time Preview](#real-time-preview)
  - [Settings UI](#settings-ui-1)
- [Dynamic Profiles](#dynamic-profiles)
  - [Configuration](#configuration-1)
  - [Background Refresh](#background-refresh)
  - [Local Cache](#local-cache)
  - [Conflict Resolution](#conflict-resolution)
  - [Security](#security)
  - [Visual Indicators](#visual-indicators)
  - [Keybinding](#keybinding)
  - [Dynamic Profiles Settings UI](#dynamic-profiles-settings-ui)
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
| **Icon** | Nerd Font icon or custom text identifier | No |
| **Working Directory** | Initial directory for the session | No |
| **Command** | Custom command (instead of default shell) | No |
| **Command Arguments** | Arguments for the custom command | No |
| **Tab Name** | Custom name for the terminal tab | No |
| **Shell** | Specific shell for this profile (overrides global) | No |
| **Login Shell** | Override global login shell setting (None/true/false) | No |
| **Tags** | Comma-separated tags for organization and filtering | No |
| **Parent Profile** | Inherit settings from another profile | No |
| **Keyboard Shortcut** | Quick-launch shortcut (e.g., `Cmd+1`) | No |
| **SSH Host** | SSH hostname for remote connections | No |
| **SSH User** | SSH username | No |
| **SSH Port** | SSH port number | No |
| **SSH Identity File** | Path to SSH identity/key file | No |
| **SSH Extra Args** | Additional SSH command-line arguments | No |
| **Hostname Patterns** | Glob patterns for SSH-based auto-switching | No |
| **Directory Patterns** | Glob patterns for CWD-based auto-switching | No |
| **Tmux Session Patterns** | Glob patterns for auto-switching (e.g., `work-*`) | No |
| **Tmux Session** | tmux session to auto-connect when this profile opens (uses create-or-attach) | No |
| **Tmux Mode** | Connection mode: Control Mode (full integration) or Normal (plain tmux in PTY) | No |
| **Badge Text** | Custom badge format for this profile | No |
| **Badge Appearance** | Override badge color, font, position, size | No |
| **Background Shader** | Custom shader path/name override | No |
| **Shader Brightness** | Shader brightness override | No |
| **Shader Text Opacity** | Shader text opacity override | No |
| **Shader Animation Speed** | Shader animation speed override | No |
| **Shader Textures** | Custom iChannel0-3 texture set override | No |

## Managing Profiles

### Settings UI

Profile management is embedded in the Settings window under the **Profiles** tab. Open Settings (`F12` or `Cmd/Ctrl + ,`) and navigate to the Profiles tab.

**Profile Management Features:**
- Create, edit, delete, and reorder profiles inline
- Up/Down buttons to change profile order
- Edit (pencil) and Delete (trash) buttons per profile
- Unsaved changes indicator

The profile drawer's **Manage** button and the menu's **Manage Profiles** action both open the Settings window to the Profiles tab.

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
    Actions -->|Manage| Settings[Settings > Profiles]

    style Toggle fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Drawer fill:#0d47a1,stroke:#2196f3,stroke-width:2px,color:#ffffff
    style List fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Actions fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
    style Launch fill:#880e4f,stroke:#c2185b,stroke-width:2px,color:#ffffff
    style Settings fill:#4a148c,stroke:#9c27b0,stroke-width:2px,color:#ffffff
```

## Creating Profiles

**Step-by-step:**

1. Open the profile drawer (`Cmd/Ctrl+Shift+P`)
2. Click **Manage**
3. Click **+ New Profile**
4. Fill in the profile settings:
   - **Name** (required): Give your profile a descriptive name
   - **Icon**: Add a Nerd Font icon for visual identification
   - **Working Directory**: Set the starting directory
   - **Command**: Override the default shell (optional)
   - **Arguments**: Space-separated command arguments
   - **Tab Name**: Custom tab title (optional)
5. Optionally click the icon picker button to choose a Nerd Font icon
6. Click **Save Profile**
7. Click **Save** to persist changes

### Profile Icon Picker

The profile icon field includes an icon picker popup with ~200 curated Nerd Font icons organized in 14 categories. Nerd Font icons render reliably in the egui-based settings UI and each icon shows a descriptive tooltip on hover.

| Category | Description | Example Icons |
|----------|-------------|---------------|
| Terminal | Shells and terminal emulators | Terminal, Bash, PowerShell, tmux, Prompt |
| Dev & Tools | Languages and development tools | Code, GitHub, Python, Rust, TypeScript, Go, C/C++ |
| Files & Data | Files, folders, and storage | File, Folder, Database, Save, Package |
| Network & Cloud | Networking and cloud services | Globe, WiFi, Cloud, Server, SSH, AWS |
| Security | Locks, keys, and access control | Lock, Shield, Key, Eye, Warning |
| Git & VCS | Version control systems | Branch, Merge, Commit, GitHub, GitLab, BitBucket |
| Weather & Nature | Weather and natural elements | Sun, Moon, Snowflake, Lightning, Rainy, Tree, Leaf |
| Containers & Infra | Containers and infrastructure | Docker, Kubernetes, CPU, Gear, Memory |
| OS & Platforms | Operating systems and platforms | Apple, Windows, Linux, Android, Homebrew |
| Status & Alerts | Status indicators and signals | Check, Bolt, Rocket, Fire, Star |
| UI Actions | Common interface actions | Search, Edit, Copy, Plus, Refresh, Save |
| Navigation | Arrows and directional icons | Arrow Up/Down, Angle Left/Right, Reply |
| People & Misc | People and miscellaneous symbols | User, Robot, Gamepad, Music, Bookmark |
| Fun & Seasonal | Seasonal and fun symbols | Ghost, Skull, Gift, Magic Wand, Sparkle, Trophy |

Click any icon to set it as the profile icon, or type a custom value directly in the text field. Use the "Clear icon" button to remove the current icon.

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

## Auto-Switching

par-term can automatically apply profiles based on the current working directory, tmux session name, or remote hostname. Auto-switched profiles apply all visual settings including icon, title, badge, and optional command execution.

### Directory-Based Profile Switching

Profiles can automatically apply when the terminal's working directory matches configured glob patterns.

**Configuration:**

Add `directory_patterns` to a profile:

```yaml
- id: 550e8400-e29b-41d4-a716-446655440000
  name: Work Projects
  directory_patterns:
    - "~/Repos/work-*"
    - "~/Repos/company-*"
    - "/opt/projects/*"
  icon: "🏢"
  badge_text: "WORK"
```

**Pattern Examples:**

| Pattern | Matches |
|---------|---------|
| `~/Repos/work-*` | `~/Repos/work-api`, `~/Repos/work-frontend` |
| `/opt/projects/*` | Any directory under `/opt/projects/` |
| `~/Repos/par-term*` | `~/Repos/par-term`, `~/Repos/par-term-core` |

- Patterns support `~` for home directory expansion
- CWD changes are detected via OSC 7 (requires shell integration)
- First matching profile wins (check profile order)
- Profile clears when CWD no longer matches any pattern

**Settings UI:**

1. Open Settings > Profiles
2. Edit a profile
3. Find the "Auto-Switch Dirs" field
4. Enter comma-separated glob patterns

### Tmux Profile Auto-Switching

Profiles can automatically apply when connecting to tmux sessions with matching names.

**Configuration:**

Add `tmux_session_patterns` to a profile:

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

**Pattern Matching:**

| Pattern | Matches |
|---------|---------|
| `dev-*` | `dev-api`, `dev-frontend`, etc. |
| `*-prod` | `api-prod`, `web-prod`, etc. |
| `*server*` | `webserver`, `api-server-1`, etc. |
| `main` | Exact match only |

- Patterns are case-insensitive
- First matching profile wins (check profile order)
- Profile clears when tmux session ends

**Settings UI:**

1. Open Settings > Profiles
2. Edit a profile
3. Find "Auto-Switch Tmux" field
4. Enter comma-separated patterns: `work-*, *-production`

### Hostname-Based Switching

Profiles automatically apply when connecting to remote hosts with matching hostnames, detected via OSC 7 shell-integration sequences (`file://hostname/path`). Hostname patterns are matched against the host extracted from the OSC 7 URL; the local host is ignored.

### Auto-Switch Priority

When multiple auto-switch mechanisms could apply, the following priority order determines which profile wins:

1. **Explicit user selection** — manual profile selection always takes precedence
2. **Hostname match** — remote host detection via OSC 7 (highest auto priority)
3. **SSH command detection** — running `ssh` process triggers profile matching
4. **Directory match** — CWD-based matching via OSC 7
5. **Tmux session match** — tmux session name pattern matching (applied via the tmux gateway separately)

Tmux session matching runs independently through the gateway tab and does not compete with hostname/directory/SSH switching.

### Auto-Switch Visual Application

When a profile is auto-applied via any switching mechanism (directory, hostname, or tmux session), the following settings are applied:

| Setting | Description |
|---------|-------------|
| **Profile icon** | Displayed in the tab bar (horizontal and vertical layouts) |
| **Tab title** | Overrides the current tab title |
| **Badge text** | Sets the badge overlay text |
| **Badge styling** | Applies badge color, alpha, font, bold, margins, size |
| **Command** | Queues the profile's command for confirmation (if configured) — see [Profile Commands and Confirmation](#profile-commands-and-confirmation) |

The original tab title saves when an auto-profile applies and restores when the auto-profile clears.

### Profile Commands and Confirmation

A profile's `command` is written into the running shell, and the thing that triggers an auto-switch is an OSC 7 sequence — which is emitted by whatever is producing terminal output, including a remote host you are SSH'd into. A `*` hostname pattern matches everything. Auto-switch therefore **never executes a profile command inline**. The command is queued in the same confirmation queue that trigger `RunCommand` and `SendText` actions use, so you see the exact command text before anything runs.

Two rules sit on top of that queue:

- **`ssh.ssh_auto_profile_switch`** gates hostname-driven switching entirely. With it off, a remote host cannot cause a profile switch at all.
- **A profile fetched from a dynamic source re-confirms every time**, even if you previously chose "Always Allow" for that exact command. Consenting to a profile *source* is not consent to arbitrary commands from it, and the source can change the command on any refresh. Local profiles do honour an earlier "Always Allow".

The approval is keyed on both the profile and the command text, so an "Always Allow" granted for one command does not carry over when the profile is edited or re-fetched with a different one. Profile-command approvals are also kept in a separate identifier space from trigger approvals, so a grant cannot leak between the two systems.

## Tmux Auto-Connect

Profiles can automatically connect to a named tmux session when opened. This is separate from the global `tmux_auto_attach` startup option — per-profile sessions let different profiles connect to different named sessions.

### Configuration

Set `tmux_session_name` on a profile to enable auto-connect. The profile uses **create-or-attach** semantics (`tmux new-session -A -s <name>`), so opening the profile either creates the session if it doesn't exist or attaches to it if it does.

```yaml
profiles:
  - name: Work
    tmux_session_name: work-session
    tmux_connection_mode: control_mode  # or: normal
```

### Connection Modes

| Mode | YAML value | Behavior |
|------|-----------|----------|
| **Control Mode** (default) | `control_mode` | Full par-term integration via `tmux -CC`. Enables pane sync, window tabs, and input routing. |
| **Normal** | `normal` | Plain tmux UI runs in the PTY. No par-term integration. |

### UI

In the profile editor (Settings → Profiles), the **Tmux Auto-Connect** collapsible section appears below the badge settings:

- **Session Name** — leave empty to disable auto-connect
- **Connection Mode** — radio buttons for Control Mode vs Normal

### Behavior

- Auto-connect only fires when `tmux_enabled = true` in global config
- If the window is already connected to tmux (gateway active), the auto-connect is skipped silently
- Errors are logged via the debug log (`make tail-log`)

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

## Per-Profile Shader Settings

Profiles can override the global background shader, allowing different visual effects per environment.

### Available Overrides

| Setting | Description |
|---------|-------------|
| `shader` | Shader path or name override |
| `shader_brightness` | Brightness adjustment override |
| `shader_text_opacity` | Text opacity override |
| `shader_animation_speed` | Animation speed override |
| `shader_texture_set` | Custom iChannel0-3 texture set (array of 4 optional paths) |

### Example

```yaml
- name: Presentation
  shader: "aurora"
  shader_brightness: 0.8
  shader_text_opacity: 0.9
```

See [Custom Shaders](CUSTOM_SHADERS.md) for the full shader system documentation.

## Per-Pane Background Settings

When using split panes, each pane can have its own custom background image that overrides the global background. Per-pane backgrounds support independent image selection, display mode, opacity, and darkening control.

### Available Controls

| Setting | Description | Range |
|---------|-------------|-------|
| **Background Image** | Custom image path for this pane | — |
| **Display Mode** | How the image fills the pane (fit, fill, stretch, tile, center) | — |
| **Opacity** | Transparency of the background image | 0.0–1.0 |
| **Darken** | Darkens the background by reducing RGB towards black, independent of opacity | 0.0–1.0 |

### Darken Control

The darken slider (0.0–1.0) reduces the RGB values of the background image towards black while leaving opacity unchanged. This allows you to dim a bright background without affecting its transparency level:

- **0.0** — No darkening applied (full brightness)
- **0.5** — Background is 50% darkened (RGB values reduced by half)
- **1.0** — Fully darkened to black

For example, a background with `opacity: 0.8` and `darken: 0.5` will be 80% opaque and 50% darker than the original image.

### Real-Time Preview

Per-pane background settings apply instantly as values change. Adjusting the image, mode, opacity, or darken level immediately updates the preview in the terminal. No manual "Apply" button is required — changes take effect in real-time.

### Example: Production vs. Development Panes

```yaml
pane_backgrounds:
  # Left pane: production environment with dim red overlay
  - index: 0
    image: "~/images/prod-bg.png"
    mode: fill
    opacity: 0.7
    darken: 0.3

  # Right pane: development environment with bright blue overlay
  - index: 1
    image: "~/images/dev-bg.png"
    mode: fit
    opacity: 0.6
    darken: 0.0
```

### Settings UI

Access per-pane background settings in **Settings > Effects > Per-Pane Background**:

1. Click on a split pane in the preview (if using split panes)
2. Configure the following for the selected pane:
   - **Image**: Browse to and select a custom background image, or leave empty to use the global background
   - **Mode**: Choose how the image fills the pane
   - **Opacity**: Adjust the transparency slider (0.0–1.0)
   - **Darken**: Adjust the darkening slider (0.0–1.0)

Changes apply immediately to the terminal without requiring manual confirmation.

## Dynamic Profiles

par-term can fetch profiles from remote URLs, enabling teams to share standardized terminal configurations. Dynamic profiles are read-only and update automatically in the background.

```mermaid
graph TD
    Sources[Remote Sources]
    Fetch[Background Fetch]
    Cache[Local Cache]
    Merge[Profile Merger]
    Local[Local Profiles]
    Combined[Combined Profile List]

    Sources --> Fetch
    Fetch --> Cache
    Cache --> Merge
    Local --> Merge
    Merge --> Combined

    style Sources fill:#0d47a1,stroke:#2196f3,stroke-width:2px,color:#ffffff
    style Fetch fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
    style Cache fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Merge fill:#e65100,stroke:#ff9800,stroke-width:3px,color:#ffffff
    style Local fill:#4a148c,stroke:#9c27b0,stroke-width:2px,color:#ffffff
    style Combined fill:#880e4f,stroke:#c2185b,stroke-width:2px,color:#ffffff
```

### Configuration

Dynamic profile sources are defined in `config.yaml` as an array:

```yaml
dynamic_profile_sources:
  - url: "https://example.com/team-profiles.yaml"
    headers:
      Authorization: "Bearer <token>"
    refresh_interval_secs: 1800
    max_size_bytes: 1048576
    fetch_timeout_secs: 10
    enabled: true
    conflict_resolution: "local_wins"
  - url: "https://internal.corp/devops-profiles.yaml"
    refresh_interval_secs: 3600
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `url` | string | (required) | URL to fetch profiles from |
| `headers` | map | `{}` | Custom HTTP headers (e.g., auth tokens) |
| `refresh_interval_secs` | integer | `1800` | How often to re-fetch the source (in seconds) |
| `max_size_bytes` | integer | `1048576` | Maximum response size (1 MB default) |
| `fetch_timeout_secs` | integer | `10` | HTTP fetch timeout in seconds |
| `enabled` | boolean | `true` | Whether this source is active |
| `conflict_resolution` | string | `"local_wins"` | How to handle ID collisions with local profiles |

There is no per-source option for allowing plain HTTP. Whether `http://` URLs are permitted comes from the single global `allow_http_profiles` setting (default `false`), which is applied to every source at load time; adding an `allow_http` key to a source entry in `config.yaml` does nothing.

### Background Refresh

Dynamic profile sources refresh automatically on a configurable timer:

- The default refresh interval is 1800 seconds (30 minutes)
- Each source can have its own interval via `refresh_interval_secs`
- Refresh occurs in the background without blocking the UI
- Failed fetches retain the previously cached version

### Local Cache

Fetched profiles are cached locally to provide offline access and faster startup:

**Cache location:** `~/.config/par-term/cache/dynamic_profiles/`

- Each source URL maps to a separate cache file
- The cache is populated on first fetch and updated on each successful refresh
- On startup, par-term loads from cache immediately and refreshes in the background

### Conflict Resolution

When a dynamic profile has the same ID as a local profile, the `conflict_resolution` setting determines which takes precedence:

| Mode | Description |
|------|-------------|
| `local_wins` | Local profile takes precedence (default) |
| `remote_wins` | Dynamic profile overrides the local one |

### Security

A dynamic profile source can influence what command a shell runs, so the fetch path is defended at several layers.

- **Scheme allowlist**: only `https` is accepted, plus `http` when the global `allow_http_profiles` is `true`. This is an allowlist, not a `file://` denylist — every other scheme (`ftp:`, `data:`, `file:`, anything else) is rejected outright, regardless of the HTTP opt-in. Scheme matching is case-insensitive, so `HTTPS://` is recognised as HTTPS rather than misclassified
- **No credentials in the clear**: even with the HTTP opt-in enabled, a source whose headers include `Authorization`, or any header name containing `token` or `secret`, is refused rather than fetched. The opt-in permits plaintext transport, never plaintext credentials
- **Redirects cannot downgrade**: an `https` source is fetched with HTTPS enforced across the whole redirect chain, so a `302` answering with an `http://` `Location` fails instead of quietly continuing in the clear. (A source you explicitly opted into HTTP for has no such protection, by definition — it is already plaintext)
- **HTTP is loud**: when a source is fetched over plain HTTP, a warning naming the URL is written to both the debug log and the standard log on every fetch
- **Size limits**: a response body larger than `max_size_bytes` (1 MB by default) fails the fetch rather than being truncated and parsed, and `fetch_timeout_secs` (10s by default) bounds the request as a whole. Either way the source keeps its previously cached profiles
- **Catch-all command profiles are rejected at merge**: a fetched profile that pairs a command with a catch-all auto-switch pattern (`*` or `**` in `hostname_patterns`, `directory_patterns` or `tmux_session_patterns`) is dropped and logged rather than added. Such a profile would fire its command on the first hostname or directory the terminal reports. You can still write one locally; a remote source cannot
- **Fetched commands are never pre-approved**: a command carried by a dynamic profile re-confirms on every auto-switch even after an "Always Allow" — see [Profile Commands and Confirmation](#profile-commands-and-confirmation)

### Visual Indicators

Dynamic profiles are visually distinguished throughout the interface:

- A `[dynamic]` badge appears next to dynamic profile names in the profile modal and profile drawer
- Dynamic profiles are read-only; opening one shows a "managed by a remote source" notice and all form fields are disabled
- Edit and delete controls are disabled for dynamic profiles in the profile list

### Keybinding

Use the `reload_dynamic_profiles` action to manually trigger an immediate refresh of all dynamic profile sources:

```yaml
keybindings:
  - key: "CmdOrCtrl+Shift+F5"
    action: "reload_dynamic_profiles"
```

### Dynamic Profiles Settings UI

Dynamic profile sources can be managed in **Settings > Profiles > Dynamic Profile Sources**:

- Add, edit, and remove remote source URLs
- Configure per-source headers, refresh interval, and size limits
- Set conflict resolution mode
- View last fetch status and timestamp per source

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

**How the file is written:** profiles carry commands and SSH arguments, so `profiles.yaml` is written atomically and owner-only. The new contents are staged in a temporary file in the same directory at mode `0600`, flushed to disk, and then renamed over the target — so the final file is mode `0600` on Unix regardless of your umask, and an interrupted or failed save leaves the previous `profiles.yaml` byte-for-byte intact rather than truncated. That matters because a truncated profiles file parses as "no profiles" rather than as an error.

## Related Documentation

- [Keyboard Shortcuts](../guides/KEYBOARD_SHORTCUTS.md) - Profile keyboard shortcuts
- [Tabs](TABS.md) - Tab management and directory inheritance
- [SSH Host Management](SSH.md) - SSH profiles and host-based auto-switching
- [Badges](BADGES.md) - Badge system and variables
- [Integrations](INTEGRATIONS.md) - Shell integration for directory tracking
