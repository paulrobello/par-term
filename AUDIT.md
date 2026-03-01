# Refactor Audit

## Executive Summary

- **Total findings**: 28 (high: 9, medium: 12, low: 7)
- **Files exceeding 800 lines (critical)**: 10
- **Files in the 500–800 line warning zone**: 48
- **Estimated total effort**: XL

This audit covers 594 Rust source files across the root crate (`src/`) and 13 sub-crates. The project is well-structured as a Cargo workspace and has clearly benefited from prior refactoring waves. The most systemic problems are: (1) the monolithic `Config` struct with 324 public fields that acts as a god object despite the file already self-documenting 30+ candidate sub-structs; (2) a duplicated `profile_modal_ui` implementation between `src/` and `par-term-settings-ui/src/`; (3) near-identical `shell_detection.rs` files copied verbatim into both the root crate and the settings-ui sub-crate; (4) 23 suppressed `clippy::too_many_arguments` warnings indicating functions that need parameter-builder structs; and (5) the `gpu_submit.rs` render entry point (1,014 lines) and `gather_data.rs` (804 lines) both exceeding the project's own 800-line hard limit.

---

## Findings (ranked by impact)

### [HIGH] R-01 — Monolithic `Config` Struct (324 fields, 30+ unextracted sub-structs) — `par-term-config/src/config/config_struct/mod.rs` (1,859 lines)

**Category**: God Object / File Size
**Effort**: L

The `Config` struct carries 324 `pub` fields covering fonts, shaders, tab bar, scroll, clipboard, automation, AI inspector, status bar, progress bar, tmux, session logging, update checking, security, and more. The file's own module-level doc comment already catalogues 35 candidate sub-struct names (e.g. `WindowConfig`, `FontConfig`, `ShaderConfig`, `TabConfig`, `AutomationConfig`). Only six sections have been extracted so far (`UpdateConfig`, `UnicodeConfig`, `SshConfig`, `SearchConfig`, `CopyModeConfig`, plus the `default_impl` split). The remaining inline fields make `Config` a de facto god object: any feature that needs configuration adds fields to this single type, which is then cloned into every window.

**Impact**: Every settings UI tab, config propagation path, and serialization round-trip touches this struct. Adding a field requires touching `Config`, `Default`, the settings UI tab, and the serialization test — the blast radius grows with every feature. Compile times suffer because changes to `Config` invalidate most of the crate. Deserialization errors surface at the top level without clear grouping.

**Remedy**: Apply `#[serde(flatten)]` sub-struct extraction in order of the table already in the mod-level doc comment. Start with the largest, most cohesive groups that already have a logical section comment in the file: `WindowConfig` (cols, rows, font_*, line_spacing, char_spacing, window_*), `ShaderConfig` (custom_shader_*, cursor_shader_*), `AutomationConfig` (triggers, coprocesses), `AiInspectorConfig` (ai_inspector_*), and `StatusBarConfig` (status_bar_*). Each `#[serde(flatten)]` extraction keeps the YAML format backward-compatible and can be done incrementally — one sub-struct per PR.

---

### [HIGH] R-02 — Duplicated `profile_modal_ui` Implementation — `src/profile_modal_ui/` vs `par-term-settings-ui/src/profile_modal_ui/`

**Category**: DRY
**Effort**: M

Two separate, diverging implementations of the profile management modal UI exist simultaneously:
- `src/profile_modal_ui/` (dialogs.rs, edit_view.rs, list_view.rs, mod.rs, state.rs) — 1,452 lines total
- `par-term-settings-ui/src/profile_modal_ui/` (edit_view.rs, form_helpers.rs, list_view.rs, mod.rs) — 1,465 lines total

The diff between the `edit_view.rs` files reveals significant divergence: different function signatures (`collapsed: &mut HashSet<String>` parameter present only in the sub-crate version), different magic number constants (the root version references `PROFILE_ICON_PICKER_MIN_WIDTH` from `ui_constants`; the sub-crate version inlines `280.0` and `300.0`), and different `CollapsingHeader` implementations. The root crate version is exported via `pub mod profile_modal_ui` in `src/lib.rs` and is imported by integration tests (`tests/profile_ui_tests.rs`) via `par_term::profile_modal_ui`. The settings-ui sub-crate version is the authoritative one used at runtime.

**Impact**: Bug fixes and new features must be applied twice. The sub-crate version has `form_helpers.rs` and a `has_ancestor` / `render_parent_selector` that may not exist in the root version. The tests target the root version, which may differ from the running code, creating a gap between what is tested and what ships.

**Remedy**: Consolidate: migrate the integration tests to import from `par_term::settings_ui::profile_modal_ui` (which is already re-exported as `crate::settings_ui` in `lib.rs`). Then remove `src/profile_modal_ui/` entirely and add a compatibility shim `pub use crate::settings_ui::profile_modal_ui;` in the root `lib.rs` if needed for test imports.

---

### [HIGH] R-03 — Duplicated `shell_detection.rs` — `src/shell_detection.rs` (256 lines) vs `par-term-settings-ui/src/shell_detection.rs` (254 lines)

