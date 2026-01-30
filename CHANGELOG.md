# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added

- **Cursor Enhancements**: iTerm2-style cursor visibility improvements (#26)
  - **Cursor Guide**: Horizontal line spanning terminal width at cursor row
    - Configurable RGBA color with low default alpha
    - Toggle via `cursor_guide_enabled` config option
  - **Cursor Shadow**: Drop shadow behind cursor for visibility
    - Configurable RGBA shadow color and X/Y offset
    - Toggle via `cursor_shadow_enabled` config option
  - **Cursor Boost**: Glow effect around cursor
    - Adjustable intensity slider (0.0-1.0)
    - Configurable RGB boost color
  - **Unfocused Cursor Style**: Control cursor appearance when window loses focus
    - `hollow`: Outline-only block cursor (default)
    - `same`: Keep normal cursor style
    - `hidden`: Hide cursor completely when unfocused
  - All enhancements configurable via Settings > Cursor tab

### Fixed

- **Tab Bar Click Reliability**: Fixed missed clicks and wrong-tab-selection issues
  - Close button now renders as overlay with manual hit-testing for reliable clicks
  - Uses `clicked_by(PointerButton::Primary)` to prevent keyboard focus from triggering tab switches
  - Added `egui_initialized` flag to prevent unreliable pointer state before first render
- **Terminal Content Overlap**: Added content offset system to prevent terminal content from overlapping with tab bar
  - Propagated `content_offset_y` through cell renderer, graphics renderer, and custom shader renderer
- **Tab Numbering**: Changed to position-based numbering that automatically renumbers when tabs are closed or reordered
  - Tabs now show "Tab 1, Tab 2, Tab 3" instead of keeping original IDs
- **Mouse Event Handling**: Fixed event ordering to check tab bar area before updating terminal mouse state

### Added

- Comprehensive tab bar UI tests (`tests/tab_bar_ui_tests.rs`)
- Tab stability integration tests (`tests/tab_stability_tests.rs`)

---

## [0.6.0] - 2026-01-29

### Added

- **Shader Gallery**: Visual gallery with screenshots of all 49+ included shaders
  - Hosted on GitHub Pages at https://paulrobello.github.io/par-term/
  - Auto-deploys on changes to shader-gallery folder
- **CLI Options**: New command-line flags for automation and scripting
  - `--screenshot <path>`: Take screenshot and save to file
  - `--shader <name>`: Override background shader
  - `--exit-after <seconds>`: Exit after specified duration
  - `--command <cmd>`: Run command instead of default shell
- **Configurable Keybindings**: Customize all keyboard shortcuts
  - Edit `~/.config/par-term/keybindings.yaml`
  - Support for modifier keys (Ctrl, Alt, Shift, Super)
- **Shader Distribution System**: Easy shader installation
  - `par-term install-shaders` CLI command
  - Downloads shaders from latest GitHub release
  - Options: `-y` (no prompt), `--force` (overwrite existing)

### Fixed

- **Custom Shader Background Handling**: Preserve solid color background when custom shader is disabled
- **Full Content Mode Compositing**: Shader output used directly without re-compositing terminal content on top

### Documentation

- Synced COMPOSITOR.md and CUSTOM_SHADERS.md with current implementation
- Updated README with CLI shader installer instructions

---

## [0.5.0] - 2026-01-29

### Added

#### Settings & Configuration
- **Standalone Settings Window**: Moved settings UI from overlay to dedicated window
  - `F12` or `Cmd+,` (macOS) / `Ctrl+,` (Linux/Windows) to open
  - Automatically brought to front when terminal gains focus
  - View and edit settings while terminal content remains visible
- **Per-Shader Configuration System**: 3-tier configuration for background and cursor shaders
  - Shader metadata defaults embedded in GLSL files (`/*! par-term shader metadata ... */`)
  - Per-shader user overrides in `shader_configs` section of config.yaml
  - Global config fallback for unspecified values
  - "Save Defaults to Shader" button to write settings back to shader files
  - Per-shader UI controls for animation_speed, brightness, text_opacity, texture channels
- **Shader Hot Reload**: Automatic shader reloading when files are modified on disk
  - Configurable via `shader_hot_reload` (default: false) and `shader_hot_reload_delay` (default: 100ms)
  - Desktop notifications on reload success/failure
  - Visual bell on compilation errors when enabled
- **Power Saving Options**: Reduce resource usage when window is unfocused
  - `pause_shaders_on_blur` (default: true): Pause shader animations when unfocused
  - `pause_refresh_on_blur` (default: false): Reduce refresh rate when unfocused
  - `unfocused_fps` (default: 30): Target FPS when window is unfocused
- **Cursor Lock Options**: Prevent applications from overriding cursor preferences
  - `lock_cursor_visibility`: Prevent apps from hiding cursor via DECTCEM
  - `lock_cursor_style`: Prevent apps from changing cursor style via DECSCUSR
  - `lock_cursor_blink`: Prevent apps from enabling cursor blink when user has it disabled
- **Background Mode Options**: Choose between theme default, solid color, or background image
  - `background_mode`: "default", "color", or "image"
  - `background_color`: Custom solid color with color picker in UI
  - Solid color passed to shaders via `iBackgroundColor` uniform
- **Resize Overlay**: Centered overlay during window resize showing dimensions
  - Displays both character (cols×rows) and pixel dimensions
  - Auto-hides 1 second after resize stops
- **Grid-Based Window Sizing**: Calculate initial window size from cols×rows
  - No visible resize on startup (like iTerm2)
  - "Use Current Size" button in settings to save current dimensions

#### Terminal Features
- **Bracketed Paste Mode Support**: Proper paste handling for shells that support it
  - Wraps pasted content with `ESC[200~`/`ESC[201~` sequences
  - Prevents accidental command execution when pasting text with newlines
  - Works with bash 4.4+, zsh, fish, and other modern shells
- **DECSCUSR Cursor Shape Support**: Dynamic cursor changes via escape sequences
  - Applications can change cursor style (block/underline/bar) and blink state
  - Respects user's `lock_cursor_style` and `lock_cursor_blink` settings
- **Multi-Character Grapheme Cluster Rendering**: Proper handling of complex Unicode
  - Flag emoji (🇺🇸) using regional indicator pairs
  - ZWJ sequences (👨‍👩‍👧‍👦) for family/profession emoji
  - Skin tone modifiers (👋🏽)
  - Combining characters (diacritics)
  - Requires par-term-emu-core-rust v0.22.0
- **Box Drawing Geometric Rendering**: Pixel-perfect TUI borders and block characters
  - Light/heavy horizontal and vertical lines (─ ━ │ ┃)
  - All corners, T-junctions, and crosses (┌ ┐ └ ┘ ├ ┤ ┬ ┴ ┼ etc.)
  - Double lines and corners (═ ║ ╔ ╗ ╚ ╝ etc.)
  - Rounded corners (╭ ╮ ╯ ╰)
  - Solid, partial, and quadrant block elements (█ ▄ ▀ ▐ ▌ etc.)
  - Eliminates gaps between adjacent cells

#### Tab Bar Enhancements
- **Tab Bar Color Configuration**: 11 new options for full color customization
  - Background, active/inactive/hover tab colors
  - Text colors, indicator colors, close button colors
  - Settings UI panel for live color editing
- **Per-Tab Custom Colors**: Right-click context menu to set individual tab colors
  - Color presets row with custom color picker
  - Color indicator dot on inactive tabs with custom colors
- **Tab Layout Improvements**:
  - Equal-width tabs that spread across available space
  - Horizontal scrolling with arrow buttons when tabs exceed minimum width
  - Configurable `tab_min_width` (default: 120px, range: 120-512px)
  - Tab borders with configurable width and color
  - Toggle for tab close button visibility
- **Inactive Tab Dimming**: Visual distinction for active tab
  - `dim_inactive_tabs` (default: true)
  - `inactive_tab_opacity` (default: 0.6)

#### Shader System
- **Cubemap Support**: Load 6-face cubemap textures for environment reflections
  - Auto-discovery of cubemap folders in settings UI dropdown
  - Standard naming convention: px/nx/py/ny/pz/nz
- **iTimeKeyPress Uniform**: Track when last key was pressed for typing effects
  - Enables screen pulses, typing animations, keystroke visualizations
  - Included keypress_pulse.glsl demo shader
- **use_background_as_channel0**: Option to use app's background image as iChannel0
  - Allows shaders to incorporate configured background image into effects
- **New Background Shaders**:
  - `rain.glsl`: Rain on glass post-processing effect
  - `singularity.glsl`: Whirling blackhole with red/blue accretion disk
  - `universe-within.glsl`: Mystical neural network with pulsing nodes
  - `convergence.glsl`: Swirling voronoi patterns with lightning bolt
  - `gyroid.glsl`: Raymarched gyroid tunnel with colorful lighting
  - `dodecagon-pattern.glsl`: BRDF metallic tile pattern
  - `arcane-portal.glsl`: Animated portal with swirling energy
  - `bumped_sinusoidal_warp.glsl`: Warped texture effect
- **Cursor Shader Overrides**: Per-shader settings for cursor effects
  - animation_speed, hides_cursor, disable_in_alt_screen

#### Window Transparency
- **Proper Window Transparency Support**: Correct alpha handling across platforms
  - Appropriate alpha mode selection based on surface capabilities
  - macOS window blur support via CGS private API
  - `transparency_affects_only_default_background` (default: true)
  - `keep_text_opaque` option to maintain text clarity
  - RLE background rendering to eliminate seams between cells

#### macOS Improvements
- **macOS Clipboard Shortcuts**: `Cmd+C` and `Cmd+V` support
- **Keyboard Shortcuts in Shader Editors**: Fixed `Cmd+A/C/V/X` in text editors

### Changed
- **Core Library Update**: Bumped `par-term-emu-core-rust` to v0.22.0 for grapheme cluster support
- **Default VSync Mode**: Changed to FIFO (most compatible across platforms)
- **Default Unfocused FPS**: Changed from 10 to 30 for better background responsiveness
- **Default Blur Radius**: Changed to 8 for better visual effect
- **Build Target**: `make build` now uses release mode; added `make build-debug` for debug builds
- **Shader Optimizations**:
  - Removed iChannel4 terminal blending dependencies from background shaders
  - Replaced pow(x, n) with multiplications
  - Precomputed constants and reduced loop iterations

### Fixed
- **Text Clarity with Shaders**: Use nearest filtering instead of linear for terminal texture
- **Shader Transparency Chaining**: Preserve transparency when both background and cursor shaders enabled
- **Double Opacity Bug**: Fixed background getting darker when cursor shader enabled with opacity < 100%
- **DPI Scaling**: Properly recalculate font metrics when moving between displays with different DPIs
- **Background Image Loading**: Fixed tilde expansion and uniform buffer layout
- **Cursor Settings**: Cursor style and blink changes now apply to running terminals
- **FPS Throttling**: Properly throttle when window unfocused with pause_refresh_on_blur
- **Selection Bug**: Modifier keys (Ctrl/Alt/Cmd) alone no longer clear text selection
- **Tab Bar Click-Through**: Tab close button clicks no longer leak to terminal
- **Alt Screen Rendering**: Fixed black screen when cursor shader disabled for alt screen apps
- **Animation Resume**: Respect user's animation settings when resuming from blur
- **Box Drawing Lines**: Adjusted thickness for cell aspect ratio consistency

### Refactored
- **Large File Extraction**: Decomposed monolithic files into focused modules
  - `config/` module directory with types.rs, defaults.rs
  - `font_manager/` with types.rs, loader.rs, fallbacks.rs
  - `settings_ui/` with shader_editor.rs, cursor_shader_editor.rs, shader_dialogs.rs
  - `custom_shader_renderer/` with pipeline.rs, cursor.rs
  - `cell_renderer/` with pipeline.rs
  - `window_state/` with tab_ops.rs, scroll_ops.rs, keyboard_handlers.rs
  - `mouse_events/` with text_selection.rs, url_hover.rs
  - `app/handler/` with notifications.rs
- **DRY Helpers**: RendererInitParams, ConfigChanges structs for cleaner code
- **GPU Utilities**: New gpu_utils.rs module with reusable sampler and texture helpers

### Documentation
- Added `docs/SHADERS.md` with complete list of 49 included shaders by category
- Updated `docs/CUSTOM_SHADERS.md` with all uniforms and configuration options
- Added code organization guidelines to CLAUDE.md

---

## [0.4.0] - 2026-01-23

### Added
- **Multi-Tab Support**: Multiple terminal tabs per window with independent PTY sessions
  - `Cmd/Ctrl+T` to create a new tab
  - `Cmd/Ctrl+W` to close tab (or window if single tab)
  - `Cmd/Ctrl+Shift+[/]` or `Ctrl+Tab/Shift+Tab` to switch tabs
  - `Cmd/Ctrl+1-9` for direct tab access
  - `Cmd/Ctrl+Shift+Left/Right` to reorder tabs
  - Tab duplication with inherited working directory
  - Visual tab bar with close buttons, activity indicators, and bell icons
  - Configurable tab bar visibility (always, when_multiple, never)
- **Multi-Window Support**: Spawn multiple independent terminal windows
  - `Cmd/Ctrl+N` to open a new terminal window
  - Each window runs its own shell process with separate scrollback and state
  - Application exits when the last window is closed
- **Native Menu Bar**: Cross-platform native menu support using the `muda` crate
  - macOS: Global application menu bar with standard macOS conventions
  - Windows/Linux: Per-window menu bar with GTK integration on Linux
  - Full keyboard accelerators for all menu items
  - Menu structure: File, Edit, View, Tab, Window (macOS), Help
- **Shader Texture Channels**: Shadertoy-compatible iChannel1-4 texture support
  - Load custom textures for use in GLSL shaders
  - Configure via `custom_shader_channel1` through `custom_shader_channel4` settings
  - Supports PNG, JPEG, and other common image formats
- **Shader Brightness Control**: New `custom_shader_brightness` setting
  - Dims shader background to improve text readability (0.05 = very dark, 1.0 = full)
- **Cursor Shader Improvements**: Enhanced cursor shader system
  - New `cursor_shader_hides_cursor` option to fully replace cursor rendering
  - Allows cursor shaders to completely control cursor appearance
- **Custom Shaders Collection**: 40+ included GLSL shaders in `shaders/` directory
  - Background effects: starfield, galaxy, underwater, CRT, bloom, clouds, happy_fractal, bumped_sinusoidal_warp
  - Cursor effects: glow, sweep, trail, warp, blaze, ripple, pacman, orbit

### Changed
- **Architecture Refactor**: Decomposed monolithic `AppState` into modular components
  - `TabManager`: Coordinates multiple tabs within each window
  - `WindowManager`: Coordinates multiple windows and handles menu events
  - `WindowState`: Per-window state (terminal, renderer, input, UI)
  - Event routing by `WindowId` and tab index for proper multi-window/tab support

### Documentation
- Added `docs/CUSTOM_SHADERS.md` - Comprehensive guide for installing and creating shaders
- Updated `docs/ARCHITECTURE.md` - Added TabManager and texture system details
- Updated README with multi-tab keyboard shortcuts and configuration

---

## [0.3.0] - 2026-01-21

### Added
- **Ghostty-Compatible Cursor Shaders**: Full support for cursor-based shader animations
  - `iCurrentCursor`, `iPreviousCursor` uniforms for cursor position and size
  - `iCurrentCursorColor` uniform for cursor color
  - `iTimeCursorChange` uniform for cursor movement timing
  - Built-in cursor shaders: sweep, warp, glow, blaze, trail, ripple, boom
- **Configurable Cursor Color**: New cursor color setting exposed to shaders
- **Cursor Style Toggle**: `Cmd+,` (macOS) / `Ctrl+,` to cycle through Block, Beam, and Underline styles
- **Geometric Cursor Rendering**: Proper visual rendering for all cursor styles

### Fixed
- **Login Shell Support**: Fixed issues with login shell initialization and environment loading

### Changed
- **Shader Editor Improvements**: Background and cursor shader editors now show filename in window header

---

## [0.2.0] - 2026-01-20

### Added
- **Intelligent Redraw Loop (Power Efficiency)**: Significantly reduced CPU/GPU usage by switching from continuous polling to event-driven rendering
  - Switched from `ControlFlow::Poll` to `ControlFlow::Wait`
  - Implemented smart wake-up logic for cursor blinking, smooth scrolling, and custom shader animations
  - Redraws are now requested only when content actually changes or animations are active
- **parking_lot Mutex Migration**: Migrated from `std::sync::Mutex` to `parking_lot::Mutex` for improved performance and robustness
  - Eliminated Mutex poisoning risk, preventing crash loops if a thread panics while holding a lock

### Fixed
- **Dropped User Input**: Fixed a critical logic error where key presses, paste operations, and middle-click paste could be silently discarded if the terminal lock was contested (e.g., during rendering). Replaced `try_lock()` with `.lock().await` for all critical input paths.
- **Audio Bell Panic**: Fixed a crash on startup on systems without audio devices. `AudioBell` now fails gracefully and returns a disabled state instead of panicking.

### Changed
- **Core Library Update**: Bumped `par-term-emu-core-rust` to v0.21.0 to leverage safe environment variable APIs and non-poisoning mutexes.

## [0.1.0] - 2025-11-24

### Fixed - Critical (2025-11-24)
- **macOS crash on startup (NSInvalidArgumentException)**: Fixed crash when calling `setDisplaySyncEnabled:` on wrong layer type
  - Added proper type checking using `objc2::runtime::AnyClass::name()` to verify layer is `CAMetalLayer`
  - Fixed class name retrieval to correctly detect layer type
  - Moved Metal layer configuration to AFTER renderer/surface creation (src/app.rs:264-270)
  - Application now starts successfully without crashing
  - Root cause: Was trying to call Metal-specific methods before wgpu created the Metal surface
  - Files: `src/macos_metal.rs:48-75`, `src/app.rs:264-270`

### Added - Configuration (2025-11-24)
- **max_fps configuration option** - Control target frame rate (matches WezTerm's naming)
  - Renamed `refresh_rate` to `max_fps` for clarity (backward compatible via alias)
  - Default: 60 FPS
  - Controls how frequently terminal requests screen redraws
  - Documentation includes macOS VSync throttling caveat
  - Files: `src/config.rs:98-104`, `src/app.rs:334`, `examples/config-complete.yaml:165-170`

### Known Limitations - Performance (2025-11-24)
- **macOS FPS throttling remains at ~22-25 FPS** despite CAMetalLayer configuration
  - Successfully configures `displaySyncEnabled = false` on CAMetalLayer
  - Verified setting is applied (logs confirm `displaySyncEnabled = false`)
  - However, FPS remains throttled at ~22-25 FPS with 40-53ms frame times
  - Root cause: Issue appears to be in wgpu's rendering pipeline, not just CAMetalLayer settings
  - wgpu may have additional VSync or frame pacing logic that can't be disabled via CAMetalLayer alone
  - Alternative approaches (WezTerm's native Cocoa, iTerm2's CVDisplayLink) bypass wgpu entirely
  - **Status**: Functional but FPS-limited. May require wgpu upstream changes or alternative rendering approach
  - Files: `src/macos_metal.rs` (new), `src/app.rs:264-270`, `src/cell_renderer.rs:107`, `src/lib.rs:13`, `src/main.rs:11`
  - Dependencies: Added `objc2`, `objc2-app-kit`, `objc2-foundation`, `objc2-quartz-core`, `raw-window-handle` for macOS

### Planned Features
- Clipboard history integration (pending core library API)
- Tmux control protocol support
- Color accessibility controls (contrast, brightness)
- Dynamic font hot-reloading
- Font subsetting for large CJK fonts
- Split pane support (horizontal/vertical)
- Search functionality (Ctrl+F)

---

## [0.2.1] - 2025-11-23 - Emoji Sequence Preservation

### Changed - Core Library Compatibility
- **Updated to par-term-emu-core-rust v0.10.0**
  - Cell struct now uses `grapheme: String` instead of `character: char` for full emoji sequence preservation
  - Supports variation selectors (⚠️ vs ⚠), skin tone modifiers (👋🏽), ZWJ sequences (👨‍👩‍👧‍👦), regional indicators (🇺🇸)
  - Cell no longer implements `Copy` trait, now `Clone` only (breaking change in rendering code)
  - Text shaping now receives complete grapheme clusters for proper emoji rendering
  - All character operations updated to extract base character from grapheme when needed
  - Changed from `copy_from_slice` to `clone_from_slice` for cell array operations

### Fixed - Emoji Rendering
- **Emoji sequences are now preserved** during text shaping instead of being broken into individual characters
- **Variation selector font selection**: Emoji with FE0F variation selector now force emoji font selection (fixes ⚠️ ❤️ rendering in color)
- **Texture filtering artifacts**: Changed from linear to nearest filtering to eliminate edge artifacts and bleeding between glyphs
- **Flag placeholder boxes**: Regional indicators no longer cache fallback boxes, only rendered via text shaping
- **Flag scaling**: Removed 1.5x scaling for flags, now same size as other emoji for visual consistency
- **Emoji modifier caching**: Variation selectors, skin tone modifiers, ZWJ, and regional indicators now skip individual glyph caching

---

## [0.2.0] - 2025-11-23 - Font Features & Hyperlinks

### Added - Font Features

#### Multiple Font Families
- **Styled font support**: Configure separate fonts for bold, italic, and bold-italic text
  - `font_family_bold`: Use professionally designed bold fonts instead of synthetic bold
  - `font_family_italic`: Use proper italic/oblique fonts
  - `font_family_bold_italic`: Use dedicated bold-italic variants
  - Smart fallback to primary font if styled fonts not configured
  - Font indexing system: 0=primary, 1=bold, 2=italic, 3=bold-italic, 4+=range fonts

#### Custom Font Ranges
- **Unicode range mapping**: Map specific fonts to Unicode character ranges
  - Configure fonts for specific codepoint ranges (e.g., 0x4E00-0x9FFF for CJK)
  - Perfect for CJK scripts (Chinese, Japanese, Korean)
  - Custom emoji fonts (Apple Color Emoji, Noto Color Emoji)
  - Mathematical symbols with specialized math fonts
  - Box drawing characters with monospace fonts
  - Font priority system: styled fonts → range fonts → fallback fonts → primary font
  - `FontRange` config structure with start/end codepoints

#### Optimized Glyph Caching
- **Compound cache keys**: Separate cache entries for each style combination
  - `GlyphCacheKey(character, bold, italic)` enables proper styled font rendering
  - Changed from `HashMap<char, GlyphInfo>` to `HashMap<GlyphCacheKey, GlyphInfo>`
  - Maintains O(1) lookup performance
  - Supports thousands of unique glyph combinations efficiently

### Added - Hyperlink Features

#### OSC 8 Hyperlink Support
- **Full OSC 8 protocol support**: Terminal hyperlinks work alongside regex detection
  - Added `hyperlink_id: Option<u32>` field to `Cell` struct
  - Cell conversion extracts `hyperlink_id` from terminal cell flags
  - `get_all_hyperlinks()`: Returns all hyperlinks from terminal
  - `get_hyperlink_url(id)`: Returns URL for specific hyperlink ID
  - `detect_osc8_hyperlinks()`: Extracts OSC 8 hyperlinks from cells
  - Combined detection: OSC 8 hyperlinks + regex URLs rendered together

### Added - Documentation

#### User Documentation
- **QUICK_START_FONTS.md**: 5-minute setup guide with step-by-step instructions
- **examples/README.md**: Comprehensive guide with Unicode reference table
- **examples/config-styled-fonts.yaml**: Bold/italic font configuration example
- **examples/config-font-ranges.yaml**: Unicode range mapping examples
- **examples/config-complete.yaml**: Complete feature showcase
- **test_fonts.sh**: Comprehensive test script with 12 test cases

#### Technical Documentation
- **IMPLEMENTATION_SUMMARY.md**: Complete technical reference
- **RELEASE_CHECKLIST.md**: Production readiness verification

### Changed

#### Core Structures
- **Cell struct**: Added `hyperlink_id: Option<u32>` field
- **FontManager**: Extended to manage styled fonts and range-specific fonts
- **GlyphCacheKey**: New compound key type for cache lookups
- **Config struct**: Added font configuration fields

#### Rendering Pipeline
- **CellRenderer**: Updated to use compound glyph cache keys
- **URL Detection**: Enhanced to combine OSC 8 and regex detection
- **Terminal Integration**: Added hyperlink accessor methods

### Fixed
- **Clippy warnings**: Fixed collapsible if statement
- **Formatting**: All code formatted with rustfmt
- **Font traits**: Added Clone/Debug implementations for FontData

### Performance
- Maintains O(1) glyph cache lookups
- Fonts loaded once, Arc-shared across glyphs
- Negligible overhead for range checks

### Testing
- All 33 tests pass (6 PTY tests ignored as expected)
- Zero compiler warnings
- Clippy clean
- Format verified

---

## [0.1.1] - Scrollbar & Clipboard Features

### Added
- **Visual Scrollbar**: GPU-accelerated scrollbar with custom WGSL shader
  - Auto-hide behavior when no scrollback content available
  - Smooth position tracking and visual feedback
  - Configurable scrollback size (default: 10,000 lines)
- **Scroll Navigation**: Multiple ways to navigate terminal history
  - Mouse wheel scrolling support
  - `PageUp`/`PageDown` for page-by-page navigation
  - `Shift+Home` to jump to top of scrollback
  - `Shift+End` to jump to bottom (current content)
- **Scrollback Rendering**: Properly displays history when scrolled up
  - Shows actual scrollback content instead of current content when scrolled
  - Combines scrollback buffer with current visible content
  - Calculates correct window of lines to display based on scroll position
- **Clipboard Integration**: Full cross-platform clipboard support
  - `Ctrl+V` to paste from clipboard
  - Middle-click paste (configurable via config)
  - Automatic line ending conversion for terminal compatibility
- **Text Selection**: Mouse-based text selection with clipboard integration
  - Click and drag to select text
  - Automatic copy to clipboard on mouse release
  - Support for single-line and multi-line selection
  - Works across scrollback buffer and current content
- **PTY Integration**: Real pseudo-terminal support
  - Automatic shell spawning on startup
  - Cross-platform shell detection (Unix/Windows)
  - PTY resize synchronization with window
  - Real-time terminal output updates at 60fps
- **Shell Exit Handling**: Graceful shutdown on shell exit
  - Exit detection with status message
  - "[Process completed - press any key to exit]" prompt
- **Styled Content Extraction**: Foundation for ANSI color rendering
  - Per-character color and attribute extraction
  - Support for bold, italic, underline attributes
- **Comprehensive Testing**: 23 tests covering core functionality

### Changed
- Improved terminal rendering to use real PTY content
- Enhanced error handling throughout the codebase
- Optimized redraw loop to 60fps

### Fixed
- Code formatting and linting issues
- Test assertions for grid padding behavior
- Module visibility for public API

---

## [0.1.0] - Initial Release

### Added
- Basic terminal window creation
- GPU-accelerated text rendering using wgpu and glyphon
- Cross-platform window management via winit
- Configuration file support (YAML)
- Font size and family configuration
- Window resizing with proper PTY synchronization
- VT sequence support via par-term-emu-core-rust
- Complete keyboard input handling
  - Special keys (arrows, function keys)
  - Modifier keys (Ctrl, Alt, Shift)
  - Control character sequences
- Cross-platform support (macOS, Linux, Windows)

---

## Notes

### Versioning
- **Major version (X.0.0)**: Breaking changes
- **Minor version (0.X.0)**: New features, backward compatible
- **Patch version (0.0.X)**: Bug fixes, minor improvements

### Links
- [GitHub Repository](https://github.com/paulrobello/par-term)
- [Core Library](https://github.com/paulrobello/par-term-emu-core-rust)

### References
- [OSC 8 Hyperlinks Spec](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda)
- [Unicode Character Ranges](https://en.wikipedia.org/wiki/Unicode_block)

---

[Unreleased]: https://github.com/paulrobello/par-term/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/paulrobello/par-term/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/paulrobello/par-term/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/paulrobello/par-term/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/paulrobello/par-term/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/paulrobello/par-term/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/paulrobello/par-term/releases/tag/v0.1.0
