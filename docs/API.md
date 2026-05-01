# API Documentation Index

> **Note:** This index is maintained manually and is not validated by CI. It may drift out of sync with the codebase after refactors or new type additions. For the authoritative, up-to-date API reference, run `make doc` and open `target/doc/par_term/index.html`.

This document provides an overview of the public types and functions exported by each par-term workspace crate. For full generated API documentation, run `make doc` and open `target/doc/par_term/index.html`.

## Table of Contents

- [par-term-config](#par-term-config)
- [par-term-fonts](#par-term-fonts)
- [par-term-input](#par-term-input)
- [par-term-keybindings](#par-term-keybindings)
- [par-term-terminal](#par-term-terminal)
- [par-term-render](#par-term-render)
- [par-term-settings-ui](#par-term-settings-ui)
- [par-term-scripting](#par-term-scripting)
- [par-term-tmux](#par-term-tmux)
- [par-term-update](#par-term-update)
- [par-term-acp](#par-term-acp)
- [par-term-ssh](#par-term-ssh)
- [par-term-mcp](#par-term-mcp)

---

## par-term-config

Configuration loading, saving, and type definitions for the terminal emulator. This is the foundational crate used by all other workspace members.

### Core Configuration

| Type | Description |
|------|-------------|
| `Config` | Main configuration struct. All terminal, display, input, and feature settings. Serialized to/from `config.yaml`. |
| `ConfigError` | Error type returned by configuration load and save operations. |
| `ALLOWED_ENV_VARS` | Slice of environment variable names permitted in config `${VAR}` substitutions. |
| `is_env_var_allowed(name)` | Returns `true` if `name` is on the substitution allowlist or has a permitted prefix. |
| `substitute_variables(input)` | Expands `${VAR}` placeholders in a YAML string using the allowlist. |
| `substitute_variables_with_allowlist(input, allow_all)` | Expands placeholders with explicit allowlist control. |

### Display and Rendering

| Type | Description |
|------|-------------|
| `VsyncMode` | Presentation mode: `Immediate`, `Mailbox`, or `Fifo`. |
| `PowerPreference` | GPU adapter preference: `None`, `LowPower`, or `HighPerformance`. |
| `BackgroundMode` | Terminal background rendering mode (solid color, image, shader). |
| `BackgroundImageMode` | How a background image is fitted: `Fit`, `Fill`, `Stretch`, `Tile`, `Center`. |
| `PaneBackground` | Per-pane background override (image path, mode, opacity). |
| `PaneBackgroundConfig` | Full pane background configuration with all options. |
| `PaneId` | Unique identifier for a split pane within a tab. |
| `PaneTitlePosition` | Where pane titles appear: `Top` or `Bottom`. |
| `ImageScalingMode` | Inline image scaling mode. |
| `DividerStyle` | Visual style for split-pane dividers: `Solid`, `Double`, `Dashed`, `Shadow`. |
| `DividerRect` | Pixel bounds for a rendered split-pane divider. |
| `SeparatorMark` | A command-separator line visible in the viewport (screen row, exit code, color). |
| `ScrollbackMark` | An absolute scrollback line index marking a prompt or command boundary. |

### Font Configuration

| Type | Description |
|------|-------------|
| `FontRange` | Maps a Unicode code-point range to a specific font family. |
| `ThinStrokesMode` | Sub-pixel thin-stroke rendering mode. |
| `DownloadSaveLocation` | Where downloaded files are saved. |
| `DroppedFileQuoteStyle` | How dropped file paths are quoted when pasted. |

### Input and Keybindings

| Type | Description |
|------|-------------|
| `KeyBinding` | A single keybinding: key combination string mapped to an action name. |
| `KeyModifier` | Modifier key bitmask (Ctrl, Shift, Alt, Super). |
| `ModifierRemapping` | Config for swapping modifier keys (e.g., swap Ctrl and Super). |
| `ModifierTarget` | Which physical modifier key a remapping targets. |
| `OptionKeyMode` | How the Option/Alt key behaves: `Normal`, `Meta`, or `Esc`. |

### Shell and Session

| Type | Description |
|------|-------------|
| `ShellType` | Detected shell type (Bash, Zsh, Fish, PowerShell, etc.) with detection logic. |
| `ShellExitAction` | What to do when the shell exits: `Close`, `Keep`, `RestartImmediately`, etc. |
| `StartupDirectoryMode` | Where new tabs open: `Home`, `CurrentTab`, or `Custom`. |
| `SessionLogFormat` | Format for session recording: `Plain`, `Html`, or `Asciicast`. |

### Terminal Display

| Type | Description |
|------|-------------|
| `CursorStyle` | Cursor shape: `Block`, `Beam`, or `Underline`. |
| `UnfocusedCursorStyle` | Cursor style when the window does not have focus. |
| `Cell` | A single terminal grid cell with character, colors, and attribute flags. |
| `LinkUnderlineStyle` | How hyperlinks are underlined: `None`, `Always`, or `Hover`. |
| `LogLevel` | Application log level. |
| `SemanticHistoryEditorMode` | How semantic history opens files. |

### Tab Bar and Window

| Type | Description |
|------|-------------|
| `TabStyle` | Tab button style: `Default`, `Powerline`, `Slant`. |
| `TabBarMode` | When the tab bar is shown: `Always`, `Auto`, `Never`. |
| `TabBarPosition` | `Top` or `Bottom`. |
| `TabTitleMode` | How tab titles are set: `Auto` or `OscOnly`. |
| `NewTabPosition` | Where new tabs appear: `AfterActive` or `AtEnd`. |
| `RemoteTabTitleFormat` | Format string for remote (SSH/tmux) tab titles. |
| `WindowType` | Window decoration style. |
| `StatusBarPosition` | `Top` or `Bottom`. |
| `TabId` | Unique identifier for a tab. |

### Status Bar

| Type | Description |
|------|-------------|
| `StatusBarWidgetConfig` | Configuration for a single status bar widget. |
| `StatusBarSection` | Which section of the status bar a widget appears in. |
| `WidgetId` | Identifies a built-in or custom widget. |
| `default_widgets()` | Returns the default list of status bar widgets. |

### Themes

| Type | Description |
|------|-------------|
| `Theme` | A named color theme with 16 terminal colors and background/foreground. |
| `Color` | An RGB color value. |

### Shaders

| Type | Description |
|------|-------------|
| `ShaderConfig` | Background shader reference: name, enabled flag, per-shader parameters. |
| `ResolvedShaderConfig` | Fully resolved shader config after metadata lookup. |
| `ShaderMetadata` | TOML-parsed metadata for a background shader. |
| `ShaderMetadataCache` | In-memory cache of background shader metadata. |
| `CursorShaderConfig` | Configuration for cursor post-processing shaders. |
| `CursorShaderMetadata` | TOML-parsed metadata for a cursor shader. |
| `ResolvedCursorShaderConfig` | Fully resolved cursor shader config after metadata lookup. |
| `CursorShaderMetadataCache` | In-memory cache of cursor shader metadata. |
| `ShaderBundleManifest` | Manifest for a bundled shader package. |
| `ShaderBackgroundBlendMode` | How the shader blends with the background. |
| `ShaderSafetyBadge` | Safety classification badge for a shader. |
| `ShaderControl` | A single UI control exposed by a shader (slider, color picker, etc.). |
| `ShaderControlKind` | Enum of control types: `Slider`, `Color`, `Toggle`, `Angle`, `Select`. |
| `AngleUnit` | Unit for angle controls: `Radians` or `Degrees`. |
| `SliderScale` | Slider value scale: `Linear` or `Logarithmic`. |
| `resolve_shader_config(config, cache)` | Resolve a `ShaderConfig` against the metadata cache. |
| `resolve_cursor_shader_config(config, cache)` | Resolve a cursor shader config. |
| `parse_shader_controls(source)` | Parse `//@control` annotations from GLSL source. |
| `parse_shader_metadata(path)` | Parse TOML metadata for a background shader. |
| `parse_cursor_shader_metadata(path)` | Parse TOML metadata for a cursor shader. |

### Profiles

| Type | Description |
|------|-------------|
| `Profile` | A named terminal session profile (shell, working directory, command, env vars). |
| `ProfileId` | UUID identifier for a profile. |
| `ProfileManager` | Loads and saves the profile list from `profiles.yaml`. |
| `ProfileSource` | Whether a profile is `Local` or `Dynamic` (fetched from a URL). |
| `TmuxConnectionMode` | How a profile connects to tmux: `Disable`, `AutoAttach`, or `ForceAttach`. |
| `DynamicProfileSource` | URL and refresh configuration for a remote profile source. |
| `ConflictResolution` | How to handle conflicts between local and remote profiles: `LocalWins` or `RemoteWins`. |

### Automation

| Type | Description |
|------|-------------|
| `TriggerConfig` | A regex trigger that fires actions when matched in terminal output. |
| `TriggerActionConfig` | The action to execute when a trigger fires. |
| `TriggerRateLimiter` | Rate limiting state for a trigger to prevent action storms. |
| `TriggerSplitDirection` | Direction for trigger-initiated split panes (`Horizontal` or `Vertical`). |
| `TriggerSplitTarget` | Which pane to use when a trigger splits (`Current` or `New`). |
| `SplitPaneCommand` | Shell command to run in a trigger-initiated split pane. |
| `CoprocessDefConfig` | Configuration for a coprocess (a subprocess wired to the PTY). |
| `RestartPolicy` | When to restart a coprocess: `Never`, `OnFailure`, `Always`. |
| `check_command_denylist(cmd)` | Returns an error if the command matches the security denylist. |
| `check_command_allowlist(cmd)` | Returns `true` if the command is on the security allowlist. |
| `warn_prompt_before_run_false` | Flag indicating a trigger skips the confirmation prompt. |

### Snippets and Actions

| Type | Description |
|------|-------------|
| `SnippetConfig` | A text snippet with optional variable substitution and keybinding. |
| `SnippetLibrary` | The full collection of snippets, indexed by ID. |
| `CustomActionConfig` | A custom action: shell command, new tab, text insert, key sequence, or split pane. |
| `BuiltInVariable` | Built-in snippet variable names (Date, Time, User, etc.). |

### Scripting

| Type | Description |
|------|-------------|
| `ScriptConfig` | Configuration for an external observer script. |

### Assistant / AI

| Type | Description |
|------|-------------|
| `AssistantPrompt` | A named prompt template for the AI assistant panel. |
| `AssistantPromptDraft` | A prompt being edited (not yet saved). |
| `AiInspectorConfig` | AI inspector panel configuration. |
| `AssistantInputHistoryMode` | How assistant input history is persisted. |
| `CustomAcpAgentConfig` | Configuration for a custom ACP agent binary. |
| `CustomAcpAgentActionConfig` | Configuration for an action that launches an ACP agent. |

### Selection and Smart Selection

| Type | Description |
|------|-------------|
| `SmartSelectionRule` | A regex rule for smart selection word boundaries. |
| `SmartSelectionPrecision` | Precision level for smart selection matching. |
| `default_smart_selection_rules()` | Returns the built-in smart selection rules. |

### Unicode

| Type | Description |
|------|-------------|
| `AmbiguousWidth` | Width for ambiguous-width Unicode codepoints: `Single` or `Double`. |
| `NormalizationForm` | Unicode normalization form: `Nfc`, `Nfd`, `Nfkc`, or `Nfkd`. |
| `UnicodeVersion` | Unicode version for width tables. |

### Progress and Alerts

| Type | Description |
|------|-------------|
| `ProgressBarStyle` | OSC progress bar display style. |
| `ProgressBarPosition` | Where the progress bar overlay appears. |
| `AlertEvent` | Events that can trigger an alert sound. |
| `AlertSoundConfig` | Sound file and volume for an alert event. |

### Integration

| Type | Description |
|------|-------------|
| `InstallPromptState` | Whether an install prompt has been dismissed. |
| `IntegrationVersions` | Version tracking for shell integration scripts. |
| `UpdateCheckFrequency` | How often to check for updates: `Daily`, `Weekly`, `Monthly`, or `Never`. |
| `ShaderInstallPrompt` | Whether the bundled shader install prompt was shown. |

---

## par-term-fonts

Font loading and HarfBuzz-based text shaping.

| Type | Description |
|------|-------------|
| `FontManager` | Manages font loading, glyph lookup, and system font fallback chains. Handles primary, bold, italic, bold-italic, Unicode-range, and fallback fonts. |
| `FontData` | A loaded font face with its raw bytes and face index. |
| `UnicodeRangeFont` | A font mapped to a specific Unicode code-point range. |
| `FALLBACK_FAMILIES` | Slice of family names tried as last-resort fallbacks (Noto, DejaVu, etc.). |
| `TextShaper` | HarfBuzz shaping engine with LRU cache. Converts text runs into positioned glyph IDs. |
| `ShapedGlyph` | A single glyph with its ID, advance width, and x/y offset. |
| `ShapedRun` | The result of shaping a text run: a sequence of `ShapedGlyph` values for one font face. |
| `ShapingOptions` | Options passed to the shaper (ligatures, kerning, font features). |

---

## par-term-input

Keyboard input processing: converting winit events to terminal byte sequences.

| Type | Description |
|------|-------------|
| `InputHandler` | Converts winit `KeyEvent` values to VT/xterm byte sequences. Handles modifier state, Option key modes, clipboard access, and modifyOtherKeys encoding. |

### Key Methods

| Method | Description |
|--------|-------------|
| `InputHandler::new()` | Creates a new input handler, initializing clipboard support. |
| `handle_key_event(event)` | Convert a key press to terminal bytes (normal mode). |
| `handle_key_event_with_mode(event, mode, app_cursor)` | Convert with modifyOtherKeys and application cursor support. |
| `paste_from_clipboard()` | Read text from the system clipboard. |
| `copy_to_clipboard(text)` | Write text to the system clipboard. |
| `clipboard_has_image()` | Check whether the clipboard contains an image (for image-aware apps). |

---

## par-term-keybindings

Runtime-configurable keybinding registry.

| Type | Description |
|------|-------------|
| `KeybindingRegistry` | Maps parsed key combinations to action name strings. Built from `Config.keybindings`. |
| `KeybindingMatcher` | Helper for matching key events against `KeyCombo` patterns with modifier remapping support. |
| `KeyCombo` | A parsed key combination (key + modifiers). |
| `ParseError` | Error returned when a keybinding string cannot be parsed. |
| `parse_key_sequence(s)` | Parse a human-readable key sequence string into a `KeyCombo`. |
| `key_combo_to_bytes(combo)` | Convert a `KeyCombo` to the VT byte sequence it represents. |

### Key Methods

| Method | Description |
|--------|-------------|
| `KeybindingRegistry::from_config(bindings)` | Build a registry from the config `keybindings` list, skipping invalid entries. |
| `KeybindingRegistry::lookup(event, mods)` | Look up the action name for a key event. |
| `KeybindingRegistry::lookup_with_options(...)` | Look up with modifier remapping and physical key support. |

---

## par-term-terminal

Terminal session management, scrollback, and terminal state access.

| Type | Description |
|------|-------------|
| `TerminalManager` | High-level wrapper around a PTY session. Manages I/O, resize, clipboard, inline graphics, scrollback, and coprocesses. |
| `ShellLifecycleEvent` | Events emitted when the shell starts, changes CWD, or exits. |
| `SearchMatch` | A single pattern match in the scrollback (line, column, length). |
| `ScrollbackMetadata` | Tracks shell-integration markers and command history for timing overlays and the AI inspector. |
| `CommandSnapshot` | Immutable record of a completed command (text, start time, exit code, duration). |
| `LineMetadata` | Timing and command metadata for a specific scrollback line, used by separator rendering. |
| `ScrollbackMark` | Re-export of `par_term_config::ScrollbackMark`. |
| `coprocess_env()` | Returns the environment variables set in coprocess subprocesses. |
| `ClipboardEntry` | Re-export: a clipboard slot entry from the core library. |
| `ClipboardSlot` | Re-export: identifies which clipboard slot (primary or clipboard). |
| `HyperlinkInfo` | Re-export: OSC 8 hyperlink metadata for a cell. |

---

## par-term-render

GPU-accelerated rendering engine: cell renderer, inline graphics, and custom shaders.

| Type | Description |
|------|-------------|
| `Renderer` | Orchestrates the three-pass GPU render pipeline: cells, graphics, and egui overlay. |
| `RendererParams` | Parameters passed to the renderer each frame (surface, device, config, etc.). |
| `CellRenderer` | Renders terminal cells using an instanced GPU pipeline with a glyph atlas. |
| `Cell` | Re-export of the cell type used by the renderer. |
| `PaneViewport` | Pixel bounds and scroll state for a single pane, used to clip rendering. |
| `GraphicsRenderer` | Renders Sixel, iTerm2, and Kitty inline graphics using RGBA texture caching. |
| `GraphicRenderInfo` | Metadata for a single inline graphic (position, size, texture ID). |
| `CustomShaderRenderer` | Applies user-defined GLSL post-processing shaders via WGSL transpilation. |
| `Scrollbar` | Renders the scrollbar and scrollback mark overlays for a pane. |
| `RenderError` | Error type for rendering operations. |
| `PaneRenderInfo` | All data needed to render one pane in a frame. |
| `DividerRenderInfo` | Position and hover state for a split-pane divider. |
| `PaneTitleInfo` | Position, text, and colors for a pane title bar. |
| `PaneDividerSettings` | Divider and focus indicator appearance settings. |
| `compute_visible_separator_marks(...)` | Map absolute scrollback marks to screen rows for the current viewport. |
| `SeparatorMark` | Re-export of `par_term_config::SeparatorMark`. |
| `ScrollbackMark` | Re-export of `par_term_config::ScrollbackMark`. |

---

## par-term-settings-ui

egui-based settings interface decoupled from the main terminal crate via traits.

### Traits (implemented by the main crate)

| Trait | Description |
|-------|-------------|
| `ProfileOps` | Profile CRUD operations (get, save, upsert, delete). |
| `ArrangementOps` | Window arrangement save/restore/delete/rename. |
| `UpdateOps` | Update check, install, and progress reporting. |
| `CoprocessOps` | Coprocess lifecycle control (start, stop, status, output). |
| `ScriptOps` | Observer script lifecycle control (start, stop, status, output, panel state). |
| `ShaderOps` | Shader installation and management (install bundled, list available, cubemaps). |
| `ShellIntegrationOps` | Shell integration install/uninstall for Bash, Zsh, Fish, etc. |

### Trait Helper Types

| Type | Description |
|------|-------------|
| `InstallResult` | Counts from a shader install operation (installed, skipped, removed). |

### Key Types

| Type | Description |
|------|-------------|
| `SettingsUI` | Main settings window UI component. |
| `SettingsTab` | Enum of settings tabs (Appearance, Terminal, Profiles, etc.). |
| `ProfileModalUI` | Modal dialog for creating and editing profiles. |
| `ProfileModalAction` | Actions returned by the profile modal (Save, Cancel, Delete). |
| `WindowArrangement` | A saved window layout (positions, sizes, tab configurations). |
| `ArrangementManager` | Loads and saves window arrangements to disk. |
| `ArrangementId` | UUID identifier for a saved arrangement. |
| `ArrangementInfo` | Lightweight arrangement metadata (id, name, window count). |
| `WindowSnapshot` | Snapshot of a single window's state within an arrangement. |
| `TabSnapshot` | Snapshot of a single tab's state within a window snapshot. |
| `MonitorInfo` | Display monitor dimensions and position for arrangement DPI handling. |
| `ShaderDetectModifiedFn` | Function pointer type for detecting modified bundled shaders. |
| `ShaderLintFn` | Function pointer type for running shader lint/readability analysis. |
| `ShaderInstallResult` | Result of shader installation (installed, skipped, removed counts). |
| `ShaderUninstallResult` | Result of shader uninstallation (removed, kept, needs confirmation). |
| `SettingsWindowAction` | Actions returned by settings UI for the main app to process. |
| `UpdateCheckResult` | Result of an update check (UpToDate, UpdateAvailable, Error, etc.). |
| `UpdateCheckInfo` | Information about an available update. |
| `ShellIntegrationInstallResult` | Result of shell integration installation. |
| `ShellIntegrationUninstallResult` | Result of shell integration uninstallation. |

---

## par-term-scripting

Observer-pattern scripting: launch Python or shell scripts that react to terminal events.

| Type | Description |
|------|-------------|
| `ScriptManager` | Manages multiple `ScriptProcess` instances for a single tab. Handles start, stop, event broadcast, and panel state. |
| `ScriptId` | `u64` identifier for a managed script subprocess. |
| `ScriptProcess` | A single script subprocess with JSON-line stdin/stdout communication. |

### Public Modules

| Module | Description |
|--------|-------------|
| `manager` | `ScriptManager` and `ScriptId` — per-tab script orchestrator. |
| `process` | `ScriptProcess` — single subprocess lifecycle and JSON-line I/O. |
| `observer` | Observer event forwarding from terminal core to script subprocesses. |
| `protocol` | `ScriptEvent` and `ScriptCommand` types used by the JSON protocol. |

---

## par-term-tmux

tmux control mode integration.

| Type | Description |
|------|-------------|
| `TmuxSession` | Manages the tmux control-mode subprocess lifecycle and state. |
| `TmuxSync` | Bidirectional state synchronization between tmux and par-term panes. |
| `SyncAction` | Actions produced by `TmuxSync` (create pane, resize, close, etc.). |
| `GatewayState` | Connection state of the tmux gateway. |
| `SessionState` | Full synchronized tmux session state (windows, panes). |
| `TmuxNotification` | A parsed tmux notification received from control mode. |
| `TmuxCommand` | A command sent to tmux control mode. |
| `ParserBridge` | Adapts the core library's tmux parser for use by `TmuxSession`. |
| `PrefixKey` | The configured tmux prefix key. |
| `PrefixState` | Current state of the tmux prefix key sequence. |
| `translate_command_key(event, prefix)` | Translate a key event through the tmux prefix state machine. |
| `FormatContext` | Variables available when expanding a tmux status format string. |
| `expand_format(format, ctx)` | Expand a tmux `#[…]` status format string. |
| `sanitize_tmux_output(s)` | Strip control sequences from tmux output for safe display. |
| `escape_keys_for_tmux(s)` | Escape key sequences that would be interpreted by tmux. |
| `TmuxWindow` | A tmux window (maps to a par-term tab). |
| `TmuxPane` | A tmux pane with its dimensions, title, and output buffer. |
| `TmuxLayout` | The layout tree for a tmux window. |
| `LayoutNode` | A node in the tmux pane layout tree. |
| `TmuxSessionInfo` | Metadata about a tmux session (name, ID, attached, window count). |
| `TmuxWindowId` | `u64` window identifier (e.g., `@0`). |
| `TmuxPaneId` | `u64` pane identifier (e.g., `%0`). |

---

## par-term-update

Self-update and release tracking. Types are exported via submodule re-exports.

### Modules

| Module | Description |
|--------|-------------|
| `update_checker` | `UpdateChecker`, `UpdateInfo`, `UpdateCheckResult`, `fetch_latest_release()` |
| `self_updater` | `UpdateResult`, `perform_update()` |
| `binary_ops` | `DownloadUrls`, `get_asset_name()`, `get_download_urls()`, `cleanup_old_binary()` |
| `install_methods` | `InstallationType`, `detect_installation()` |
| `manifest` | Bundled asset tracking (`Manifest`, `ManifestFile`, `FileType`) |
| `http` | HTTP utilities (`download_file()`, `validate_update_url()`) |

### Key Types (via `self_updater` re-export)

| Type | Description |
|------|-------------|
| `UpdateResult` | Result of applying a self-update (old/new versions, install path, restart flag). |
| `DownloadUrls` | Binary and checksum download URLs for a release. |
| `InstallationType` | How par-term is installed: `Homebrew`, `CargoInstall`, `MacOSBundle`, `StandaloneBinary`. |

### Key Functions (via `self_updater` re-export)

| Function | Description |
|----------|-------------|
| `cleanup_old_binary()` | Remove the old binary left over after an in-place update. |
| `detect_installation()` | Detect the current installation type. |
| `get_asset_name()` | Get the platform-specific release asset filename. |
| `get_download_urls(api_url)` | Fetch binary and checksum URLs from the GitHub API. |

---

## par-term-acp

Agent Communication Protocol (ACP) implementation for AI coding agent integration.

| Type | Description |
|------|-------------|
| `Agent` | Manages an ACP agent subprocess lifecycle: spawn, handshake, message routing, and permission handling. |
| `AgentStatus` | Connection status: `Disconnected`, `Connecting`, `Connected`, or `Error(String)`. |
| `AgentMessage` | Messages from the agent manager to the UI: status changes, session updates, permission requests, config updates. |
| `SafePaths` | Directories pre-approved for agent write access without user confirmation. |
| `AgentConfig` | Configuration for an ACP agent (name, binary path, args, env vars). |
| `discover_agents(config_dir)` | Discover available agent configs from TOML/YAML files in the config directory. |
| `JsonRpcClient` | Asynchronous JSON-RPC 2.0 client over stdio. |
| `Request` / `Response` | JSON-RPC 2.0 request and response types. |
| `RpcError` | JSON-RPC error wrapper. |
| `IncomingMessage` | An incoming JSON-RPC message (request or notification). |

### Re-exported Protocol Types

| Type | Description |
|------|-------------|
| `InitializeParams` | Parameters for the ACP `initialize` handshake. |
| `InitializeResult` | Result returned after successful initialization. |
| `ClientCapabilities` | Capabilities advertised by the par-term client. |
| `ClientInfo` | Client name and version. |
| `SessionNewParams` | Parameters for creating a new ACP session. |
| `SessionPromptParams` | Parameters for sending a prompt to an agent session. |
| `SessionUpdate` | Incremental update from an agent session (text, tool calls, results). |
| `SessionUpdateParams` | Wrapper for session update notifications. |
| `SessionResult` | Final result of an agent session. |
| `RequestPermissionParams` | Parameters for a permission request from the agent. |
| `RequestPermissionResponse` | User's response to a permission request. |
| `PermissionOption` | Options presented in a permission request (allow, deny, etc.). |
| `PermissionOutcome` | The chosen outcome for a permission request. |
| `ContentBlock` | A content block in a session update (text, image, tool use, etc.). |
| `ToolCallInfo` | Information about a tool call being made by the agent. |
| `ToolCallUpdateInfo` | Incremental update to an in-progress tool call. |
| `FsReadParams` | Parameters for an `fs/read` tool call. |
| `FsWriteParams` | Parameters for an `fs/write` tool call. |
| `FsListDirectoryParams` | Parameters for an `fs/list_directory` tool call. |
| `FsFindParams` | Parameters for an `fs/find` tool call. |
| `FsCapabilities` | Filesystem capabilities advertised during initialization. |

### Public Modules

| Module | Description |
|--------|-------------|
| `protocol` | ACP message types (initialize, session, permission, content blocks, tool calls). |
| `jsonrpc` | JSON-RPC 2.0 client, request/response types, and error handling. |
| `permissions` | Permission dispatch, auto-approval logic, and `SafePaths`. |
| `message_handler` | Background async task routing incoming messages to the UI. |
| `agents` | Agent discovery and configuration loading from TOML/YAML files. |
| `fs_ops` | Low-level filesystem operations (read, write, list, find). |
| `fs_tools` | RPC handler functions for `fs/*` tool calls from the agent. |
| `session` | Session-new parameter builders (MCP server descriptor, Claude wrapper metadata). |
| `harness` | Test harness for ACP agent smoke tests and transcript capture. |

---

## par-term-ssh

SSH host discovery, config parsing, and known-hosts scanning.

| Type | Description |
|------|-------------|
| `SshHost` | A discovered SSH host with alias, hostname, user, port, identity file, and proxy jump. |
| `SshHostSource` | Where the host was found: `Config`, `KnownHosts`, `History`, or `Mdns`. |
| `discover_local_hosts()` | Aggregate SSH hosts from all discovery sources into a deduplicated list. |

### Key `SshHost` Methods

| Method | Description |
|--------|-------------|
| `display_name()` | The host alias or hostname used for display. |
| `connection_target()` | The resolved hostname or alias for connecting. |
| `ssh_args()` | Build the argument list for `ssh` (port, identity file, proxy jump, target). |
| `connection_string()` | Format `user@host:port` for display. |

---

## par-term-mcp

Minimal MCP (Model Context Protocol) server over stdio. Exposes tools for ACP agent integrations.

| Item | Description |
|------|-------------|
| `run_mcp_server()` | Start the stdio JSON-RPC 2.0 MCP server loop. Blocks until stdin is closed. |
| `set_app_version(version)` | Set the application version reported during MCP initialization. |
| `TerminalScreenshotRequest` | IPC request written by the MCP server for the GUI to fulfill. |
| `TerminalScreenshotResponse` | IPC response written by the GUI with the screenshot data or error. |
| `ShaderDiagnosticsRequest` | IPC request for live shader state and compile errors. |
| `ShaderDiagnosticsResponse` | IPC response with shader diagnostics data or error. |
| `ShaderDiagnostics` | Per-shader diagnostics (background and cursor) with last errors and WGSL paths. |
| `ShaderDiagnosticsEntry` | Single shader diagnostic entry (shader name, enabled, last error, WGSL path). |
| `screenshot_request_path()` | Get the path to the screenshot request IPC file. |
| `screenshot_response_path()` | Get the path to the screenshot response IPC file. |
| `shader_diagnostics_request_path()` | Get the path to the shader diagnostics request IPC file. |
| `shader_diagnostics_response_path()` | Get the path to the shader diagnostics response IPC file. |
| `CONFIG_UPDATE_PATH_ENV` | Env var name for overriding the config update file path. |
| `SCREENSHOT_REQUEST_PATH_ENV` | Env var name for the screenshot request IPC path. |
| `SCREENSHOT_RESPONSE_PATH_ENV` | Env var name for the screenshot response IPC path. |
| `SHADER_DIAGNOSTICS_REQUEST_PATH_ENV` | Env var name for the shader diagnostics request IPC path. |
| `SHADER_DIAGNOSTICS_RESPONSE_PATH_ENV` | Env var name for the shader diagnostics response IPC path. |
| `SCREENSHOT_FALLBACK_PATH_ENV` | Env var name for a static fallback screenshot path (harness use). |
| `CONFIG_UPDATE_FILENAME` | Default filename for the config update IPC file. |
| `SCREENSHOT_REQUEST_FILENAME` | Default filename for the screenshot request IPC file. |
| `SCREENSHOT_RESPONSE_FILENAME` | Default filename for the screenshot response IPC file. |
| `SHADER_DIAGNOSTICS_REQUEST_FILENAME` | Default filename for the shader diagnostics request IPC file. |
| `SHADER_DIAGNOSTICS_RESPONSE_FILENAME` | Default filename for the shader diagnostics response IPC file. |

---

## Related Documentation

- [Architecture Overview](architecture/ARCHITECTURE.md) — How the crates fit together
- [Configuration Reference](CONFIG_REFERENCE.md) — All `Config` fields documented
- [Contributing](../CONTRIBUTING.md) — Development setup and workflow
- [Environment Variables](guides/ENVIRONMENT_VARIABLES.md) — Runtime environment variable reference