**Category**: DRY
**Effort**: S

The two `shell_detection.rs` files differ by exactly one `expect()` message string. They define identical `ShellInfo` struct, identical `Display` impl, and identical discovery logic. Both are independently compiled into the workspace.

**Impact**: Any bug in the shell discovery logic (path lookup, caching, platform quirks) must be fixed in two places. If the files drift further, callers that pass a `par_term::ShellInfo` to a function expecting `par_term_settings_ui::ShellInfo` will produce a type mismatch.

**Remedy**: Move the canonical implementation into `par-term-config` or a new micro-crate `par-term-platform`. Have both `par-term` and `par-term-settings-ui` depend on it. The settings-ui crate already declares `shell_detection` as `pub mod` — replace the file with a one-line re-export.

---

### [HIGH] R-04 — `gpu_submit.rs` Exceeds Hard Limit — `src/app/window_state/render_pipeline/gpu_submit.rs` (1,014 lines)

**Category**: File Size / God Object
**Effort**: M

`submit_gpu_frame` at 1,014 lines is the single largest function-containing file in the project (excluding `config_struct/mod.rs`). It orchestrates prettifier substitution, egui overlay rendering, GPU state upload, progress bars, badges, visual bells, and the split-pane vs single-pane dispatch — seven distinct concerns in one method chain. The file already has an internal `GpuUploadResult` struct and a private `render_egui_frame` helper, but `submit_gpu_frame` itself spans ~370 lines and `update_gpu_renderer_state` spans ~250 lines.

**Impact**: The function is too large to reason about locally. A reviewer cannot tell at a glance which GPU state is shared between the upload phase and the render phase. Any panic or incorrect state can have distant causes. The file violates the project's own 800-line guideline.

**Remedy**: Extract three focused functions: `upload_terminal_state(tab_data, renderer) -> GpuUploadResult`, `render_progress_and_overlays(actions, sizing)`, and `dispatch_render_pass(sizing, surface_texture, egui_data)`. The `update_gpu_renderer_state` method (lines ~402–654) is already self-contained enough to move to `renderer_ops.rs`. The `render_egui_frame` helper (lines ~655–1009) can move to `egui_overlays.rs`.

---

### [HIGH] R-05 — `gather_data.rs` Exceeds Hard Limit — `src/app/window_state/render_pipeline/gather_data.rs` (804 lines)

**Category**: File Size
**Effort**: M

`gather_render_data` at 804 lines is the only public method in the file and accumulates all terminal snapshot data for a frame. The file's own doc comment (lines 12–52) describes the Claude Code-specific block (~200 lines, ~line 260–465) as a candidate for extraction into a `ClaudeCodePrettifierBridge` struct and explicitly defers it.

**Impact**: The file barely exceeds the 800-line hard limit and is a known technical debt item. The Claude Code-specific logic is deeply interleaved with shared mutable borrows of `tab.prettifier` and `tab.pane_manager`, making it difficult to unit test the heuristic session detection separately.

**Remedy**: Implement the `ClaudeCodePrettifierBridge` struct described in the existing doc comment. The struct takes shared references at construction time, resolving the borrow-checker surface. This immediately reduces `gather_data.rs` by ~200 lines and makes the heuristic logic independently testable.

---

### [HIGH] R-06 — Duplicated Shader Metadata Parse Functions — `par-term-config/src/shader_metadata.rs` (914 lines)

**Category**: DRY / File Size
**Effort**: S

`parse_shader_metadata` and `parse_cursor_shader_metadata` share identical YAML-block extraction logic. `ShaderMetadataCache` and `CursorShaderMetadataCache` are structurally identical HashMap wrappers with identical `new()`, `get()`, `get_or_load()`, and `invalidate()` methods, differing only in the generic type parameter. The file's own doc comment (lines 37–45) explicitly acknowledges this duplication.

**Impact**: Any fix to the YAML extraction (e.g., handling edge-case comment termination) or cache eviction logic must be applied in two places. The file is 914 lines, 114 over the warning threshold.

**Remedy**: Introduce a `fn extract_yaml_block(source: &str) -> Option<&str>` free function that both parse functions call. Replace `ShaderMetadataCache` and `CursorShaderMetadataCache` with a generic `MetadataCache<T: for<'de> serde::Deserialize<'de>>`. This is already noted as the future fix in the doc comment — implement it.

---

### [HIGH] R-07 — `instance_builders.rs` Exceeds Hard Limit — `par-term-render/src/cell_renderer/instance_builders.rs` (984 lines)

**Category**: File Size
**Effort**: M

This file implements `CellRenderer` methods for building GPU instance buffers: background row building, text/glyph instance building, and cursor overlay building. These are logically distinct phases of a render pass that have grown together into one large file.

**Impact**: Background instance building (RLE merging, cursor overlay logic) is interleaved with text glyph placement logic. The file is the fourth largest in the project and violates the 800-line hard limit.

**Remedy**: Split into `bg_instance_builder.rs` (background/cursor background logic) and `text_instance_builder.rs` (glyph/text placement logic). Both are `impl CellRenderer` blocks so the split is mechanical with no API change.

