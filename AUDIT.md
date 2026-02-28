# par-term Refactoring Audit

> Generated 2025-02-28. Covers the main `par-term` crate (`src/`) and workspace
> sub-crates. Each item is tagged with an ID for issue/PR tracking and rated by
> **Impact** (how much it improves maintainability) and **Effort** (implementation
> cost), both on a Low / Medium / High scale.

---

## 1. God Struct Decomposition

These oversized structs accumulate unrelated responsibilities. Splitting them
into focused sub-structs improves readability, testability, and compile times.

### AUD-001: Config (340 flat fields)

| | |
|---|---|
| **File** | `par-term-config/src/config/config_struct/mod.rs` (1,934 lines) |
| **Impact** | High |
| **Effort** | High |

The `Config` struct has ~340 `pub` fields in a single flat struct. Navigation is
difficult and every settings change recompiles the entire struct.

**Suggested split** (using `#[serde(flatten)]` for backwards compatibility):

| Sub-struct | Field groups |
|---|---|
| `FontConfig` | font_family, font_size, font_weight, bold_is_bright, ... |
| `ColorConfig` | foreground, background, cursor_color, selection_color, ansi_colors, ... |
| `WindowConfig` | window_width, window_height, window_padding, opacity, decorations, ... |
| `ShellConfig` | shell_program, shell_args, working_directory, login_shell, env_vars, ... |
| `KeyboardConfig` | use_physical_keys, modifier_remapping, option_key_mode, ... |
| `MouseConfig` | copy_on_select, mouse_scroll_lines, click_to_focus, ... |
| `PaneConfig` | pane_padding, pane_title_height, show_pane_titles, pane_border_color, ... |
| `TabBarConfig` | tab_bar_position, tab_bar_height, show_tab_bar, tab_bar_colors, ... |
| `ShaderConfig` | custom_shader, cursor_shader, shader_params, ... |
| `StatusBarConfig` | status_bar_enabled, status_bar_components, ... |
| `ScrollConfig` | scrollback_lines, scroll_multiplier, ... |
| `BellConfig` | bell_style, visual_bell_duration, ... |
| `PrettifierConfig` | prettifier_enabled, prettifier_theme, ... |

**Constraint**: serde YAML compatibility must be preserved — `#[serde(flatten)]`
keeps the flat file format while organizing the Rust API.

---

### AUD-002: Tab (44 fields, 4 LEGACY)

| | |
|---|---|
| **File** | `src/tab/mod.rs` (752 lines) |
| **Impact** | High |
| **Effort** | Medium |

The `Tab` struct mixes terminal I/O, UI state, pane management, and caching.
Four fields are marked `LEGACY` with documented migration paths to per-pane state:

| Legacy field | Migration target | ~Call sites |
|---|---|---|
| `scroll_state` | `Pane.scroll_state` | ~10 |
| `mouse` | `Pane.mouse` | ~15 (selection done via #210) |
| `cache` | `Pane.cache` | ~7 |
| `prettifier_pipeline` | `Pane.prettifier_pipeline` | ~17 |

**Suggested grouping** (in addition to LEGACY migration):

| Group | Fields |
|---|---|
| Process/IO | `terminal`, `runtime`, `shell_command`, `env_vars`, `coprocess_id`, `anti_idle_*` |
| UI display | `title`, `icon`, `badge_label`, `has_activity`, `bell_state`, `color_*` |
| Pane layout | `pane_manager` (already exists), plus LEGACY fields once migrated |
| Session | `session_logger`, `profile`, `is_profile_tab` |

---

### AUD-003: WindowState (37 fields — ARC-001 in progress)

| | |
|---|---|
| **File** | `src/app/window_state/mod.rs` (192 lines) |
| **Impact** | High |
| **Effort** | Medium (incremental) |

Already decomposed into 5 sub-state structs (`FocusState`, `OverlayState`,
`UpdateState`, `WatcherState`, `TriggerState`), plus 5 extracted state objects
(`shader_state`, `overlay_ui`, `agent_state`, `cursor_anim`, `debug`).

**Remaining extraction candidates**:

| Candidate | Fields to extract |
|---|---|
| `EguiState` | `egui_ctx`, `egui_state`, `pending_egui_events`, `egui_initialized` |
| `ConfigState` | `config`, `config_changed_by_agent`, `pending_font_rebuild`, `config_save_state` |
| `ClipboardState` | `clipboard_image_click_guard`, plus clipboard methods from `InputHandler` |

---

## 2. Large Files (>800 lines)

Files over 800 lines are harder to navigate and review. Target is <500 lines per
CLAUDE.md guidelines.

| ID | File | Lines | Suggested action |
|---|---|---|---|
| AUD-010 | `src/app/window_state/agent_messages.rs` | 1,006 | Split by message type (agent config, tool calls, responses) |
| AUD-011 | `src/prettifier/renderers/json.rs` | 995 | Extract JSON token parser into sub-module |
| AUD-012 | `src/session_logger.rs` | 965 | Split into logger core + format writers |
| AUD-013 | `src/url_detection.rs` | 946 | Extract URL regex compilation + tests into sub-module |
| AUD-014 | `src/app/window_state/render_pipeline/mod.rs` | 914 | Further split render passes into separate files |
| AUD-015 | `src/ai_inspector/shader_context.rs` | 879 | Extract context-gathering helpers |
| AUD-016 | `src/prettifier/renderers/toml.rs` | 865 | Extract TOML token parser |
| AUD-017 | `src/copy_mode.rs` | 857 | Split state machine from key dispatch |
| AUD-018 | `src/prettifier/boundary.rs` | 850 | Extract boundary detection algorithms |
| AUD-019 | `src/prettifier/renderers/yaml.rs` | 838 | Extract YAML token parser |
| AUD-020 | `src/app/window_state/render_pipeline/gather_data.rs` | 837 | Split per-pane vs single-pane data gathering |
| AUD-021 | `src/app/triggers.rs` | 832 | Extract trigger matching from trigger execution |
| AUD-022 | `src/app/window_manager/scripting.rs` | 820 | Split command dispatch from command implementations |
| AUD-023 | `src/prettifier/renderers/log.rs` | 816 | Extract log-level parsing |
| AUD-024 | `src/profile/dynamic.rs` | 800 | Extract profile resolution from profile application |

---

## 3. Duplicated Pattern Extraction

Repeated code patterns that could be centralized into helper methods.

### AUD-030: active_tab() / active_tab_mut() Chains (308 call sites)

| | |
|---|---|
| **Impact** | Medium |
| **Effort** | Medium |

308 calls to `active_tab()` / `active_tab_mut()` with identical `if let Some(tab)`
unwrapping. Many do the same thing: read a field, lock the terminal, or write
input.

**Suggested helpers on `WindowState`**:

```rust
/// Run a closure with the active tab, if any.
fn with_active_tab<R>(&self, f: impl FnOnce(&Tab) -> R) -> Option<R>;
fn with_active_tab_mut<R>(&mut self, f: impl FnOnce(&mut Tab) -> R) -> Option<R>;

/// Domain-specific shortcuts:
fn active_terminal(&self) -> Option<Arc<RwLock<TerminalManager>>>;
fn active_scroll_state(&self) -> Option<&ScrollState>;
fn active_scroll_state_mut(&mut self) -> Option<&mut ScrollState>;
```

---

### AUD-031: Terminal Lock Boilerplate (156 call sites)

| | |
|---|---|
| **Impact** | Medium |
| **Effort** | Medium |

156 `try_write()` / `try_read()` patterns with identical error handling and
`// try_lock: intentional` doc comments.

**Suggested helpers on `Tab` or `TerminalManager`**:

```rust
/// Non-blocking read access; returns None on contention.
fn try_with_terminal<R>(&self, f: impl FnOnce(&TerminalManager) -> R) -> Option<R>;

/// Non-blocking write access; returns None on contention.
fn try_with_terminal_mut<R>(&self, f: impl FnOnce(&mut TerminalManager) -> R) -> Option<R>;
```

---

### AUD-032: Window Redraw Requests (173 call sites)

| | |
|---|---|
| **Impact** | Low |
| **Effort** | Low |

173 instances of `if let Some(window) = &self.window { window.request_redraw() }`.

**Suggested**: Add `WindowState::request_redraw(&self)` method (already partially
exists — verify coverage and unify all call sites).

---

### AUD-033: if-let-Some(window) Guard Pattern (105 call sites)

| | |
|---|---|
| **Impact** | Low |
| **Effort** | Low |

105 instances of `if let Some(window) = &self.window { ... }` for operations
beyond just `request_redraw` (cursor setting, title setting, etc).

**Suggested**: Add `WindowState::with_window<R>(&self, f: impl FnOnce(&Window) -> R) -> Option<R>`.

---

## 4. Trait Opportunities

The codebase defines only ~9 traits total (mostly in the prettifier subsystem).
Adding traits would improve testability, enable mocking, and document interfaces.

### AUD-040: Terminal Access Trait

| | |
|---|---|
| **Impact** | Medium |
| **Effort** | Medium |

Create a `TerminalAccess` trait to unify how components interact with terminal
state. Would enable mock terminals in tests for the `app/` module.

```rust
trait TerminalAccess {
    fn is_alt_screen_active(&self) -> bool;
    fn should_report_mouse_motion(&self, button_pressed: bool) -> bool;
    fn modify_other_keys_mode(&self) -> u8;
    fn application_cursor(&self) -> bool;
    fn encode_mouse_event(&self, button: u8, col: usize, row: usize, motion: bool, mods: u8) -> Vec<u8>;
}
```

---

### AUD-041: UIElement Trait

| | |
|---|---|
| **Impact** | Medium |
| **Effort** | Medium |

`TabBarUI`, `StatusBarUI`, `OverlayUiState`, and various egui panels share
a lifecycle pattern (init, update, draw, handle_input). A shared trait would
document this contract and enable generic overlay management.

---

### AUD-042: EventHandler Trait

| | |
|---|---|
| **Impact** | Low |
| **Effort** | Medium |

Mouse, keyboard, and window events flow through `WindowState` methods with no
shared interface. A trait would enable composition and testing of individual
handlers.

---

## 5. Test Coverage Gaps

27 integration test files exist in `tests/`, but several critical subsystems
lack coverage.

### AUD-050: app/ Module (~0% unit test coverage)

| | |
|---|---|
| **Impact** | High |
| **Effort** | High |

The `src/app/` module (mouse events, input handling, render pipeline,
window lifecycle) has almost no tests. This is the core event loop and
the most frequently modified code.

**Priority test targets**:
- Mouse event coordinate translation (`pixel_to_cell`, `pixel_to_pane_cell`)
- Selection state management (start, extend, copy)
- Key event routing (modifier detection, shortcut dispatch)
- Render data gathering (split-pane vs single-pane)

---

### AUD-051: Keybindings Module (0% coverage)

| | |
|---|---|
| **Impact** | Medium |
| **Effort** | Low |

`par-term-keybindings/` has no tests. Keybinding lookup, chord detection,
and modifier remapping are testable in isolation.

---

### AUD-052: Copy Mode (no tests)

| | |
|---|---|
| **Impact** | Medium |
| **Effort** | Medium |

`src/copy_mode.rs` (857 lines) implements a vim-like selection mode with
word/line motions, search, and clipboard integration — all untested.

---

### AUD-053: Renderer (minimal coverage)

| | |
|---|---|
| **Impact** | Medium |
| **Effort** | High |

GPU rendering code is hard to test but the data preparation (glyph atlas
lookups, cell-to-instance conversion, styled segment processing) can be
tested without a GPU context.

---

## 6. Module Organization

### AUD-060: Platform-Specific Code Consolidation

| | |
|---|---|
| **Impact** | Low |
| **Effort** | Low |

`#[cfg(target_os = ...)]` blocks are scattered across ~20 files. The largest
concentrations are in input handling (8 blocks), menu (7), tab setup (6),
and config methods (6).

**Suggested**: Extract platform-specific behavior into a `platform/` module with
trait-based dispatch, consolidating platform differences in one place.

---

### AUD-061: Magic Numbers → Named Constants

| | |
|---|---|
| **Impact** | Low |
| **Effort** | Low |

Many numeric literals are used inline (thresholds, pixel sizes, timing values).
Priority targets:
- Mouse drag threshold (`8.0` px in `mouse_move.rs`)
- Scrollbar dimensions and timing
- Animation durations and easing constants
- Config save debounce interval (100ms, already a const but pattern not widespread)

---

### AUD-062: Legacy Field Cleanup (4 fields in Tab)

| | |
|---|---|
| **Impact** | Medium |
| **Effort** | Medium |

Four `Tab` fields are marked `LEGACY` with documented migration plans (see
AUD-002). Each requires updating 7-17 call sites to route through `PaneManager`
when in split-pane mode.

**Migration order** (by dependency):
1. `scroll_state` (~10 sites) — independent, no blockers
2. `cache` (~7 sites) — needed by prettifier
3. `prettifier_pipeline` (~17 sites) — depends on cache migration
4. `mouse` (~15 sites) — selection already done (#210), remaining are non-selection fields

---

## Summary by Priority

### Critical (do first)
| ID | Item | Impact | Effort |
|---|---|---|---|
| AUD-001 | Config struct decomposition | High | High |
| AUD-002 | Tab LEGACY field migration | High | Medium |
| AUD-050 | app/ module test coverage | High | High |

### High Value (good ROI)
| ID | Item | Impact | Effort |
|---|---|---|---|
| AUD-003 | WindowState continued extraction | High | Medium |
| AUD-030 | active_tab() helper extraction | Medium | Medium |
| AUD-031 | Terminal lock helper extraction | Medium | Medium |
| AUD-051 | Keybindings test coverage | Medium | Low |
| AUD-062 | LEGACY field cleanup | Medium | Medium |

### Incremental Improvements
| ID | Item | Impact | Effort |
|---|---|---|---|
| AUD-010–024 | Large file decomposition | Medium | Low each |
| AUD-032–033 | Window/redraw helper extraction | Low | Low |
| AUD-040–042 | Trait introduction | Medium | Medium |
| AUD-052–053 | Copy mode / renderer tests | Medium | Medium-High |
| AUD-060–061 | Platform consolidation / magic numbers | Low | Low |
