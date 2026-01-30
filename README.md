# par-term

[![Crates.io](https://img.shields.io/crates/v/par-term)](https://crates.io/crates/par-term)
![Runs on Linux | MacOS | Windows](https://img.shields.io/badge/runs%20on-Linux%20%7C%20MacOS%20%7C%20Windows-blue)
![Arch x86-64 | ARM | AppleSilicon](https://img.shields.io/badge/arch-x86--64%20%7C%20ARM%20%7C%20AppleSilicon-blue)
![Crates.io Downloads](https://img.shields.io/crates/d/par-term)
![License](https://img.shields.io/badge/license-MIT-green)

A cross-platform, GPU-accelerated terminal emulator frontend built with Rust, powered by [par-term-emu-core-rust](https://github.com/paulrobello/par-term-emu-core-rust). Designed for high performance, modern typography, and rich graphics support.

[!["Buy Me A Coffee"](https://www.buymeacoffee.com/assets/img/custom_images/orange_img.png)](https://buymeacoffee.com/probello3)

![par-term screenshot](https://raw.githubusercontent.com/paulrobello/par-term/main/screenshot.png)

## What's New in 0.5.0

### 🪟 Standalone Settings Window

Settings moved from an overlay to a dedicated window for a better editing experience.

- **Keyboard Shortcuts**: `F12` or `Cmd+,` (macOS) / `Ctrl+,` (Linux/Windows)
- **Stay Visible**: Settings window automatically brought to front when terminal gains focus
- **Edit While Viewing**: Configure settings while terminal content remains fully visible

### 🎨 Per-Shader Configuration System

Advanced 3-tier configuration for background and cursor shaders.

- **Shader Metadata**: Embed defaults directly in GLSL files (`/*! par-term shader metadata ... */`)
- **Per-Shader Overrides**: Customize each shader independently in `shader_configs` section
- **Global Fallback**: Unspecified values fall back to global config
- **Save to Shader**: "Save Defaults to Shader" button writes settings back to shader files
- **Shader Hot Reload**: Auto-reload shaders when files change (`shader_hot_reload: true`)

### 🔤 Enhanced Unicode Rendering

Pixel-perfect rendering for complex Unicode and TUI applications.

- **Grapheme Clusters**: Proper rendering of flag emoji (🇺🇸), ZWJ sequences (👨‍👩‍👧‍👦), skin tone modifiers (👋🏽)
- **Box Drawing**: Geometric rendering for all box drawing characters (─ │ ┌ ┐ └ ┘ ├ ┤ etc.)
- **Block Elements**: Solid, partial, and quadrant blocks (█ ▄ ▀ ▐ ▌) render without gaps
- **DECSCUSR**: Applications can change cursor style dynamically via escape sequences

### 🗂️ Tab Bar Enhancements

Improved tab management with full color customization.

- **Color Configuration**: 11 new options for complete tab bar styling
- **Per-Tab Colors**: Right-click context menu to set individual tab colors
- **Equal-Width Layout**: Tabs spread evenly with horizontal scrolling when needed
- **Inactive Dimming**: Visual distinction with configurable opacity for inactive tabs
- **Border Styling**: Configurable tab borders with active tab highlighting

### 🔒 Window Transparency

Proper transparency support across platforms.

- **macOS Blur**: Window blur effect via CGS private API
- **Alpha Handling**: Correct alpha mode selection based on surface capabilities
- **Text Clarity**: `keep_text_opaque` option maintains readable text at low opacity
- **Selective Transparency**: `transparency_affects_only_default_background` preserves colored backgrounds

### 🎮 Shader System Improvements

- **Cubemap Support**: Load 6-face cubemap textures for environment reflections
- **iTimeKeyPress**: Track key presses for typing effect shaders
- **use_background_as_channel0**: Use app's background image in shader effects
- **9 New Shaders**: rain, singularity, universe-within, convergence, gyroid, dodecagon-pattern, arcane-portal, bumped_sinusoidal_warp, keypress_pulse

### 🔋 Power Saving

- **pause_shaders_on_blur**: Pause shader animations when window unfocused (default: true)
- **pause_refresh_on_blur**: Reduce refresh rate when unfocused
- **unfocused_fps**: Configurable FPS when not in focus (default: 30)

### 🖱️ Cursor Lock Options

Prevent applications from overriding your cursor preferences.

- **lock_cursor_visibility**: Block apps from hiding cursor
- **lock_cursor_style**: Block apps from changing cursor shape
- **lock_cursor_blink**: Block apps from enabling blink when you've disabled it

### 📋 Terminal Improvements

- **Bracketed Paste**: Proper paste handling for modern shells (bash 4.4+, zsh, fish)
- **Resize Overlay**: Shows cols×rows and pixel dimensions during window resize
- **Grid-Based Sizing**: Initial window size calculated from cols×rows (no startup resize flash)

<details>
<summary><strong>What's New in 0.4.0</strong></summary>

### Multi-Tab Support
- `Cmd/Ctrl+T` new tab, `Cmd/Ctrl+W` close tab
- `Cmd/Ctrl+Shift+[/]` or `Ctrl+Tab` to switch tabs
- `Cmd/Ctrl+1-9` direct tab access
- Tab bar with close buttons, activity indicators, bell icons

### Multi-Window Support
- `Cmd/Ctrl+N` new window with independent PTY session
- Each window has its own tabs, scrollback, and state

### Native Menu Bar
- Cross-platform menus via [muda](https://github.com/tauri-apps/muda)
- Full keyboard accelerators for all menu items

### Custom Shader Enhancements
- Shadertoy-compatible iChannel1-4 texture support
- `custom_shader_brightness` for better text readability
- `cursor_shader_hides_cursor` for shader-controlled cursors

</details>

<details>
<summary><strong>What's New in 0.3.0</strong></summary>

### Ghostty-Compatible Cursor Shaders
- `iCurrentCursor`, `iPreviousCursor`, `iCurrentCursorColor`, `iTimeCursorChange` uniforms
- Built-in cursor shaders: sweep, warp, glow, blaze, trail, ripple, boom
- Geometric cursor rendering for all styles

### Fixes
- Login shell initialization and environment loading

</details>

<details>
<summary><strong>What's New in 0.2.0</strong></summary>

### Power Efficiency
- Event-driven rendering with `ControlFlow::Wait`
- Smart redraws only when content changes

### Stability
- Fixed dropped input during heavy rendering
- `parking_lot` mutex migration
- Graceful audio fallback

</details>

## Features

### Core Terminal Frontend
- **Cross-platform Support**: Native performance on macOS (Metal), Linux (Vulkan/X11/Wayland), and Windows (DirectX 12).
- **Multi-Window & Multi-Tab**: Multiple windows with independent tab sessions per window.
- **GPU-Accelerated Rendering**: Powered by `wgpu` with custom glyph atlas for blazing-fast text rasterization.
- **Inline Graphics**: Full support for Sixel, iTerm2, and Kitty graphics protocols.
- **Real PTY Integration**: Full pseudo-terminal support for interactive shell sessions.
- **Advanced Sequence Support**: VT100/VT220/VT320/VT420/VT520 compatibility via `par-term-emu-core-rust`.
- **Intelligent Reflow**: Full content reflow on window resize, preserving scrollback and visible state.

### Modern UI & Visuals
- **Custom GLSL Shaders**: 49+ included shaders with hot reload, per-shader config, and cubemap support.
- **Background Images**: Support for PNG/JPEG backgrounds with configurable opacity and scaling modes.
- **Window Transparency**: True per-pixel alpha with macOS blur support and text clarity options.
- **Visual Bell**: Flash-based alerts for terminal bell events.
- **Dynamic Themes**: Support for iTerm2-style color schemes (Dracula, Monokai, Solarized, etc.).
- **Standalone Settings**: Dedicated settings window (F12) for live configuration editing.

### Typography & Fonts
- **Styled Font Variants**: Explicit support for separate Bold, Italic, and Bold-Italic font families.
- **Unicode Range Mapping**: Assign specific fonts to Unicode ranges (perfect for CJK, Emoji, or Symbols).
- **Text Shaping**: HarfBuzz-powered shaping for ligatures, complex scripts, and emoji sequences.
- **Grapheme Clusters**: Proper rendering of flag emoji, ZWJ sequences, skin tone modifiers.
- **Box Drawing**: Geometric rendering for pixel-perfect TUI borders and block characters.
- **Smart Fallback**: Automatic system font discovery and fallback chain.

### Selection & Clipboard
- **Advanced Selection**: Block/Rectangular, Line-based, and Word-based selection modes.
- **Multi-platform Clipboard**: Seamless integration with system clipboards via `arboard`.
- **Middle-click Paste**: Standard Unix-style middle-click paste support.
- **Automatic Copy**: Optional "copy on select" behavior.

### Hyperlinks & URL Detection
- **OSC 8 Support**: Native support for application-provided hyperlinks.
- **Regex Detection**: Automatic detection of URLs in terminal output.
- **Interactive Links**: Ctrl+Click to open links in your default browser, with hover highlighting and tooltips.

## Documentation

- **[Quick Start Guide](QUICK_START_FONTS.md)** - Get up and running with custom fonts.
- **[Architecture Overview](docs/ARCHITECTURE.md)** - High-level system architecture and components.
- **[Shader Gallery](https://paulrobello.github.io/par-term/)** - Visual gallery of 49+ included shaders with screenshots.
- **[Custom Shaders Guide](docs/CUSTOM_SHADERS.md)** - Create custom GLSL shaders with hot reload and per-shader config.
- **[Compositor Details](docs/COMPOSITOR.md)** - Deep dive into the rendering architecture.
- **[Examples](examples/README.md)** - Comprehensive configuration examples.
- **[Core Library](https://github.com/paulrobello/par-term-emu-core-rust)** - Documentation for the underlying terminal engine.

## Installation

### Homebrew (macOS)

```bash
brew install --cask paulrobello/tap/par-term
```

### From Source

Requires Rust 1.85+ (2024 edition) and modern graphics drivers:

```bash
# Clone the repository
git clone https://github.com/paulrobello/par-term
cd par-term

# Build and run (debug)
cargo run

# Build optimized release version
cargo build --release
./target/release/par-term
```

### macOS Bundle

To create a native macOS `.app` bundle with a dock icon:

```bash
make bundle
make run-bundle
```

### Linux Dependencies

On Linux (Ubuntu/Debian), you need GTK3 and X11/Wayland libraries:
```bash
sudo apt install libgtk-3-dev libxkbcommon-dev libwayland-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libasound2-dev
```

## Installing Shaders

par-term includes 49+ custom GLSL shaders for background effects and cursor animations. These need to be installed to your config directory.

### Built-in Installer (Recommended)

Use the built-in CLI command to download and install all shaders from the latest release:

```bash
# Install shaders (with confirmation prompt)
par-term install-shaders

# Install without prompts
par-term install-shaders -y

# Force overwrite existing shaders
par-term install-shaders --force
```

### Shell Script

Alternatively, use the shell script installer:

```bash
# Download and run the installer
curl -sL https://raw.githubusercontent.com/paulrobello/par-term/main/install_shaders.sh | sh
```

Or download and run manually:
```bash
curl -O https://raw.githubusercontent.com/paulrobello/par-term/main/install_shaders.sh
chmod +x install_shaders.sh
./install_shaders.sh
```

### Manual Install

1. Download `shaders.zip` from the [latest release](https://github.com/paulrobello/par-term/releases/latest)
2. Extract to your config directory:
   - **macOS/Linux**: `~/.config/par-term/shaders/`
   - **Windows**: `%APPDATA%\par-term\shaders\`

### From Source

If building from source, copy the shaders folder manually:
```bash
# macOS/Linux
cp -r shaders ~/.config/par-term/

# Windows (PowerShell)
Copy-Item -Recurse shaders $env:APPDATA\par-term\
```

### Using Shaders

Once installed, enable shaders in your `config.yaml`:
```yaml
# Background shader
custom_shader: "starfield.glsl"
custom_shader_enabled: true

# Cursor shader
cursor_shader: "cursor_glow.glsl"
cursor_shader_enabled: true
```

See the [Shader Gallery](docs/SHADERS.md) for previews of all included shaders.

## Keyboard Shortcuts

### Window & Tab Management

| Shortcut | Action |
|----------|--------|
| `Cmd/Ctrl + N` | New window |
| `Cmd/Ctrl + T` | New tab |
| `Cmd/Ctrl + W` | Close tab (or window if single tab) |
| `Cmd/Ctrl + Q` | Quit (Windows/Linux) |
| `Cmd/Ctrl + Shift + ]` | Next tab |
| `Cmd/Ctrl + Shift + [` | Previous tab |
| `Ctrl + Tab` | Next tab (alternative) |
| `Ctrl + Shift + Tab` | Previous tab (alternative) |
| `Cmd/Ctrl + 1-9` | Switch to tab 1-9 |
| `Cmd/Ctrl + Shift + Left` | Move tab left |
| `Cmd/Ctrl + Shift + Right` | Move tab right |

### Navigation & Editing

| Shortcut | Action |
|----------|--------|
| `PageUp` / `PageDown` | Scroll up/down one page |
| `Shift + Home` | Jump to top of scrollback |
| `Shift + End` | Jump to bottom (current) |
| `Cmd/Ctrl + C` | Copy selection |
| `Cmd/Ctrl + V` | Paste from clipboard |
| `Cmd/Ctrl + Shift + K` | Clear scrollback buffer |
| `Cmd/Ctrl + Shift + H` | Clipboard history |
| `Ctrl + L` | Clear visible screen |
| `Cmd/Ctrl + +/-/0` | Adjust font size / Reset |
| `Ctrl + Shift + S` | Take screenshot |

### UI Toggles

| Shortcut | Action |
|----------|--------|
| `F1` | Toggle Help panel |
| `F3` | Toggle FPS overlay |
| `F5` | Reload configuration |
| `F11` | Toggle fullscreen |
| `F12` | Open Settings window |
| `Cmd + ,` / `Ctrl + ,` | Open Settings window (alternative) |

## Configuration

Configuration is stored in YAML format:
- **Unix**: `~/.config/par-term/config.yaml`
- **Windows**: `%APPDATA%\par-term\config.yaml`

```yaml
cols: 80
rows: 24
font_size: 13.0
font_family: "JetBrains Mono"
theme: "dark-background"
window_opacity: 0.95
scrollbar_position: "right"

# Tab bar settings
tab_bar_mode: "when_multiple"  # always, when_multiple, never
tab_bar_height: 28.0
tab_show_close_button: true
tab_inherit_cwd: true
dim_inactive_tabs: true
inactive_tab_opacity: 0.6

# Transparency settings
keep_text_opaque: true
transparency_affects_only_default_background: true
blur_radius: 8  # macOS only

# Power saving
pause_shaders_on_blur: true
unfocused_fps: 30

# Cursor lock options (prevent apps from overriding)
lock_cursor_visibility: false
lock_cursor_style: false
lock_cursor_blink: false

# Custom shader settings
custom_shader: "starfield.glsl"
custom_shader_enabled: true
shader_hot_reload: true  # Auto-reload on file changes

# Per-shader overrides (optional)
shader_configs:
  starfield.glsl:
    animation_speed: 0.8
    brightness: 0.3
```

See `examples/config-complete.yaml` for a full list of options.

## Technology

- **Terminal Engine**: [par-term-emu-core-rust](https://github.com/paulrobello/par-term-emu-core-rust)
- **Graphics**: `wgpu` (WebGPU for Rust)
- **Text**: `swash` + `rustybuzz` (custom glyph atlas)
- **UI**: `egui` for settings and overlays
- **Windowing**: `winit`
- **Async**: `tokio`

## Contributing

Contributions are welcome! Please ensure you run `make checkall` before submitting any pull requests.

```bash
make fmt       # Format code
make lint      # Run clippy
make test      # Run test suite
make checkall  # Run all of the above
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Author

Paul Robello - probello@gmail.com

## Links

- **GitHub**: [https://github.com/paulrobello/par-term](https://github.com/paulrobello/par-term)
- **Core Library**: [https://github.com/paulrobello/par-term-emu-core-rust](https://github.com/paulrobello/par-term-emu-core-rust)