---

### [HIGH] R-08 — 23 Suppressed `clippy::too_many_arguments` — across `src/` and `par-term-render/`

**Category**: Missing Abstraction / DRY
**Effort**: M

Twenty-three `#[allow(clippy::too_many_arguments)]` suppressions appear across the codebase:
- `src/app/window_state/render_pipeline/gpu_submit.rs` (×2)
- `src/app/window_state/render_pipeline/pane_render.rs` (×2)
- `src/app/window_state/render_pipeline/tab_snapshot.rs`
- `src/tab_bar_ui/tab_rendering.rs` (×2)
- `par-term-render/src/renderer/shaders.rs` (×3)
- `par-term-render/src/cell_renderer/background.rs`
- `par-term-render/src/cell_renderer/mod.rs`
- `par-term-render/src/cell_renderer/pane_render.rs` (×2)
- `par-term-render/src/renderer/rendering.rs`
- `par-term-render/src/custom_shader_renderer/mod.rs`
- `par-term-render/src/graphics_renderer.rs`
- `par-term-render/src/scrollbar.rs` (×2)
- `par-term-terminal/src/terminal/rendering.rs` (×2)
- `src/app/mouse_events/coords.rs`

Most of these arise from rendering functions that accept 8–14 configuration parameters assembled from separate config fields.

**Impact**: Functions with 10+ parameters are error-prone (swapped arguments cause silent bugs), untestable in isolation, and resist further refactoring. The `init_custom_shader` in `par-term-render/src/renderer/shaders.rs` accepts 14 parameters directly from `Config` fields.

**Remedy**: Introduce parameter structs for the hot paths. For example, `CustomShaderInitParams { path, enabled, animation, animation_speed, opacity, full_content, brightness, channel_paths, cubemap_path, use_background_as_channel0 }` eliminates the 14-parameter `init_custom_shader` signature. Similarly, `PaneRenderParams` and `ScrollbarParams` can bundle the repeated geometry and visual arguments.

---

### [HIGH] R-09 — `automation_tab.rs` and `appearance_tab.rs` Exceed Hard Limit — `par-term-settings-ui/src/automation_tab.rs` (1,031 lines), `par-term-settings-ui/src/appearance_tab.rs` (976 lines)

**Category**: File Size
**Effort**: M

`automation_tab.rs` contains the full egui rendering for both Triggers and Coprocesses sections in a single flat file with no sub-module split. `appearance_tab.rs` consolidates theme, auto dark mode, fonts, text shaping, font rendering, and cursor sections. While tab files are by nature broad, both exceed the 800-line hard limit.

**Impact**: The `automation_tab.rs` file renders two functionally distinct sections (trigger definitions vs. coprocess management). A developer editing coprocess restart policy logic must search through ~500 lines of trigger UI code. At 1,031 lines, this is the largest settings tab file.

**Remedy**: For `automation_tab.rs`: extract `triggers_section.rs` and `coprocesses_section.rs` as private sub-modules of a new `automation_tab/` directory. For `appearance_tab.rs`: split out `fonts_section.rs` and `cursor_section.rs` as sub-modules (the same pattern used in `window_tab/`, `advanced_tab/`, `input_tab/`, and `prettifier_tab/`).

---

### [MEDIUM] R-10 — `ai_inspector_tab.rs` Exceeds Hard Limit — `par-term-settings-ui/src/ai_inspector_tab.rs` (847 lines)

**Category**: File Size
**Effort**: S

The AI inspector settings tab renders agent configuration, auto-approve/YOLO mode, context settings, scope selection, and view mode in one flat 847-line file.

**Impact**: Exceeds the 800-line hard limit. Features that touch only context settings must be found within a larger undifferentiated file.

**Remedy**: Extract `agent_config_section.rs` (agent selection, custom agents) and `context_section.rs` (auto-context, scope, view mode) as private sub-modules of a new `ai_inspector_tab/` directory.

---

### [MEDIUM] R-11 — `par-term-acp/src/protocol.rs` Exceeds Hard Limit (866 lines)

**Category**: File Size
**Effort**: S

The ACP protocol file is a single flat file of 50+ structs and enums covering initialize, session management, content blocks, permission requests, file system operations, and config updates. There is no grouping beyond comments.

**Impact**: Any developer adding a new RPC method must search the entire 866-line file to find the correct section. The file violates the 800-line hard limit.

**Remedy**: Convert to a module directory `protocol/` with sub-files: `initialize.rs`, `session.rs`, `permissions.rs`, `fs_ops.rs`, `config_update.rs`. Re-export everything from `protocol/mod.rs`. This is a pure mechanical reorganization with no logic changes.

---

### [MEDIUM] R-12 — `par-term-acp/src/agent.rs` Exceeds Hard Limit (864 lines)

**Category**: File Size / God Object
**Effort**: M

`agent.rs` manages the agent lifecycle (spawning, handshaking, routing, permission dispatch), the `handle_incoming_messages` background task (200+ lines, lines 510–704), and test helpers. The `Agent` struct owns `status`, `session_id`, `client`, `auto_approve`, plus the `SafePaths` for permission checking — mixing connectivity state with file-system policy.

