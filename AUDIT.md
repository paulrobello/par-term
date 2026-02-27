# par-term Project Audit

**Project**: par-term v0.23.0 -- Cross-platform GPU-accelerated terminal emulator
**Date**: 2026-02-26
**Scope**: Architecture, design patterns, security, code quality, documentation
**Codebase**: ~79,000 lines across 164 Rust source files, 13 workspace sub-crates

---

## Executive Summary

par-term is a well-architected Rust terminal emulator with a clean 13-crate workspace, GPU-accelerated rendering via wgpu, and comprehensive feature set including inline graphics, custom shaders, ACP agents, tmux integration, and split panes. The project demonstrates strong competence in GPU pipeline design, async I/O, and cross-platform development.

57 findings have been resolved. All tests pass, clippy produces 0 new warnings, and the overall architecture is sound.

---

## Remaining Findings

### Critical

| # | Category | Finding | Location |
|---|----------|---------|----------|
| C1 | Code Quality | `render()` still 2,482 lines after initial decomposition (-28%). Further extraction needed. | `src/app/window_state.rs` |

### High

| # | Category | Finding | Location |
|---|----------|---------|----------|
| H9 | Architecture | `window_state.rs` (~6,348L) and `window_manager.rs` (~3,022L) still exceed thresholds; C2 is resolved but C1 extraction remains | `src/app/` |

---

## Detailed Findings

### C2 — WindowState God Object (RESOLVED)

All three sub-struct extractions are complete:
- `CursorAnimState` (4 fields) — extracted earlier
- `ShaderState` (6 fields) — extracted earlier
- `AgentState` (10 fields) — `src/app/agent_state.rs`, accessed as `self.agent_state.*`
- `TmuxState` (7 fields) — `src/app/tmux_state.rs`, accessed as `self.tmux_state.*`
- `OverlayUiState` (20 fields) — `src/app/overlay_ui_state.rs`, accessed as `self.overlay_ui.*`

`WindowState` now has ~47 top-level fields (down from ~82). All tests pass, clippy is clean.

### C1 — Monolithic render() (partial)

Already extracted: `process_agent_messages_tick()` (524L), `handle_tab_bar_action_after_render()` (118L), `handle_clipboard_history_action_after_render()` (25L), `handle_inspector_action_after_render()` (323L).

`render()` is still 2,482 lines. Continue extracting phases (FPS throttle, scroll animation, cell generation, egui overlays, GPU submission) until render() is ~50 lines:

```rust
pub(crate) fn render(&mut self) {
    if self.is_shutting_down { return; }
    if !self.should_render_frame() { return; }
    self.update_frame_metrics();
    self.update_animations();
    self.sync_layout();
    let render_data = self.gather_render_data();
    self.process_agent_messages();
    let egui_output = self.render_egui_overlays(&render_data);
    self.submit_gpu_render(render_data, egui_output);
    self.update_post_render_state();
}
```

### H9 — Oversized Files (partial)

Remaining above 800-line threshold (will decrease as C2 extraction continues):
- `src/app/window_state.rs` (~6,300L)
- `src/app/window_manager.rs` (~3,022L)
- Several split sub-files still over threshold (e.g. `key_handler.rs` 1,326L, `notifications.rs` 1,545L, `rendering.rs` 1,285L)

---

## Remediation Roadmap

- [x] **C2**: Extract `AgentState`, `TmuxState`, `OverlayUiState` from `WindowState` — COMPLETE
- [ ] **C1**: Continue extracting render phases until render() is ~50 lines
- [ ] **M12**: Convert remaining `unwrap()` calls in `window_state.rs` / `window_manager.rs` to `expect()` with context
- [ ] **H9**: Further split oversized sub-files; tackle `window_manager.rs`
