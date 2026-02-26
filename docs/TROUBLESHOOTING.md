# Troubleshooting

Centralized guide for diagnosing and resolving common issues with par-term. Each issue follows a consistent pattern: symptom, cause, and solution.

## Table of Contents

- [Overview](#overview)
- [Using Debug Logging](#using-debug-logging)
- [Installation Issues](#installation-issues)
  - [macOS Gatekeeper Blocking the App](#macos-gatekeeper-blocking-the-app)
  - [Missing Linux Dependencies](#missing-linux-dependencies)
  - [Build From Source Failures](#build-from-source-failures)
- [Display and Rendering Issues](#display-and-rendering-issues)
  - [Black Screen or No Output](#black-screen-or-no-output)
  - [Font Rendering Problems](#font-rendering-problems)
  - [HiDPI and Scaling Issues](#hidpi-and-scaling-issues)
  - [Inline Graphics Not Displaying](#inline-graphics-not-displaying)
- [Shader Issues](#shader-issues)
  - [Shader Not Loading](#shader-not-loading)
  - [Black or White Screen With Shader](#black-or-white-screen-with-shader)
  - [Text Hard to Read With Shader](#text-hard-to-read-with-shader)
  - [Shader Compilation Errors](#shader-compilation-errors)
  - [Low Frame Rate With Shaders](#low-frame-rate-with-shaders)
  - [Default Cursor Showing Through Cursor Shader](#default-cursor-showing-through-cursor-shader)
  - [Debugging Shaders](#debugging-shaders)
- [Terminal Behavior Issues](#terminal-behavior-issues)
  - [Shell Integration Not Working](#shell-integration-not-working)
  - [Keyboard Shortcuts Not Recognized](#keyboard-shortcuts-not-recognized)
  - [Copy and Paste Issues](#copy-and-paste-issues)
  - [Mouse Behavior Issues](#mouse-behavior-issues)
- [SSH and Remote Issues](#ssh-and-remote-issues)
  - [Hosts Not Appearing in Quick Connect](#hosts-not-appearing-in-quick-connect)
  - [Profile Not Auto-Switching on SSH](#profile-not-auto-switching-on-ssh)
  - [mDNS Hosts Not Appearing](#mdns-hosts-not-appearing)
  - [Shell Integration on Remote Hosts](#shell-integration-on-remote-hosts)
- [Performance Issues](#performance-issues)
  - [High CPU Usage](#high-cpu-usage)
  - [Slow Rendering](#slow-rendering)
  - [High Memory Usage](#high-memory-usage)
- [Configuration Issues](#configuration-issues)
  - [Config Not Loading](#config-not-loading)
  - [Settings Not Saving](#settings-not-saving)
  - [Profile Switching Issues](#profile-switching-issues)
  - [Arrangement Restore Issues](#arrangement-restore-issues)
- [Update Issues](#update-issues)
  - [Self-Update Refused](#self-update-refused)
  - [Platform Binary Not Found](#platform-binary-not-found)
  - [Update Failed Network Error](#update-failed-network-error)
  - [Permission Denied During Update](#permission-denied-during-update)
  - [Updated App Blocked by Gatekeeper](#updated-app-blocked-by-gatekeeper)
  - [Old Version Still Running After Update](#old-version-still-running-after-update)
- [Getting Help](#getting-help)
- [Related Documentation](#related-documentation)

## Overview

This guide covers the most common issues encountered when installing, configuring, and using par-term. For each issue, the **Symptom** describes what you observe, the **Cause** explains why it happens, and the **Solution** provides actionable steps to fix it.

Before diving into specific issues, enable debug logging -- it is the primary diagnostic tool for most problems.

## Using Debug Logging

par-term writes debug logs to a file rather than the terminal, so logging never interferes with your session.

**Log file location:**

| Platform | Path |
|----------|------|
| macOS/Linux | `/tmp/par_term_debug.log` |
| Windows | `%TEMP%\par_term_debug.log` |

**Enabling debug logging:**

```bash
# Via command-line flag (highest priority)
par-term --log-level debug

# Via environment variable (also mirrors to stderr)
RUST_LOG=debug par-term

# Via config file (~/.config/par-term/config.yaml)
log_level: debug
```

**Log levels** (from least to most verbose): `off`, `error`, `warn`, `info`, `debug`, `trace`

**Monitoring logs in real time:**

```bash
tail -f /tmp/par_term_debug.log
```

**Filtering logs by component:**

```bash
tail -f /tmp/par_term_debug.log | grep --line-buffered "shader"
tail -f /tmp/par_term_debug.log | grep --line-buffered "terminal"
tail -f /tmp/par_term_debug.log | grep --line-buffered "renderer"
```

**Capturing logs for a bug report:**

```bash
# Start with trace logging, reproduce the issue, then exit
par-term --log-level trace

# Copy the log file
cp /tmp/par_term_debug.log ~/Desktop/par-term-debug.log
```

The Settings UI also provides debug logging controls under **Settings > Advanced > Debug Logging**, including a log level dropdown, log file path display, and an Open Log File button. Changes take effect immediately without restarting.

> **Note:** Some internal components use custom `debug_*!()` macros controlled by the `DEBUG_LEVEL` environment variable (separate from `log_level`). Set `DEBUG_LEVEL=4` for maximum custom debug output.

## Installation Issues

### macOS Gatekeeper Blocking the App

**Symptom:** macOS reports that par-term "is damaged and can't be opened" or shows an "unidentified developer" warning.

**Cause:** macOS applies a quarantine attribute to binaries downloaded from the internet. Since par-term release binaries are not notarized with an Apple Developer ID, Gatekeeper blocks execution.

**Solution:**

Remove the quarantine attribute:

```bash
# For the release binary
xattr -cr target/release/par-term

# For the .app bundle
xattr -cr /Applications/par-term.app
```

> **Note:** The Homebrew cask install (`brew install --cask paulrobello/tap/par-term`) handles this automatically.

### Missing Linux Dependencies

**Symptom:** Build errors or runtime crashes on Linux related to missing libraries (GTK, X11, Wayland, XCB, audio).

**Cause:** par-term requires GTK3 and X11/Wayland libraries for window management and display.

**Solution:**

Install the required packages for your distribution:

**Ubuntu/Debian:**

```bash
sudo apt install libgtk-3-dev libxkbcommon-dev libwayland-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libasound2-dev
```

**Fedora/RHEL:**

```bash
sudo dnf install gtk3-devel libxkbcommon-devel wayland-devel libxcb-devel alsa-lib-devel
```

**Arch Linux:**

```bash
sudo pacman -S gtk3 libxkbcommon wayland libxcb alsa-lib
```

### Build From Source Failures

**Symptom:** Compilation errors when running `cargo build` or `make build`.

**Cause:** Missing Rust toolchain, outdated compiler version, or missing system dependencies.

**Solution:**

1. Ensure you have Rust 1.85+ installed (2024 edition required):

   ```bash
   rustup update stable
   rustc --version
   ```

2. On Linux, install the required system dependencies listed in [Missing Linux Dependencies](#missing-linux-dependencies).

3. Ensure modern graphics drivers are installed (Vulkan on Linux, Metal on macOS, DirectX 12 on Windows).

4. Clean and rebuild:

   ```bash
   cargo clean
   cargo build --release
   ```

## Display and Rendering Issues

### Black Screen or No Output

**Symptom:** The terminal window opens but displays a black screen with no text or cursor.

**Cause:** GPU driver incompatibility, missing graphics backend, or wgpu initialization failure.

**Solution:**

1. Update your graphics drivers to the latest version.
2. Check the debug log for GPU-related errors:

   ```bash
   par-term --log-level debug
   grep -i "wgpu\|gpu\|surface\|adapter" /tmp/par_term_debug.log
   ```

3. On Linux, verify Vulkan support:

   ```bash
   vulkaninfo --summary
   ```

4. If using a virtual machine or remote desktop, ensure GPU passthrough or software rendering is available.

### Font Rendering Problems

**Symptom:** Characters appear as blank squares (tofu), fonts look blurry, or incorrect glyphs display.

**Cause:** Missing fonts, font fallback chain not finding the correct glyphs, or font hinting settings.

**Solution:**

1. Ensure your configured font is installed on the system. par-term defaults to a built-in font if the configured one is not found.
2. Toggle font hinting in **Settings > Appearance > Font Hinting** -- enabling hinting improves sharpness at common sizes.
3. Install a Nerd Font for icon support in the tab bar and profile picker.
4. Check font-related messages in the debug log:

   ```bash
   par-term --log-level debug
   grep -i "font\|glyph\|shap" /tmp/par_term_debug.log
   ```

### HiDPI and Scaling Issues

**Symptom:** UI elements appear too large, too small, or incorrectly scaled on high-DPI displays.

**Cause:** Pixel-dimension config values or window positions not scaling correctly for the display's DPI factor.

**Solution:**

1. Ensure your operating system's display scaling is configured correctly.
2. par-term auto-detects the display scale factor. If text or UI elements appear incorrect, try adjusting the font size in **Settings > Appearance**.
3. For multi-monitor setups with mixed DPI values, par-term stores window positions in logical pixels and applies per-monitor DPI conversion automatically. If positions seem off, save and restore a window arrangement to recalibrate.

### Inline Graphics Not Displaying

**Symptom:** Sixel, iTerm2, or Kitty inline images do not appear in the terminal.

**Cause:** The graphics protocol is not enabled, the image data is too large, or the terminal is in split-pane mode with an older version.

**Solution:**

1. Verify inline graphics support is enabled -- par-term supports Sixel, iTerm2, and Kitty protocols by default.
2. Ensure the image data is valid. Test with a known working image:

   ```bash
   # iTerm2 protocol test
   printf '\033]1337;File=inline=1;size=%d:%s\a' "$(wc -c < image.png)" "$(base64 < image.png)"
   ```

3. Check the debug log for graphics-related messages:

   ```bash
   par-term --log-level debug
   grep -i "sixel\|iterm\|kitty\|graphic\|image" /tmp/par_term_debug.log
   ```

## Shader Issues

par-term has two separate shader systems -- **background shaders** (`custom_shader`) and **cursor shaders** (`cursor_shader`). When debugging one type, temporarily disable the other to isolate the issue.

### Shader Not Loading

**Symptom:** No visual effect after enabling a shader.

**Cause:** Shader file not found, shader not enabled in config, or configuration not reloaded.

**Solution:**

1. Verify the shader file exists in `~/.config/par-term/shaders/`
2. Check that `custom_shader_enabled: true` is set in your config
3. Press `F5` to reload configuration
4. Check the debug log for shader loading errors

### Black or White Screen With Shader

**Symptom:** Terminal content becomes invisible after enabling a shader.

**Cause:** The shader is not outputting correct alpha values or UV coordinates are out of range.

**Solution:**

1. Ensure `fragColor.a = 1.0` for opaque output in your shader
2. Verify UV coordinates are in the 0.0 to 1.0 range
3. Check that `texture(iChannel4, uv)` is sampling terminal content correctly
4. Try switching to background-only mode: `custom_shader_full_content: false`

### Text Hard to Read With Shader

**Symptom:** Text is blurry, distorted, or the shader background is too bright to read against.

**Cause:** Full-content mode distorting text, text opacity too low, or shader brightness too high.

**Solution:**

1. Use background-only mode: set `custom_shader_full_content: false`
2. Increase text opacity: `custom_shader_text_opacity: 1.0`
3. Lower shader brightness: `custom_shader_brightness: 0.3` (range: 0.05 to 1.0)
4. Reduce effect intensity within the shader code itself

### Shader Compilation Errors

**Symptom:** Error messages in the log about GLSL compilation failures.

**Cause:** Incompatible GLSL syntax or unsupported constructs.

**Solution:**

Common GLSL fixes:

- Use `texture()` not `texture2D()`
- Declare constants with the `const` keyword
- Use proper array syntax: `vec3[N] arr = vec3[N](...)`
- Do not include a `#version` directive (par-term adds it automatically)
- Expand `mat2(vec4)` to `mat2(v.x, v.y, v.z, v.w)` for GLSL 450 compatibility

Check the transpiled output for details -- see [Debugging Shaders](#debugging-shaders).

### Low Frame Rate With Shaders

**Symptom:** Stuttering or choppy animation when shaders are active.

**Cause:** Shader complexity exceeds GPU capacity at the target frame rate.

**Solution:**

1. Reduce shader complexity (fewer loops, simpler math)
2. Lower animation speed: `custom_shader_animation_speed: 0.5`
3. Disable animation entirely: `custom_shader_animation: false`
4. Enable power saving: `pause_shaders_on_blur: true` to pause animations when the window loses focus

### Default Cursor Showing Through Cursor Shader

**Symptom:** The default block/beam cursor is visible behind or through your cursor shader effect.

**Cause:** The renderer is drawing the default cursor in addition to the cursor shader.

**Solution:**

Set `cursor_shader_hides_cursor: true` in your config. This tells the renderer to skip drawing the default cursor when a cursor shader is active. Recommended for shaders that fully replace the cursor appearance.

### Debugging Shaders

par-term writes intermediate shader files to `/tmp/` for debugging:

| File | Description |
|------|-------------|
| `/tmp/par_term_<shader_name>_shader.wgsl` | Transpiled WGSL output for each shader |
| `/tmp/par_term_debug_wrapped.glsl` | Wrapped GLSL input (last shader processed) |

Enable `shader_hot_reload: true` in your config for faster iteration during shader development. Set `shader_hot_reload_delay: 100` (milliseconds) for the debounce interval.

## Terminal Behavior Issues

### Shell Integration Not Working

**Symptom:** Directory tracking, prompt navigation, or command notifications do not work. Tab titles do not update to show the current directory.

**Cause:** Shell integration script not installed, not sourced, or the shell session was not restarted after installation.

**Solution:**

1. Install shell integration:

   ```bash
   par-term install-shell-integration
   ```

   Or open **Settings > Integrations** and click **Install Shell Integration**.

2. Restart your shell or source the RC file:

   ```bash
   source ~/.bashrc    # bash
   source ~/.zshrc     # zsh
   ```

3. Verify the integration script exists:

   ```bash
   ls ~/.config/par-term/shell_integration.*
   ```

4. Check that your shell RC file contains the source line:

   ```bash
   # >>> par-term shell integration >>>
   [ -f "$HOME/.config/par-term/shell_integration.bash" ] && source "$HOME/.config/par-term/shell_integration.bash"
   # <<< par-term shell integration <<<
   ```

5. If reinstalling, uninstall first:

   ```bash
   par-term uninstall-shell-integration
   par-term install-shell-integration
   ```

### Keyboard Shortcuts Not Recognized

**Symptom:** Keybindings do not trigger the expected action, or modifier keys seem wrong.

**Cause:** On Linux/Windows, par-term uses `Ctrl+Shift` modifiers (not plain `Ctrl`) for most shortcuts to avoid conflicts with terminal control codes like `Ctrl+C`. macOS uses `Cmd`.

**Solution:**

1. Check the correct modifier for your platform:
   - **macOS**: `Cmd` for most shortcuts
   - **Linux/Windows**: `Ctrl+Shift` for most shortcuts
2. Review your keybindings in **Settings > Input > Keybindings**
3. See [KEYBOARD_SHORTCUTS.md](KEYBOARD_SHORTCUTS.md) for the complete shortcut reference
4. Custom keybindings in `config.yaml` override defaults -- check for conflicts

### Copy and Paste Issues

**Symptom:** `Cmd+V` or `Ctrl+V` does not paste, or copied text is empty.

**Cause:** On macOS, the system menu accelerators (muda) intercept `Cmd+V/C/A` before the terminal processes them. Clicking the window to focus it can clear the clipboard selection.

**Solution:**

1. Ensure you are using the correct shortcut for your platform (`Cmd+C/V` on macOS, `Ctrl+Shift+C/V` on Linux/Windows)
2. If paste does not work in the settings window, par-term injects paste events directly -- this is handled automatically
3. For clipboard images, `Cmd+V` forwards to the terminal when the clipboard contains an image but no text
4. If focus-clicking clears your clipboard, this is a known mitigation where the first mouse click that focuses the window is suppressed

### Mouse Behavior Issues

**Symptom:** Mouse clicks or scroll events behave unexpectedly, or text selection triggers accidentally on trackpad.

**Cause:** Applications running in the terminal may capture mouse events (mouse tracking mode). Trackpad jitter can cause micro-selections.

**Solution:**

1. To select text when an application has mouse tracking enabled, hold `Shift` while clicking or dragging to bypass the application's mouse capture
2. par-term includes a drag dead-zone to suppress accidental micro-selections from trackpad jitter
3. Check mouse-related settings in **Settings > Input > Mouse**

## SSH and Remote Issues

### Hosts Not Appearing in Quick Connect

**Symptom:** The SSH Quick Connect dialog (`Cmd+Shift+S` / `Ctrl+Shift+S`) shows no hosts or is missing expected entries.

**Cause:** SSH config file is missing or contains only wildcard entries, known_hosts is unreadable, or mDNS is not enabled.

**Solution:**

1. Verify `~/.ssh/config` exists and contains valid `Host` entries (not just `Host *` wildcards)
2. Check that `~/.ssh/known_hosts` is readable
3. For network-discovered hosts, enable mDNS in **Settings > SSH > mDNS/Bonjour Discovery** or set `enable_mdns_discovery: true` in config

### Profile Not Auto-Switching on SSH

**Symptom:** Connecting to a remote host does not trigger automatic profile switching.

**Cause:** Auto-switch is disabled, profile hostname patterns do not match, or shell integration is not installed on the remote host.

**Solution:**

1. Ensure `ssh_auto_profile_switch: true` in your config
2. Verify the profile has `hostname_patterns` that match the remote hostname
3. Install shell integration on the remote host for OSC 1337 hostname reporting (see [Shell Integration on Remote Hosts](#shell-integration-on-remote-hosts))

### mDNS Hosts Not Appearing

**Symptom:** Local network SSH hosts are not discovered in the Quick Connect dialog.

**Cause:** mDNS discovery is disabled, scan timeout is too short, or remote hosts do not advertise the SSH service.

**Solution:**

1. Enable mDNS in **Settings > SSH > mDNS/Bonjour Discovery** or set `enable_mdns_discovery: true`
2. Increase `mdns_scan_timeout_secs` for slower networks (range: 1 to 10 seconds)
3. Ensure remote hosts advertise `_ssh._tcp` via Bonjour (macOS) or Avahi (Linux)

### Shell Integration on Remote Hosts

**Symptom:** Directory tracking and prompt navigation do not work in SSH sessions.

**Cause:** Shell integration is only installed locally and needs to be installed on each remote host separately.

**Solution:**

1. From the menu bar while connected to a remote host: **Shell > Install Shell Integration on Remote Host...**
2. Or run manually in the SSH session:

   ```bash
   curl -sSL https://paulrobello.github.io/par-term/install-shell-integration.sh | sh
   ```

3. Restart the remote shell after installation for changes to take effect
4. Requirements: `curl` must be available on the remote host and you need write permission to modify shell RC files

## Performance Issues

### High CPU Usage

**Symptom:** par-term consumes excessive CPU even when idle.

**Cause:** Animated shaders rendering continuously, high refresh rate, or frequent terminal redraws.

**Solution:**

1. Enable power saving when the window is not focused:

   ```yaml
   pause_shaders_on_blur: true
   pause_refresh_on_blur: true
   unfocused_fps: 30
   ```

2. Disable animated shaders if not needed: `custom_shader_animation: false`
3. Reduce animation speed: `custom_shader_animation_speed: 0.5`
4. Check if a background process in the terminal is producing continuous output

### Slow Rendering

**Symptom:** Visible lag when scrolling, typing, or resizing the terminal.

**Cause:** Complex shaders, large scrollback buffer, or GPU driver performance.

**Solution:**

1. Reduce shader complexity or disable shaders temporarily to isolate the cause
2. Update GPU drivers to the latest version
3. Check the debug log for rendering bottlenecks:

   ```bash
   par-term --log-level debug
   grep -i "render\|frame\|fps" /tmp/par_term_debug.log
   ```

### High Memory Usage

**Symptom:** par-term memory usage grows over time.

**Cause:** Large scrollback buffers, many open tabs, or cached inline graphics.

**Solution:**

1. Reduce the scrollback buffer size in **Settings > Terminal > Scrollback Lines**
2. Close unused tabs
3. Large inline images are cached in RGBA texture memory -- closing tabs with many inline images frees this memory

## Configuration Issues

### Config Not Loading

**Symptom:** Settings revert to defaults on startup, or config changes have no effect.

**Cause:** Config file is missing, has syntax errors, or is in the wrong location.

**Solution:**

1. Verify the config file location:
   - **macOS/Linux**: `~/.config/par-term/config.yaml`
   - **Windows**: `%APPDATA%\par-term\config.yaml`

2. Check for YAML syntax errors. A common issue is incorrect indentation or missing quotes:

   ```bash
   # Validate YAML syntax
   python3 -c "import yaml; yaml.safe_load(open('$HOME/.config/par-term/config.yaml'))"
   ```

3. Press `F5` to reload the configuration without restarting par-term
4. Check the debug log for config-related errors:

   ```bash
   par-term --log-level debug
   grep -i "config\|yaml\|parse" /tmp/par_term_debug.log
   ```

### Settings Not Saving

**Symptom:** Changes made in the Settings UI revert after closing or restarting par-term.

**Cause:** The settings were not explicitly saved, or the config file is not writable.

**Solution:**

1. In the Settings UI, ensure you click **Save** after making changes
2. Verify the config directory is writable:

   ```bash
   ls -la ~/.config/par-term/
   ```

3. Check that no other process has the config file locked

### Profile Switching Issues

**Symptom:** Profiles do not apply correctly, or the wrong profile activates.

**Cause:** Profile order matters for auto-switching (first match wins), or directory patterns do not match the current path.

**Solution:**

1. Check profile order in **Settings > Profiles** -- the first matching profile wins
2. For directory-based switching, verify patterns use correct glob syntax:
   - Patterns support `~` for home directory expansion
   - Pattern matching is case-sensitive
3. Directory switching requires shell integration for OSC 7 CWD tracking
4. Profiles are stored in `~/.config/par-term/profiles.yaml`

### Arrangement Restore Issues

**Symptom:** Restoring a window arrangement places windows on the wrong monitor, or tabs appear with incorrect properties.

**Cause:** Monitor layout has changed since the arrangement was saved, or the arrangement file is corrupted.

**Solution:**

1. par-term maps saved monitors to current monitors by name first, then by index, and falls back to the primary monitor. If your monitor configuration has changed, windows may land on a different display.
2. Window positions are clamped to ensure at least 100 pixels remain visible -- if a window appears in an unexpected position, this clamping moved it onto the visible area.
3. Save a new arrangement to capture the current monitor layout.
4. Arrangements are stored in:
   - **macOS/Linux**: `~/.config/par-term/arrangements.yaml`
   - **Windows**: `%APPDATA%\par-term\arrangements.yaml`

## Update Issues

### Self-Update Refused

**Symptom:** Running `par-term self-update` displays a message about Homebrew or cargo installation and refuses to update.

**Cause:** par-term detected that it was installed via a package manager (Homebrew or cargo) and will not perform in-place updates to avoid inconsistencies with the package manager's tracking.

**Solution:**

Use the appropriate package manager command:

```bash
# Homebrew
brew upgrade --cask par-term

# Cargo
cargo install par-term
```

### Platform Binary Not Found

**Symptom:** Error: "Could not find par-term-<platform> in the latest release"

**Cause:** The latest GitHub release does not contain a binary for your OS and architecture.

**Solution:**

Check the [releases page](https://github.com/paulrobello/par-term/releases) for supported platforms. If your platform is not listed, build from source.

### Update Failed Network Error

**Symptom:** Error: "Failed to fetch release info"

**Cause:** Network connectivity issue or GitHub API rate limit.

**Solution:**

1. Check your internet connection
2. Try again later -- GitHub enforces API rate limits
3. par-term enforces a minimum one-hour interval between API requests to avoid rate limiting

### Permission Denied During Update

**Symptom:** Error: "Failed to replace binary: Permission denied"

**Cause:** The running process does not have write access to its installation directory.

**Solution:**

On Linux, if the binary is in a system directory (e.g., `/usr/local/bin/`), you may need elevated permissions. Consider installing to a user-writable location or using a package manager instead.

### Updated App Blocked by Gatekeeper

**Symptom:** After updating, macOS reports the app "is damaged and can't be opened."

**Cause:** macOS quarantine attributes were applied to the downloaded update. The self-updater normally strips these via `xattr -cr`, but this can fail if the update was interrupted.

**Solution:**

```bash
xattr -cr /Applications/par-term.app
```

If the problem persists, verify that the binary is not corrupted by comparing its checksum against the release asset on the [GitHub releases page](https://github.com/paulrobello/par-term/releases).

### Old Version Still Running After Update

**Symptom:** After a successful update, `par-term --version` still shows the old version.

**Cause:** The update replaces the binary on disk, but the running process continues using the old version until restarted.

**Solution:**

Close and reopen par-term. A restart is required after every update to use the new version.

## Getting Help

If the solutions in this guide do not resolve your issue:

1. **Collect diagnostic information:**
   - Start par-term with `--log-level trace`
   - Reproduce the issue
   - Copy `/tmp/par_term_debug.log` for your bug report

2. **Include system information:**
   - Operating system and version
   - GPU model and driver version
   - par-term version (`par-term --version`)
   - Installation method (Homebrew, cargo, standalone binary, app bundle)

3. **Config file location:**
   - **macOS/Linux**: `~/.config/par-term/config.yaml`
   - **Windows**: `%APPDATA%\par-term\config.yaml`

4. **Report bugs** on GitHub Issues: [https://github.com/paulrobello/par-term/issues](https://github.com/paulrobello/par-term/issues)

## Related Documentation

- [Debug Logging](LOGGING.md) - Full logging configuration and module filtering
- [Custom Shaders Guide](CUSTOM_SHADERS.md) - Shader creation, uniforms, and debugging
- [Integrations](INTEGRATIONS.md) - Shell integration installation and management
- [SSH Host Management](SSH.md) - SSH profiles, discovery, and auto-switching
- [Profiles](PROFILES.md) - Profile system, auto-switching, and storage
- [Arrangements](ARRANGEMENTS.md) - Window layout save and restore
- [Self-Update](SELF_UPDATE.md) - Update checking, installation types, and CLI usage
- [Keyboard Shortcuts](KEYBOARD_SHORTCUTS.md) - Complete keybinding reference
- [Content Prettifier](PRETTIFIER.md) - Content detection and rendering configuration
- [Architecture Overview](ARCHITECTURE.md) - System design and component overview