**Impact**: `SafePaths` (a security policy concern) is defined next to `AgentStatus` and `AgentMessage` (protocol concerns). The `handle_incoming_messages` free function at 200 lines is hard to test because it requires a live `mpsc` channel setup. The file violates the 800-line hard limit.

**Remedy**: Move `handle_incoming_messages` to `message_handler.rs`. Move `SafePaths` to `permissions.rs` (which already exists). Extract the connection-establishment handshake sequence from `Agent::connect` into a `handshake.rs` module.

---

### [MEDIUM] R-13 — `par-term-terminal/src/terminal/mod.rs` Exceeds Hard Limit (843 lines)

**Category**: File Size
**Effort**: M

`TerminalManager` has sub-modules (`clipboard`, `graphics`, `hyperlinks`, `rendering`, `scrollback`, `spawn`) but the `mod.rs` still holds 843 lines of core struct definition, scrollback marker tracking, and the main `read()` / `write()` methods.

**Impact**: The `mod.rs` is the primary integration point for PTY sessions and is heavily accessed from both the async reader thread and the sync winit event loop. At 843 lines it exceeds the hard limit.

**Remedy**: Extract `marker_tracking.rs` for the shell lifecycle marker state machine (`last_shell_marker`, `command_start_pos`, `captured_command_text`, `shell_lifecycle_events`) and its update logic. The `TerminalManager` struct definition and public delegation API can remain in `mod.rs` but should shrink to under 400 lines.

---

### [MEDIUM] R-14 — `par-term-render/src/cell_renderer/block_chars/box_drawing.rs` Exceeds Hard Limit (817 lines)

**Category**: File Size
**Effort**: L

This file is an 817-line `match` statement mapping 128+ Unicode box-drawing characters to geometric `LineSegment` arrays. It is essentially a static lookup table implemented as code.

**Impact**: The file is 17 lines over the hard limit but the content is purely declarative. Adding new box drawing characters (e.g., for rounded corners in newer Unicode) means extending an already large match arm.

**Remedy**: Convert the `match` to a static `phf` (perfect hash function) map or a `LazyLock<HashMap<char, BoxDrawingGeometry>>`. This reduces the file to ~50 lines of initialization code and a ~750-line data table that can be split into `box_drawing_light.rs`, `box_drawing_heavy.rs`, and `box_drawing_double.rs` grouped by line style.

---

### [MEDIUM] R-15 — `par-term-render/src/renderer/shaders.rs` in Warning Zone (797 lines)

**Category**: File Size / Too Many Arguments
**Effort**: M

`shaders.rs` contains `init_custom_shader` (14 arguments, `#[allow(clippy::too_many_arguments)]`), `init_cursor_shader` (similar signature), and `update_custom_shader`. The two init functions are near-identical in structure, differing only in which renderer field they populate.

**Impact**: The `init_custom_shader` / `init_cursor_shader` duplication means any change to shader initialization must be applied twice. Three suppressed lint violations in one file is a code smell.

**Remedy**: Introduce a `ShaderInitParams` builder struct (see R-08). Factor out the common initialization sequence into a generic `init_shader_renderer(params: ShaderInitParams, cell_renderer: &CellRenderer) -> (Option<CustomShaderRenderer>, Option<String>)`. The background vs cursor distinction becomes a caller-side concern.

---

### [MEDIUM] R-16 — `par-term-render/src/renderer/rendering.rs` in Warning Zone (790 lines)

**Category**: File Size
**Effort**: M

`rendering.rs` implements the `Renderer::render` entry point and the full per-frame render path. It is closely related to `render_passes.rs` (659 lines) and `shaders.rs` (797 lines), with the three files together forming the renderer's core. The boundary between them is not always clear.

**Impact**: The render path is distributed across three files totaling ~2,246 lines, making it difficult to trace the full sequence of GPU operations in a single reading.

**Remedy**: Consolidate the render orchestration flow into a single `render_orchestrator.rs` file (the "what happens each frame" narrative) and push the pass-specific details into `render_passes.rs`. Move the `Renderer::render` method to `render_orchestrator.rs` and keep `rendering.rs` only for frame-level helpers.

---

### [MEDIUM] R-17 — `par-term-config/src/shader_metadata.rs` in Warning Zone (914 lines)

**Category**: File Size / DRY
Already captured as R-06 for the DRY violation. The file size (914 lines) is a secondary consequence of the duplication and warrants a separate tracking entry for the appendix.

**Category**: File Size
**Effort**: S (resolves with R-06)

---

### [MEDIUM] R-18 — `src/app/window_state/action_handlers.rs` in Warning Zone (748 lines)

**Category**: File Size / God Object
**Effort**: M

`action_handlers.rs` contains four distinct handler groups in one file: tab bar actions (~95 lines), clipboard history actions (~32 lines), AI Inspector actions (~344 lines), and integrations welcome dialog (~237 lines). The file's own doc comment (lines 8–23) lists these sections explicitly. The AI Inspector section alone handles 17 `InspectorAction` variants.

