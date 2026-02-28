# Audit Remediation Report

> **Project**: par-term
> **Audit Date**: 2025-02-28
> **Remediation Date**: 2025-02-28
> **Severity Filter Applied**: all

---

## Execution Summary

| Phase | Status | Agent | Issues Targeted | Resolved | Partial | Manual |
|-------|--------|-------|----------------|----------|---------|--------|
| 1 — Critical Security | Skipped | — | 0 | 0 | 0 | 0 |
| 2 — Critical Architecture | Done | fix-architecture | 5 | 5 | 0 | 0 |
| 3a — Security (remaining) | Skipped | — | 0 | 0 | 0 | 0 |
| 3b — Architecture (remaining) | Done | fix-architecture | 5 | 5 | 0 | 0 |
| 3c — Code Quality | Done | fix-code-quality | 3 | 3 | 0 | 0 |
| 3d — Documentation | Skipped | — | 0 | 0 | 0 | 0 |
| 4 — Verification | Done | — | — | — | — | — |

**Overall**: 13 issues resolved, 0 partial, 0 require manual intervention.
17 issues remain for future work (see below).

---

## Resolved Issues

### Architecture — Phase 2

- **[AUD-003]** WindowState EguiState extraction — `src/app/window_state/mod.rs` — Extracted
  `egui_ctx`, `egui_state`, `pending_egui_events`, `egui_initialized` into `EguiState` sub-struct.
  Continues ARC-001 decomposition. (7 files updated)

- **[AUD-030]** active_tab() helper extraction — `src/app/window_state/impl_helpers.rs` — Added
  `with_active_tab()` and `with_active_tab_mut()` helpers. Converted 20+ call sites across
  `scroll_ops.rs` and `copy_mode_handler.rs`.

- **[AUD-031]** Terminal lock helpers — `src/tab/mod.rs` — Added `try_with_terminal()` and
  `try_with_terminal_mut()` on Tab. Converted 7 call sites across 4 files.

- **[AUD-032]** request_redraw() unification — `src/app/window_state/impl_helpers.rs` — Unified
  28 inline `if let Some(window) = &self.window { window.request_redraw() }` patterns to
  `self.request_redraw()` across 9 files.

- **[AUD-033]** with_window() helper — `src/app/window_state/impl_helpers.rs` — Added
  `with_window()` helper for single-operation window access.

### Architecture — Phase 3b (File Decomposition)

- **[AUD-010]** agent_messages.rs split — `src/app/window_state/agent_messages.rs` (1,006 → 660 lines) —
  Extracted `agent_config.rs` (326 lines) and `agent_screenshot.rs` (45 lines).

- **[AUD-013]** url_detection.rs split — `src/url_detection.rs` (946 → 589 lines) — Extracted
  24 test functions into `url_detection/tests.rs` (358 lines).

- **[AUD-017]** copy_mode.rs split — `src/copy_mode.rs` (857 → 637 lines) — Extracted
  12 test functions into `copy_mode/tests.rs` (221 lines).

- **[AUD-021]** triggers.rs split — `src/app/triggers.rs` (832 → 430 lines) — Extracted
  `mark_line.rs` (90 lines), `prettify.rs` (295 lines), `sound.rs` (66 lines).

- **[AUD-022]** scripting.rs split — `src/app/window_manager/scripting.rs` (820 → 620 lines) —
  Extracted `config_change.rs` (220 lines).

### Code Quality — Phase 3c

- **[AUD-051]** Keybindings test coverage — `par-term-keybindings/tests/keybinding_integration_tests.rs` —
  Added 73 integration tests covering parse/registry/lookup pipeline, all modifier combinations,
  key aliases, physical keys, error cases, key_combo_to_bytes, and display formatting. (0% → comprehensive)

- **[AUD-052]** Copy mode tests — `tests/copy_mode_tests.rs` — Added 97 integration tests
  covering state machine transitions, cursor motions, visual modes, selection computation,
  marks, search, viewport helpers, word motions, and edge cases. (0% → comprehensive)

- **[AUD-061]** Magic numbers → constants — `src/ui_constants.rs` — Extracted
  `DRAG_THRESHOLD_PX`, `CLICK_RESTORE_THRESHOLD_PX`, `SCROLLBAR_MARK_HIT_RADIUS_PX`,
  `VISUAL_BELL_FLASH_DURATION_MS` from inline literals and deduplicated across 6 files.

---

## Requires Manual Intervention

These items are too large or architecturally complex for automated remediation.
They require human design decisions and incremental implementation.

### [AUD-001] Config Struct Decomposition (340 fields)
- **Why**: Splitting 340 fields into sub-structs with `#[serde(flatten)]` affects every file
  that reads config — potentially 100+ call sites across all sub-crates. Requires careful
  backward-compatibility testing of YAML serialization.
- **Recommended approach**: Implement one sub-struct at a time (start with `FontConfig`),
  verify serde round-trip, then proceed to the next group.
- **Estimated effort**: Large (multi-session)

### [AUD-002] Tab LEGACY Field Migration (4 fields, ~49 call sites)
- **Why**: Each LEGACY field requires migrating call sites to route through PaneManager
  when in split-pane mode. Selection (`mouse`) was partially done in PR #210. Remaining
  fields (`scroll_state`, `cache`, `prettifier_pipeline`) have cross-cutting dependencies.
