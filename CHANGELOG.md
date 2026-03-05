# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- `make install-shell-integration` Makefile target to copy all three shell integration scripts (bash, zsh, fish) to `~/.config/par-term/`.
- Expanded Nerd Font icon presets in Settings: added "UI Actions" (16 icons: Search, Edit, Copy, Clipboard, Cut, Trash, Plus, Close, Refresh, Filter, List, Link, External Link, Save, Apply, Ban/Cancel) and "Navigation" (16 icons: Arrow/Angle/Caret/Long Arrow/Reply/Level/Rotate variants) categories, plus 4 more icons in "Status & Alerts".
- New `snap_window_to_grid` config option (default: `true`) with Settings UI toggle under Window → Display. When enabled, the window snaps to exact terminal cell boundaries on resize, eliminating blank background gaps between the terminal grid and window edges. Disabled automatically in split-pane mode.

### Changed
- Pane padding no longer applies when there is only one pane (no splits); in split mode, an automatic base padding equal to half the divider width is added to prevent content rendering under the divider, with `pane_padding` config applied on top of that.
- Default `pane_padding` changed from `4.0` to `1.0` pixels.
- Default `window_padding` changed from `0.0` to `1.0` pixels.

### Fixed
- Theme changes from Settings UI, config reload (F5), and system theme switching now apply to all split pane terminals, not just the primary pane.
- Cell grid is now centered within each pane, distributing remainder pixels evenly on all sides. Previously, the `floor()` truncation when converting pixels to cols/rows left a visible gap at the right and bottom edges showing a different shade than the terminal content. The centering approach matches Alacritty, Kitty, and other modern terminal emulators.
- Scrollbar no longer leaks from the original pane to a newly-split pane. After a vertical/horizontal split, the new pane incorrectly showed the original pane's scrollbar because `gather_render_data` was writing `tab.terminal`'s scrollback length into the focused pane's cache — even though the focused (new) pane has its own terminal with no scrollback. The cache update is now skipped in multi-pane mode, and the per-pane scrollback length from `gather_pane_render_data` always overwrites stale values.
- URL/file-path highlights and underlines no longer appear in the wrong pane when splitting. `detect_urls()` was reading cells from `tab.terminal` (the primary terminal) instead of the focused pane's terminal, causing detected URLs from one pane to be rendered in another. Mouse hover and Cmd/Ctrl+Click also used window-global cell coordinates instead of pane-local coordinates, making hit-testing incorrect in split mode.
- Spurious visual bell flash when splitting panes. `check_bell()` and `check_notifications()` were reading bell count from the tab's primary terminal but comparing against the focused pane's `BellState.last_count`. After a split, focus moves to the new pane (with `last_count=0`), while the original terminal may have accumulated bell events, falsely triggering the visual bell.
- Scrollbar width no longer scales with window size during resize. The scrollbar NDC uniform cache (`last_scrollbar_state`) did not include window dimensions, so resizing without changing the terminal grid skipped re-uploading uniforms, causing the scrollbar to visually stretch or shrink.
- Terminal text no longer renders behind the scrollbar. The focused pane's column count is now reduced by the scrollbar width when the scrollbar is visible, so text wraps before the scrollbar area. When the scrollbar is hidden, the full terminal width is available. Per-pane PTY resize is done inline before cell gathering to avoid SIGWINCH storms from competing resize calls.
- Scrollbar now renders correctly in the focused pane after a split. The scrollbar GPU uniform cache did not include pane viewport bounds, so when a split changed the pane dimensions without changing scroll state or window size, the cache key was identical and `update_scrollbar_for_pane()` was never called — leaving the scrollbar with stale full-window geometry that rendered off-screen. The cache now includes viewport x/y/width/height.
- Command history UI (`Cmd+R`) icons now render correctly: status icons were using standard Unicode geometric shapes (●, ✗, ○) not present in any font in the egui stack; replaced with confirmed Nerd Font PUA codepoints (`\u{f05d}` check-circle, `\u{f057}` times-circle) and ASCII `?` for unknown. Navigation hint arrows fixed the same way in command history, clipboard history, and paste-special UIs.
- Command history and scrollbar marks now show the correct exit code for failed commands. Three layered fixes: (1) bash shell integration was capturing `$?=0` because `__bp_interactive_mode` ran between bash-preexec saving the real exit code and `__par_term_prompt_command` reading `$?`; fixed to use `${__bp_last_ret_value:-$?}`. (2) `ScrollbackMetadata::apply_event` now applies the D-marker exit code to the command snapshot when the core history is read before `end_command_execution` runs. (3) `CommandHistory::update_exit_code_if_unknown` now updates from any differing `Some` value (not only from `None`), so when scrollback marks are iterated oldest-first and the same command appears multiple times, the most-recent execution's exit code always wins.

### Added
- New `link_highlight_color_enabled` config option (default: `true`) and matching Settings UI checkbox ("Change text color on hover"). When disabled, detected URLs and file paths are underlined without changing the text colour — hover cursor change and underline decoration remain active.

