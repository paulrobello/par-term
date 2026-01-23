# par-term

[![Crates.io](https://img.shields.io/crates/v/par-term)](https://crates.io/crates/par-term)
![Runs on Linux | MacOS | Windows](https://img.shields.io/badge/runs%20on-Linux%20%7C%20MacOS%20%7C%20Windows-blue)
![Arch x86-64 | ARM | AppleSilicon](https://img.shields.io/badge/arch-x86--64%20%7C%20ARM%20%7C%20AppleSilicon-blue)
![Crates.io Downloads](https://img.shields.io/crates/d/par-term)
![License](https://img.shields.io/badge/license-MIT-green)

A cross-platform, GPU-accelerated terminal emulator frontend built with Rust, powered by [par-term-emu-core-rust](https://github.com/paulrobello/par-term-emu-core-rust). Designed for high performance, modern typography, and rich graphics support.

[!["Buy Me A Coffee"](https://www.buymeacoffee.com/assets/img/custom_images/orange_img.png)](https://buymeacoffee.com/probello3)

![par-term screenshot](https://raw.githubusercontent.com/paulrobello/par-term/main/screenshot.png)

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
- **[Compositor Details](docs/COMPOSITOR.md)** - Deep dive into the rendering architecture.
- **[Examples](examples/README.md)** - Comprehensive configuration examples.
- **[Core Library](https://github.com/paulrobello/par-term-emu-core-rust)** - Documentation for the underlying terminal engine.

## Installation

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

On Linux (Ubuntu/Debian), you may need:
```bash
sudo apt install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `PageUp` / `PageDown` | Scroll up/down one page |
| `Shift + Home` | Jump to top of scrollback |
| `Shift + End` | Jump to bottom (current) |
| `Ctrl + V` | Paste from clipboard |
| `Ctrl + Shift + K` | Clear scrollback buffer |
| `Ctrl + L` | Clear visible screen |
| `Ctrl + +/-/0` | Adjust font size / Reset |
| `Ctrl + Shift + S` | Take screenshot |
| `F1` | Toggle Help panel |
| `F3` | Toggle FPS overlay |
| `F5` | Reload configuration |
| `F11` | Toggle fullscreen / Shader editor |
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