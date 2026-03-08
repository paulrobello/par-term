# Enterprise Deployment Guide

This guide covers bulk installation, managed configuration, update policies, and multi-user deployment of par-term in organizational environments.

## Table of Contents

- [Installation Methods](#installation-methods)
  - [Standalone Binary (Recommended for Enterprise)](#standalone-binary-recommended-for-enterprise)
  - [macOS App Bundle](#macos-app-bundle)
  - [Homebrew](#homebrew)
  - [Build from Source](#build-from-source)
- [Scripted Deployment](#scripted-deployment)
  - [macOS / Linux Script](#macos--linux-script)
  - [Windows Script](#windows-script)
- [Configuration Management](#configuration-management)
  - [Config File Location](#config-file-location)
  - [Deploying a Managed Config](#deploying-a-managed-config)
  - [Environment Variable Overrides](#environment-variable-overrides)
  - [Config Variable Substitution](#config-variable-substitution)
- [Update Management](#update-management)
  - [Disabling Auto-Update](#disabling-auto-update)
  - [Version Pinning](#version-pinning)
  - [Managed Update Workflow](#managed-update-workflow)
- [MDM and Jamf Deployment (macOS)](#mdm-and-jamf-deployment-macos)
  - [pkg Installer](#pkg-installer)
  - [LaunchAgent vs App Bundle](#launchagent-vs-app-bundle)
  - [Gatekeeper Considerations](#gatekeeper-considerations)
- [Multi-User Deployment](#multi-user-deployment)
  - [Per-User Config vs Shared Baseline](#per-user-config-vs-shared-baseline)
  - [Read-Only Shared Config](#read-only-shared-config)
- [Security Considerations](#security-considerations)
  - [Automation and Trigger Safety](#automation-and-trigger-safety)
  - [Session Logging](#session-logging)
  - [AI Panel (ACP)](#ai-panel-acp)
- [Troubleshooting Deployments](#troubleshooting-deployments)
- [Related Documentation](#related-documentation)

---

## Installation Methods

### Standalone Binary (Recommended for Enterprise)

The standalone binary has no runtime dependencies beyond the GPU driver and system libraries. It is the easiest to deploy, version-pin, and replace.

**Download URL pattern** (substitute the target version and platform):
```
https://github.com/paulrobello/par-term/releases/download/v<VERSION>/par-term-<PLATFORM>
```

Supported platform suffixes:
| Platform | Suffix |
|----------|--------|
| macOS (Apple Silicon) | `aarch64-apple-darwin` |
| macOS (Intel) | `x86_64-apple-darwin` |
| Linux (x86_64) | `x86_64-unknown-linux-gnu` |
| Windows (x86_64) | `x86_64-pc-windows-msvc.exe` |

**Install to a system-wide location:**
```bash
# macOS / Linux
sudo install -m 755 par-term-aarch64-apple-darwin /usr/local/bin/par-term

# Windows (PowerShell, elevated)
Copy-Item par-term.exe "C:\Program Files\par-term\par-term.exe"
```

### macOS App Bundle

The `.app` bundle is suitable for Jamf/MDM push. Download `par-term.app.tar.gz` from the GitHub release and extract to `/Applications`:

```bash
sudo tar -xzf par-term.app.tar.gz -C /Applications
sudo xattr -rd com.apple.quarantine /Applications/par-term.app
```

The `xattr` step removes the Gatekeeper quarantine flag — necessary for scripted installs that bypass the first-launch approval dialog.

### Homebrew

For teams that manage Homebrew centrally (e.g., via Brewfile or a custom tap):

```bash
brew install paulrobello/tap/par-term
```

Pin a specific version to prevent automatic upgrades:
```bash
brew pin par-term
```

Unpin before a planned upgrade window:
```bash
brew unpin par-term && brew upgrade par-term && brew pin par-term
```

### Build from Source

For security-sensitive environments that require reproducible builds from source:

```bash
cargo install --locked par-term
# Or from a local checkout:
cargo build --profile dev-release --locked
```

`--locked` ensures the exact dependency versions from the committed `Cargo.lock` are used.

---

## Scripted Deployment

### macOS / Linux Script

```bash
#!/usr/bin/env bash
set -euo pipefail

PAR_TERM_VERSION="0.25.0"
INSTALL_DIR="/usr/local/bin"
PLATFORM="aarch64-apple-darwin"   # adjust for target arch
BINARY="par-term-${PLATFORM}"
RELEASE_URL="https://github.com/paulrobello/par-term/releases/download/v${PAR_TERM_VERSION}/${BINARY}"

echo "Downloading par-term ${PAR_TERM_VERSION}..."
curl -fsSL "${RELEASE_URL}" -o "/tmp/${BINARY}"
chmod 755 "/tmp/${BINARY}"

echo "Installing to ${INSTALL_DIR}/par-term..."
sudo install -m 755 "/tmp/${BINARY}" "${INSTALL_DIR}/par-term"
rm "/tmp/${BINARY}"

# Deploy base config if not already present
CONFIG_DIR="${HOME}/.config/par-term"
mkdir -p "${CONFIG_DIR}"
if [[ ! -f "${CONFIG_DIR}/config.yaml" ]]; then
    cp /etc/par-term/config.yaml "${CONFIG_DIR}/config.yaml"
    echo "Deployed default config to ${CONFIG_DIR}/config.yaml"
fi

echo "par-term ${PAR_TERM_VERSION} installed successfully."
```

### Windows Script

```powershell
# deploy-par-term.ps1
$Version  = "0.25.0"
$Platform = "x86_64-pc-windows-msvc"
$InstDir  = "C:\Program Files\par-term"
$Url      = "https://github.com/paulrobello/par-term/releases/download/v$Version/par-term-$Platform.exe"

Write-Host "Downloading par-term $Version..."
New-Item -ItemType Directory -Force -Path $InstDir | Out-Null
Invoke-WebRequest -Uri $Url -OutFile "$InstDir\par-term.exe"

# Deploy base config if not already present
$ConfigDir = "$env:APPDATA\par-term"
New-Item -ItemType Directory -Force -Path $ConfigDir | Out-Null
if (-not (Test-Path "$ConfigDir\config.yaml")) {
    Copy-Item "C:\ProgramData\par-term\config.yaml" "$ConfigDir\config.yaml"
    Write-Host "Deployed default config."
}

Write-Host "par-term $Version installed to $InstDir."
```

---

## Configuration Management

### Config File Location

par-term follows the [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html):

| Platform | Default Path |
|----------|-------------|
| macOS / Linux | `~/.config/par-term/config.yaml` |
| macOS (XDG override) | `$XDG_CONFIG_HOME/par-term/config.yaml` |
| Windows | `%APPDATA%\par-term\config.yaml` |

The config directory is also created automatically on first launch if it does not exist.

### Deploying a Managed Config

**Strategy 1 — Seed on first install** (most common): Copy a baseline `config.yaml` to each user's config directory during provisioning. Users can freely modify it afterward.

**Strategy 2 — System-wide read-only baseline**: Place a corporate baseline at a shared path and use `XDG_CONFIG_HOME` to point all users to it:

```bash
# /etc/profile.d/par-term.sh
export XDG_CONFIG_HOME="/etc/par-term/config"
```

> **Note:** With a read-only shared config, users cannot save settings changes through the UI. Use this only when strict configuration control is required.

**Strategy 3 — Config variable substitution**: Ship a single `config.yaml` that uses `${VAR}` placeholders resolved from environment variables. This lets the same file work across machines with different preferences:

```yaml
font_family: "${PAR_TERM_FONT:-JetBrains Mono}"
font_size: 14.0
shell: "${SHELL}"
window_title: "${USER}@${HOSTNAME}"
```

See [Config Variable Substitution](#config-variable-substitution) below.

### Environment Variable Overrides

Set these in `/etc/profile.d/par-term.sh` (macOS/Linux) or system environment variables (Windows) to apply org-wide defaults:

| Variable | Purpose |
|----------|---------|
| `XDG_CONFIG_HOME` | Override the config directory location |
| `PAR_TERM_FONT` | Default font (when used in config with `${PAR_TERM_FONT:-...}`) |
| `DEBUG_LEVEL` | Enable debug logging (`0`–`4`); set to `0` in production |
| `RUST_LOG` | Standard log filter; leave unset in production |

Refer to [Environment Variables Reference](ENVIRONMENT_VARIABLES.md) for the complete list.

### Config Variable Substitution

`config.yaml` supports `${VAR_NAME}` and `${VAR_NAME:-default}` syntax. Only variables in the standard allowlist (plus `PAR_TERM_*` and `LC_*` prefixed names) are substituted by default.

To define org-specific variables, prefix them with `PAR_TERM_`:
```yaml
# config.yaml
font_family: "${PAR_TERM_FONT:-Fira Code}"
font_size:   14.0
```

```bash
# /etc/profile.d/par-term.sh
export PAR_TERM_FONT="JetBrains Mono"
```

---

## Update Management

### Disabling Auto-Update

For environments where updates are managed centrally, disable the auto-update check in `config.yaml`:

```yaml
update_check_frequency: never
```

Valid values: `startup`, `daily`, `weekly`, `monthly`, `never`.

### Version Pinning

| Install Method | Pin Command |
|----------------|-------------|
| Homebrew | `brew pin par-term` |
| Standalone binary | Replace binary file only during planned maintenance windows |
| Cargo | `cargo install --locked --version 0.25.0 par-term` |

### Managed Update Workflow

Recommended update cadence for enterprise deployments:

1. **Test release** in a staging environment before rolling out to production users.
2. **Download** the new binary from GitHub Releases and verify the SHA-256 checksum.
3. **Replace** the binary in the shared install directory during a maintenance window.
4. **Notify** users — par-term must be restarted to pick up a new binary.

No database migrations or service restarts are required; par-term reads its config at launch.

---

## MDM and Jamf Deployment (macOS)

### pkg Installer

Wrap the binary in a standard `.pkg` installer for Jamf/MDM distribution:

```bash
# Create payload
mkdir -p /tmp/par-term-pkg/usr/local/bin
cp par-term-aarch64-apple-darwin /tmp/par-term-pkg/usr/local/bin/par-term
chmod 755 /tmp/par-term-pkg/usr/local/bin/par-term

# Build pkg
pkgbuild \
  --root /tmp/par-term-pkg \
  --identifier com.paulrobello.par-term \
  --version 0.25.0 \
  --install-location / \
  par-term-0.25.0.pkg
```

Upload the `.pkg` to Jamf Pro and deploy via a policy scoped to the target computer group.

### LaunchAgent vs App Bundle

- **Binary in `/usr/local/bin`**: Appropriate for users who launch par-term from another terminal or via a shell alias. No LaunchAgent is needed.
- **App Bundle in `/Applications`**: Appropriate for users who launch par-term from the Dock or Spotlight. Use Jamf's "App Deployment" policy with the `.app.tar.gz` from the GitHub release.

### Gatekeeper Considerations

par-term is not currently notarized by Apple. For scripted MDM installs, remove the quarantine attribute immediately after copying:

```bash
sudo xattr -rd com.apple.quarantine /Applications/par-term.app
# or for the binary:
sudo xattr -d com.apple.quarantine /usr/local/bin/par-term
```

Alternatively, configure a Jamf policy to run this command after installation, or provision a system-wide Privacy Policy exception via a Configuration Profile.

---

## Multi-User Deployment

### Per-User Config vs Shared Baseline

par-term stores config in the **user's home directory** by default. This means:

- Each user gets their own independent config file.
- Changes one user makes do not affect other users.
- You can provision a per-user baseline by copying `config.yaml` during user account setup (e.g., from a login script or Jamf policy scoped to user sessions).

### Read-Only Shared Config

To enforce a corporate baseline that users cannot modify:

```bash
# Place config in a system directory
sudo mkdir -p /etc/par-term
sudo cp corporate-config.yaml /etc/par-term/config.yaml
sudo chmod 444 /etc/par-term/config.yaml

# Point all users to it via /etc/profile.d
cat > /etc/profile.d/par-term.sh <<'EOF'
export XDG_CONFIG_HOME="/etc/par-term"
EOF
```

> **Caveat:** The par-term Settings UI will display an error when attempting to save changes if the config file is read-only. Communicate this to users or disable the Settings UI shortcut in the managed config.

---

## Security Considerations

### Automation and Trigger Safety

par-term's [Automation](AUTOMATION.md) system can execute shell commands in response to terminal output patterns. In enterprise deployments:

- `require_user_action` defaults to `true` — users must confirm before commands run.
- Set `require_user_action: false` only for commands your org explicitly approves.
- The Settings UI displays an amber warning banner when any trigger has `require_user_action: false`.
- The built-in command denylist blocks known dangerous commands but is bypassable via obfuscation. Do not rely on it as a security boundary.

To disable automation entirely in managed deployments, deploy a config with no `triggers` entries and no `automation_scripts`.

### Session Logging

[Session logging](SESSION_LOGGING.md) records raw terminal I/O to a local file. When enabled, it may capture passwords, API keys, and other sensitive data despite the built-in redaction heuristics.

For environments with data-handling requirements (PCI-DSS, HIPAA, SOC 2):

- Disable session logging in the managed config: `session_log_enabled: false`
- Or restrict the log directory permissions so only the owning user can read logs.
- The `redact_passwords` option (default: `true`) applies heuristic redaction but is not a compliance-grade control.

### AI Panel (ACP)

The [AI panel](ASSISTANT_PANEL.md) launches AI coding agents (Claude Code, Codex CLI, etc.) as subprocesses with filesystem access. In environments where AI tooling is restricted:

- Disable the panel: `acp_enabled: false`
- Or deploy a config with no `custom_acp_agents` entries to prevent custom agent registration.

---

## Troubleshooting Deployments

| Symptom | Likely Cause | Resolution |
|---------|-------------|------------|
| `par-term: command not found` after install | Binary not in `PATH` | Add install directory to `PATH` in `/etc/profile` |
| App bounces in Dock then quits (macOS) | Quarantine flag not cleared | Run `xattr -rd com.apple.quarantine /Applications/par-term.app` |
| Settings changes lost on restart | Config file is read-only | Check file permissions; inform users or provide a writable path |
| Black screen on launch | GPU driver issue or missing Vulkan/Metal support | See [Troubleshooting](TROUBLESHOOTING.md#black-screen-or-no-output) |
| Missing Linux libraries on startup | `libxcb-*` not installed | Run `sudo apt-get install libxcb-render0 libxcb-shape0 libxcb-xfixes0` |
| Users cannot save settings | `XDG_CONFIG_HOME` points to read-only dir | Provide a writable per-user config path or remove the override |

---

## Related Documentation

- [Getting Started](GETTING_STARTED.md) — Installation for individual users
- [Config Reference](CONFIG_REFERENCE.md) — All configuration options with defaults
- [Environment Variables](ENVIRONMENT_VARIABLES.md) — Variables recognized at startup
- [Self-Update](SELF_UPDATE.md) — Built-in update behavior and how to disable it
- [Session Logging](SESSION_LOGGING.md) — Log format, location, and redaction
- [Automation](AUTOMATION.md) — Trigger safety and command execution model
- [Troubleshooting](TROUBLESHOOTING.md) — Diagnosing common issues
