# par-term Project Audit

**Project**: par-term v0.23.0 -- Cross-platform GPU-accelerated terminal emulator
**Date**: 2026-02-26
**Scope**: Architecture, design patterns, security, code quality, documentation
**Codebase**: ~79,000 lines across 164 Rust source files, 13 workspace sub-crates

---

## Executive Summary

par-term is a well-architected Rust terminal emulator with a clean 13-crate workspace, GPU-accelerated rendering via wgpu, and comprehensive feature set including inline graphics, custom shaders, ACP agents, tmux integration, and split panes. The project demonstrates strong competence in GPU pipeline design, async I/O, and cross-platform development.

51 findings have been resolved across seven phases. The most pressing remaining issues center around:
- **God Object**: `WindowState` (6,508 lines, ~90 fields, impl across 16 files)
- **Monolithic render function**: `render()` at 3,462 lines
- **Dual-mutex contention**: 150 `try_lock()` calls with silent skip on contention

All tests pass, clippy produces 0 warnings, and the overall architecture is sound. The issues identified are evolutionary -- the kind that accumulate in a fast-moving, feature-rich project.

---

## Remaining Findings

### Critical

| # | Category | Finding | Location |
|---|----------|---------|----------|
| C1 | Code Quality | `render()` function is 3,462 lines -- the longest function in the codebase | `src/app/window_state.rs` |
| C2 | Architecture | `WindowState` is a god object: ~90 fields, 6,508 lines, impl across 16 files (~21,600 lines total) | `src/app/window_state.rs` |

### High

| # | Category | Finding | Location |
|---|----------|---------|----------|
| H9 | Architecture | 47 files exceed 500-line target; 28 files exceed 800-line refactor threshold | See table in Section 1.2 |

### Medium

| # | Category | Finding | Location |
|---|----------|---------|----------|
| M7 | Architecture | Dual-mutex hierarchy with 150 `try_lock()` calls -- silent skip on contention, undocumented which are safe | `src/app/` (16 files) |
| M12 | Code Quality | ~678 remaining `unwrap()` calls in non-render paths -- no panic context on failure | Project-wide |

---

## Detailed Findings

### 1. Architecture & Design Patterns

#### 1.1 WindowState God Object (C2)

`WindowState` holds ~90 fields spanning renderer state, tab management, input handling, 15+ UI overlays, ACP agent state, tmux integration, shader watching, clipboard, cursor animation, file transfers, and more. Its `impl` blocks span 16 files totaling ~21,600 lines.

**Recommendation**: Extract cohesive subsystems into owned sub-structs:
- `AgentState` -- agent, agent_rx, agent_tx, agent_client, pending_send_handles, agent_skill_*
- `TmuxState` -- tmux_session, tmux_sync, tmux_pane_to_native_pane, etc.
- `OverlayUiState` -- help_ui, clipboard_history_ui, search_ui, and 12 more panels
- `CursorAnimState` -- cursor_opacity, last_cursor_blink, last_key_press, cursor_blink_timer
- `ShaderState` -- shader_watcher, shader_metadata_cache, cursor_shader_metadata_cache, shader_reload_error

#### 1.2 Oversized Files (H9)

The project guideline says "Keep files under 500 lines; refactor files exceeding 800 lines." The worst offenders:

| Lines | File |
|-------|------|
| 6,508 | `src/app/window_state.rs` |
| 3,157 | `par-term-config/src/config/mod.rs` |
| 3,022 | `src/app/window_manager.rs` |
| 2,804 | `par-term-render/src/renderer/mod.rs` |
| 2,361 | `src/tab_bar_ui.rs` |
| 2,318 | `src/app/tmux_handler.rs` |
| 2,315 | `src/prettifier/renderers/markdown.rs` |
| 2,145 | `src/app/input_events.rs` |
| 2,138 | `par-term-render/src/cell_renderer/render.rs` |
| 1,778 | `src/ai_inspector/panel.rs` |
| 1,743 | `src/pane/manager.rs` |
| 1,704 | `src/app/handler.rs` |
| 1,638 | `src/prettifier/renderers/diff.rs` |
| 1,633 | `src/tab/mod.rs` |

---

### 2. Code Quality

#### 2.1 Monolithic Render Function (C1)

The `render()` method at 3,462 lines handles FPS throttling, scroll animation, tab titles, font rebuilds, resize, cell generation, ACP agent message processing, cursor animation, all egui overlays (15+ panels), and GPU submission.

**Recommendation**: Extract into a coordinator:
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

#### 2.2 Remaining unwrap() Calls (M12)

~678 `unwrap()` calls remain across the codebase. The highest-risk render-path panics (`self.window.as_ref().unwrap()`) have been fixed with early-return guards. The remaining calls are in storage, I/O, and initialization paths -- lower risk but still provide no error context on failure. Audit and convert to `expect("context")` or proper error propagation as encountered.

---

### 3. Documentation

#### 3.1 Rustdoc Coverage by Crate

| Crate | Coverage | Assessment |
|-------|----------|------------|
| par-term (main) | ~84% | Good |
| par-term-scripting | ~89% | Good |
| par-term-tmux | ~88% | Good |
| par-term-ssh | ~85% | Good |
| par-term-acp | ~85% | Good |
| par-term-settings-ui | ~73% | Acceptable |
| par-term-update | ~72% | Acceptable |
| par-term-input | ~71% | Acceptable |
| par-term-terminal | ~100% | Good |
| par-term-config | ~75% | Good |
| par-term-render | ~69% | Needs improvement |
| par-term-mcp | ~60% | Needs improvement |
| par-term-keybindings | ~58% | Needs improvement |
| par-term-fonts | ~52% | Needs improvement |

---

## Remediation Roadmap

### Phase 3: Structural Refactoring (2-4 weeks)

- [ ] **C2**: Extract `AgentState`, `ShaderState`, `TmuxState`, `OverlayUiState` from `WindowState`
- [ ] **C1**: Break `render()` into coordinator calling focused sub-methods
- [x] **M13**: Restrict `Tab` field visibility, deprecate legacy fields *(done: 42 fields restricted)*

### Phase 5: Ongoing (as encountered)

- [x] **M9**: Add `thiserror` error types to `par-term-config` and `par-term-render` *(done)*
- [x] **M15**: `Arc<Vec<Cell>>` for cell cache -- eliminates one full Vec allocation per cache-hit frame *(done)*
- [ ] **M12**: Continue auditing remaining ~678 `unwrap()` calls; convert to `expect()` with context
- [x] **L11**: Add doc-tests to key public API items in sub-crates *(done: 20+ doc-tests added)*