**Impact**: Any addition to the AI Inspector action dispatch grows a file already at 748 lines. The integrations response handler (handling shader install, shell integration install, conflict resolution) is logically unrelated to clipboard history.

**Remedy**: Split into `action_handlers/tab_bar.rs`, `action_handlers/inspector.rs`, and `action_handlers/integrations.rs`, re-exporting all three from a thin `action_handlers/mod.rs`.

---

### [MEDIUM] R-19 — Dead Code Accumulation in `src/app/config_updates.rs`

**Category**: DRY / Missing Abstraction
**Effort**: S

`ConfigChanges` has 10 fields marked `#[allow(dead_code)]` with comments like "Detected but not yet consumed by a live-reload handler". These stale detectors are tracked but never dispatched. Similarly, `src/app/file_transfers/types.rs` has 4 dead fields for diagnostics that are never displayed.

**Impact**: Dead code suppression with `TODO(dead_code): ... or remove by v0.26` comments indicates deferred cleanup. These `dead_code` fields inflate the `ConfigChanges::detect()` function unnecessarily and create a false impression of completeness.

**Remedy**: Either implement the handlers (live reload for `cursor_shader_config`, `window_type`, `target_monitor`, `anti_idle`, `dynamic_profile_sources`) or delete the detection fields. For v0.26 candidates: add a milestone label/comment and track in a GitHub issue rather than in source code.

---

### [MEDIUM] R-20 — `par-term-config/src/defaults/misc.rs` Near Warning Threshold (523 lines)

**Category**: File Size
**Effort**: S

`defaults/misc.rs` is a flat collection of 100+ `pub fn default_*()` functions used as `#[serde(default = "...")]` callbacks. These are organized by section comment but not split into further sub-modules.

**Impact**: The file is approaching the 500-line warning threshold. Every new config field with a custom default adds a function here, growing the file further.

**Remedy**: Align with the sub-struct extraction plan from R-01: as `Config` fields are moved into sub-structs (e.g., `ShaderConfig`, `AutomationConfig`), move their corresponding default functions into the sub-crate files where the sub-structs live. The `defaults/misc.rs` should shrink to near zero as sub-structs implement their own `Default`.

---

### [MEDIUM] R-21 — `par-term-update/src/self_updater.rs` in Warning Zone (704 lines)

**Category**: File Size
**Effort**: S

The self-updater covers installation type detection, binary hash verification, download-and-replace logic, platform-specific atomic swap, and Homebrew/cargo branch dispatch. These are distinct concerns.

**Impact**: At 704 lines, the file is approaching the hard limit. The hash verification and download logic are tightly coupled to the binary replacement logic.

**Remedy**: Extract `install_methods.rs` (Homebrew / cargo / macOS bundle / standalone strategy dispatch) and `binary_ops.rs` (atomic swap, hash verification, cleanup of `.old` files). Keep `self_updater.rs` as the orchestrating entry point under 300 lines.

---

### [MEDIUM] R-22 — `par-term-keybindings/src/matcher.rs` and `parser.rs` Approaching Hard Limit (719, 699 lines)

**Category**: File Size
**Effort**: S

`matcher.rs` (719 lines) and `parser.rs` (699 lines) are the two files in `par-term-keybindings`. Together they form the entire crate. The `matcher.rs` file contains `KeybindingMatcher`, modifier normalization, and the large `matches_combo` method with extensive platform normalization (`cmd_or_ctrl` logic on macOS vs other platforms).

**Impact**: Both files are within 100 lines of the hard limit. The platform normalization in `matcher.rs` (`cmd_or_ctrl` expansion) and the NamedKey alias table in `parser.rs` are both independently large blocks.

**Remedy**: Extract `platform.rs` for the `cmd_or_ctrl` platform resolution and the `NamedKey` alias table. Keep `matcher.rs` and `parser.rs` focused on their core responsibilities.

---

### [LOW] R-23 — Stale Re-export Shim Files in `src/`

**Category**: Module Boundary
**Effort**: S

Several files in `src/` are thin re-export wrappers created during sub-crate extraction but no longer needed as standalone modules:
- `src/themes.rs` (6 lines): `pub use par_term_config::Theme;`
- `src/text_shaper.rs` (6 lines): re-exports `par_term_fonts::text_shaper`
- `src/cell_renderer.rs` (7 lines): re-exports `par_term_render::cell_renderer`
- `src/renderer.rs` (~15 lines): re-exports `par_term_render::renderer`
- `src/terminal.rs` (~15 lines): re-exports `par_term_terminal`
- `src/scrollback_metadata.rs` (4 lines): re-exports `par_term_terminal::scrollback_metadata`
- `src/update_checker.rs` (4 lines): re-exports `par_term_update::update_checker`
- `src/self_updater.rs` (6 lines): re-exports `par_term_update::self_updater`

**Impact**: These shims add indirection without abstraction. Any caller using `crate::themes::Theme` could directly use `par_term_config::Theme`. The shims also inflate `src/lib.rs` with `pub mod` declarations.

