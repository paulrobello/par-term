# Integrations Install System Design

**Date:** 2026-02-02
**Status:** Draft
**Author:** Claude + Paul Robello

## Overview

Add a unified system for installing, updating, and uninstalling optional par-term integrations:
- **Shell Integration**: Scripts for bash/zsh/fish that enable prompt navigation, CWD tracking, command status, etc.
- **Custom Shaders**: Background effects and cursor shaders with supporting textures

Both support:
- One-liner `curl | bash` installation via GitHub Pages
- In-app installation from Settings UI
- First-run welcome dialog prompting for both
- Version tracking with "once per version" prompting
- Safe uninstallation that preserves user modifications

## File Structure

### New/Modified Files

```
par-term/
├── shell_integration/
│   ├── install.sh                           # Curl-able installer script
│   ├── par_term_shell_integration.bash
│   ├── par_term_shell_integration.zsh
│   ├── par_term_shell_integration.fish
│   └── README.md
├── shaders/
│   ├── manifest.json                        # NEW: tracks bundled files
│   ├── *.glsl
│   └── textures/
├── gh-pages/                                # RENAMED from shader-gallery/
│   ├── index.html                           # Landing page or redirect
│   ├── gallery/                             # Moved shader gallery
│   ├── install-shaders.sh                   # Copy for clean URL
│   └── install-shell-integration.sh         # Copy for clean URL
├── src/
│   ├── shell_integration_installer.rs       # NEW: install/uninstall logic
│   ├── integrations_ui.rs                   # NEW: combined welcome dialog
│   ├── shader_installer.rs                  # UPDATE: manifest support
│   ├── shader_install_ui.rs                 # UPDATE: migrate to integrations_ui
│   └── settings_ui/
│       └── integrations_tab.rs              # NEW: settings tab
└── .github/workflows/
    └── pages.yml                            # UPDATE: deploy from gh-pages/
```

### Installation Paths

- Shell integration scripts: `~/.config/par-term/shell_integration.{bash,zsh,fish}`
- Shaders and textures: `~/.config/par-term/shaders/`
- Manifest: `~/.config/par-term/shaders/manifest.json`

## Config Schema Changes

### New Types

```rust
/// Tracks installed and prompted versions for an integration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntegrationVersions {
    /// Version when shaders were installed (e.g., "0.2.0")
    pub shaders_installed_version: Option<String>,
    /// Version when user was last prompted about shaders
    pub shaders_prompted_version: Option<String>,
    /// Version when shell integration was installed
    pub shell_integration_installed_version: Option<String>,
    /// Version when user was last prompted about shell integration
    pub shell_integration_prompted_version: Option<String>,
}

/// State of an integration's install prompt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallPromptState {
    /// Prompt user when appropriate (default)
    #[default]
    Ask,
    /// User said "never ask again"
    Never,
    /// Currently installed
    Installed,
}
```

### Config Updates

```rust
// In Config struct - replace shader_install_prompt with:
pub shader_install_state: InstallPromptState,
pub shell_integration_state: InstallPromptState,
pub integration_versions: IntegrationVersions,
```

### Version Prompt Logic

```
Should prompt if:
  - State is Ask (not Never or Installed)
  - AND (never prompted OR prompted_version < current_version)
  - AND (not installed OR installed_version < current_version)
```

## Shader Manifest System

### Manifest Format

```json
{
  "version": "0.2.0",
  "generated": "2026-02-02T12:00:00Z",
  "files": [
    {
      "path": "crt.glsl",
      "sha256": "a1b2c3d4e5f6...",
      "type": "shader",
      "category": "retro"
    },
    {
      "path": "cursor_glow.glsl",
      "sha256": "g7h8i9j0k1l2...",
      "type": "cursor_shader",
      "category": "effects"
    },
    {
      "path": "textures/noise.png",
      "sha256": "m3n4o5p6q7r8...",
      "type": "texture"
    }
  ]
}
```

### File Types

- `shader` - Background shaders (*.glsl)
- `cursor_shader` - Cursor effect shaders
- `texture` - Images used by shaders
- `doc` - Documentation files

### Install Behavior

```
For each file in new manifest:
  If file doesn't exist:
    → Install
  If file exists AND hash matches current manifest:
    → Skip (unchanged bundled file)
  If file exists AND hash differs:
    → Prompt: "File modified. Overwrite? [Yes/No/Diff]"

For files in OLD manifest but NOT in new:
  If hash matches old manifest:
    → Delete (unmodified, removed from bundle)
  If hash differs:
    → Prompt: "Removed from bundle but modified. Delete? [Yes/No]"

Files NOT in any manifest:
  → Leave alone (user-created)
```

