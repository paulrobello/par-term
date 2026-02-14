# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

par-term is a cross-platform GPU-accelerated terminal emulator frontend built in Rust. It uses the [par-term-emu-core-rust](https://github.com/paulrobello/par-term-emu-core-rust) library for VT sequence processing, PTY management, and inline graphics protocols (Sixel, iTerm2, Kitty). The frontend provides GPU-accelerated rendering via wgpu with custom WGSL shaders, including support for custom post-processing shaders (Ghostty/Shadertoy-compatible GLSL).

**Language**: Rust (Edition 2024)
**Platform**: Cross-platform (macOS, Linux, Windows)
**Graphics**: wgpu (Vulkan/Metal/DirectX 12)
**Version**: 0.16.0

## Development Commands

### Build & Run

**IMPORTANT**: When not actively debugging, always use `make release` to compile. Debug builds are significantly slower and may not represent actual performance. Only use debug builds when you need to add logging or step through code.

```bash
make build          # Debug build (only for debugging)
make release        # Optimized release build (preferred)
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

**IMPORTANT**: When stopping a debug instance, NEVER use `killall par-term` as this will kill ALL par-term processes including the terminal you're working in. Instead, target the specific debug process:
```bash
# Find the debug process (running from target/debug/)
ps aux | grep "target/debug/par-term" | grep -v grep

# Kill only the debug instance by PID
kill <PID>
```

### Live Debugging Workflow
When debugging UI issues (tab clicks, mouse events, etc.), use this workflow:

```bash
# 1. Kill any running debug builds
pkill -f "target/debug/par-term" 2>/dev/null || true

# 2. Build with changes
cargo build

# 3. Start debug build in background
DEBUG_LEVEL=4 cargo run &>/dev/null &

# 4. Watch logs in real-time (filter by component)
tail -f /tmp/par_term_debug.log | grep --line-buffered "TAB\|CLICK\|MOUSE"

# Or check specific log entries after testing
grep -E "TAB|CLICK" /tmp/par_term_debug.log | tail -30
```

**Important**: When testing, use the **debug build window** (started via `cargo run`), not the app bundle (`/Applications/par-term.app`). The app bundle won't have your code changes.

### Adding Debug Logging

The project uses custom debug macros (not the standard `log` crate). **Do NOT use `log::info!` etc.**

```rust
// Correct - use crate:: prefix for the custom macros
crate::debug_info!("CATEGORY", "message {}", var);   // INFO level (DEBUG_LEVEL=2+)
crate::debug_log!("CATEGORY", "message");            // DEBUG level (DEBUG_LEVEL=3+)
crate::debug_trace!("CATEGORY", "message");          // TRACE level (DEBUG_LEVEL=4)
crate::debug_error!("CATEGORY", "message");          // ERROR level (DEBUG_LEVEL=1+)

// Wrong - these won't appear in /tmp/par_term_debug.log
log::info!("message");  // Goes to stdout, not debug log
```

Log entries appear as: `[timestamp] [LEVEL] [CATEGORY] message`

### Interactive Debugging Session (for Claude)

When the user reports a UI issue, follow this workflow:

1. **Add targeted debug logging** to the suspected code path
2. **Kill existing debug builds**: `pkill -f "target/debug/par-term"`
3. **Build**: `cargo build`
4. **Start the debug build**: `DEBUG_LEVEL=4 cargo run &`
5. **Ask user to interact** with the debug window (not the app bundle!)
6. **Check logs**: `grep "CATEGORY" /tmp/par_term_debug.log | tail -30`
7. **Analyze** what the logs reveal about the issue

Common log categories: `TAB`, `TAB_BAR`, `TAB_ACTION`, `MOUSE`, `RENDER`, `SHADER`, `TERMINAL`, `APP`

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

## Task Tracking Requirements

**IMPORTANT**: Always use the task system (TaskCreate/TaskUpdate) for ALL work, even small jobs. This enables external monitoring of progress.

### When to Create Tasks
- **Always** create tasks before starting any work
- Single-step tasks are fine - the goal is visibility, not complexity
- Break multi-step work into individual tasks

### Task Workflow
1. **Create tasks** at the start of any request using `TaskCreate`
2. **Mark in_progress** when starting work on a task using `TaskUpdate`
3. **Mark completed** when the task is done
4. **Use TaskList** to show current progress

### Example Task Flow
```
User: "Fix the scrollbar rendering bug"

1. TaskCreate: "Investigate scrollbar rendering issue"
2. TaskUpdate: status=in_progress
3. [Do investigation work]
4. TaskUpdate: status=completed
5. TaskCreate: "Fix scrollbar calculation in renderer"
6. TaskUpdate: status=in_progress
7. [Make the fix]
8. TaskUpdate: status=completed
9. TaskCreate: "Verify scrollbar fix with tests"
10. TaskUpdate: status=in_progress
11. [Run tests]
12. TaskUpdate: status=completed
```

### Why This Matters
- External tools can monitor Claude's progress in real-time
- Users can see what Claude is currently working on
- Provides audit trail of completed work
- Enables progress reporting for long-running tasks

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

## Code Organization Guidelines

### File Size Limits
- **Target**: Keep files under 500 lines of code
- **Hard limit**: Files exceeding 800 lines should be refactored
- **Rationale**: Smaller files are easier to navigate, test, and maintain

### When to Extract Modules
Extract code into a new module when:
1. A file exceeds 500 lines
2. A logical grouping of functions/structs emerges (e.g., all clipboard-related code)
3. Code is reused across multiple files
4. A single struct has many associated methods that can be grouped by functionality
5. Tests for a component become complex enough to warrant their own file

### Module Organization Patterns
When splitting a large file into a module directory:
```
# Before: src/large_file.rs (800+ lines)

# After: src/large_file/
├── mod.rs          # Public API, re-exports, and orchestration
├── types.rs        # Data structures and type definitions
├── core.rs         # Core logic and primary functionality
├── helpers.rs      # Internal utility functions
└── tests.rs        # Unit tests (if complex)
```

Example from this codebase:
- `src/app/` splits the monolithic app into `handler.rs`, `input_events.rs`, `mouse_events.rs`
- `src/terminal/` splits terminal logic into `spawn.rs`, `rendering.rs`, `graphics.rs`, `clipboard.rs`
- `src/cell_renderer/` splits rendering into `render.rs`, `atlas.rs`, `background.rs`, `types.rs`

### DRY Principles
- **Extract shared utilities**: Common patterns should live in dedicated utility modules
- **Prefer composition**: Use trait implementations and composition over code duplication
- **Centralize constants**: Define magic numbers and strings in a central location (e.g., `constants.rs` or at module top)
- **Create helper traits**: When multiple types need similar functionality, define a trait

### Utility Module Guidelines
Create utility modules for:
- Color conversion functions → `src/utils/color.rs`
- Geometry calculations → `src/utils/geometry.rs`
- String processing helpers → `src/utils/text.rs`
- Platform-specific abstractions → `src/platform/`

### Signs You Need to Refactor
- Copy-pasting code between files
- Scrolling extensively to find functions in a file
- Multiple unrelated responsibilities in one file
- Difficulty writing focused unit tests
- Import lists growing unwieldy

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
font_size: 12.0               # Font size in points
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

# Power saving when window loses focus
pause_shaders_on_blur: true   # Pause shader animations when unfocused (default: true)
pause_refresh_on_blur: false  # Reduce refresh rate when unfocused (default: false)
unfocused_fps: 10             # Target FPS when unfocused (default: 10)
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

**IMPORTANT**: par-term has TWO separate custom shader systems. Do not confuse them when debugging:

1. **Background Shaders** (`custom_shader`): Full-screen post-processing effects behind terminal text
   - Config: `custom_shader: "filename.glsl"`, `custom_shader_enabled: true`
   - Debug files: `/tmp/par_term_<name>_shader.wgsl`
   - Has access to `iChannel0-4` textures, full Shadertoy uniforms

2. **Cursor Shaders** (`cursor_shader`): Effects around/following the cursor
   - Config: `cursor_shader: "filename.glsl"`, `cursor_shader_enabled: true`
   - Debug files: `/tmp/par_term_<name>_shader.wgsl`
   - Has access to cursor-specific uniforms: `iCurrentCursor`, `iPreviousCursor`, `iTimeCursorChange`, etc.

Both use the same transpiler (`src/custom_shader_renderer/transpiler.rs`) and GLSL format.

**Creating a shader:**
1. Create GLSL shader file in `~/.config/par-term/shaders/` (user config directory)
2. Use Shadertoy-compatible format with `mainImage(out vec4 fragColor, in vec2 fragCoord)`
3. Available uniforms: `iTime`, `iResolution`, `iMouse` (vec4), `iChannel4` (terminal texture)
4. Set `custom_shader: "filename.glsl"` or `cursor_shader: "filename.glsl"` in config
5. Enable with `custom_shader_enabled: true` or `cursor_shader_enabled: true`
6. Once the shader is tested and ready for distribution, copy it to the repo's `shaders/` directory

**Porting Shadertoy shaders**: Shaders are fully Shadertoy compatible:
- `iChannel0-3`: User texture channels (same as Shadertoy)
- `iChannel4`: Terminal content texture (par-term specific)
- `iMouse`: vec4 with full Shadertoy compatibility (xy=current position, zw=click position)
- Y-axis now matches Shadertoy convention (fragCoord.y=0 at bottom) via transpiler flip; no changes needed in user shaders.

**Debugging shaders:**
- Transpiled WGSL is written to `/tmp/par_term_<shader_name>_shader.wgsl`
- Wrapped GLSL is written to `/tmp/par_term_debug_wrapped.glsl` (last shader only)
- Check these files to see the actual generated code
- When debugging one shader type, temporarily disable the other to avoid confusion

**IMPORTANT**: Always develop new shaders in `~/.config/par-term/shaders/` first. Only move shaders to the repo `shaders/` folder when they are complete and ready to be included in the distribution.

### Adding a New Configuration Option
1. Add field to `Config` struct in `src/config.rs`
2. Add default function if needed (e.g., `default_my_option()`)
3. Update `Default` impl for `Config`
4. Add serde attributes: `#[serde(default = "default_my_option")]`
5. Use config value in relevant component
6. **REQUIRED**: Add UI controls to the appropriate tab in `src/settings_ui/`
   - All user-configurable options MUST be exposed in the Settings UI
   - Choose the appropriate tab (e.g., `bell_tab.rs` for notifications, `window_tab.rs` for display)
   - Use checkboxes for booleans, sliders for numeric ranges, dropdowns for enums
   - Remember to set `settings.has_changes = true` and `*changes_this_frame = true` on change
7. **REQUIRED**: Update quick search keywords in `src/settings_ui/sidebar.rs` → `tab_search_keywords()`
   - Add relevant search terms for the new option so users can find it via the Settings search box
   - Include the setting name, synonyms, and related concepts (e.g., for a "blur_radius" setting, add keywords like `"blur"`, `"radius"`, `"background blur"`)
   - Keywords are matched case-insensitively as substrings

### Adding Snippet or Action Keybindings

When adding keybindings for snippets or actions:

1. **Snippets with keybindings**: Set the `keybinding` field in the snippet config (e.g., `Ctrl+Shift+D`)
2. **Auto-generation**: The system auto-generates keybindings during config load via `generate_snippet_action_keybindings()`
3. **Action format**: Snippets use `snippet:<id>`, actions use `action:<id>` as the keybinding action name
4. **Manual setup**: For actions without keybinding fields, add to keybindings list manually in config.yaml:
   ```yaml
   keybindings:
     - key: "Ctrl+Shift+T"
       action: "snippet:date_stamp"
     - key: "Ctrl+Shift+R"
       action: "action:run_tests"
   ```
5. **Execution**: `execute_keybinding_action()` in `input_events.rs` handles `snippet:` and `action:` prefixes

**Important**: Use `try_lock()` from sync contexts when accessing `tab.terminal` (tokio::sync::Mutex). See MEMORY.md for details.

