# Integrations

par-term provides optional integrations to enhance your terminal experience, including shell integration for improved directory tracking and a shader collection for visual effects.

## Table of Contents
- [Overview](#overview)
- [Shell Integration](#shell-integration)
  - [Features](#shell-features)
  - [Installation](#shell-installation)
  - [Supported Shells](#supported-shells)
  - [How It Works](#how-it-works)
  - [Troubleshooting](#troubleshooting-shell)
- [Remote Shell Integration](#remote-shell-integration)
- [Shader Installation](#shader-installation)
  - [Included Shaders](#included-shaders)
  - [Installation Methods](#shader-installation-methods)
  - [Manifest System](#manifest-system)
  - [Uninstallation](#shader-uninstallation)
- [Settings UI](#settings-ui)
- [CLI Commands](#cli-commands)
- [Related Documentation](#related-documentation)

## Overview

Integrations in par-term are optional enhancements that extend functionality:

```mermaid
graph TD
    Integrations[Integrations System]
    Shell[Shell Integration]
    Shaders[Shader Collection]
    UI[Settings UI]
    CLI[CLI Commands]

    Integrations --> Shell
    Integrations --> Shaders
    Integrations --> UI
    Integrations --> CLI

    Shell --> Bash[bash]
    Shell --> Zsh[zsh]
    Shell --> Fish[fish]

    Shaders --> Background[Background Effects]
    Shaders --> Cursor[Cursor Effects]

    class Integrations primary
    class Shell active
    class Shaders data
    class UI external
    class CLI accent
    class Bash,Zsh,Fish,Background,Cursor neutral

    classDef primary fill:#e65100,stroke:#ff9800,stroke-width:3px,color:#ffffff
    classDef active fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
    classDef data fill:#0d47a1,stroke:#2196f3,stroke-width:2px,color:#ffffff
    classDef external fill:#4a148c,stroke:#9c27b0,stroke-width:2px,color:#ffffff
    classDef accent fill:#880e4f,stroke:#c2185b,stroke-width:2px,color:#ffffff
    classDef neutral fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
```

## Shell Integration

Shell integration enhances the terminal experience by enabling communication between your shell and par-term.

### Shell Features

- **Directory Tracking (OSC 7)**: Tab titles automatically update to show the current working directory
- **Command Notifications (OSC 777)**: Desktop notifications for long-running commands
- **Prompt Navigation (OSC 133)**: Navigate between command prompts using keyboard shortcuts
- **Current Working Directory Sync**: New tabs can inherit the current directory from the active tab

### Shell Installation

**Method 1: Settings UI (Recommended)**

1. Press `F12` to open Settings
2. Navigate to the **Integrations** tab
3. Click **Install Shell Integration**

**Method 2: CLI Command**

```bash
# Auto-detect shell
par-term install-shell-integration

# Specify shell explicitly
par-term install-shell-integration --shell bash
par-term install-shell-integration --shell zsh
par-term install-shell-integration --shell fish
```

**Method 3: Manual Installation (via curl)**

```bash
curl -sSL https://paulrobello.github.io/par-term/install-shell-integration.sh | sh
```

### Supported Shells

| Shell | Script Location | RC File |
|-------|-----------------|---------|
| **bash** | `~/.config/par-term/shell_integration.bash` | `~/.bashrc` or `~/.bash_profile` |
| **zsh** | `~/.config/par-term/shell_integration.zsh` | `~/.zshrc` |
| **fish** | `~/.config/par-term/shell_integration.fish` | `~/.config/fish/config.fish` |

### How It Works

The installation process:

1. Detects your current shell from the `$SHELL` environment variable
2. Downloads the appropriate shell integration script
3. Saves it to `~/.config/par-term/`
4. Installs file transfer utilities (`pt-dl`, `pt-ul`, `pt-imgcat`) to `~/.config/par-term/bin/`
5. Adds a source block to your shell's RC file wrapped in markers:

```bash
# >>> par-term shell integration >>>
if [ -d "$HOME/.config/par-term/bin" ]; then
    export PATH="$HOME/.config/par-term/bin:$PATH"
fi
if [ -f "$HOME/.config/par-term/shell_integration.bash" ]; then
    source "$HOME/.config/par-term/shell_integration.bash"
fi
# <<< par-term shell integration <<<
```

### Troubleshooting Shell

**Integration not working after installation:**
1. Restart your shell or run `source ~/.bashrc` (or equivalent)
2. Verify the script exists: `ls ~/.config/par-term/shell_integration.*`
3. Check your RC file for the source line

**Reinstalling:**
```bash
par-term install-shell-integration
# Or uninstall first:
par-term uninstall-shell-integration
par-term install-shell-integration
```

## Remote Shell Integration

When working on a remote host via SSH, you can install shell integration directly from par-term without manually running commands.

### Installing on a Remote Host

**Method 1: Application Menu (Recommended)**

1. Establish an SSH connection to the remote host in a terminal tab
2. From the menu bar: **Shell > Install Shell Integration on Remote Host...**
3. A confirmation dialog appears showing the exact command that will be sent
4. Click **Install** to send the command, or **Cancel** to dismiss

The command sent to the remote host:

```bash
curl -sSL https://paulrobello.github.io/par-term/install-shell-integration.sh | sh
```

**Method 2: Manual Installation**

If the menu option is not available, run the install command directly in your SSH session:

```bash
curl -sSL https://paulrobello.github.io/par-term/install-shell-integration.sh | sh
```

**Requirements:**
- An active SSH session to the remote host
- `curl` available on the remote host
- Permission to modify shell RC files on the remote host

> **📝 Note:** The install script auto-detects the remote shell (bash, zsh, or fish) and installs the appropriate integration script. Restart the remote shell after installation for changes to take effect.

## Shader Installation

par-term includes a collection of 73 ready-to-use GLSL shaders (61 background + 12 cursor), cubemap environments, and texture packs.

### Included Shaders

**Background Effects:**
- Terminal-aware effects for progress, command state, pane focus, and scrollback depth
- CRT/Retro effects (scanlines, phosphor glow)
- Matrix rain, starfield, galaxy
- Plasma, fire, underwater
- Abstract visualizations
- Nature effects (clouds, rain, snow)
- Cubemap-based ambience shaders backed by bundled cubemap textures

**Texture Assets:**
- Cubemap environments under `shaders/textures/cubemaps/`
- Texture packs under `shaders/textures/packs/` for noise, gradients, paper, metal, and starfields

**Cursor Effects:**
- Glow, trail, ripple
- Blaze, sweep, warp
- Particle effects

See [SHADERS.md](SHADERS.md) for the complete shader gallery.

### Shader Installation Methods

**Method 1: First-Run Dialog**

On first launch, par-term offers to install the shader collection. Choose:
- **Install Selected**: Downloads and installs immediately
- **Skip**: Dismiss for this session
- **Never Ask**: Saves preference, never ask again

**Method 2: Settings UI**

1. Press `F12` to open Settings
2. Navigate to the **Integrations** tab
3. In the **Custom Shaders** section, click **Install**

**Method 3: CLI Command**

```bash
# Interactive (with confirmation prompt)
par-term install-shaders

# Non-interactive
par-term install-shaders -y
par-term install-shaders --force
```

**Method 4: Manual Installation (via curl)**

```bash
curl -sSL https://paulrobello.github.io/par-term/install-shaders.sh | sh
```

**Method 5: Combined Installation**

Install both shaders and shell integration at once:

```bash
par-term install-integrations
par-term install-integrations -y  # Non-interactive
```

### Manifest System

par-term tracks installed bundled shaders, cubemaps, and texture packs using a manifest file for safe updates and uninstallation. The `install-shaders` command installs files tracked by `shaders/manifest.json`, including GLSL shaders and bundled texture assets.

**Location:** `~/.config/par-term/shaders/manifest.json`

**Manifest Contents:**
```json
{
  "version": "0.31.0",
  "generated": "2026-03-07T18:17:58.976671+00:00",
  "files": [
    {
      "path": "crt.glsl",
      "sha256": "ca7bb2d0faeb09740206d3c2ede153f4...",
      "type": "shader",
      "category": "retro"
    },
    {
      "path": "textures/packs/noise/soft-value-128.png",
      "sha256": "8ed431a79fa7244de7f8c70b7c43583c...",
      "type": "texture",
      "category": "texture-pack-noise"
    }
  ]
}
```

**File Status Detection:**
- **Unchanged**: Hash matches manifest (safe to update/remove)
- **Modified**: You've edited a bundled shader
- **UserCreated**: Your custom shader (not in manifest)
- **Missing**: Listed in manifest but deleted

### Shader Uninstallation

```bash
# Interactive (prompts for confirmation)
par-term uninstall-shaders

# Force removal
par-term uninstall-shaders --force
```

**What gets removed:**
- Bundled shader, cubemap, and texture-pack files with matching checksums
- Empty directories

**What gets preserved:**
- User-created shaders (not in manifest)
- Modified bundled shaders (different checksum)

## Settings UI

The Integrations tab in Settings (`F12`) provides a graphical interface for managing integrations.

**Shell Integration Section:**
- Installation status indicator
- Detected shell display
- Install/Reinstall/Uninstall buttons
- Manual installation command (click to copy)

**Custom Shaders Section:**
- Installation status with shader count
- Installed version display
- Install/Reinstall/Uninstall buttons
- Open Folder button (reveals shader directory)
- Manual installation command (click to copy)

## CLI Commands

**Shell Integration:**
```bash
par-term install-shell-integration [--shell bash|zsh|fish]
par-term uninstall-shell-integration
```

**Shaders:**
```bash
par-term install-shaders [-y|--yes] [-f|--force]
par-term uninstall-shaders [-f|--force]
```

**Combined:**
```bash
par-term install-integrations [-y|--yes]
```

## Related Documentation

- [SHADERS.md](SHADERS.md) - Complete shader gallery and descriptions
- [CUSTOM_SHADERS.md](CUSTOM_SHADERS.md) - Creating custom shaders
- [SSH Host Management](SSH.md) - SSH host profiles and quick connect
- [../README.md](../README.md) - Project overview
