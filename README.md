# par-term

[![Crates.io](https://img.shields.io/crates/v/par-term)](https://crates.io/crates/par-term)
![Runs on Linux | MacOS | Windows](https://img.shields.io/badge/runs%20on-Linux%20%7C%20MacOS%20%7C%20Windows-blue)
![Arch x86-64 | ARM | AppleSilicon](https://img.shields.io/badge/arch-x86--64%20%7C%20ARM%20%7C%20AppleSilicon-blue)
![Crates.io Downloads](https://img.shields.io/crates/d/par-term)
![License](https://img.shields.io/badge/license-MIT-green)

A cross-platform, GPU-accelerated terminal emulator frontend built with Rust, powered by [par-term-emu-core-rust](https://github.com/paulrobello/par-term-emu-core-rust). Designed for high performance, modern typography, and rich graphics support.

[!["Buy Me A Coffee"](https://www.buymeacoffee.com/assets/img/custom_images/orange_img.png)](https://buymeacoffee.com/probello3)

![par-term screenshot](https://raw.githubusercontent.com/paulrobello/par-term/main/screenshot.png)

## What's New in 0.4.0

### 🗂️ Multi-Tab Support

Each window now supports multiple terminal tabs, each with its own independent PTY session.

- **New Tab**: `Cmd+T` (macOS) / `Ctrl+T` to create a new tab
- **Close Tab**: `Cmd+W` closes tab (or window if single tab)
- **Tab Switching**: `Cmd+Shift+[` / `Cmd+Shift+]` or `Ctrl+Tab` / `Ctrl+Shift+Tab`
- **Direct Tab Access**: `Cmd+1` through `Cmd+9` to switch to specific tabs
- **Tab Reordering**: `Cmd+Shift+Left/Right` to move tabs
- **Duplicate Tab**: Create new tab with same working directory
- **Tab Bar**: Visual tab bar with close buttons, activity indicators, and bell icons
- **Configurable**: Tab bar visibility (always, when_multiple, never), height, and styling

### 🪟 Multi-Window Support

Spawn multiple independent terminal windows, each with its own PTY session and tabs.

- **New Window**: `Cmd+N` (macOS) / `Ctrl+N` to open a new terminal window
- **Close Window**: `Cmd+W` (macOS) / `Ctrl+W` to close the current window
- **Independent Sessions**: Each window runs its own shell process with separate scrollback and state
- **Clean Shutdown**: Application exits when the last window is closed

### 📋 Native Menu Bar

Cross-platform native menu support using the [muda](https://github.com/tauri-apps/muda) crate.

- **macOS**: Global application menu bar with standard macOS conventions
- **Windows/Linux**: Per-window menu bar with GTK integration on Linux
- **Full Keyboard Accelerators**: All menu items have proper keyboard shortcuts

#### Menu Structure

| Menu | Items |
|------|-------|
| **File** | New Window, New Tab, Close Tab, Close Window, Quit (Windows/Linux) |
| **Edit** | Copy, Paste, Select All, Clear Scrollback, Clipboard History |
| **View** | Toggle Fullscreen, Font Size (+/-/Reset), FPS Overlay, Settings |
| **Tab** | New Tab, Close Tab, Next/Previous Tab, Move Tab Left/Right, Duplicate Tab |
| **Window** (macOS) | Minimize, Zoom |
| **Help** | Keyboard Shortcuts, About |

### 🏗️ Architecture Improvements

- **TabManager**: New multi-tab coordinator manages tab lifecycle within each window
- **WindowManager**: Multi-window coordinator handles window lifecycle and menu events
- **WindowState**: Per-window state cleanly separated from application-level state
- **Event Routing**: Events properly routed to the correct window and tab

## What's New in 0.3.0

### 🎨 Ghostty-Compatible Cursor Shaders

Full support for cursor-based shader animations compatible with [Ghostty](https://ghostty.org/) custom shaders.

- **Cursor Uniforms**: `iCurrentCursor`, `iPreviousCursor`, `iCurrentCursorColor`, `iTimeCursorChange` uniforms for cursor trail effects
- **Configurable Cursor Color**: New cursor color setting in the UI, exposed as `iCurrentCursorColor` to shaders
- **Cursor Style Toggle**: `Cmd+,` (macOS) / `Ctrl+,` to cycle through Block, Beam, and Underline cursor styles
- **Built-in Cursor Shaders**: Includes sweep, warp, glow, blaze, trail, ripple, and boom effects
- **Geometric Cursor Rendering**: Proper visual rendering for all cursor styles (Block, Beam, Underline)

### 🐚 Shell & Terminal Fixes

- **Login Shell Support**: Fixed issues with login shell initialization and environment loading

### 🖼️ Shader Editor Improvements

- **Filename Display**: Background and cursor shader editors now show the filename being edited in the window header

## What's New in 0.2.0

### 🔋 Intelligent Redraw Loop (Power Efficiency)

Significantly reduced CPU and GPU usage by switching from continuous polling to event-driven rendering.

- **Smart Redraws**: Redraws are only requested when terminal content changes or when animations (scrolling, cursor blink, shaders) are active.
- **Improved Battery Life**: Implemented `ControlFlow::Wait` logic, allowing the application to sleep during inactivity instead of maxing out VSync.

### 🛡️ Robustness & Stability

- **Fixed Dropped Input**: Resolved a critical issue where keystrokes and paste operations could be silently discarded during heavy rendering.
- **parking_lot Mutex Migration**: Migrated to `parking_lot` to eliminate Mutex poisoning risks.
- **Graceful Audio Fallback**: Prevents crashes if audio output devices are missing; the terminal bell now fails gracefully.

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
- **Custom WGSL Shaders**: High-performance scrollbar and post-processing effects.
- **Background Images**: Support for PNG/JPEG backgrounds with configurable opacity and scaling modes.
- **Transparency**: True per-pixel alpha transparency (macOS CAMetalLayer optimization).
- **Visual Bell**: Flash-based alerts for terminal bell events.
- **Dynamic Themes**: Support for iTerm2-style color schemes (Dracula, Monokai, Solarized, etc.).

### Typography & Fonts
- **Styled Font Variants**: Explicit support for separate Bold, Italic, and Bold-Italic font families.
- **Unicode Range Mapping**: Assign specific fonts to Unicode ranges (perfect for CJK, Emoji, or Symbols).
- **Text Shaping**: HarfBuzz-powered shaping for ligatures, complex scripts, and emoji sequences.
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
- **[Custom Shaders Guide](docs/CUSTOM_SHADERS.md)** - Install and create custom GLSL shaders for backgrounds and cursor effects.
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
| `Cmd + ,` / `Ctrl + ,` | Cycle cursor style (Block/Beam/Underline) |

### UI Toggles

| Shortcut | Action |
|----------|--------|
| `F1` | Toggle Help panel |
| `F3` | Toggle FPS overlay |
| `F5` | Reload configuration |
| `F11` | Toggle fullscreen |
| `F12` | Toggle Settings UI |

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