**Remedy**: Gradually migrate call sites to import directly from the sub-crate. For items used externally (e.g., via integration tests), keep the re-export in `lib.rs` as a single line rather than a dedicated file. Remove the wrapper files one-by-one as their last call site is updated.

---

### [LOW] R-24 — `src/profile_modal_ui/` Contains Obsolete `dialogs.rs`

**Category**: Module Boundary / Dead Code
**Effort**: S

`src/profile_modal_ui/dialogs.rs` (124 lines) contains a delete-confirmation dialog implementation. The `par-term-settings-ui/src/profile_modal_ui/list_view.rs` contains its own delete confirmation dialog (in `render_delete_confirmation`). If `src/profile_modal_ui/` is to be retired (R-02), this file is dead code.

**Impact**: 124 lines of UI code that does not appear to be exercised by the active settings UI path.

**Remedy**: Resolve as part of R-02: once the root `src/profile_modal_ui/` module is replaced with a re-export of the sub-crate version, remove `dialogs.rs` and the associated `mod dialogs` declaration.

---

### [LOW] R-25 — Magic Numbers in `par-term-settings-ui/src/profile_modal_ui/edit_view.rs`

**Category**: DRY
**Effort**: S

`par-term-settings-ui/src/profile_modal_ui/edit_view.rs` uses inline float literals `280.0` (min icon picker width) and `300.0` (max icon picker height) rather than named constants. The root `src/ui_constants.rs` defines `PROFILE_ICON_PICKER_MIN_WIDTH = 280.0` and `PROFILE_ICON_PICKER_MAX_HEIGHT = 300.0` for this exact purpose, but these are not accessible to the sub-crate.

**Impact**: If the icon picker dimensions need adjustment, there is one place in `ui_constants.rs` and a second place with raw literals in the sub-crate — they will drift.

**Remedy**: Move the `PROFILE_ICON_PICKER_*` constants (and other profile modal geometry) to `par-term-config/src/ui_constants.rs` (or a new `par-term-config/src/layout_constants.rs`) so both crates can share them without a circular dependency.

---

### [LOW] R-26 — Over-broad `pub` Visibility on Root Crate Items

**Category**: Module Boundary
**Effort**: M

The root crate has 363 top-level `pub struct`/`pub enum`/`pub fn` items but only 44 `pub(crate)` items. Most internal types (e.g., `TabBarUI`, `StatusBarUI`, `CopyModeState`, `FileTransferState`, `OverlayState`) are declared `pub` even though they are only ever referenced from within the root crate or its sub-modules.

**Impact**: An overly permissive `pub` surface makes it unclear which types form the intentional public API and which are implementation details. External code consuming `par-term` as a library (e.g., integration tests, the settings UI) can reach types it should not depend on.

**Remedy**: Audit top-level `pub` items that are not exported via `lib.rs`'s re-exports and change them to `pub(crate)`. Focus first on types under `src/app/` and `src/tab/` — these are clearly internal.

---

### [LOW] R-27 — `par-term-config/src/config/prettifier/mod.rs` in Warning Zone (562 lines)

**Category**: File Size
**Effort**: S

The prettifier config module holds the full YAML deserialization types for all renderer configurations (markdown, JSON, YAML, TOML, XML, CSV, diff, log, diagrams, SQL, stack trace) in a single flat file with a companion `renderers.rs` (257 lines) and `resolve.rs` (190 lines).

**Impact**: Adding a new prettifier format requires editing both the `mod.rs` and `renderers.rs`. At 562 lines, `mod.rs` is already in the warning zone.

**Remedy**: Group the per-renderer config structs into a `renderers/` sub-directory, matching the `src/prettifier/renderers/` layout in the root crate.

---

### [LOW] R-28 — `par-term-tmux/src/sync.rs` Approaching Warning Threshold (559 lines)

**Category**: File Size
**Effort**: S

`sync.rs` in the tmux integration crate handles tmux control-mode session synchronization including window/pane tracking and layout updates. It is the largest file in `par-term-tmux` and a single flat module.

**Impact**: As tmux feature depth grows (e.g., pane synchronization, window arrangement), this file is likely to grow further.

**Remedy**: Extract `pane_sync.rs` for pane-level sync state and `window_sync.rs` for window-level sync state from `sync.rs`.

---

## Dependency Graph

Findings that can be worked on in parallel (no shared state or sequential dependency):

**Wave 1** (no prerequisites — pure extractions or deletions):
- R-03: Consolidate `shell_detection.rs` into `par-term-config` or a shared crate
- R-06: Genericize `ShaderMetadataCache` and extract `extract_yaml_block()`
- R-08: Introduce parameter-builder structs for the 23 `too_many_arguments` sites
- R-11: Split `par-term-acp/src/protocol.rs` into grouped sub-files
- R-14: Convert `box_drawing.rs` match to a static map
- R-19: Resolve `dead_code` fields in `config_updates.rs` and `file_transfers/types.rs`
- R-22: Extract `platform.rs` from `par-term-keybindings`
- R-23: Remove stale re-export shim files from `src/`
- R-25: Move profile modal geometry constants to `par-term-config`
- R-27: Group prettifier renderer configs into a `renderers/` sub-directory
- R-28: Extract `pane_sync.rs` and `window_sync.rs` from `par-term-tmux/src/sync.rs`

