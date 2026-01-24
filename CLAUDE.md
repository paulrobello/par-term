# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

par-term is a cross-platform GPU-accelerated terminal emulator frontend built in Rust. It uses the [par-term-emu-core-rust](https://github.com/paulrobello/par-term-emu-core-rust) library for VT sequence processing, PTY management, and inline graphics protocols (Sixel, iTerm2, Kitty). The frontend provides GPU-accelerated rendering via wgpu with custom WGSL shaders, including support for custom post-processing shaders (Ghostty/Shadertoy-compatible GLSL).

**Language**: Rust (Edition 2024)
**Platform**: Cross-platform (macOS, Linux, Windows)
**Graphics**: wgpu (Vulkan/Metal/DirectX 12)
**Version**: 0.2.0

## Development Commands

### Build & Run
```bash
make build          # Debug build
make release        # Optimized release build
make run            # Run in debug mode
make run-release    # Run in release mode

# Or directly with cargo
cargo build
cargo run
cargo build --release
```

### Testing
```bash
make test           # Run all tests
make test-verbose   # Run tests with output
cargo test          # Direct cargo test

# Run specific test
make test-one TEST=test_name
cargo test test_name

# Note: Some tests require active PTY sessions and are marked #[ignore]
cargo test -- --include-ignored  # Run all tests including ignored ones
```

### Code Quality
```bash
make all            # Format, lint, test, and build
make pre-commit     # Run pre-commit checks (fmt-check, lint, test)
make ci             # Full CI checks (fmt-check, lint-all, test, check-all)

make fmt            # Format code with rustfmt
make fmt-check      # Check formatting without modifying

make lint           # Run clippy
make lint-all       # Run clippy on all targets
cargo clippy -- -D warnings
```

### Debugging & Logging
```bash
make run-debug      # Run with DEBUG_LEVEL=3 (logs to /tmp/par_term_debug.log)
make run-trace      # Run with DEBUG_LEVEL=4 (most verbose)
make tail-log       # Monitor debug log in real-time
make watch-graphics # Monitor graphics-related logs only
make clean-logs     # Clean debug logs
```

### Graphics & Shader Testing
```bash
make test-graphics     # Test graphics with debug logging
make test-animations   # Test Kitty animations
make test-fonts        # Run comprehensive text shaping test suite
make benchmark-shaping # Run text shaping performance benchmark
```

### Profiling
```bash
make profile           # CPU profiling with flamegraph (generates flamegraph.svg)
make profile-perf      # Profile with perf (Linux only)
make profile-instruments # Profile with Instruments (macOS only)
```

### Other Commands
```bash
make clean          # Clean build artifacts
make doc-open       # Generate and open documentation
make watch          # Watch for changes and rebuild (requires cargo-watch)
make coverage       # Generate test coverage (requires cargo-tarpaulin)
make bundle         # Create macOS .app bundle (macOS only)
```

## Architecture Overview

### Core Components

The application follows a layered architecture with clear separation of concerns, organized into modular directories:

**App Layer** (`src/app/`)
- Main event loop using winit's `ApplicationHandler`.
- `mod.rs`: Manages application state (`AppState`) and initialization.
- `handler.rs`: Winit event handling and UI orchestration.
- `input_events.rs` & `mouse_events.rs`: Logic for keyboard/mouse interaction and shortcuts.
- `bell.rs`, `mouse.rs`, `render_cache.rs`, `debug_state.rs`: Sub-states for `AppState`.

**Terminal Layer** (`src/terminal/`)
- `mod.rs`: `TerminalManager` orchestrates PTY lifecycle.
- `spawn.rs`: Shell and process spawning logic.
- `rendering.rs`: Converts terminal state to renderer cells.
- `graphics.rs`: Sixel and inline graphics metadata management.
- `clipboard.rs`: Clipboard history and sync.
- `hyperlinks.rs`: OSC 8 hyperlink tracking.

**Renderer Layer** (`src/renderer/`, `src/cell_renderer/`, `src/graphics_renderer.rs`)
- `renderer/mod.rs`: High-level rendering coordinator.
- `renderer/graphics.rs` & `renderer/shaders.rs`: Sixel rendering orchestration and shader management.
- `cell_renderer/mod.rs`: GPU-accelerated cell-based rendering coordinator.
- `cell_renderer/render.rs`: Core render loop and instance buffer building.
- `cell_renderer/atlas.rs`: Dynamic glyph atlas and LRU cache management.
- `cell_renderer/background.rs`: Background image management.
- `cell_renderer/types.rs`: Core rendering data structures.

**Custom Shader Renderer** (`src/custom_shader_renderer/`)
- `mod.rs`: Orchestrates post-processing effects.
- `transpiler.rs`: GLSL to WGSL transpilation via `naga`.
- `types.rs`: Shader uniform data structures.

**Font System** (`src/font_manager.rs`)
- `FontManager`: Manages font loading with fallback chain using `swash` and `fontdb`.

**Text Shaping** (`src/text_shaper.rs`)
- HarfBuzz-based text shaping via `rustybuzz` for ligatures and complex scripts.

**Settings UI** (`src/settings_ui/`)
- `mod.rs`: egui-based settings overlay state.
- `*_tab.rs`: Individual modules for each settings section (font, theme, window, etc.).


### Data Flow

1. **Window Events** → `App::resumed()` → Initialize renderer, terminal, spawn shell
2. **Keyboard Input** → `App::window_event()` → `InputHandler` → `TerminalManager::write()` → PTY
3. **PTY Output** → `App::about_to_wait()` → `TerminalManager::read()` → Parse VT sequences → Extract styled segments → `Renderer::update_cells()`
4. **Sixel Graphics** → `TerminalManager::get_sixel_graphics()` → `Renderer::update_sixel_graphics()` → Create/cache GPU textures
5. **Rendering** → `App::about_to_wait()` → `Renderer::render()` → Three render passes:
   - `CellRenderer::render()` (backgrounds + text + scrollbar)
   - `GraphicsRenderer::render()` (sixel graphics on top of cells)
   - `egui_renderer::render()` (settings UI overlay)
6. **Scrolling** → Mouse wheel/PageUp/PageDown → Update `scroll_offset` → `Renderer::update_scrollbar()` → Re-render with offset

### Shaders

The project includes custom WGSL shaders in `src/shaders/`:
- `cell_bg.wgsl`: Renders colored rectangles for cell backgrounds
- `cell_text.wgsl`: Renders glyphs from atlas texture with color and positioning
- `sixel.wgsl`: Renders RGBA textures for inline graphics (Sixel, iTerm2, Kitty) with alpha blending
- `scrollbar.wgsl`: Renders scrollbar track and thumb
- `background_image.wgsl`: Renders background images with multiple display modes (fit, fill, stretch, tile, center)

All built-in shaders use instanced rendering for performance. User custom shaders are GLSL (Shadertoy-compatible) and transpiled to WGSL via naga.

## Key Design Patterns

### Async/Blocking Boundary
- Uses tokio runtime for async PTY I/O operations
- `TerminalManager::read()` and `write()` are synchronous wrappers around async PTY operations
- Event loop runs in sync context, bridges to async for PTY operations

### GPU Text Rendering
- Glyph atlas approach: all glyphs rasterized to a single texture
- Cache glyphs in `HashMap<(char, usize), GlyphInfo>` (char + font index)
- Fallback font chain: try primary font, then system fallbacks for missing glyphs
- Instanced rendering: one draw call per frame for all glyphs

### GPU Graphics Rendering (Sixel)
- RGBA texture caching: each sixel graphic stored as GPU texture with position-based ID
- Texture cache: `HashMap<u64, SixelTextureInfo>` maps position to texture + bind group
- Cell-based positioning: graphics positioned at terminal (row, col) coordinates
- Automatic texture lifecycle: created on first use, cleared when graphics removed
- Instanced rendering: all graphics drawn in single pass with per-instance bind groups
- Render order: cells → sixel graphics → egui (ensures correct layering)

### Scrollback & Viewport
- Terminal maintains scrollback buffer (default 10,000 lines)
- `scroll_offset`: 0 = bottom (current), >0 = scrolled up
- Renderer receives only visible cells for the viewport
- Scrollbar position calculated from: `scroll_offset`, `visible_lines`, `total_lines`

## Platform-Specific Notes

### macOS
- Uses Metal backend
- Native clipboard via arboard
- Platform-specific code in `src/macos_metal.rs` for CAMetalLayer optimization
- App bundle creation: `make bundle`

### Linux
- Requires X11/Wayland libs: `libxcb-render0-dev`, `libxcb-shape0-dev`, `libxcb-xfixes0-dev`
- Uses Vulkan backend (fallback to others if unavailable)

### Windows
- Uses DirectX 12 backend
- Native clipboard via arboard

## Configuration Files

Configuration location (XDG-compliant):
- Linux/macOS: `~/.config/par-term/config.yaml`
- Windows: `%APPDATA%\par-term\config.yaml`

Key settings:
```yaml
cols: 80                      # Terminal columns
rows: 24                      # Terminal rows
font_size: 13.0               # Font size in points
font_family: "JetBrains Mono" # Primary font
scrollback_lines: 10000       # Lines of scrollback
max_fps: 60                   # Target FPS
vsync_mode: immediate         # immediate, mailbox, or fifo
window_padding: 10.0          # Padding in pixels
window_opacity: 1.0           # 0.0-1.0 for transparency
exit_on_shell_exit: true      # Auto-close when shell exits
middle_click_paste: true      # Enable middle-click paste
auto_copy_selection: true     # Auto-copy selections to clipboard

# Text shaping
enable_text_shaping: true     # HarfBuzz text shaping
enable_ligatures: true        # Programming font ligatures
enable_kerning: true          # Kerning adjustments

# Background image
background_image: "~/image.png"
background_image_mode: fit    # fit, fill, stretch, tile, center
background_image_opacity: 1.0

# Custom shaders (GLSL, Shadertoy-compatible)
custom_shader: "crt.glsl"     # Relative to ~/.config/par-term/shaders/
custom_shader_enabled: true
custom_shader_animation: true
custom_shader_animation_speed: 1.0
```

## Testing Considerations

- Some tests are marked `#[ignore]` because they require active PTY sessions
- Use `cargo test -- --include-ignored` to run all tests
- Tests use `tempfile` for temporary configuration files
- Integration tests in `tests/` directory test config, terminal, and input modules

## Dependencies of Note

- **par-term-emu-core-rust**: Terminal emulation core (VT sequences, PTY, graphics protocols)
- **winit**: Cross-platform window management
- **wgpu**: GPU abstraction layer (Vulkan/Metal/DX12)
- **swash**: Font rasterization with colored glyph support
- **fontdb**: System font discovery and fallback
- **rustybuzz**: HarfBuzz text shaping for ligatures and complex scripts
- **unicode-segmentation**: Grapheme cluster detection
- **unicode-bidi**: Bidirectional text support
- **arboard**: Cross-platform clipboard
- **tokio**: Async runtime for PTY I/O
- **serde/serde_yaml**: Configuration serialization
- **egui/egui-wgpu/egui-winit**: Settings UI overlay
- **naga**: GLSL to WGSL shader transpilation
- **rodio**: Audio playback for bell
- **notify-rust**: Desktop notifications
- **parking_lot**: Mutex implementation (prevents poisoning)

## Common Development Workflows

### Adding a New Keyboard Shortcut
1. Add key handling in `src/app.rs` → `window_event()` → `WindowEvent::KeyboardInput`
2. If needed, add sequence generation in `src/input.rs` → `InputHandler`

### Modifying Rendering
1. Cell backgrounds: Edit `src/cell_renderer.rs` background pipeline or `src/shaders/cell_bg.wgsl`
2. Text rendering: Edit `src/cell_renderer.rs` text pipeline or `src/shaders/cell_text.wgsl`
3. Scrollbar: Edit `src/scrollbar.rs` or `src/shaders/scrollbar.wgsl`

### Changing Configuration Options
1. Add field to `Config` struct in `src/config.rs`
2. Add default function if needed
3. Update `Default` impl
4. Use config value in relevant component

### Debugging PTY Issues
- Enable logging: `RUST_LOG=debug cargo run`
- Check `TerminalManager::read()` and `write()` for I/O errors
- Verify VT sequence parsing in par-term-emu-core-rust

### Profiling Rendering Performance
- Use release build: `cargo build --release`
- Check frame time in render loop (logged when >10ms)
- Profile GPU usage with platform tools (Xcode Instruments, RenderDoc, etc.)
- Use `make profile` for CPU flamegraph generation

### Adding a Custom Shader
1. Create GLSL shader file in `~/.config/par-term/shaders/` (user config directory)
2. Use Shadertoy-compatible format with `mainImage(out vec4 fragColor, in vec2 fragCoord)`
3. Available uniforms: `iTime`, `iResolution`, `iMouse`, `iChannel0` (terminal texture)
4. Set `custom_shader: "filename.glsl"` in config
5. Enable with `custom_shader_enabled: true`
6. Once the shader is tested and ready for distribution, copy it to the repo's `shaders/` directory

**IMPORTANT**: Always develop new shaders in `~/.config/par-term/shaders/` first. Only move shaders to the repo `shaders/` folder when they are complete and ready to be included in the distribution.

### Adding a New Configuration Option
1. Add field to `Config` struct in `src/config.rs`
2. Add default function if needed (e.g., `default_my_option()`)
3. Update `Default` impl for `Config`
4. Add serde attributes: `#[serde(default = "default_my_option")]`
5. Use config value in relevant component
6. Optionally add to `settings_ui.rs` for runtime editing