### Fixed
- URL/file hover cursor no longer flickers between pointer and iBeam on every render frame. `detect_urls` previously reset `hovered_url` and set the cursor to iBeam before rebuilding the URL list, relying on the next mouse-move event to restore the pointer; this caused a visible one-frame flicker on every cache-miss render. Hover/cursor state is now owned exclusively by `mouse_move` — `detect_urls` only updates `detected_urls`.
- URL/file-path highlighting now correctly applies both foreground color and underline decoration in the pane render path. Detection was working but the visual styling was being applied to a discarded cell buffer; it is now applied directly to pane cells before GPU submission. Underline rendering was also missing entirely from the pane renderer and has been added.
- Close-tab confirmation now detects running jobs via OS process tree inspection (same approach as iTerm2), eliminating the previous requirement for shell integration to be configured. Any direct child process of the shell that is not in `jobs_to_ignore` will trigger the confirmation dialog.
- Close-tab/pane confirmation now routes through the full cleanup path on user confirmation, ensuring session undo capture, tab bar resize handling, alert sounds, and window-close detection all fire correctly.
- Cursor remaining visible and pinned to its terminal position while scrolling with the mouse wheel or scrollbar. The pane render path now hides the cursor whenever the viewport is scrolled into scrollback (`scroll_offset > 0`), matching the behaviour of the non-pane path.

### Added
- Configurable visual bell flash color (`notification_visual_bell_color`, default: white `[255, 255, 255]`) with color picker in Settings → Notifications → Bell.
- Dedicated visual bell shader (`visual_bell.wgsl`) for simple fullscreen quad rendering without vertex/instance buffers.
- Visual bell rendering in the pane render path (`render_split_panes`) so the flash overlay appears correctly in single-pane and split-pane modes.
- Per-pane selection state isolation for split-pane mode: each pane now owns its own selection, click tracking, and drag state, preventing garbled clipboard content when selecting across panes.
- Implemented `ScriptCommand` handlers for `WriteText`, `Notify`, `SetBadge`, `SetVariable`, `RunCommand`, and `ChangeConfig` with permission opt-ins and rate limiting.
- New `docs/ENVIRONMENT_VARIABLES.md` and `docs/API.md` references.
- Three-mutex policy documented in `src/lib.rs` and `docs/MUTEX_PATTERNS.md`.
- Try-lock failure telemetry for tracking dropped operations.
- Expanded test suites for tab bar UI and settings window.
- `src/ui_constants.rs` to centralize UI layout dimensions.
- Customizable `timeout_secs` for snippet shell commands.
- 73 keybinding integration tests covering parse/registry/lookup pipeline, modifiers, key aliases, physical keys, and error cases.
- 97 copy mode state machine tests covering motions, visual modes, selection, marks, search, and edge cases.

### Changed
- `pause_refresh_on_blur` now defaults to `true`: the reduced-FPS mode (using `unfocused_fps`, default 30) is enabled out of the box when the window loses focus, reducing CPU/GPU usage in the background.
- `tab_inactive_outline_only` now defaults to `true`: inactive tabs render as outline-only (no background fill) by default, giving the tab bar a cleaner appearance.
- Dark Background, Light Background, and Pastel (Dark Background) themes: all colors now accurately reflect iTerm2's plist values converted from NSCalibratedRGBColorSpace (genericRGB) to sRGB. This corrects foreground, cursor, selection, and all 16 ANSI colors for these three themes.
- High Contrast theme: corrected all 16 ANSI colors, foreground, cursor, and selection to match iTerm2's sRGB values (previously used crude max-saturation values that didn't match the iTerm2 preset).
- Default pane background opacity changed from 0.85 to 1.0 (fully opaque).
- Default font size increased from 12 to 13.
- `minimum_contrast` slider in Settings capped at 0.99; a saved value of exactly 1.0 (the old legacy default) is automatically migrated to 0.0 (disabled) on load.