- **Recommended approach**: Follow the migration order in AUDIT.md: scroll_state → cache →
  prettifier_pipeline → mouse (remaining non-selection fields).
- **Estimated effort**: Medium per field (4 sessions total)

### [AUD-050] app/ Module Test Coverage
- **Why**: Testing mouse events, input handling, and render pipeline requires mocking
  `WindowState` with a renderer, window, and terminal — heavyweight integration test
  infrastructure that doesn't exist yet.
- **Recommended approach**: First implement AUD-040 (TerminalAccess trait) to enable
  mock terminals, then build test harness incrementally.
- **Estimated effort**: Large (ongoing)

### [AUD-040] TerminalAccess Trait
- **Why**: Requires agreement on trait boundary and method set. Must not break the
  `Arc<tokio::sync::RwLock<TerminalManager>>` pattern used throughout.
- **Recommended approach**: Define trait with the 5 methods listed in AUDIT.md,
  impl for TerminalManager, then migrate consumers incrementally.
- **Estimated effort**: Medium

### [AUD-041] UIElement Trait
- **Why**: Requires architectural agreement on lifecycle methods. TabBarUI, StatusBarUI,
  and overlay panels have different init/draw signatures.
- **Estimated effort**: Medium

### [AUD-042] EventHandler Trait
- **Why**: Low impact relative to effort. Mouse/keyboard/window handlers have
  fundamentally different signatures.
- **Estimated effort**: Medium

### [AUD-053] Renderer Test Coverage
- **Why**: GPU rendering code requires either a headless GPU context or significant
  refactoring to separate data preparation from GPU calls.
- **Estimated effort**: Large

### Remaining Large File Splits (AUD-011, 012, 014-016, 018-020, 023-024)
- **Why**: Prettifier renderers (011, 016, 018, 019, 023) share internal structure
  that benefits from coordinated refactoring. Render pipeline files (014, 020) are
  tightly coupled to the three-pass rendering architecture.
- **Estimated effort**: Low each, but many files

### [AUD-060] Platform Code Consolidation
- **Why**: Scattered `#[cfg(target_os)]` blocks are low impact. A `platform/` module
  would add indirection without strong benefit until the codebase grows further.
- **Estimated effort**: Low

### [AUD-062] Legacy Field Cleanup
- **Why**: Blocked by AUD-002 (Tab LEGACY migration). Can only be cleaned up after
  all call sites are migrated.
- **Estimated effort**: Low (after AUD-002 is done)

---

## Verification Results

- Format: Pass
- Lint (clippy): Pass (1 `pub(self)` warning fixed during verification)
- Tests: Pass (1,033 unit tests + 170 new integration tests = 1,203+ total)
- Build: Pass

---

## Files Changed

### Created (8 new files)
- `src/app/window_state/egui_state.rs`
- `src/app/window_state/agent_config.rs`
- `src/app/window_state/agent_screenshot.rs`
- `src/app/triggers/mark_line.rs`
- `src/app/triggers/prettify.rs`
- `src/app/triggers/sound.rs`
- `src/app/window_manager/scripting/config_change.rs`
- `par-term-keybindings/tests/keybinding_integration_tests.rs`

### Created (module conversions — flat file → directory)
- `src/url_detection/mod.rs` + `src/url_detection/tests.rs`
- `src/copy_mode/mod.rs` + `src/copy_mode/tests.rs`
- `src/app/triggers/mod.rs` (was `src/app/triggers.rs`)
- `src/app/window_manager/scripting/mod.rs` (was `src/app/window_manager/scripting.rs`)

### Created (test files)
- `tests/copy_mode_tests.rs`

### Modified (20 files)
- `src/app/copy_mode_handler.rs`
- `src/app/handler/window_state_impl/about_to_wait.rs`
- `src/app/handler/window_state_impl/handle_window_event.rs`
- `src/app/handler/window_state_impl/shell_exit.rs`
- `src/app/keyboard_handlers.rs`
- `src/app/mouse_events/clipboard_image_guard.rs`
- `src/app/mouse_events/coords.rs`
- `src/app/mouse_events/mouse_button.rs`
- `src/app/mouse_events/mouse_move.rs`
- `src/app/mouse_events/mouse_wheel.rs`
- `src/app/renderer_init.rs`
- `src/app/scroll_ops.rs`
- `src/app/tab_ops/lifecycle.rs`
- `src/app/tmux_handler/notifications/session.rs`
- `src/app/window_manager/menu_actions.rs`
- `src/app/window_state/agent_messages.rs`
- `src/app/window_state/impl_helpers.rs`
- `src/app/window_state/impl_init.rs`
- `src/app/window_state/mod.rs`
- `src/app/window_state/render_pipeline/mod.rs`
- `src/pane/types/pane.rs`
- `src/tab/mod.rs`
- `src/tab/profile_tracking.rs`
- `src/ui_constants.rs`

---

## Next Steps

1. Review `Requires Manual Intervention` items and create GitHub issues for each
2. Start with **AUD-001** (Config decomposition) as it has the highest impact
3. **AUD-002** (Tab LEGACY migration) can proceed in parallel — `scroll_state` first
4. **AUD-040** (TerminalAccess trait) unblocks **AUD-050** (app/ test coverage)
5. Re-run `/audit` after completing the manual items to get an updated assessment
