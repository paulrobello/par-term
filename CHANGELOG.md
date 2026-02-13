# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added

- **Install Shell Integration on Remote Host** (#135): New Shell menu with option to install shell integration on remote hosts via SSH
  - Shell > Install Shell Integration on Remote Host... sends curl command to active terminal
  - Confirmation dialog shows exact command before sending
  - Uses existing install script hosted on GitHub Pages

- **Navigate to Settings from Application Menu** (#127): Platform-aware settings access from native application menu
  - macOS: "par-term" application menu with Settings... (Cmd+,), About, and standard macOS items (Services, Hide, Hide Others, Show All, Quit)
  - Windows/Linux: Edit > Preferences... (Ctrl+Shift+,) follows platform conventions
  - View > Settings... (F12) retained on all platforms for backward compatibility

- **Profile Selection on New Tab Button** (#129): Split button on the tab bar for quick profile-based tab creation
  - `+` button creates a default tab (existing behavior preserved)
  - `▾` chevron opens a profile dropdown with "Default" at top + all profiles in order with icons
  - Chevron only appears when profiles exist
  - Works in both horizontal and vertical tab bar layouts
  - New config option `new_tab_shortcut_shows_profiles` (default: false) to make Cmd+T / Ctrl+Shift+T show the profile picker instead
  - Escape key dismisses the dropdown
  - Settings checkbox in Window > Tab Behavior section

- **Shell Selection Per Profile** (#128): Configure a specific shell for each profile with platform-aware detection
  - New `shell` field on profiles selects a shell independently of the `command` field
  - New `login_shell` field per profile overrides the global login shell setting (None = inherit, true/false = override)
  - Platform-aware shell detection: parses `/etc/shells` on Unix/macOS, checks known locations on Windows (PowerShell, cmd, Git Bash, WSL, MSYS2, Cygwin)
  - Shell dropdown in profile editor with all detected shells + "Default (inherit global)" option
  - Priority: profile `command` > profile `shell` > global `custom_shell` / `$SHELL`
  - Inheritable via profile parent system
  - Search keywords added to Settings Profiles tab for discoverability

---

## [0.15.0] - 2026-02-12

### Added

- **Directory-Based Profile Switching** (#114): Automatically switch profiles based on current working directory
  - New `directory_patterns` field on profiles (glob patterns like `/Users/*/projects/work-*`)
  - CWD changes detected via OSC 7 trigger profile matching
  - Priority: explicit user selection > hostname match > directory match > default
  - Settings UI for editing directory patterns per profile
  - Does not override explicit user profile selection or hostname-based switching

- **Profile Emoji Picker** (#114): Emoji picker popup for the profile icon field in the profile modal
  - Curated grid of ~70 terminal-relevant emojis in 9 categories (Terminal, Dev & Tools, Files & Data, Network & Cloud, Security, Status & Alerts, Containers & Infra, People & Roles, Misc)
  - Scrollable popup with category headers and one-click selection
  - Users can still type custom emojis directly in the text field
  - "Clear icon" button to remove the current icon

- **Full Profile Auto-Switch Application** (#114): Auto-switched profiles now apply all visual settings
  - **Directory switching**: Applies profile icon in tab bar, overrides tab title, applies badge text and badge styling (color, alpha, font, bold, margins, size), executes profile command
  - **Hostname switching**: Brought to full parity — applies icon, title, badge text/styling, and command execution on remote host detection
  - **Tmux session switching**: Brought to full parity — applies icon, title, badge text/styling, and command execution on session name match
  - Profile icon displayed in both horizontal and vertical tab bar layouts
  - Original tab title saved and restored when auto-profile clears

- **Tab Style Variants** (#112): Cosmetic tab bar presets with 5 built-in styles
  - Dark (default), Light, Compact, Minimal, and High Contrast presets
  - Each preset applies coordinated color/size/spacing adjustments
  - Exposed as dropdown in Settings > Window > Tab Bar
  - Config: `tab_style: dark|light|compact|minimal|high_contrast`

- **Alert Sounds** (#112): Configurable sound effects for terminal events
  - Per-event sound configuration: Bell, Command Complete, New Tab, Tab Close
  - Each event supports: enable/disable, volume, frequency, duration, custom sound file
  - Custom sound files: WAV/OGG/FLAC format with `~` home directory expansion
  - Falls back to legacy `notification_bell_sound` for backward compatibility
  - Config: `alert_sounds` map with per-event `AlertSoundConfig` entries
  - UI in Settings > Notifications > Alert Sounds

- **Fuzzy Command History Search** (#118): Searchable overlay for browsing and selecting from command history
  - Fuzzy matching with ranked results via Skim algorithm (`fuzzy-matcher` crate)
  - Match highlighting with yellow underline on matching characters
  - Exit code indicators (green/red/gray) and relative timestamps per entry
  - Keyboard navigation: Arrow Up/Down, Enter to insert, Esc to close
  - Commands captured from OSC 133 shell integration markers and core library history
  - Deduplicated entries (re-executing moves command to top)
  - History persisted across sessions to `~/.config/par-term/command_history.yaml`
  - Config: `command_history_max_entries` (default: 1000), exposed in Settings > Terminal > Command History
  - Keybinding: Cmd+R (macOS), Ctrl+Alt+R (Linux/Windows)

- **Import/Export Preferences** (#91): Import and export terminal configuration
  - Export current config to a YAML file via native file dialog
  - Import preferences from a local YAML file (replace or merge modes)
  - Import preferences from a URL (replace or merge modes)
  - Merge mode only overrides values that differ from defaults, preserving user customizations
  - Validation ensures imported config is well-formed before applying
  - UI in Settings > Advanced > Import/Export Preferences

- **Session Undo — Reopen Closed Tabs** (#115): Recover accidentally closed tabs
  - Captures tab metadata (CWD, title, position, pane layout, custom color) on close
  - Reopen with Cmd+Z (macOS) or Ctrl+Shift+Z (Linux/Windows)
  - Toast notification shows undo keybinding hint and countdown
  - Configurable timeout: `session_undo_timeout_secs` (default: 5s, 0 = disabled)
  - Configurable queue depth: `session_undo_max_entries` (default: 10)
  - **Preserve shell session**: `session_undo_preserve_shell` (default: false) — when enabled,
    closing a tab hides the shell instead of killing it; undo restores the full session with
    scrollback, running processes, and pane layout intact
  - Restores tab at original position with title, custom color, and split pane layout
  - Expired entries automatically pruned from queue (hidden tabs killed on expiry)
  - UI controls in Settings > Terminal > Startup

- **Session Restore on Startup** (#117): Automatically save and restore session state
  - Saves open windows, tabs, pane layouts, and working directories on clean exit
  - Restores full session on next launch including split pane trees with ratios
  - Working directories validated on restore; missing directories fall back to `$HOME`
  - Takes precedence over `auto_restore_arrangement` when both are enabled
  - Graceful degradation: corrupt/missing session file creates default window
  - Session file cleared after successful restore to prevent stale state
  - Config: `restore_session: true` (default: false)
  - UI checkbox in Settings > Terminal > Startup

- **Tab Bar Position** (#116): Configurable tab bar placement with three positions
  - **Top** (default): Current horizontal tab bar at the top of the window
  - **Bottom**: Horizontal tab bar below terminal content
  - **Left**: Vertical sidebar with scrollable tab list, active indicator, and drag-and-drop reordering
  - Configurable sidebar width for Left position (default 160px, range 100–300)
  - All positions support tab bar visibility modes (always/when_multiple/never)
  - Live switching via Settings UI dropdown without restart
  - Config: `tab_bar_position: top|bottom|left`, `tab_bar_width: 160.0`
  - UI in Settings > Window > Tab Bar

### Improved

- **Profile management moved to Settings window** (#125): Profile create/edit/delete/reorder UI is now embedded inline in the Settings > Profiles tab, replacing the terminal-window modal dialog. The profile drawer's "Manage" button and the menu's "Manage Profiles" action now open the Settings window to the Profiles tab. This eliminates the disjointed settings → terminal → modal flow.
- **Settings quick search**: Added missing search keywords across all settings tabs for better discoverability of settings via the search box

### Fixed

- **Paste/Copy/Select-All in egui overlays**: Cmd+V, Cmd+C, and Cmd+A now correctly route to egui text fields when a modal dialog is active (profile modal, search overlay, clipboard history, command history, shader install, integrations). Previously, macOS menu accelerators (muda) intercepted these shortcuts and sent them to the terminal instead.
- **Directory pattern tilde expansion**: Profile directory patterns using `~` (e.g., `~/Repos/par-term*`) now correctly expand to the home directory before matching. Previously, `~` was treated as a literal character and never matched.
- **Comprehensive HiDPI/DPI scaling fix**: All pixel-dimension config values are now correctly scaled from logical pixels to physical pixels on HiDPI displays (e.g., Retina at scale_factor=2). Previously, many values rendered at half their intended size. Fixed values include:
  - Tab bar content offset (#121), window padding, scrollbar width
  - Pane padding, divider width/hit width, title height, focus border width
  - Command separator thickness, cursor shadow offset/blur, cursor glow radius
  - Cursor bar/underline thickness, cursor boost glow
  - Tmux status bar height in layout calculations, tab bar mouse click guard
  - All values are correctly rescaled when moving windows between displays with different DPIs
- **Pane divider drag resize** now uses the configured divider width instead of a hardcoded 1.0px value, fixing inaccurate resize calculations when divider width differs from 1.0
- **Profile parent selector** now prevents creating inheritance cycles (e.g., A→B→A) by checking ancestor chains before listing valid parents

### Changed

- **Default update check frequency** changed from weekly to daily for faster update discovery
- **Text shaper cache** upgraded from arbitrary FIFO eviction to proper LRU eviction via the `lru` crate, improving cache hit rates for frequently-shaped text runs

### Removed

- Removed unused `Config::new()` constructor and builder methods (`with_dimensions`, `with_font_size`, `with_font_family`, `with_scrollback`); use `Config::default()` instead
- Removed unused `CustomShaderRenderer` methods: `update_from_resolved_config()`, `update_channels_from_resolved_config()`
- Removed unused `SettingsWindow::instance` field, `GraphicsRenderer::surface_format` field, `CustomShaderRenderer::text_opacity` field
- Removed dead `ansi_to_rgb()` utility function and tmux layout parsing stub (`parse_layout_to_splits`, `LayoutSplit`)
- Removed stale TODO comment about cursor geometric rendering (already implemented via `CellRenderer::cursor_overlay`)

---

## [0.14.0] - 2026-02-11

### Added

- **Self-Update** (#82): par-term can now download and install updates in-place
  - CLI: `par-term self-update` with `--yes` flag for non-interactive use
  - Settings UI: "Check Now" and "Install Update" buttons in Advanced > Updates
  - Detects installation method (Homebrew, cargo, .app bundle, standalone binary)
  - Homebrew/cargo installs show appropriate upgrade instructions instead
  - macOS .app bundle: extracts and replaces bundle contents from zip
  - Linux/Windows standalone: atomic binary replacement
  - Async update with progress indication in Settings UI

- **Command Separator Lines**: Horizontal separator lines between shell commands in the terminal grid
  - Renders thin lines at prompt boundaries using existing shell integration (OSC 133) marks
  - Exit-code coloring: green for success, red for failure, gray for unknown
  - Configurable thickness (0.5-5.0 px), opacity (0.0-1.0), and custom fixed color
  - Works with any prompt height (single-line, multi-line, tall starship/powerline prompts)
  - Separator placed at the top of the prompt (PromptStart marker), not the cursor line
  - Works in both single-pane and split-pane modes
  - Respects trigger mark custom colors
  - Settings UI in Terminal > Command Separators section
  - Config: `command_separator_enabled`, `command_separator_thickness`, `command_separator_opacity`, `command_separator_exit_color`, `command_separator_color`
  - Disabled by default (opt-in)

- **Shell Integration Event Queuing** (core library): OSC 133 markers now queue with cursor positions
  - Each shell integration marker records the absolute cursor line at parse time
  - Eliminates marker batching where multiple markers between frames collapsed to one
  - Ensures accurate separator/mark placement for multi-line prompts
  - `__PAR_TERM` environment variable set for shell integration script detection

- **Variable Substitution in Config** (#102): Use `${VAR}` environment variable references in `config.yaml` values
  - All string config fields support substitution (e.g., `background_image: "${HOME}/Pictures/bg.png"`)
  - Default values via `${VAR:-default}` syntax for unset variables
  - Escape with `$${VAR}` to produce the literal `${VAR}`
  - Unset variables without defaults are left unchanged

- **Drag-and-Drop Tab Reordering** (#106): Reorder tabs by dragging them in the tab bar
  - Click and drag any tab to move it to a new position
  - Floating ghost tab follows the cursor during drag with semi-transparent preview
  - Blue insertion indicator line with glow effect shows the drop target
  - Dragged tab dims in place to indicate it is being moved
  - Escape key cancels the drag operation
  - Single-tab windows correctly suppress drag initiation
  - Existing keyboard reorder shortcuts (Cmd/Ctrl+Shift+Arrow) continue to work

- **Window Arrangements** (#103): Save and restore window layouts (iTerm2 parity)
  - **Save**: Capture current window positions, sizes, tab CWDs, and active tab indices as named arrangements
  - **Restore**: Recreate saved layouts, replacing all current windows
  - **Monitor-aware**: Positions stored relative to monitor origin for portability across display configurations
  - **Monitor matching**: Restores windows to correct monitors by name, then index fallback, then primary monitor
  - **Position clamping**: Ensures restored windows are visible even if monitor layout has changed
  - **Auto-restore on startup**: Configure an arrangement to restore automatically when the app launches
  - **Settings UI**: New "Arrangements" tab (📐) with save, restore, rename, delete, and reorder controls
  - **Menu integration**: "Save Window Arrangement..." item in View menu
  - **Keybinding support**: `save_arrangement` and `restore_arrangement:<name>` keybinding actions
  - **YAML persistence**: Arrangements stored in `~/.config/par-term/arrangements.yaml`

- **Remember Settings Section States** (#105): Settings window section expand/collapse states now persist across sessions
  - Collapsible section states (expanded/collapsed) are tracked and saved to `collapsed_settings_sections` in config
  - States persist when the settings window is closed and reopened within the same session
  - States persist across app restarts via automatic save on settings window close
  - States are also saved when explicitly clicking "Save" in settings

### Changed

- **Default Font Size**: Increased default `font_size` from 10.0 to 12.0 for better readability out of the box

### Fixed

- **Duplicate arrangement names allowed**: Saving a window arrangement with the same name as an existing one now prompts to overwrite instead of creating a duplicate entry
- **Update notification clipped on some systems**: Shortened the new version notification body text and added a timeout to prevent content being cut off in small system notification windows

---

## [0.13.0] - 2026-02-10

### Added

- **Vi-Style Copy Mode** (#99): Keyboard-driven text selection and navigation (iTerm2 parity)
  - Enter via configurable keybinding (`toggle_copy_mode` / `enter_copy_mode` action)
  - Modal state machine: all keyboard input navigates an independent cursor through terminal buffer including scrollback
  - **Navigation**: `h/j/k/l` directional, `0/$` line start/end, `^` first non-blank, arrow keys, Home/End
  - **Word motions**: `w/b/e` word forward/backward/end, `W/B/E` WORD (whitespace-delimited) variants
  - **Page motions**: `Ctrl+U/D` half page, `Ctrl+B/F` full page, `gg` top, `G` bottom, `{count}G` goto line
  - **Count prefix**: `{count}` before any motion (e.g., `5j` moves down 5 lines)
  - **Visual selection**: `v` character, `V` line, `Ctrl+V` block/rectangular modes
  - **Yank**: `y` in visual mode copies selection to clipboard and exits copy mode
  - **Search**: `/pattern` forward, `?pattern` backward, `n/N` repeat search (case-insensitive, wrapping)
  - **Marks**: `m{a-z}` set mark, `'{a-z}` jump to mark
  - **Cursor**: Steady block cursor in copy mode, real terminal cursor hidden
  - **Status bar**: egui overlay at bottom showing mode (COPY/VISUAL/V-LINE/V-BLOCK/SEARCH) and position
  - **Auto-scroll**: Viewport follows cursor when it moves offscreen
  - **Tab switch**: Copy mode exits automatically when switching tabs
  - **Escape**: Exits visual mode first, then copy mode on second press; `q` exits immediately
  - **Settings UI**: Enable/disable copy mode, auto-exit on yank, show/hide status bar (Settings > Input > Copy Mode)
  - **Default keybinding**: `Cmd+Shift+C` (macOS) / `Ctrl+Shift+Space` (Linux/Windows), configurable in Settings > Input > Keybindings
  - **Help panel**: Full copy mode reference added to F1 help

- **Unicode Normalization**: Configurable Unicode normalization form (NFC/NFD/NFKC/NFKD/None) for text processing. NFC is the default. Exposed in Settings > Terminal > Unicode section. Live-updates across all tabs when changed.

- **Snippets & Actions Completion** (#101): Complete remaining snippets and actions features
  - **Custom Variables UI**: Collapsible editor in snippet edit form for managing per-snippet variables (name/value grid with add/delete)
  - **Key Sequence Simulation**: `KeySequence` actions now parse and send terminal byte sequences (Ctrl combos, arrow keys, F-keys, Enter, etc.)
  - **Snippet Import/Export**: Export all snippets to YAML file and import from YAML with duplicate detection and keybinding conflict resolution

### Fixed

- **Emoji rendering**: Fixed color emoji not rendering. Swash render sources were ordered to try `Outline` first, but Apple Color Emoji on macOS has outline data that produced tiny monochrome glyphs instead of color bitmaps. Reversed source order to try `ColorBitmap` > `ColorOutline` > `Outline` so emoji fonts render as colored bitmaps while regular text fonts fall through to outlines. Also fixed TTC face index being discarded when loading fonts from fontdb.

- **Tmux pane resize via mouse drag**: Fixed mouse drag events not being forwarded to the PTY when terminal mouse tracking is enabled (e.g., tmux). The `button_pressed` state was not being set when the click was consumed by mouse tracking, so subsequent motion events were silently dropped in ButtonEvent mode. Clicking to change tmux pane focus worked, but dragging to resize did not.

- **Text baseline alignment**: Fixed subtle per-glyph rounding artifacts that could cause characters on the same line to appear at slightly different vertical positions. The baseline position is now rounded once per row and bearing offsets are applied as exact integers, eliminating scale_y-induced rounding inconsistencies.

- **File/URL link highlighting offset**: Fixed link highlighting and click targets being shifted to the right when multi-byte UTF-8 characters (prompt icons, Unicode text, etc.) appeared earlier in the line. Regex byte offsets are now correctly mapped to terminal column indices.

- **Absolute file path link detection**: Added detection of absolute file paths (e.g., `/Users/name/.config`) in link highlighting regex. Previously only relative (`./`, `../`) and home-relative (`~/`) paths were matched.

---

## [0.12.0] - 2026-02-10

### Added

- **Snippets & Actions System** (#86): Text automation and custom actions (iTerm2 parity)
  - **Text Snippets**: Save frequently-used text blocks for quick insertion
    - Variable substitution with `\(variable)` syntax
    - 10 built-in variables: `date`, `time`, `datetime`, `hostname`, `user`, `path`, `git_branch`, `git_commit`, `uuid`, `random`
    - **Session variables**: Access live terminal state via `\(session.*)` syntax (12 session variables)
    - Custom variables per snippet, keyboard shortcut assignment, folder organization
    - **Auto-execute**: Optional checkbox to send Enter after inserting snippet content
  - **Custom Actions**: User-defined macros triggered via keyboard shortcuts
    - **ShellCommand**: Execute shell commands with notifications and error handling
    - **InsertText**: Insert text with variable substitution
    - **KeySequence**: Placeholder for future keyboard simulation
  - **Settings UI**: Two new tabs — Snippets (📝) and Actions (🚀) — with full CRUD, keybinding recording, and conflict detection
  - **Keybinding Auto-Generation**: Snippets and actions with keybinding field auto-generate keybindings on config load

- **Progress Bar Rendering** (#92): Thin overlay progress bars rendered via egui at the top or bottom of the terminal window
  - Supports OSC 9;4 protocol states (Normal, Warning, Error, Indeterminate)
  - Configurable style (bar or bar-with-text), position, height, opacity, and per-state colors
  - Animated indeterminate bar oscillates smoothly; multiple concurrent bars stack vertically
  - Named concurrent progress bars (OSC 934) fully supported
  - Full settings UI in new "Progress Bar" tab

- **Progress Bar Shader Uniforms**: New `iProgress` vec4 uniform exposes progress bar state to custom GLSL shaders
  - Components: `x` = state, `y` = percent, `z` = isActive, `w` = active bar count
  - Enables shader effects that respond to progress (screen-edge glows, color shifts, particle effects)

- **Paste Delay** (#93): New `paste_delay_ms` config option (0-500ms) adds a configurable delay between pasted lines. Useful for slow terminals or remote connections.

- **Paste Transforms: Newline Control** (#93): Three new paste transformations via Paste Special:
  - `Paste as Single Line`, `Add Newlines`, `Remove Newlines`

- **Current Command in Window Title** (#94): Window title bar shows `[command_name]` when a command is running via shell integration. Reverts when command finishes.

- **Shell Integration Badge Variables** (#94): New `\(session.exit_code)` and `\(session.current_command)` badge variables.

- **Remote Host Integration** (#94): OSC 1337 RemoteHost sequence support. Remote hostname and username synced to badge variables from both OSC 7 and OSC 1337 sequences.

- **Image Scaling Quality** (#90): Configurable texture filtering for inline images — `nearest` (pixel-perfect) or `linear` (smooth) via `image_scaling_mode` config.

- **Image Aspect Ratio Control** (#90): Toggle to preserve or ignore aspect ratio for inline images via `image_preserve_aspect_ratio` config.

- **Prompt on Quit**: Configurable confirmation dialog before closing with active sessions via `prompt_on_quit` config.

- **Pane Title Bars** (#88): GPU-rendered title bars for split panes showing OSC title, CWD path, or fallback pane name
  - Configurable height, position, text color, and background color
  - Enable via `show_pane_titles: true` in config

- **Divider Style Customization** (#88): Four visual styles for pane dividers — Solid, Double, Dashed, Shadow — via `pane_divider_style` config.

### Changed

- **Core Library**: Updated `par-term-emu-core-rust` from 0.33.0 to 0.35.0 (OSC 934 named progress bars, OSC 1337 SetUserVar)
- **Cross-Platform Keybindings Overhaul**: Redesigned default keybindings on Linux/Windows to avoid conflicts with standard terminal control codes. macOS keybindings unchanged. Now follows WezTerm, Kitty, GNOME Terminal, and Windows Terminal conventions.
- **Terminfo**: Modernized `par-term.terminfo` entry with direct-color, bracketed paste, and status line capabilities
- **Dependencies**: Updated `libc`, `zip`, `notify`, `tempfile` to latest versions

### Fixed

- **Dingbat/Symbol Characters Rendering as Colored Emoji**: Fixed by reordering font fallback chain to prefer monochrome symbol fonts before color emoji fonts
- **Snippet/Action Row Overflow**: Buttons anchored to right with auto-truncating content preview
- **Platform-Specific Keybinding Display**: Shows `Cmd` on macOS and `Ctrl` on Linux/Windows
- **Ctrl+C Not Sending SIGINT on Linux/Windows**: Now uses `Ctrl+Shift+C` for copy, allowing bare `Ctrl+C` to pass through
- **Pane Focus Indicator Settings** (#88): Focus indicator and color now read from config correctly
- **Pane Background Opacity** (#88): Slider now wired to rendering pipeline
- **Divider Hover Color** (#88): Hover state now passed to renderer
- **Divider Width/Hit Width Not Updating** (#88): Values now propagate on config change
- **Background Solid Color in Split Panes** (#88): Custom color now used instead of theme color
- **Double Divider Style** (#88): Proper double lines with gap when width >= 4px
- **Shadow Divider Style** (#88): Beveled effect rendered within divider bounds

---

## [0.11.0] - 2026-02-06

### Added

- **Triggers, Trigger Actions & Coprocesses** (#84): Full frontend UI and event wiring for automation features
  - **Regex Triggers**: Define regex patterns that match terminal output and fire actions
    - 7 action types: Highlight, Notify, MarkLine, SetVariable, RunCommand, PlaySound, SendText
    - Config persistence in `config.yaml` via `triggers` array with `TriggerConfig` structs
    - Triggers synced into core `TriggerRegistry` on startup and settings save
    - Regex validation in Settings UI with error feedback
  - **Trigger Highlight Rendering**: Matched text highlighted with configurable fg/bg colors
    - Colors overlaid on terminal cells during rendering
    - Automatic expiry cleanup each frame
  - **Trigger Action Dispatch**: Per-frame polling of core `ActionResult` events
    - `RunCommand`: Spawns detached process with args
    - `PlaySound`: Plays WAV/OGG/FLAC/MP3 from `~/.config/par-term/sounds/` via rodio; `"bell"` or empty plays built-in tone
    - `SendText`: Writes text to PTY with optional delay
    - `Notify`: Desktop notifications via `notify-rust`
    - `MarkLine`: Scrollbar marks with configurable color and label, visible in scrollbar tooltips
    - `SetVariable`: Custom session variables synced to badge overlay (e.g., capture git branch for badge display)
  - **Trigger Marks on Scrollbar**: MarkLine trigger actions create color-coded marks on the scrollbar
    - Marks include label text shown in scrollbar tooltips
    - Rebuild strategy eliminates duplicate marks when triggers fire multiple times per frame
    - Historical marks preserved in scrollback; visible-grid marks rebuilt from fresh scan results
    - Marks cleared automatically when scrollback is cleared
  - **Coprocesses**: Background processes that receive terminal output
    - Per-tab `CoprocessManager` with auto-start support for configured coprocesses
    - Config persistence via `coprocesses` array with `CoprocessDefConfig` structs
    - Settings UI with name, command, args, auto_start, and copy_terminal_output controls
    - **Restart policy**: Configurable restart behavior (Never, Always, OnFailure) with optional delay
    - **Output viewer**: Collapsible per-coprocess output display in Automation settings tab
    - **Start/Stop controls**: Start and stop coprocesses directly from Settings UI
    - **Error display**: Failed coprocess starts show error messages inline in the UI
  - **Automation Settings Tab**: New "Automation" tab (⚡) in Settings UI
    - Collapsible sections for Triggers and Coprocesses
    - Inline add/edit forms with type-specific action editors (color pickers, sliders, text inputs)
    - Enable/disable toggles, delete with confirmation
    - Searchable via sidebar keywords (trigger, regex, automation, coprocess, action, pattern, etc.)

- **Minimum Contrast Enforcement**: WCAG-based accessibility feature for readable text
  - Automatically adjusts text color when contrast ratio against background is too low
  - Uses WCAG luminance formula for perceptually accurate contrast calculation
  - New config option: `minimum_contrast` (range 1.0-21.0, default: 1.0 = disabled)
  - Set to 4.5 for WCAG AA compliance, 7.0 for WCAG AAA compliance
  - Settings UI: Appearance → Colors section with slider

- **Semantic History**: Click file paths to open them in your editor (iTerm2 parity)
  - Detects file paths in terminal output with optional line:column numbers
  - Supports formats: `/path/file.rs`, `src/main.rs:42`, `file.py:10:5`
  - Ctrl+click (Cmd+click on macOS) opens file in configured editor
  - Directories open in system file manager (Finder/Explorer/Nautilus)
  - Configurable editor selection mode:
    - **Custom** - Use a user-specified editor command
    - **Environment Variable** - Use `$EDITOR`/`$VISUAL` (default)
    - **System Default** - Open with system's default application
  - Falls back to system default if configured editor unavailable
  - Uses login shell (`$SHELL -lc`) to run editor commands, ensuring user's PATH is available
  - New config options:
    - `semantic_history_enabled` - Enable/disable feature (default: true)
    - `semantic_history_editor_mode` - Editor selection mode: `custom`, `environment_variable`, `system_default`
    - `semantic_history_editor` - Custom editor command with placeholders: `{file}`, `{line}`, `{col}`
  - Example editor commands: `code -g {file}:{line}`, `vim +{line} {file}`
  - Settings UI: Terminal → Semantic History section with editor mode dropdown

- **Configurable Log Level**: Runtime log level control for the `log` crate bridge
  - New config option: `log_level` (off/error/warn/info/debug/trace, default: off)
  - CLI flag: `--log-level <LEVEL>` overrides config setting
  - Settings UI: Advanced → Debug Logging section with dropdown and "Open Log File" button
  - Log output routed to `/tmp/par_term_debug.log`

### Changed

- **Unified Logging**: Replaced `env_logger` with custom `log::Log` bridge that routes all `log::info!()`, `log::error!()` etc. to `/tmp/par_term_debug.log`. Ensures logs are always captured in macOS app bundles and Windows GUI apps where stderr is invisible. When `RUST_LOG` is set, also mirrors to stderr for terminal debugging.

- **Coprocess PATH Resolution**: Coprocesses now inherit the user's login shell PATH, fixing "command not found" errors when running from macOS app bundles with minimal PATH environments.

### Fixed

- **Trigger MarkLine deduplication**: Fixed duplicate scrollbar marks when triggers fire multiple times per frame due to PTY read batching. Uses a rebuild strategy that preserves historical marks in scrollback while deduplicating visible-grid marks.

- **Scrollbar command text capture**: Mark tooltips now correctly show command text instead of output lines, reading from scrollback metadata when the command mark scrolls off the visible grid.

- **Trigger marks cleared on scrollback clear**: Trigger marks are now properly removed when the scrollback buffer is cleared (e.g., via `clear` command or Cmd+K).

- **Settings Quick Search**: Expanded search keywords to cover all settings options, making it easier to find specific settings like "minimum contrast", "semantic history", "anti-idle", and many more

- **Core Library API Integration**: Enabled previously dormant APIs from par-term-emu-core-rust
  - Recording API now accessible for future instant replay features
  - Shell integration stats API now accessible for badge variables and status bar

---

## [0.10.0] - 2026-02-04

### Added

- **Close Confirmation for Running Jobs**: Confirmation dialog when closing tabs/panes with active processes
  - Detects running commands via shell integration markers
  - Shows dialog with command name and options to "Close Anyway" or "Cancel"
  - New config option: `confirm_close_running_jobs` (default: false)
  - New config option: `jobs_to_ignore` - list of process names to skip (default: common shells like bash/zsh/fish, utilities like less/more/man)
  - Settings UI: Terminal → Behavior section with checkbox and editable ignore list
  - Escape key dismisses the confirmation dialog

- **Shell Exit Action**: Configurable behavior when shell process exits
  - New config option: `shell_exit_action` replaces boolean `exit_on_shell_exit`
  - Five exit action options:
    - `close` - Close the tab/pane (default, matches old `exit_on_shell_exit: true`)
    - `keep` - Keep pane open showing terminated shell (matches old `exit_on_shell_exit: false`)
    - `restart_immediately` - Spawn new shell instantly when previous exits
    - `restart_with_prompt` - Show "[Process exited. Press Enter to restart...]" message and wait
    - `restart_after_delay` - Restart shell after 1 second delay
  - Settings UI: Terminal → Behavior section with dropdown selector
  - Backward compatible: old boolean config values auto-migrate (`true`→Close, `false`→Keep)

- **Modifier Key Remapping**: Remap modifier keys to different functions
  - Configure left/right Ctrl, Alt, and Super keys independently
  - Remap options: None (disabled), Ctrl, Alt, Shift, Super
  - New config option: `modifier_remapping` with `left_ctrl`, `right_ctrl`, `left_alt`, `right_alt`, `left_super`, `right_super`
  - Settings UI: Input → Modifier Remapping section
  - Use cases: swap Ctrl and Caps Lock, use Ctrl as Cmd on macOS

- **Language-Agnostic Key Bindings**: Keybindings that work consistently across keyboard layouts
  - New config option: `use_physical_keys` (default: false)
  - When enabled, keybindings match by physical key position (scan code) instead of character produced
  - Makes shortcuts like Ctrl+Z work the same on QWERTY, AZERTY, Dvorak, etc.
  - Physical key syntax for keybindings: `[KeyCode]` (e.g., `Ctrl+[KeyZ]`)
  - Settings UI: Input → Keyboard section checkbox
  - Supports all letter keys, digits, punctuation, and function keys

- **modifyOtherKeys Protocol Support**: XTerm extension for enhanced keyboard input reporting
  - Applications can enable via `CSI > 4 ; mode m` escape sequence (mode 0=off, 1=special keys, 2=all keys)
  - When enabled, keys with modifiers are reported as `CSI 27 ; modifier ; keycode ~`
  - Allows applications to distinguish between e.g., Ctrl+i and Tab
  - Query support via `CSI ? 4 m` returns current mode
  - Requires par-term-emu-core-rust v0.30.0+

- **Tmux Profile Auto-Switching**: Automatically apply profiles when connecting to tmux sessions
  - New profile field `tmux_session_patterns` - glob patterns to match session names (e.g., `work-*`, `*-production`)
  - Case-insensitive pattern matching with wildcards: prefix (`dev-*`), suffix (`*-prod`), contains (`*server*`), exact match
  - Profile's `badge_text` is applied to the gateway tab when matched
  - Fixed profile option: `tmux_profile` config to always use a specific profile for all tmux connections
  - Settings UI: "Auto-Switch Tmux" field in profile editor with pattern hints
  - Profile cleared automatically when tmux session ends

- **GPU Power Preference**: Control which GPU is used for rendering on multi-GPU systems
  - New config option: `power_preference` with three modes:
    - `none` - Let the system decide (default)
    - `low_power` - Prefer integrated GPU (Intel/AMD iGPU) for battery savings
    - `high_performance` - Prefer discrete GPU (NVIDIA/AMD) for maximum performance
  - Settings UI: Window → Performance section with dropdown selector
  - Note: Requires app restart to take effect (GPU adapter selected at startup)

- **Reduce Flicker**: iTerm2-style flicker reduction for smoother terminal updates
  - Delays screen redraws while cursor is hidden (DECTCEM off)
  - Many terminal programs hide cursor during bulk updates (scrolling, screen redraws)
  - Batches rapid updates to reduce visual flicker and tearing
  - New config options:
    - `reduce_flicker` - Enable/disable feature (default: true)
    - `reduce_flicker_delay_ms` - Maximum delay before forced render (1-100ms, default: 16ms)
  - Settings UI: Window → Performance → Flicker Reduction section
  - Automatically bypasses delay for UI interactions (help, search, dialogs)
  - Respects `lock_cursor_visibility` config option

- **Maximize Throughput Mode**: Manual toggle for prioritizing bulk output processing over immediate responsiveness
  - Batches screen updates during bulk terminal output (e.g., `cat /usr/share/dict/words`)
  - Reduces CPU overhead when processing large outputs by throttling render rate
  - Toggle with `Cmd+Shift+T` (macOS) or `Ctrl+Shift+T` (other platforms)
  - Toast notification confirms mode toggle
  - New config options:
    - `maximize_throughput` - Enable/disable feature (default: false)
    - `throughput_render_interval_ms` - Render interval in throughput mode (50-500ms, default: 100ms)
  - Settings UI: Window → Performance → Throughput Mode section
  - Unlike reduce_flicker, this mode always batches regardless of cursor visibility

- **Per-Profile Badge Configuration**: Full badge customization per profile (iTerm2 parity)
  - Profiles can now override all badge settings, not just the text format
  - New optional profile fields:
    - `badge_color` - RGB color override
    - `badge_color_alpha` - Opacity override (0.0-1.0)
    - `badge_font` - Font family override
    - `badge_font_bold` - Bold toggle override
    - `badge_top_margin` / `badge_right_margin` - Position overrides
    - `badge_max_width` / `badge_max_height` - Size constraint overrides
  - Profile Modal UI: New collapsible "Badge Appearance" section with:
    - Color picker with checkbox to enable override
    - Opacity slider
    - Font input and bold checkbox
    - Margin and size controls
  - Each setting can be individually overridden or use global defaults
  - Supports profile inheritance - child profiles inherit badge settings from parent
  - Use case: Different badge appearances per environment (e.g., red "PROD", green "DEV")

### Fixed

- **Arrow Keys in `less` and Other Pagers**: Fixed arrow keys not working in programs that enable application cursor key mode (DECCKM)
  - Programs like `less`, `more`, and some TUI apps enable DECCKM mode which expects SS3 sequences (`ESC O A`) instead of CSI sequences (`ESC [ A`)
  - Arrow keys now correctly send `ESC O A/B/C/D` when application cursor mode is enabled
  - Added `application_cursor()` method to `TerminalManager` to expose DECCKM state from core library

---

## [0.9.0] - 2026-02-04

### Added

- **Welcome Dialog Changelog Link**: Added "View Changelog" hyperlink to the welcome/onboarding popup
  - Links to GitHub CHANGELOG.md for easy access to release notes
  - Appears below the "A GPU-accelerated terminal emulator" subtitle

- **Settings UI: Profiles Tab**: New "Profiles" tab in Settings window for profile management
  - "Open Profile Manager" button to launch the profile modal from settings
  - Profile drawer button visibility toggle (`show_profile_drawer_button` config option)
  - Overview of profile features and display options

- **Configurable tmux Status Bar Format** (#67): Customize tmux status bar content via format strings
  - `tmux_status_bar_left`: Format string for left side (default: `[{session}] {windows}`)
  - `tmux_status_bar_right`: Format string for right side (default: `{pane} | {time:%H:%M}`)
  - Supported variables: `{session}`, `{windows}`, `{pane}`, `{time:FORMAT}`, `{hostname}`, `{user}`
  - Settings UI controls in Advanced > tmux Integration section
  - Native tmux format query commands following iTerm2's approach (deferred full async implementation)

### Changed

- **Default Font Size**: Reduced default `font_size` from 13.0 to 10.0
- **Default Shader Brightness**: Reduced default `custom_shader_brightness` from 100% to 15% for better text readability out of the box
- **Dev Tools Build**: Test utilities (`test-cr`, `test-grid`) now require `--features dev-tools` flag to build/install
- **Default Profile Drawer Button**: Profile drawer toggle button now hidden by default (`show_profile_drawer_button: false`)
- **Settings UI Sections**: "Background & Effects" and "Cursor Shader" sections now expand by default for easier discovery

### Fixed

- **Segfault on Exit**: Fixed crash when closing terminal window while settings window is open
  - Settings window is now explicitly closed before app teardown
  - egui state is cleaned up before other window resources
  - Audio stream cleanup order fixed to prevent use-after-free

- **Settings Window Size Display**: Fixed Settings UI not updating current cols/rows when the terminal window is resized (Windows)
  - The "Current: NxM" display in Window → Display now updates in real-time during resize

- **Windows ARM64 Build**: Fixed build failure on Windows ARM64 due to `ring` crate requiring clang
  - Switched `ureq` HTTP client from `rustls` to `native-tls` backend
  - Uses system TLS (Schannel on Windows, OpenSSL on Linux, Security.framework on macOS)
  - No longer requires clang/LLVM toolchain for Windows builds

- **VM GPU Compatibility**: Fixed app failing to start in virtual machine environments (Parallels, etc.)
  - Windows: Now uses DirectX 12 backend instead of Vulkan (which fails in Parallels VMs)
  - Linux: Added OpenGL fallback when Vulkan is unavailable or non-compliant
  - Resolves "Adapter is not Vulkan compliant" errors in VM environments

- **HTTPS Request Panic**: Fixed panic when making HTTPS requests (update checker, shader installer)
  - Explicitly configure `ureq` to use native-tls provider instead of defaulting to rustls
  - Added `http.rs` module with properly configured HTTP agent

- **Font Size Change Crash**: Fixed crash when changing font size in Settings
  - wgpu only allows one surface per window; old renderer must be dropped before creating new one
  - Now properly releases old surface before creating new renderer

- **Windows File Watching**: Fixed shader hot reload not working on Windows
  - The `notify` crate was configured with macOS-only backend, leaving Windows without file watching support
  - Now uses platform-specific configuration: kqueue on macOS, ReadDirectoryChangesW on Windows, inotify on Linux

- **Windows Taskbar Icon Loss** (#79): Fixed icon reverting to generic Windows icon when pinning app to taskbar
  - Added Windows resource embedding via `winres` build dependency
  - Created `assets/par-term.ico` with multiple icon sizes (16, 32, 48, 64, 128, 256)
  - Icon is now properly embedded in the Windows executable

### Added

- **Windows Install Script**: Added `scripts/install-windows.bat` for building on Windows
  - Sets up Visual Studio environment before cargo install
  - Required for native dependencies that need MSVC toolchain

---

## [0.8.0] - 2026-02-03

### Added

- **Configurable Startup Directory** (#74): Control where new terminal sessions start
  - **Three modes**: `home` (default), `previous` (remember last session), `custom` (user-specified path)
  - **Session persistence**: Previous session mode saves working directory on close and restores on next launch
  - **Graceful fallback**: If saved/custom directory doesn't exist, falls back to home directory
  - **Settings UI**: New "Startup Directory" section in Terminal → Shell settings tab with mode dropdown and path picker
  - **Legacy compatibility**: Existing `working_directory` config still works and takes precedence if set
  - Config options: `startup_directory_mode`, `startup_directory`, `last_working_directory`

- **Badge System** (#73): iTerm2-style semi-transparent text overlays in the terminal corner
  - **Badge text overlay**: Displays dynamic session information in top-right corner
  - **Dynamic variables**: 12 built-in variables using `\(session.*)` syntax
    - `session.hostname`, `session.username`, `session.path` - Basic session info
    - `session.job`, `session.last_command` - Command tracking
    - `session.profile_name`, `session.tty` - Profile and TTY info
    - `session.columns`, `session.rows` - Terminal dimensions
    - `session.bell_count`, `session.selection`, `session.tmux_pane_title` - Advanced
  - **Configurable appearance**: RGBA color, opacity, font family, bold toggle
  - **Configurable position**: Top/right margins, max width/height as fraction of terminal
  - **OSC 1337 support**: Base64-encoded `SetBadgeFormat` escape sequence with security checks
  - **Settings UI**: Full badge configuration tab with General, Appearance, Position, and Variables sections
  - Config options: `badge_enabled`, `badge_format`, `badge_color`, `badge_color_alpha`, `badge_font`, `badge_font_bold`, `badge_top_margin`, `badge_right_margin`, `badge_max_width`, `badge_max_height`

- **Scrollbar Mark Tooltips** (#69): Hover over scrollbar command markers to see command details
  - **Command info**: Shows command text (truncated if long), execution time, duration, and exit code
  - **Optional feature**: Disabled by default, enable via Settings → Terminal → Scrollbar → "Show tooltips on hover"
  - Config option: `scrollbar_mark_tooltips`

- **Tab Bar Stretch & HTML Titles**: Tabs can now stretch to fill the bar by default (`tab_stretch_to_fill`), and tab titles support limited HTML markup (`<b>`, `<i>`, `<u>`, `<span style="color:...">`) via `tab_html_titles`.
- **Native Paste/Copy Keys**: Recognize `NamedKey::Paste`/`NamedKey::Copy` plus Cmd/Ctrl+V/C across platforms, covering keyboards that emit dedicated paste/copy keys.
- **Settings Reset to Defaults**: Settings UI now includes a "Reset to Defaults" button with a confirmation dialog. It rebuilds the config from defaults, resyncs all staged temp values, clears searches, and marks changes for save so users can restore a clean baseline in one click.
- **Scrollbar Command Markers Toggle**: Added a Settings → Terminal → Scrollbar option (`scrollbar_command_marks`, default on) to show/hide command status markers in the scrollbar.

### Changed

- **Core Library Update**: Updated to `par-term-emu-core-rust` v0.28.0 (published crates.io version)
- **Tab Stretch Default**: `tab_stretch_to_fill` now defaults to true so tabs auto-distribute available width while respecting `tab_min_width`.
- **Shader Install Overwrite Prompt**: Onboarding integrations now detect user-modified bundled shaders and prompt to overwrite, skip modified files, or cancel before installing the latest shader pack. Installation uses manifest-aware logic that preserves user edits by default.
- **Settings Reinstall Prompt Parity**: The Settings > Integrations > Custom Shaders reinstall button now shows the same overwrite/skip prompt when bundled shaders were modified, and surfaces progress/status inline.

### Fixed

- **Windows Console Window on Launch**: Fixed extra console window appearing when launching par-term on Windows. Added `windows_subsystem = "windows"` attribute to hide the console window in release builds.
- **Config Refresh for New Windows**: Creating a new window now reloads config from disk first, so changes made in other windows (like integration install states written during onboarding) apply immediately and avoid stale prompts.
- **Scrollbar Command Mark Colors**: Command markers now retain exit codes even when shells emit OSC exit codes without CommandFinished history entries, ensuring success/failure colors render reliably.
- **Bash Shell Integration Exit Codes**: Bash integration now emits numeric exit codes in OSC 133;D (no literal `$?`), restoring correct marker coloring.

---

## [0.7.0] - 2026-02-02

### Fixed

- **tmux Pane Display on Initial Connect**: Fixed tmux panes not rendering when attaching to existing sessions. The `close_exited_panes` logic was incorrectly closing tmux display panes (which don't have local shells) immediately after creation. Now skips shell exit checks for tabs displaying tmux content.
- **tmux Tabs Not Closing on Session End**: Fixed tmux display tabs remaining open after the tmux session ends. Now properly closes all tabs that were displaying tmux window content when the session terminates, and clears pane mappings.
- **Shift+Enter Key Behavior**: Shift+Enter now sends LF (`\n`) instead of CR (`\r`), matching iTerm2 behavior. This enables soft line breaks in applications like Claude Code that distinguish between Enter (submit) and Shift+Enter (new line).
- **Multi-Window Focus Routing**: Menu actions (Cmd+T, Cmd+V, etc.) now correctly route to the focused window instead of an arbitrary window when multiple windows are open
- **Settings UI Layout**: Content area now properly fills available window space instead of leaving empty space at the bottom
- **Settings UI Control Widths**: Applied consistent width constants to sliders and text inputs across all settings tabs
- **Tab Bar Content Overlap**: Fixed issue where shell content's first line was hidden behind the tab bar when tabs were enabled. Content offset and terminal dimensions are now updated immediately when creating or closing tabs that change tab bar visibility (e.g., going from 1→2 tabs with `when_multiple` mode). Also fixed incorrect pixel dimensions being passed to PTY when syncing tab bar height.
- **tmux Path Detection**: tmux path is now resolved at runtime (not just at config load). Searches PATH and common installation locations (`/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin`) when the configured path is `tmux`. This fixes tmux integration for users with existing configs and when par-term is launched from macOS Finder where PATH may be incomplete.

### Added

- **Integrations Install System**: Unified installation for optional par-term enhancements
  - **Shell Integration**: Scripts for bash/zsh/fish enabling prompt navigation, CWD tracking, and command status
    - Install via CLI: `par-term install-shell-integration`
    - Install via curl: `curl -fsSL https://paulrobello.github.io/par-term/install-shell-integration.sh | bash`
    - Uninstall via CLI: `par-term uninstall-shell-integration`
  - **Shader Bundle with Manifest**: Tracks bundled files vs user-created files using SHA256 hashes
    - Safe uninstall preserves user modifications
    - Manifest-aware reinstall detects and warns about modified files
    - Uninstall via CLI: `par-term uninstall-shaders`
  - **Welcome Dialog**: First-run prompt offering to install both integrations
    - Version-aware prompting (only asks once per version)
    - Checkbox selection for shaders and/or shell integration
    - Skip and "Never Ask" options
  - **Settings UI Tab**: New "Integrations" tab (🔌) for managing installations
    - Install/Reinstall/Uninstall buttons for each integration
    - Status indicators showing installed version
    - Copy-able curl commands for manual installation
  - **GitHub Pages Hosting**: Curl-installable scripts at paulrobello.github.io/par-term/

- **Settings UI Completeness**: Added missing UI controls for config options that were previously only configurable via config.yaml
  - **Tab Bar Tab**: tab_bar_mode (always/when_multiple/never), tab_bar_height, tab_show_index, tab_inherit_cwd, max_tabs
  - **Window Tab**: allow_title_change checkbox to control whether apps can change window title via OSC sequences
  - **Cursor Tab**: cursor_shadow_blur slider for shadow blur radius
  - **Cursor Shader Section**: cursor_shader_color picker, cursor_shader_trail_duration, cursor_shader_glow_radius, cursor_shader_glow_intensity sliders
  - **Background Tab**: shader_hot_reload_delay slider (shown when hot reload is enabled)
- **Edit Config File Button**: New button in settings footer to open config.yaml in system's default text editor

- **tmux Status Bar**: Native status bar display when connected to tmux sessions
  - Shows session name, window list with active marker, focused pane ID, and time
  - Renders at bottom of terminal using egui (outside terminal content area)
  - Configurable refresh interval via `tmux_status_bar_refresh_ms` (default: 1000ms)
  - Toggle via `tmux_show_status_bar` config option and Settings UI
  - See [#67](https://github.com/paulrobello/par-term/issues/67) for planned enhancements (configurable content, tmux format strings)

- **Auto-Close Exited Panes**: Panes automatically close when their shell process exits
  - Works with split panes - each pane closes independently when its shell exits
  - Tab closes when all panes have exited (respects `exit_on_shell_exit` config)
  - Properly handles tmux panes (which don't have local shells)

- **tmux Control Mode Enhancements**: Improved multi-client support and bidirectional sync (#62)
  - **Bidirectional pane resize**: Resizing panes in par-term now updates external tmux clients
  - **Multi-client size sync**: Sets `window-size smallest` on connect so tmux respects par-term's size
  - **Focus-aware size assertion**: Re-asserts client size when par-term window gains focus
  - Horizontal divider drags sync height, vertical divider drags sync width (no cascade issues)

- **Session Logging and Recording**: Automatic session logging to record terminal output (#60)
  - **Automatic logging**: Enable via `auto_log_sessions` config option
  - **Multiple log formats**:
    - Plain text: Simple output without escape sequences (smallest files)
    - HTML: Rendered output with colors preserved (viewable in browser)
    - Asciicast: asciinema-compatible format for replay and sharing
  - **Configurable log directory**: XDG-compliant default (`~/.local/share/par-term/logs/`)
  - **Archive on close**: Ensure session is fully written when tab closes
  - **Hotkey toggle**: `Cmd/Ctrl+Shift+R` to start/stop session recording on demand
  - **Visual feedback**: Toast notification when recording starts/stops
  - **CLI option**: `--log-session` flag to enable logging at startup
  - Config options: `auto_log_sessions`, `session_log_format`, `session_log_directory`, `archive_on_close`
  - Settings UI section under "Session Logging" with format selector, directory picker, and log count display

- **Profile System**: iTerm2-style profiles for saved terminal configurations (#65)
  - **Profile Manager**: Create, edit, delete, and reorder named profiles
  - **Profile Drawer**: Collapsible right-side panel for quick profile access
    - Toggle button at window edge
    - Single-click to select, double-click to open
    - "Manage" button opens full management modal
  - **Profile Modal**: Full CRUD interface for profile management
    - Create new profiles with "+ New Profile" button
    - Edit existing profiles (double-click or edit button)
    - Delete profiles with confirmation dialog
    - Reorder profiles with up/down buttons
  - **Profile Settings**:
    - Name and emoji icon for visual identification
    - Working directory with browse button
    - Custom command with arguments
    - Custom tab name override
  - **Persistence**: Profiles saved to `~/.config/par-term/profiles.yaml`
  - **Integration**: Opening a profile creates a new tab with the configured settings

- **Window Management Features**: Implement missing window management features from iTerm2 (#56)
  - **Window Type**: Start in different window modes (`window_type` config option)
    - Normal: Standard window (default)
    - Fullscreen: Start in fullscreen mode
    - Edge-anchored: Position window at screen edges (top/bottom/left/right) for dropdown-style terminals
  - **Target Monitor**: Open window on specific monitor (`target_monitor` config option)
    - Set monitor index (0 = primary) for multi-monitor setups
    - Auto-centers window on target monitor, or edges to that monitor for edge-anchored modes
  - **Lock Window Size**: Prevent window resize (`lock_window_size` config option)
    - Disables window resizing when enabled
  - **Window Number Display**: Show window index in title bar (`show_window_number` config option)
    - Displays "[N]" suffix in window title when multiple windows open
    - Useful for keyboard navigation between windows
  - **Maximize Vertically**: Stretch window to full screen height (Shift+F11 or View menu)
    - Maintains current width and X position while spanning full monitor height
  - Settings UI controls for all options in Window & Display section

- **Unicode Width Configuration**: Configurable Unicode version and ambiguous width settings (#46)
  - **Unicode Version**: Select from Unicode 9.0 through 16.0, or Auto (latest)
    - Different versions have different character width tables, especially for emoji
    - Use older versions for compatibility with legacy systems
  - **Ambiguous Width**: Treatment of East Asian Ambiguous characters
    - Narrow (1 cell): Western default
    - Wide (2 cells): CJK default for Chinese/Japanese/Korean environments
  - Config options: `unicode_version`, `ambiguous_width`
  - Settings UI dropdowns in Terminal tab
  - Ensures proper cursor positioning and text alignment across different contexts

- **Paste Special** (`Cmd/Ctrl+Shift+V`): Transform clipboard content before pasting (#41)
  - Command palette UI with fuzzy search filtering
  - 26 text transformations across 4 categories:
    - **Shell Escaping**: Single quotes, double quotes, backslash escaping
    - **Case Conversion**: UPPERCASE, lowercase, Title Case, camelCase, PascalCase, snake_case, SCREAMING_SNAKE, kebab-case
    - **Whitespace**: Trim, trim lines, collapse spaces, tabs↔spaces, remove empty lines, normalize line endings
    - **Encoding**: Base64 encode/decode, URL encode/decode, hex encode/decode, JSON escape/unescape
  - Live preview showing original and transformed content
  - Keyboard navigation (↑↓ to navigate, Enter to apply, Escape to cancel)
  - Double-click to apply transformation
  - Integration with clipboard history: `Shift+Enter` in clipboard history opens paste special
  - Configurable keybinding via Settings UI

- **Session Ended Notification**: Desktop notification when a shell process exits (#54)
  - Useful for long-running commands where users switch to other applications
  - Per-tab tracking ensures notification fires only once per session
  - Config option: `notification_session_ended` (default: false)
  - Settings UI checkbox in Bell & Notifications section

- **Suppress Notifications When Focused**: Smart notification filtering (#54)
  - Skip desktop notifications when the terminal window is already focused
  - Visual and audio bells are unaffected (user can see/hear them)
  - Reduces notification noise when actively using the terminal
  - Config option: `suppress_notifications_when_focused` (default: true)
  - Settings UI checkbox in Bell & Notifications section

- **Advanced Mouse Features**: Implement mouse/pointer features from iTerm2 (#43)
  - **Platform-appropriate URL modifier**: Cmd+click on macOS, Ctrl+click on Windows/Linux to open URLs
  - **Option+Click moves cursor**: Position cursor at clicked location using arrow key sequences
    - Config option: `option_click_moves_cursor` (default: true)
    - Only works at bottom of scrollback (not scrolled back)
    - Disabled on alternate screen (TUI apps handle their own cursor)
    - Uses shell's cursor position to calculate movement delta
  - **Focus follows mouse**: Auto-focus window when cursor enters
    - Config option: `focus_follows_mouse` (default: false, opt-in)
  - **Horizontal scroll reporting**: Report horizontal scroll to apps with mouse tracking
    - Uses button codes 66 (left) and 67 (right)
    - Config option: `report_horizontal_scroll` (default: true)
  - **Rectangular selection**: Now uses Option+Cmd (matching iTerm2), freeing Option alone for cursor positioning
  - Settings UI controls in Mouse Behavior section

- **Auto-Quote Dropped Files**: Automatically quote file paths when dragging and dropping files into the terminal (#39)
  - Handles spaces and special shell characters safely
  - Configurable quote styles: single quotes (default), double quotes, backslash escaping, or none
  - Config option: `dropped_file_quote_style`
  - Settings UI in Mouse tab under Selection & Clipboard

- **Anti-Idle Keep-Alive**: Prevent SSH and connection timeouts by periodically sending invisible characters (#47)
  - Configurable idle threshold (10-3600 seconds, default: 60)
  - Configurable keep-alive character (NUL, ESC, ENQ, Space, or custom ASCII code)
  - Tracks both keyboard input and terminal output as activity
  - Per-tab activity tracking with automatic keep-alive on idle
  - Config options: `anti_idle_enabled`, `anti_idle_seconds`, `anti_idle_code`
  - Settings UI in Shell tab with presets dropdown and custom code input

- **Initial Startup Text**: Auto-send configurable text/commands when a session starts (#48)
  - Config options: `initial_text`, `initial_text_delay_ms`, `initial_text_send_newline`
  - Escape support: `\n`, `\r`, `\t`, `\xHH`, `\e`; normalizes to CR for Enter behavior
  - Optional delay before send to let the shell initialize; optional auto-newline to execute
  - Settings UI: Shell tab provides multi-line input, delay, and newline toggle

- **Answerback String**: Configurable answerback string for terminal identification (#45)
  - Responds to ENQ (0x05) control character with user-defined string
  - Used for legacy terminal identification in multi-terminal environments
  - Default: empty (disabled) for security
  - Config option: `answerback_string`
  - Settings UI in Shell tab with security warning

- **Smart Selection & Word Boundaries**: Enhanced double-click text selection with configurable patterns (#42)
  - **Word boundary characters**: Configurable characters considered part of a word
    - Default: `/-+\~_.` (iTerm2 compatible)
    - Config option: `word_characters`
  - **Smart selection rules**: Regex-based patterns with precision levels for intelligent selection
    - 11 default patterns: HTTP URLs, SSH/Git/File URLs, file paths, email addresses, IPv4 addresses, Java/Python imports, C++ namespaces, quoted strings, UUIDs
    - 5 precision levels: VeryHigh, High, Normal, Low, VeryLow (higher precision patterns match first)
    - Enable/disable individual rules or smart selection entirely
    - Config options: `smart_selection_enabled`, `smart_selection_rules`
  - **Settings UI** in Mouse tab:
    - Word characters text field
    - Smart selection toggle
    - List of rules with enable/disable checkboxes (hover for regex/precision details)
    - "Reset rules to defaults" button
  - Cached regex compilation for optimal performance

- **Terminal Search** (Cmd/Ctrl+F): Search through scrollback buffer with match highlighting (#24)
  - egui-based search bar overlay with real-time incremental search
  - Match highlighting with configurable colors for regular and current match
  - Navigation between matches with Enter/Shift+Enter or Cmd/Ctrl+G
  - Search options: case sensitive (Aa), regex mode (.*), whole word (\b)
  - Automatic scroll to current match with match counter display
  - Debounced search (150ms) for responsive typing
  - Proper Unicode support: correctly handles multi-byte characters (emoji, CJK)
  - New config options: `search_highlight_color`, `search_current_highlight_color`
  - Keyboard shortcuts: Cmd/Ctrl+F (open), Escape (close), Enter (next), Shift+Enter (prev)

- **Automatic Update Checking**: Configurable update check frequency with desktop notifications (#34)
  - Check for new par-term releases from GitHub automatically
  - Four frequency options: Never, Daily, Weekly (default), Monthly
  - Desktop notification when updates are available (one notification per version)
  - Platform-specific instructions: macOS users see Homebrew update command
  - "Skip This Version" option to suppress notifications for specific releases
  - New config options: `update_check_frequency`, `last_update_check`, `skipped_version`
  - Settings UI section under "Updates" with version info and "Check Now" button
  - Checks run on startup (5 second delay) and periodically while running

- **Shader Install Prompt on First Startup**: Automatic detection and install offer when shaders folder is missing (#33)
  - Modal dialog appears on first launch if `~/.config/par-term/shaders/` is empty or missing
  - Three options: "Yes, Install" (download shaders), "Never" (save preference), "Later" (dismiss for session)
  - Downloads shader pack from GitHub releases automatically with progress spinner
  - Installation runs in background thread for responsive UI during download
  - New config option: `shader_install_prompt` (ask/never/installed)
  - Escape key closes dialog (when not installing)
  - Can still install manually via `par-term install-shaders` CLI command

- **Font Rendering Options**: Anti-aliasing and thin strokes controls for improved text appearance (#32)
  - **Anti-aliasing**: Toggle font smoothing on/off for crisp or smooth text
    - Disable for sharp, pixelated text at small sizes
    - Config option: `font_antialias` (default: true)
  - **Hinting**: Control font hinting for pixel-aligned glyphs
    - Improves text clarity at small sizes by aligning to pixel boundaries
    - Config option: `font_hinting` (default: true)
  - **Thin Strokes Mode**: iTerm2-inspired font smoothing modes for HiDPI displays
    - `never`: Standard stroke weight everywhere
    - `retina_only`: Lighter strokes on HiDPI displays (default)
    - `dark_backgrounds_only`: Lighter strokes on dark backgrounds
    - `retina_dark_backgrounds_only`: Lighter strokes only on HiDPI + dark backgrounds
    - `always`: Always use lighter strokes
    - Config option: `font_thin_strokes`
  - All options accessible in Settings > Font > Rendering Options
  - Changes take effect immediately by clearing and re-rasterizing the glyph cache

- **Activity and Idle Notifications**: Desktop notifications for terminal activity (#29)
  - **Activity notification**: Triggers when terminal output resumes after inactivity
    - Useful for alerting when long-running commands complete
    - Configurable threshold (default 10 seconds of inactivity)
    - Enable via `notification_activity_enabled` config option
  - **Silence/Idle notification**: Triggers when terminal has been idle too long
    - Useful for detecting stalled processes or completed commands
    - Configurable threshold (default 300 seconds / 5 minutes)
    - Enable via `notification_silence_enabled` config option
  - Both accessible in Settings UI under "Bell & Notifications"

- **Option Key as Meta/Esc Configuration**: Essential feature for emacs/vim users (#23)
  - Configure left and right Option/Alt key behavior independently
  - Three modes: Normal (special characters), Meta (high bit), Esc (ESC prefix)
  - Default mode is "Esc" for best terminal compatibility (M-x, M-f, M-b, etc.)
  - New "Keyboard Input" section in Settings UI
  - Config options: `left_option_key_mode` and `right_option_key_mode`

- **Cursor Text Color**: Configurable text color under block cursor (#25)
  - New `cursor_text_color` option to customize text visibility under block cursor
  - Optional: When not set, uses automatic contrast calculation (dark text on bright cursor, bright text on dark cursor)
  - Settings UI with checkbox toggle and color picker in Cursor tab
  - Only affects block cursor style (beam and underline don't obscure text)

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

### Changed

- **Core Library Update**: Updated to `par-term-emu-core-rust` v0.26.0 (includes recording type re-exports for session logging)

### Fixed

- **tmux Control Mode Client Size**: Fixed `refresh-client -C` command format (was using comma separator instead of `x`)
  - Command now correctly sends `refresh-client -C 80x24` instead of `refresh-client -C 80,24`
  - Enables proper multi-client sizing where tmux respects par-term's dimensions
- **Default Keybindings Not Available for Existing Users**: New default keybindings are now automatically merged into existing user configs
  - When loading config, any new default keybindings whose actions don't exist in user's config are added
  - Ensures existing users get access to new features like `paste_special` without manual config editing
- **Thin Strokes Rendering**: Corrected subpixel mask alpha handling so thin strokes remain visible instead of disappearing when enabled.
- **Font Rendering Toggles**: Thin strokes, antialiasing, and hinting now apply immediately when clicking "Apply font changes" in Settings (no restart required).
- **Tab Bar Click Reliability**: Fixed missed clicks and wrong-tab-selection issues
  - Close button now renders as overlay with manual hit-testing for reliable clicks
  - Uses `clicked_by(PointerButton::Primary)` to prevent keyboard focus from triggering tab switches
  - Added `egui_initialized` flag to prevent unreliable pointer state before first render
- **Max FPS Setting Not Honored**: Fixed `max_fps` config option not being enforced when window is focused
  - Previously, FPS throttling only applied when window was unfocused with `pause_refresh_on_blur` enabled
  - Now `max_fps` properly caps frame rate even when VSync runs at a higher monitor refresh rate (e.g., 120Hz)
  - Also fixed settings UI changes to `max_fps` not restarting tab refresh tasks with the new value
- **Terminal Content Overlap**: Added content offset system to prevent terminal content from overlapping with tab bar
  - Propagated `content_offset_y` through cell renderer, graphics renderer, and custom shader renderer
- **Tab Numbering**: Changed to position-based numbering that automatically renumbers when tabs are closed or reordered
  - Tabs now show "Tab 1, Tab 2, Tab 3" instead of keeping original IDs
- **Mouse Event Handling**: Fixed event ordering to check tab bar area before updating terminal mouse state
- **Startup Crash with Missing Background Image**: App no longer crashes when configured background image file is missing
  - Now logs a warning and continues without the background image
  - Fixes blank screen issue when shaders folder is missing but config references images inside it
- **Window Number Not Showing in Title**: Fixed `show_window_number` config option not working consistently
  - Window number now appears in all title updates (shell integration, OSC title changes, URL tooltips)
  - Added `format_title()` helper method to ensure consistent title formatting across all code paths
- **Cmd+W Closes Entire App Instead of Tab**: Fixed smart close behavior for Cmd+W keyboard shortcut
  - Cmd+W now closes the current tab first; only closes the window if it was the last tab
  - Menu item renamed from "Close Window" to "Close" to reflect the smart close behavior

### Added

- Comprehensive tab bar UI tests (`tests/tab_bar_ui_tests.rs`)
- Tab stability integration tests (`tests/tab_stability_tests.rs`)

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

[Unreleased]: https://github.com/paulrobello/par-term/compare/v0.14.0...HEAD
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