**Wave 2** (after Wave 1):
- R-02: Retire `src/profile_modal_ui/` — depends on R-03 being resolved (type unification) and confirming tests compile against the sub-crate version
- R-09: Split `automation_tab.rs` and `appearance_tab.rs` into sub-modules — depends on R-08 (parameter structs that the tabs also use)
- R-10: Split `ai_inspector_tab.rs` — independent but benefits from Wave 1 constant moves (R-25)
- R-12: Extract `handle_incoming_messages` from `agent.rs` — depends on R-11 (protocol types stabilized)
- R-13: Extract `marker_tracking.rs` from `terminal/mod.rs` — independent but benefits from Wave 1 cleanup
- R-15: Factor out `ShaderInitParams` for `shaders.rs` — depends on R-08 (parameter structs)
- R-21: Split `self_updater.rs` — independent
- R-24: Remove `dialogs.rs` — depends on R-02

**Wave 3** (after Wave 2):
- R-01: Extract `Config` sub-structs — depends on R-03 (to know where to put `ShellConfig`), R-20 (defaults migrate with sub-structs)
- R-04: Reduce `gpu_submit.rs` — depends on R-08 (parameter structs eliminate the large argument lists) and R-15 (shader init refactored)
- R-05: Extract `ClaudeCodePrettifierBridge` from `gather_data.rs` — depends on R-04 (render pipeline stabilized)
- R-07: Split `instance_builders.rs` — independent of Wave 2 but benefits from R-08
- R-16: Consolidate render orchestration files — depends on R-04 and R-15
- R-18: Split `action_handlers.rs` — depends on R-04 (AI inspector actions reference GPU submit state)
- R-20: Shrink `defaults/misc.rs` — depends on R-01 (sub-structs carry their own defaults)
- R-26: Tighten `pub` visibility — depends on R-02 (profile_modal_ui) and R-23 (shim files)

---

## Appendix: Files by Line Count (descending)