### Fixed
- Fixed scrollbar cross-tab contamination: switching tabs no longer shows the previous tab's command marks in the scrollbar. Two root causes addressed: (1) `marks_override_scrollbar` now only forces the scrollbar visible when `scrollback_len > 0`, preventing a fresh tab's prompt mark (OSC 133;A) from making the scrollbar appear when there is nothing to scroll; (2) the scrollbar GPU cache key now includes marks count so tabs sharing the same `(scroll_offset, total_lines, rows)` values correctly refresh GPU mark data when marks differ.
- Fixed `clear` command not removing command markers from the scrollbar: ESC[2J and ESC[3J now emit a `ScreenCleared` event from the core library which the terminal manager uses to reset `ScrollbackMetadata` and `MarkerTracker`, clearing all scrollbar marks immediately.
- Fixed shell integration not detecting running commands: the OSC 133;C (CommandExecuted) handler was not parsing the optional command text from params[2], so `set_command()` was never called and the "confirm before closing tabs with running jobs" check always reported no running job.
- Fixed close-tab confirmation never showing when using the tab close button with "confirm before closing tabs with running jobs" enabled: `check_current_tab_running_job` and `check_current_pane_running_job` were acquiring a write lock (`try_write`) when only a read lock is needed (`blocking_read`), causing lock contention that always returned early.
- Fixed startup precedence: `auto_restore_arrangement` now takes priority over `restore_session` when both are enabled, so named arrangements explicitly override automatic session restoration.
- Fixed alternating launch bug where typing in a restored session immediately exits par-term with code 0: session capture was incorrectly saving `pane_layout = Some(Leaf{...})` for single-pane tabs (since R-32 made `pane_manager` always `Some`). On restore, `restore_pane_layout()` would spawn a second shell and drop the original pane, whose `Drop` impl killed the first shell's PTY via the shared `Arc`. Single-pane tabs now save `pane_layout = None` and restore using the tab-level CWD, consistent with the original design intent. A matching guard in `restore_session()` protects against old session files.
- Fixed search highlights (Cmd+F) never appearing: all tabs use the pane-manager render path, which gathers cells independently per pane. Highlights were being applied to the main frame cells that are never rendered. Highlights are now applied to the focused pane's cells in `gpu_submit.rs` after `gather_pane_render_data`, immediately before rendering.
- Fixed search navigation buttons (▲/▼) and close button (×) rendering as empty boxes: replaced Unicode arrows/multiply sign with Font Awesome icon codepoints (`\u{f062}`, `\u{f063}`, `\u{f00d}`) that are present in the embedded icon font.
- Fixed scrollbar thumb/track color changes in Settings not taking effect until a scroll event: `update_scrollbar_appearance` now resets `last_scrollbar_state` to force the next `update_scrollbar()` call to re-upload GPU uniforms regardless of scroll position.

### Changed
- Moved scrollbar settings (width, colors, autohide, command markers) from the Terminal tab to the Window tab in Settings, where window-layout controls are grouped.

- Fixed cursor text color (block cursor) setting having no visible effect: the default initial color when enabling the setting was `[0,0,0]` (black), identical to the auto-contrast result for the default white cursor, so enabling it produced no change. Initial color is now red `[255,0,0]` to make the effect immediately visible. Also fixed a secondary bug where `current_col` was not incremented on glyph resolution failure, causing the color override to be applied to the wrong column after any unrenderable character.
- Fixed cursor text color (block cursor) having no effect in split-pane mode: the pane renderer (`build_pane_instance_buffers`) never applied cursor text color logic to text instances. The same `cursor_is_block_on_this_row` / `render_fg_color` logic used in the single-pane path is now applied in the pane renderer as well.
- Fixed tab bar (left position) showing new-tab buttons at the bottom instead of the top: the "New Tab" and "New Tab from Profile" buttons in the vertical left-side tab bar are now rendered first, above the existing tabs.
- Fixed non-uniform text brightness: characters at certain column positions appeared slightly dimmer than neighbours due to `cell_width` being stored as a non-integer float. Per-column rounding caused `scale_x` to alternate (e.g. 7/7.8 ≈ 0.90 vs 8/7.8 ≈ 1.03), making glyphs sample the atlas at different rates. Cell dimensions are now rounded to integer pixels at initialisation, font-change, and cols/rows calculation so every character renders at scale 1.0.
- Fixed beam/underline cursor invisible when focused: cursor bar was rendered before text glyphs in the pane renderer, causing text to overdraw it. Added 3-phase rendering to `render_pane_to_view` (cell bgs → text → cursor overlays) so the cursor bar is always drawn on top.
- Fixed hollow cursor not appearing when window loses focus: the pane renderer (used for all tmux gateway sessions and any tab that has ever had splits) had no cursor overlay mechanism. Added beam cursor bar and hollow-border overlay instances to `build_pane_instance_buffers`, with hollow borders always alpha=1.0 regardless of blink phase so they remain visible throughout the blink cycle.
- Fixed hollow cursor not appearing when window loses focus (standard single-pane path): beam cursor was covered by text due to bg-pipeline running before text-pipeline; added 3-phase rendering to all three render methods (`render`, `render_to_texture`, `render_to_view`). Also fixed hollow cursor border alpha using blink opacity (0 when blinking off) and an `is_block` guard preventing borders from rendering when cursor style was beam or underline.
- Fixed ▄/▀ half-block gradient banding regression (e.g. `rich.palette`): half-block characters are now rendered entirely through the text pipeline (two quads per cell) instead of split across BG and text pipelines, eliminating cross-pipeline coordinate seams caused by floating-point sensitivity in optimized builds.
- Fixed macOS silent exit when pressing Cmd+, (Settings) before clicking in the window: replaced `PredefinedMenuItem::quit` (which called `[NSApp terminate:]` → `exit(0)`, bypassing all Rust cleanup) with a custom menu item that performs graceful shutdown, and moved menu installation before the blocking GPU initialization to ensure accelerators are active immediately.
- Fixed custom background shaders (e.g., rain) rendering as fully transparent in split-pane mode after the macOS alpha-coverage fix; the shader's render pass now uses final-mode opacity instead of chain mode, producing opaque premultiplied output.
- Fixed translucent right-half background on macOS: in Default and Color background modes, the right portion of the window (empty cells with default background) was see-through to the desktop because `LoadOp::Clear` alone is unreliable for alpha on macOS Metal with per-pixel transparency enabled. All background modes now render a full-screen opaque quad via `bg_image_pipeline`; Default mode creates a solid-color texture from the theme background colour so the pipeline always has coverage. Also fixed a pre-existing bug where the pane-viewport fill quad in split-pane mode used pixel coordinates instead of NDC, causing it to render entirely off-screen.
- Fixed cursor guide and cursor shadow having no visual effect: both effects were built only in the legacy single-pane render path (`build_cursor_overlay_instances`), but all tabs now use the pane-manager path (`build_pane_instance_buffers`). Guide and shadow overlay instances are now built in the pane renderer as well. Also reduced hollow cursor border width from 2 px to 1 px for a cleaner appearance.
- Fixed semantic file-path highlighting bleeding across tmux pane separators: the file-path detection regex now stops at Unicode box-drawing characters (U+2500–U+257F), so the link highlight colour is no longer applied to cells in the adjacent tmux pane.
- Fixed drag-selection often failing to copy text to clipboard due to `try_write()` race condition; mouse-release copy now uses `blocking_write()` to guarantee the selection is captured.
- Fixed clicking between tmux panes overwriting clipboard contents via accidental micro-selections; pane-focus clicks are now fully consumed before reaching selection-anchor code.
- Fixed text selection in split-pane mode reading from the wrong terminal buffer; selection now correctly reads from the focused pane's terminal.
- Fixed double-click and triple-click word/line selection occasionally failing to highlight due to the same `try_write()` contention.
- Wired `process_sync_actions` in TmuxSync dispatch to handle session, layout, output, and flow-control notifications.
- Fixed highlight flickering in `detect_urls` by preserving stale lists on lock misses.
- Resolved `window_opacity` state corruption during `render_to_texture`.
- Improved left/right modifier remapping logic.
- Resolved various panic-prone `.expect()` calls and improved error handling across modules.
- Added response size limits for update checker and ACP file reads.
- Fixed orphaned trigger processes and improved cleaning of tmux control mode on session end.
- Fixed potential panics in command truncation with multi-byte UTF-8 characters.
- Resolved dead code tracking for v0.26 removal.
- Annotated all `unsafe` blocks with `// SAFETY:` justifications.

### Security
- Migrated from `serde_yml` to `serde_yaml_ng` to resolve vulnerabilities.
- Enforced command allowlists for `ExternalCommandRenderer`.
- Blocked HTTP profile URLs by default and added warnings for MitM risks.
- Strengthened update checker with domain allowlists and binary content validation.
- Improved permissions for session logs and MCP IPC files.
- Added password redaction warnings for session logging.
- Prevented accidental commit of local API tokens via `.gitignore`.
- Added path traversal prevention for config paths and shader names.
- Hardened tmux command escaping to prevent truncation via null bytes.

### Changed
- Refactored minimum contrast from WCAG ratio scale (1.0–21.0) to iTerm2-compatible perceived brightness scale (0.0–1.0). Settings slider and labels updated accordingly.

### Refactored
- Full codebase refactor audit (28 findings): eliminated all `#[allow(clippy::too_many_arguments)]` suppressions (23→0) via parameter builder structs; extracted `GlobalShaderConfig`, `AiInspectorConfig`, and `StatusBarConfig` sub-structs from the monolithic `Config` struct; split 12 files exceeding 800 lines into focused sub-modules; removed all dead-code suppressions; consolidated duplicate implementations (`shell_detection`, `profile_modal_ui`). 1,065 tests pass with zero regressions.
- Centralized `make_block` / `make_block_with_command` test factories in `prettifier/testing.rs`; removed 24 duplicate local definitions across detector and renderer test files (R-30).
- Decomposed 890-line `submit_gpu_frame()` into `update_gpu_renderer_state()`, `render_egui_frame()`, and `scroll_offset_from_tab()` helpers (R-31).
- Added 7 semantic color accessors to `ThemeColors` (`dim_color`, `string_color`, `key_color`, `error_color`, `number_color`, `comment_color`, `accent_color`); extracted shared `guide_segment`, `plain_segment`, `dim_segment` helpers to `renderers/mod.rs`; migrated 250+ raw `palette[N]` indices across json/yaml/toml/xml parsers (R-34).
- Split 925-line `markdown/tests.rs` into 5 focused sub-files under `tests/` (R-36).
- Split `cli.rs` (664 lines) into `cli/mod.rs` (arg types + dispatch) and `cli/install.rs` (install procedures) (R-37).
- Extracted `apply_config_to_windows` (~700 lines) from `settings_actions.rs` into `window_manager/config_propagation.rs`; `settings_actions.rs` reduced from 795 → 233 lines (R-39).
- Extracted `xml.rs` inline tests into `renderers/xml/mod.rs` + `renderers/xml/tests.rs`, consistent with other renderer layout (R-40).
- Extracted `regex_detector.rs` inline tests (410 lines) into `regex_detector/mod.rs` + `regex_detector/tests.rs` (R-41).
- Split `file_transfers.rs` (780 lines) into `file_transfers/types.rs`, `file_transfers/overlay.rs`, and `file_transfers/mod.rs` (R-42).
- Extracted rule-loading logic from `config_bridge.rs` into `prettifier/rule_loader.rs`; `config_bridge.rs` reduced from 718 → ~170 lines (R-43).
- Extracted per-pane accessor routing from `tab/mod.rs` into `tab/pane_accessors.rs`; `mod.rs` reduced from 805 → 664 lines (R-45).
- Split `copy_mode/mod.rs` (637 lines) into module directory: `types.rs`, `cursor.rs`, `motion.rs`, `visual.rs`, `search.rs` + 83-line orchestrator (R-46).
- Extracted `TabBarUI` struct to `tab_bar_ui/state.rs` and `render_horizontal` to `tab_bar_ui/horizontal.rs`; `mod.rs` reduced from 714 → 361 lines (R-47).
- Extracted `ConfigurableDetector` subtrait from `ContentDetector`; `RendererRegistry` dispatches through `as_configurable_mut()` instead of silent empty default methods (R-51).
- Extracted `src/prettifier/` (93 files, 22 778 L) and `src/ansi_colors.rs` into a new `par-term-prettifier` workspace sub-crate; debug macros shimmed to `log::*`; config re-exported via a facade module so internal files required zero import changes; CI workflow updated with Layer 2 publish step (R-03).
- Promoted `src/app/window_state/render_pipeline/` to `src/app/render_pipeline/` using a `#[path]` attribute in `window_state/mod.rs`; logical module path and all `pub(super)` / `super::` semantics preserved with zero caller changes (R-14).
- Eliminated all 19 flat `.rs` files from `src/app/` root: 13 `impl WindowState` files and sub-state structs moved to `window_state/`; per-pane types (`BellState`, `MouseState`, `RenderCache`) moved to `pane/`; copy mode split into `copy_mode/handler.rs` + `copy_mode/search.rs`; `TmuxState` moved to `tmux_handler/`; `src/app/` now has only `mod.rs` at root level (R-02).
- Decomposed `WindowState` and `Config` into cohesive sub-state objects.
- Extracted `EguiState` sub-struct from `WindowState` (ARC-001 continuation).
- Added `with_active_tab()`, `with_active_tab_mut()`, `with_window()`, `request_redraw()` helpers on `WindowState`; converted 60+ call sites.
- Added `try_with_terminal()` / `try_with_terminal_mut()` helpers on `Tab`; converted 7 call sites.
- Split `agent_messages.rs` (1,006 lines) into `agent_config.rs` + `agent_screenshot.rs`.
- Split `triggers.rs` (832 lines) into `mark_line`, `prettify`, and `sound` sub-modules.
- Split `scripting.rs` (820 lines), extracting `config_change` sub-module.
- Converted `url_detection.rs`, `copy_mode.rs` into module directories with separate test files.
- Extracted `DRAG_THRESHOLD_PX`, `CLICK_RESTORE_THRESHOLD_PX`, `SCROLLBAR_MARK_HIT_RADIUS_PX`, `VISUAL_BELL_FLASH_DURATION_MS` into `ui_constants.rs`.
- Migrated terminal access from `Mutex` to `RwLock` for better read concurrency.
- Extracted shared initialization logic for tabs and panes.
- Unified GLSL transpiler templates and added WGSL injection validation.
- De-duplicated `Makefile` variables and targets.
- Added per-pane state accessors on `Tab` (`active_scroll_state`, `active_mouse`, `active_cache`, `active_bell` + mut variants) routing through `PaneManager` in split-pane mode; updated ~30 call sites (AUD-002/AUD-062).
- Created `src/platform/` module consolidating platform-specific notification delivery and modifier key detection, reducing `#[cfg]` blocks across 8 files (AUD-060).
- Split 8 large files (>800 lines) into focused sub-modules: `json.rs`, `toml.rs`, `yaml.rs`, `log.rs` renderers; `boundary.rs`; `session_logger.rs`; `shader_context.rs`; `profile/dynamic.rs` (AUD-011/012/015/016/018/019/023/024).
- Created `TerminalAccess`, `UIElement`, and `EventHandler` traits in `src/traits.rs` (AUD-040/041/042).
- Added 128 new tests: coordinate translation, pane bounds, and tests extracted during file splits (AUD-050/053).

### Documentation
- Added legacy field migration plans for `Tab` struct.
- Documented 3-tier shader resolution chain.
- Updated `CONTRIBUTING.md`, `docs/CONCURRENCY.md`, `docs/STATE_LIFECYCLE.md`, and `docs/ARCHITECTURE.md` with deep technical overviews.
- Simplified `README.md` with a quick start guide.
- Added per-module documentation for re-exports, locking rules, and architectural patterns.

### Changed
- Centralized config saves with a 100ms debounce.
- Prettifier is now disabled by default.
- Enabled automatic CI triggers for main and PRs.

### Performance
- Eliminated per-frame GPU buffer allocations for pane backgrounds using a uniform buffer cache.
- Implemented scratch `Vec` reuse in `CellRenderer`.
- Added regex caching for triggers.
- Replaced per-frame `StyledLine` clones with borrows.
- Integrated native filesystem watchers for config hot-reload.

---

## [0.24.0] - 2026-02-27

### Fixed
- **Box-Drawing Line Thickness**: Snapped box-drawing pixel rectangles to integer boundaries for consistent line thickness.
- **Prettifier Improvements**: Fixed source-to-rendered line mapping, synced cell dimensions for inline graphics, and implemented Claude Code integration enhancements.
- **Security & Reliability**: Sanitized paste control characters, restricted MCP IPC file permissions, and redacted passwords from session logs.
- **System**: Implemented graceful shutdown sequence and restricted config variable substitution to an allowlist.

### Changed
- **Internal Architecture**: Decomposed `window_state.rs` into focused sub-modules and extracted render coordination functions.

---

## [0.23.0] - 2026-02-25

### Added
- **Content Prettifier**: New system to detect and render structured content (Markdown, JSON, etc.) with syntax highlighting and format-specific enhancements.

### Changed
- **Font Hinting**: Enabled by default for improved text sharpness.
- **Dependencies**: Updated workspace dependencies to latest versions.

### Fixed
- **Settings Search**: Fixed and updated search keywords across all settings tabs.
- **Split Pane Mode**: Fixed inline graphics, scrollback, and scrollbar rendering in split-pane layouts.
- **Window Arrangements**: Resolved DPI-related positioning and sizing issues on multi-monitor setups.
- **Rendering**: Fixed character artifacts in glyph atlas and improved symbol rendering from emoji fonts.
- **Usability**: Improved text selection in mouse-tracking apps and fixed trackpad micro-selection jitter.

---
## [0.22.0] - 2026-02-22

### Added
- **Assistant Panel**: Added code block rendering, message queueing/cancellation, and multi-line chat input.
- **ACP Integration**: Support for custom ACP agents (including Ollama) and better context restoration across reconnects.
- **Debugging**: New `par-term-acp-harness` for reproducing Assistant Panel sessions and `terminal_screenshot` MCP tool.
- **Aesthetics**: New `glass-sphere-bounce.glsl` shader and sharpened tab bar borders.

### Changed
- **Dependencies**: Updated `par-term-emu-core-rust` and rebranded Claude ACP bridge package.
- **Security**: Split screenshot permissions from YOLO mode.

### Fixed
- **Performance**: Resolved input and shader lag by refining idle-throttling logic.
- **ACP Handshaking**: Fixed connection failures in app bundles and nested session blocking.
- **UI/UX**: Resolved chat input visibility issues, UTF-8 command truncation panics, and Escape key behavior.

---
## [0.21.0] - 2026-02-20

### Added
- **Customization**: Replaced emoji presets with ~120 Nerd Font icons and added support for per-tab custom icons and manual renaming.
- **Tab Behavior**: New `tab_title_mode` for finer control over automatic title updates.

### Changed
- **Power Efficiency**: Major reduction in idle CPU usage (~103% to ~18-25%) via adaptive polling and conditional dirty tracking.
- **UI Responsiveness**: Decoupled idle wakeup cadence from FPS and throttled inactive tab refresh.

### Fixed
- **Multi-Window Layouts**: Fixed tab property restoration for arrangements with multiple windows.
- **Responsiveness**: Resolved input lag during heavy output by switching to `try_lock()` in the render path.
- **Rendering**: Fixed tab bar corner thickness, scrollbar overlap, and vertically squashed Unicode symbols.

---
## [0.20.0] - 2026-02-20

### Added
- **Updates**: Hourly update check frequency and a new clickable status bar widget for available updates.
- **UI/UX**: Dropdown new-tab menu, real-time pane background previews, and a file transfer progress overlay.
- **Shaders**: New `rain-glass.glsl` background shader and an outline-only mode for inactive tabs.

### Changed
- **Defaults**: Disabled window padding by default and set `tab_bar_mode` to `always`.

### Fixed
- **File Transfers**: Fixed uploads hanging over SSH and implemented background threads for PTY writes.
- **Split Panes**: Corrected mouse event routing and divider resize logic in split-pane mode.
- **Rendering**: Resolved inline image display issues for large files and fixed live window padding updates.

---
## [0.19.0] - 2026-02-19

### Added
- **Link Highlighting**: Configurable link highlight colors, underlining support, and stipple underline style.
- **Settings**: Auto-focus for settings search input.

### Fixed
- **Shutdown**: Implemented fast window shutdown by moving I/O to background threads.
- **Symbols**: Fixed media control character rendering as colored emoji.
- **Distribution**: Reduced crate package size by excluding non-essential files.

---
## [0.18.0] - 2026-02-18

### Added
- **Quick Settings**: Added BG and Cursor Shader toggles to the quick settings strip.
- **Focus Tracking**: Forward CSI focus-in/out sequences to PTYs for applications like tmux.

### Fixed
- **Rendering**: Fixed dingbat/symbol characters rendering as colored emoji instead of monochrome.
- **Input**: Suppressed focus clicks to prevent accidental clipboard loss in mouse-aware apps.
- **Shell Detection**: Improved shell detection with multi-strategy fallback.
- **Settings**: Fixed empty icons in the settings sidebar and resolved version display issues.

### Refactored
- Collapsed `src/config/` re-export layer (~4,800 lines of duplicates removed).
- Extracted SSH, keybinding, scripting, update, input, MCP, and tmux subsystems into dedicated workspace crates.

---
## [0.17.1] - 2026-02-18

### Changed
- Updated workspace dependencies including `zip`, `mdns-sd`, and `ureq`.

### Fixed
- **macOS**: Resolved self-update quarantine issues by stripping Gatekeeper attributes.
- **CI**: Fixed workspace subcrate publishing order.

---
## [0.17.0] - 2026-02-17

### Added
- **Assistant Panel**: DevTools-style panel for terminal inspection and ACP agent integration.
- **Shader Assistant**: Context-triggered shader expertise for agents.
- **File Transfers**: Native UI for iTerm2 OSC 1337 transfers.
- **Per-Pane Backgrounds**: Independent background images for each split pane.
- **Scripting**: New Python-based scripting manager for reacting to terminal events.
- **Team Features**: Dynamic profile loading from remote URLs.
- **Aesthetics**: Auto dark mode and automatic tab styling based on system theme.

### Changed
- Refactored core modules (fonts, terminal, settings, rendering) into dedicated workspace crates.
- Renamed "AI Inspector" to "Assistant".

### Fixed
- Resolved Shift+Tab interception issues.
- Implemented instant window shutdown on macOS.

---
## [0.16.0] - 2026-02-13

### Added
- **Status Bar**: Configurable bar with widgets for system monitoring and session info.
- **Remote Integration**: Support for installing shell integration via SSH.
- **Native Menus**: Platform-appropriate settings access from application menus.
- **SSH Host Management**: Integrated SSH config parsing and Quick Connect dialog.
- **Profile Improvements**: Profile selection on new-tab button and per-profile shell overrides.

---
## [0.15.0] - 2026-02-12

### Added
- **Auto-Switching**: Automatically switch profiles based on current working directory patterns.
- **UI/UX**: Nerd Font icon picker for profiles and support for tab style variants.
- **Audio**: Configurable alert sounds for terminal events.
- **History**: Fuzzy search overlay for command history.
- **Session Management**: Session undo (reopen closed tabs) and automatic session restoration on startup.
- **Layout**: Support for bottom and left tab bar positions.

### Improved
- Moved profile management directly into the Settings window.

### Fixed
- Resolved HiDPI/DPI scaling issues across all UI components.
- Fixed keyboard shortcut routing in egui overlays.

---
## [0.14.0] - 2026-02-11

### Added
- **Self-Update**: In-place update system detecting installation method (Homebrew, cargo, bundle, etc.).
- **Command Separators**: Optional horizontal lines between shell commands using OSC 133 marks.
- **Config Variables**: Environment variable substitution in `config.yaml` using `${VAR}` syntax.
- **Tab Reordering**: Drag-and-drop support for reordering tabs in the tab bar.
- **Window Arrangements**: Save and restore named window layouts with monitor-aware positioning.
- **Settings Persistence**: Persistent expand/collapse states for settings window sections.

### Changed
- Increased default `font_size` to 12.0.

### Fixed
- Improved update notifications and resolved duplicate arrangement name issues.

---
## [0.13.0] - 2026-02-10

### Added
- **Copy Mode**: Keyboard-driven text selection and navigation (Vi-style).
- **Unicode Normalization**: Support for NFC (default), NFD, NFKC, and NFKD forms.
- **Snippets & Actions**: Completed custom variables UI, key sequence simulation, and import/export.

### Fixed
- Resolved emoji rendering issues, tmux pane resize via mouse drag, and link highlighting offsets.

---
## [0.12.0] - 2026-02-10

### Added
- **Snippets & Actions**: New system for text automation and custom macros.
- **Progress Bars**: Thin overlay bars supporting OSC 9;4 and OSC 934 protocols.
- **Paste Improvements**: Configurable paste delay and new newline-control transformations.
- **Pane Enhancements**: GPU-rendered title bars and customizable divider styles.
- **Integration**: OSC 1337 RemoteHost support and current command display in window title.

### Changed
- Major cross-platform keybinding overhaul and modernized terminfo.

### Fixed
- Resolved pane focus indicator settings, background opacity issues, and Linux Ctrl+C behavior.

---
## [0.11.0] - 2026-02-06

### Added
- **Automation**: New "Automation" settings tab for managing regex triggers and coprocesses.
- **Triggers**: Match terminal output to fire actions (highlight, notify, play sound, send text, etc.).
- **Coprocesses**: Background processes that receive terminal output with restart policies.
- **Accessibility**: WCAG-based minimum contrast enforcement.
- **Semantic History**: Ctrl+click (Cmd+click) on file paths to open them in a configured editor.
- **Logging**: Configurable runtime log level control.

### Changed
- Unified logging bridge and improved coprocess PATH resolution.

### Fixed
- Resolved trigger mark deduplication and improved scrollbar command text capture.

---
## [0.10.0] - 2026-02-04

### Added
- **Confirm Close**: Confirmation dialog when closing tabs/panes with active jobs.
- **Exit Action**: Configurable behavior when a shell process exits (close, keep, restart).
- **Modifier Remapping**: Independent remapping for left/right Ctrl, Alt, and Super keys.
- **Physical Keys**: Option to match keybindings by physical position (scan code).
- **Keyboard Protocols**: Support for XTerm `modifyOtherKeys` extension.
- **Performance**: iTerm2-style flicker reduction and manual "Maximize Throughput" mode.
- **Customization**: GPU power preference and per-profile badge configuration.

### Fixed
- Resolved arrow key issues in `less` and other pagers using DECCKM mode.

---
## [0.9.0] - 2026-02-04

### Added
- **Profiles Tab**: New tab in Settings for profile management and drawer visibility toggle.
- **tmux Formatting**: Customizable tmux status bar content via format strings.
- **Welcome Dialog**: Added a link to the changelog in the onboarding popup.

### Fixed
- Resolved segfaults on exit, Windows ARM64 build failures, and HTTPS request panics.
- Improved Windows taskbar icon handling and file watching.

---
## [0.8.0] - 2026-02-03

### Added
- **Startup Directory**: Control over initial working directory (home, previous, or custom).
- **Badge System**: Semi-transparent text overlays with dynamic session variables.
- **Tab Enhancements**: Support for tab stretching and HTML markup in titles.
- **UI/UX**: Tooltips for scrollbar marks and "Reset to Defaults" button in Settings.

### Changed
- Updated core library and enabled tab stretching by default.

### Fixed
- Resolved Windows console window visibility and bash shell integration exit codes.

---
## [0.7.0] - 2026-02-02

### Added
- **Integrations**: Unified installation system for shell integration and shader bundles.
- **Settings**: Added missing UI controls for various configuration options.
- **tmux**: Native status bar display and improved multi-client sync in control mode.
- **Session Logging**: Automatic recording of terminal output in text, HTML, or asciicast formats.
- **Profile System**: Full CRUD for named profiles with a collapsible drawer.
- **Window Management**: New window types (fullscreen, edge-anchored) and target monitor selection.
- **Unicode**: Configurable Unicode version and ambiguous width settings.
- **Paste Special**: Command palette for transforming clipboard content before pasting.
- **Notifications**: Desktop alerts for session exit, activity, and silence.
- **Mouse**: Advanced mouse features including Option+Click cursor movement and focus-follows-mouse.
- **Selection**: Smart selection rules and auto-quoting for dropped files.
- **Search**: Incremental search through scrollback buffer with match highlighting.
- **Font**: Rendering options for anti-aliasing, hinting, and thin strokes.

### Fixed
- Resolved tmux pane display issues, Shift+Enter behavior, and multi-window focus routing.
- Improved DPI scaling across all UI components and fixed various rendering overlaps.

---
## [0.6.0] - 2026-01-29

### Added

- **Shader Gallery**: Visual gallery with screenshots of all 49+ included shaders
  - Hosted on GitHub Pages at https://paulrobello.github.io/par-term/
  - Auto-deploys on changes to gh-pages folder
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

- **Option+Click Cursor Movement**: Use arrow key sequences instead of absolute cursor positioning
  - Shells interpret arrow keys correctly for cursor movement within command line
  - Queries terminal's actual cursor position to calculate movement delta
- **Option+Click Selection Conflict**: Prevent text selection when Option+click moves cursor
  - Button press state now set after special click handlers return
  - Rectangular selection changed to Option+Cmd (matching iTerm2)
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

[Unreleased]: https://github.com/paulrobello/par-term/compare/v0.24.0...HEAD
[0.24.0]: https://github.com/paulrobello/par-term/compare/v0.23.0...v0.24.0
[0.23.0]: https://github.com/paulrobello/par-term/compare/v0.22.0...v0.23.0
[0.22.0]: https://github.com/paulrobello/par-term/compare/v0.21.0...v0.22.0
[0.21.0]: https://github.com/paulrobello/par-term/compare/v0.20.0...v0.21.0
[0.20.0]: https://github.com/paulrobello/par-term/compare/v0.19.0...v0.20.0
[0.19.0]: https://github.com/paulrobello/par-term/compare/v0.18.0...v0.19.0
[0.18.0]: https://github.com/paulrobello/par-term/compare/v0.17.1...v0.18.0
[0.17.1]: https://github.com/paulrobello/par-term/compare/v0.17.0...v0.17.1
[0.17.0]: https://github.com/paulrobello/par-term/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/paulrobello/par-term/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/paulrobello/par-term/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/paulrobello/par-term/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/paulrobello/par-term/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/paulrobello/par-term/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/paulrobello/par-term/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/paulrobello/par-term/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/paulrobello/par-term/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/paulrobello/par-term/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/paulrobello/par-term/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/paulrobello/par-term/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/paulrobello/par-term/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/paulrobello/par-term/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/paulrobello/par-term/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/paulrobello/par-term/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/paulrobello/par-term/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/paulrobello/par-term/releases/tag/v0.1.0