### Uninstall Behavior

```
For each file in shaders directory:
  If in manifest AND hash matches:
    → Delete
  If in manifest AND hash differs:
    → Prompt: "Modified bundled file. Delete? [Yes/No]"
  If NOT in manifest:
    → Leave alone (user-created)

Delete manifest.json last
```

## Shell Integration

### Script Naming

- Paths use hyphens: `~/.config/par-term/`
- Functions use underscores: `par_term_prompt_mark`

### RC File Markers

```bash
# >>> par-term shell integration >>>
if [ -f "${XDG_CONFIG_HOME:-$HOME/.config}/par-term/shell_integration.bash" ]; then
  source "${XDG_CONFIG_HOME:-$HOME/.config}/par-term/shell_integration.bash"
fi
# <<< par-term shell integration <<<
```

### Install Logic

1. Detect shell from `$SHELL`
2. Create `~/.config/par-term/` if needed
3. Write integration script (embedded via `include_str!`)
4. Add source block with markers to RC file
5. Update config with installed version

### Uninstall Logic

1. Remove script files from `~/.config/par-term/`
2. For each RC file (.bashrc, .zshrc, config.fish):
   - If exact marker block found → Remove it
   - If markers modified → Show manual instructions
3. Update config state

## UI Components

### Combined Welcome Dialog

Shows on first run when neither integration is installed:

```
┌─────────────────────────────────────────────────────────┐
│              Welcome to par-term v0.2.0                 │
├─────────────────────────────────────────────────────────┤
│  par-term has optional enhancements available:          │
│                                                         │
│  ☑ Custom Shaders (49+ effects)                        │
│    CRT, matrix rain, plasma, cursor effects, etc.       │
│                                                         │
│  ☑ Shell Integration                                   │
│    Directory tracking, command status, prompt nav       │
│    Detected shell: zsh                                  │
│                                                         │
│           [Install Selected]  [Skip]  [Never Ask]       │
│                                                         │
│  You can install these later from Settings (F12)        │
└─────────────────────────────────────────────────────────┘
```

### Settings UI - Integrations Tab

```
┌─────────────────────────────────────────────────────────┐
│  INTEGRATIONS                                           │
├─────────────────────────────────────────────────────────┤
│  Shell Integration                                      │
│  Status: ● Installed (v0.2.0) for zsh                  │
│  [Reinstall]  [Uninstall]                              │
│                                                         │
│  Manual: curl -fsSL https://paulrobello.github.io/     │
│          par-term/install-shell-integration.sh | bash  │
├─────────────────────────────────────────────────────────┤
│  Custom Shaders                                         │
│  Status: ● Installed (v0.2.0) - 61 files               │
│  [Reinstall]  [Uninstall]                              │
│                                                         │
│  Manual: curl -fsSL https://paulrobello.github.io/     │
│          par-term/install-shaders.sh | bash            │
└─────────────────────────────────────────────────────────┘
```

Status indicators:
- `●` Green - Installed
- `○` Gray - Not installed
- `⟳` Yellow - Update available

## CLI Commands

### Existing (to update)

```bash
par-term install-shaders      # Update for manifest support
```

### New Commands

```bash
# Shell integration
par-term install-shell-integration
par-term install-shell-integration --shell zsh
par-term uninstall-shell-integration

# Shader management
par-term uninstall-shaders

# Convenience
par-term install-integrations  # Install both
```

## GitHub Pages Setup

### URL Structure

```
https://paulrobello.github.io/par-term/
├── index.html                      # Landing or redirect to gallery
├── gallery/                        # Shader gallery (moved)
├── install-shaders.sh
└── install-shell-integration.sh
```

### Workflow Updates

Update `.github/workflows/pages.yml`:
- Change artifact path from `shader-gallery` to `gh-pages`
- Add trigger paths for `shell_integration/**`

## Migration Path

1. Existing `shader_install_prompt` config migrates to `shader_install_state`
2. If `shader_install_prompt` was `Installed`, set `shaders_installed_version` to current version
3. Generate manifest for existing shader installations on first run

## Build Requirements

- Add manifest generation script/build step
- Embed shell integration scripts via `include_str!`
- Include manifest in shaders.zip release artifact

## Testing Considerations

- Test install/uninstall on bash, zsh, fish
- Test manifest diff detection with modified files
- Test RC file marker detection and removal
- Test migration from old config format
- Test combined dialog with various initial states