| File | Lines | Status |
|------|-------|--------|
| `par-term-config/src/config/config_struct/mod.rs` | 1,859 | CRITICAL |
| `par-term-settings-ui/src/automation_tab.rs` | 1,031 | CRITICAL |
| `src/app/window_state/render_pipeline/gpu_submit.rs` | 1,014 | CRITICAL |
| `par-term-render/src/cell_renderer/instance_builders.rs` | 984 | CRITICAL |
| `par-term-settings-ui/src/appearance_tab.rs` | 976 | CRITICAL |
| `par-term-config/src/shader_metadata.rs` | 914 | CRITICAL |
| `par-term-acp/src/protocol.rs` | 866 | CRITICAL |
| `par-term-acp/src/agent.rs` | 864 | CRITICAL |
| `par-term-settings-ui/src/ai_inspector_tab.rs` | 847 | CRITICAL |
| `par-term-terminal/src/terminal/mod.rs` | 843 | CRITICAL |
| `par-term-render/src/cell_renderer/block_chars/box_drawing.rs` | 817 | CRITICAL |
| `src/app/window_state/render_pipeline/gather_data.rs` | 804 | CRITICAL |
| `par-term-render/src/renderer/shaders.rs` | 797 | WARNING |
| `par-term-render/src/renderer/rendering.rs` | 790 | WARNING |
| `par-term-settings-ui/src/status_bar_tab.rs` | 784 | WARNING |
| `src/prettifier/custom_renderers.rs` | 766 | WARNING |
| `src/bin/par-term-acp-harness.rs` | 762 | WARNING |
| `src/app/window_manager/window_lifecycle.rs` | 753 | WARNING |
| `par-term-settings-ui/src/snippets_tab.rs` | 750 | WARNING |
| `src/app/window_state/action_handlers.rs` | 748 | WARNING |
| `par-term-terminal/src/scrollback_metadata.rs` | 745 | WARNING |
| `par-term-render/src/renderer/mod.rs` | 725 | WARNING |
| `par-term-render/src/graphics_renderer.rs` | 719 | WARNING |
| `par-term-keybindings/src/matcher.rs` | 719 | WARNING |
| `par-term-settings-ui/src/background_tab/shader_channel_settings.rs` | 708 | WARNING |
| `par-term-update/src/self_updater.rs` | 704 | WARNING |
| `par-term-render/src/cell_renderer/pane_render.rs` | 703 | WARNING |
| `par-term-render/src/cell_renderer/mod.rs` | 700 | WARNING |
| `par-term-keybindings/src/parser.rs` | 699 | WARNING |
| `src/prettifier/pipeline/pipeline_impl.rs` | 697 | WARNING |
| `par-term-settings-ui/src/profile_modal_ui/edit_view.rs` | 692 | WARNING |
| `par-term-fonts/src/font_manager/mod.rs` | 686 | WARNING |
| `par-term-render/src/cell_renderer/background.rs` | 680 | WARNING |
| `src/profile_modal_ui/edit_view.rs` | 679 | WARNING |
| `src/pane/manager/tmux_layout.rs` | 663 | WARNING |
| `src/app/window_state/agent_messages.rs` | 660 | WARNING |
| `par-term-render/src/renderer/render_passes.rs` | 659 | WARNING |
| `par-term-render/src/custom_shader_renderer/mod.rs` | 657 | WARNING |
| `src/prettifier/claude_code.rs` | 653 | WARNING |
| `par-term-settings-ui/src/input_tab/keybindings.rs` | 649 | WARNING |
| `par-term-render/src/scrollbar.rs` | 648 | WARNING |
| `src/tab_bar_ui/tab_rendering.rs` | 645 | WARNING |
| `src/app/tmux_handler/notifications/layout.rs` | 645 | WARNING |
| `src/app/tab_ops/lifecycle.rs` | 642 | WARNING |
| `src/ai_inspector/panel/mod.rs` | 636 | WARNING |
| `par-term-settings-ui/src/notifications_tab.rs` | 635 | WARNING |
| `src/app/window_manager/scripting/mod.rs` | 632 | WARNING |
| `src/tab/mod.rs` | 631 | WARNING |
| `src/settings_window.rs` | 630 | WARNING |
| `src/ai_inspector/panel/chat_view.rs` | 629 | WARNING |
| `src/app/input_events/keybinding_actions.rs` | 623 | WARNING |
| `src/prettifier/config_bridge.rs` | 612 | WARNING |
| `src/tab/manager.rs` | 609 | WARNING |
| `src/app/copy_mode_handler.rs` | 607 | WARNING |
| `src/app/tmux_handler/gateway.rs` | 606 | WARNING |
| `par-term-config/src/themes.rs` | 605 | WARNING |
| `par-term-render/src/cell_renderer/atlas.rs` | 600 | WARNING |
| `par-term-settings-ui/src/profiles_tab.rs` | 594 | WARNING |
| `par-term-settings-ui/src/scripts_tab.rs` | 593 | WARNING |
| `src/url_detection/mod.rs` | 589 | WARNING |
| `src/app/file_transfers/mod.rs` | 585 | WARNING |
| `src/menu/mod.rs` | 583 | WARNING |
| `src/app/window_manager/config_propagation.rs` | 582 | WARNING |
| `src/prettifier/renderers/sql_results.rs` | 580 | WARNING |
| `src/prettifier/renderers/diagrams/renderer.rs` | 571 | WARNING |
| `src/session_logger/core.rs` | 564 | WARNING |
| `src/status_bar/mod.rs` | 562 | WARNING |
| `par-term-config/src/config/prettifier/mod.rs` | 562 | WARNING |
| `src/badge.rs` | 561 | WARNING |
| `src/app/mouse_events/mouse_button.rs` | 561 | WARNING |
| `par-term-render/src/renderer/state.rs` | 561 | WARNING |
| `par-term-config/src/automation.rs` | 561 | WARNING |
| `par-term-tmux/src/sync.rs` | 559 | WARNING |
| `par-term-acp/src/agents.rs` | 551 | WARNING |
| `src/tab_bar_ui/title_utils.rs` | 546 | WARNING |
| `src/app/tab_ops/profile_ops.rs` | 539 | WARNING |
| `src/prettifier/renderers/diff/renderer.rs` | 538 | WARNING |
| `par-term-settings-ui/src/window_tab/tab_bar.rs` | 538 | WARNING |
| `par-term-settings-ui/src/settings_ui/state.rs` | 534 | WARNING |
| `src/pane/types/pane.rs` | 531 | WARNING |
| `par-term-config/src/snippets.rs` | 529 | WARNING |
| `par-term-tmux/src/session.rs` | 525 | WARNING |
| `par-term-settings-ui/src/settings_ui/display.rs` | 525 | WARNING |
| `par-term-config/src/defaults/misc.rs` | 523 | WARNING |
| `src/prettifier/registry.rs` | 519 | WARNING |
| `par-term-input/src/lib.rs` | 518 | WARNING |
| `par-term-render/src/custom_shader_renderer/transpiler.rs` | 515 | WARNING |
| `src/app/window_state/impl_helpers.rs` | 511 | WARNING |
| `src/prettifier/renderers/yaml/parser.rs` | 510 | WARNING |
| `src/prettifier/renderers/json/parser.rs` | 509 | WARNING |
| `src/prettifier/buffer.rs` | 506 | WARNING |
| `par-term-mcp/src/lib.rs` | 504 | WARNING |
| `src/search/mod.rs` | 501 | WARNING |

**Summary**

| Severity | Count |
|----------|-------|
| Critical (> 800 lines) | 12 files |
| Warning (500–800 lines) | 79 files |

---

## Summary

- **Critical**: 9 | **High**: 0 | **Medium**: 12 | **Low**: 7
- **Overall Architecture Health**: Fair
- **Key Concern**: The `Config` struct with 324 public fields is the central architectural liability — it makes every refactoring more expensive and grows as a catch-all for every new feature. Extracting it into focused sub-structs (R-01) is the highest-leverage long-term investment and unlocks parallelism in downstream work.
