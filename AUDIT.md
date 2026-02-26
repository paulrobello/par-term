# par-term Project Audit

**Project**: par-term v0.23.0 -- Cross-platform GPU-accelerated terminal emulator
**Date**: 2026-02-26
**Scope**: Architecture, design patterns, security, code quality, documentation
**Codebase**: ~79,000 lines across 164 Rust source files, 13 workspace sub-crates

---

## Executive Summary

par-term is a well-architected Rust terminal emulator with a clean 13-crate workspace, GPU-accelerated rendering via wgpu, and comprehensive feature set including inline graphics, custom shaders, ACP agents, tmux integration, and split panes. The project demonstrates strong competence in GPU pipeline design, async I/O, and cross-platform development.

57 findings have been resolved across eight phases. The remaining issues are ongoing refactoring work:
- **God Object (partial)**: `WindowState` reduced from ~90 to ~82 top-level fields; `CursorAnimState` and `ShaderState` extracted. `AgentState`, `TmuxState`, `OverlayUiState` still to extract.
- **Monolithic render function (partial)**: `render()` reduced from 3,462 → 2,482 lines (-28%). Further extraction ongoing.
- **Oversized files**: Most major files split. `window_state.rs` and `window_manager.rs` remain large pending further C2 work.

All tests pass, clippy produces 0 new warnings, and the overall architecture is sound.

---

## Remaining Findings

### Critical

| # | Category | Finding | Location |
|---|----------|---------|----------|
| C2 | Architecture | `WindowState` still has ~82 top-level fields. `AgentState`, `TmuxState`, `OverlayUiState` not yet extracted. | `src/app/window_state.rs` |
| C1 | Code Quality | `render()` still 2,482 lines after initial decomposition (-28%). Further extraction needed. | `src/app/window_state.rs` |

### High

| # | Category | Finding | Location |
|---|----------|---------|----------|
| H9 | Architecture | `window_state.rs` (~6,300L) and `window_manager.rs` (~3,022L) still exceed thresholds; will reduce as C2 progresses | `src/app/` |

---

## Detailed Findings

### 1. Architecture & Design Patterns

#### 1.1 WindowState God Object (C2 — partial)

`CursorAnimState` (4 fields) and `ShaderState` (6 fields) have been extracted. Still pending:
- `AgentState` -- agent, agent_rx, agent_tx, agent_client, pending_send_handles, agent_skill_*
- `TmuxState` -- tmux_session, tmux_sync, tmux_pane_to_native_pane, etc.
- `OverlayUiState` -- help_ui, clipboard_history_ui, search_ui, and 12 more panels

#### 1.2 Oversized Files (H9 — mostly resolved)

Files split in this phase:

| Before | After |
|--------|-------|
| `par-term-config/src/config/mod.rs` (3,157L) | `mod.rs` (31L) + `config_struct.rs` (2,201L) + `config_methods.rs` (929L) + `env_vars.rs` (134L) + `acp.rs` (47L) |
| `par-term-render/src/renderer/mod.rs` (2,829L) | `mod.rs` (556L) + `rendering.rs` (1,285L) + `accessors.rs` (601L) + `state.rs` (414L) |
| `par-term-render/src/cell_renderer/render.rs` (2,138L) | `render.rs` (450L) + `instance_buffers.rs` (1,016L) + `pane_render.rs` (679L) |
| `src/app/handler.rs` (1,717L) | `handler/window_state_impl.rs` (1,301L) + `app_handler_impl.rs` (427L) |
| `src/app/input_events.rs` (2,171L) | `input_events/key_handler.rs` (1,326L) + `keybinding_actions.rs` (865L) |
| `src/app/tmux_handler.rs` (2,359L) | `tmux_handler/notifications.rs` (1,545L) + `gateway.rs` (828L) |
| `src/prettifier/renderers/markdown.rs` (2,315L) | `markdown.rs` + `markdown_blocks.rs` + `markdown_highlight.rs` + `markdown_inline.rs` |
| `src/prettifier/renderers/diff.rs` (1,638L) | `diff.rs` + `diff_parser.rs` + `diff_word.rs` |
| `src/ai_inspector/panel.rs` (1,778L) | `panel.rs` + `panel_helpers.rs` |
| `src/pane/manager.rs` (1,743L) | `manager.rs` + `tmux_helpers.rs` |
| `src/tab_bar_ui.rs` (2,361L) | `tab_bar_ui/mod.rs` + `title_utils.rs` |

Remaining above 800-line threshold: `window_state.rs`, `window_manager.rs`, and some split sub-files. These will decrease as C2 extraction continues.

---

### 2. Code Quality

#### 2.1 Monolithic Render Function (C1 — partial)

Extracted in this phase:
- `process_agent_messages_tick()` — 524 lines (agent message dispatch, config updates, AI inspector refresh)
- `handle_tab_bar_action_after_render()` — 118 lines
- `handle_clipboard_history_action_after_render()` — 25 lines
- `handle_inspector_action_after_render()` — 323 lines

`render()` reduced from 3,462 → 2,482 lines (-28%). Continue extracting phases (FPS throttle, scroll animation, cell generation, egui overlays, GPU submission) into focused sub-methods.

#### 2.2 unwrap() Calls (M12 — resolved for all paths except window_state)

Converted in this phase:
- **Sub-crates** (par-term-tmux, par-term-settings-ui, par-term-fonts): 9 calls
- **par-term-render**: 23 calls across 5 files
- **src/app/ (non-window_state)**: 3 calls
- **src/ non-app** (prettifier, pane, ai_inspector, search, snippets, etc.): 123 calls

Remaining: `unwrap()` calls in `src/app/window_state.rs` and `window_manager.rs` — audit as part of ongoing C2 work.

---

### 3. Architecture: try_lock() Documentation (M7 — resolved)

85 `try_lock()` calls documented across 12 files in `src/app/` with comments explaining:
- Why `try_lock()` is used instead of blocking
- What happens when contention occurs (frame skipped, UI not updated, etc.)

1 FIXME flagged in `tmux_handler/notifications.rs` (`handle_tmux_session_ended`) where a missed lock could leave the terminal in tmux control mode indefinitely.

---

### 4. Documentation

#### 4.1 Rustdoc Coverage by Crate

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

### Phase 3: Structural Refactoring (ongoing)

- [x] **C2 partial**: Extract `CursorAnimState`, `ShaderState` from `WindowState` *(done)*
- [ ] **C2 remaining**: Extract `AgentState`, `TmuxState`, `OverlayUiState` from `WindowState`
- [x] **C1 partial**: Break `render()` into coordinator + 4 sub-methods (-28%) *(done)*
- [ ] **C1 remaining**: Continue extracting render phases until render() is ~50 lines
- [x] **M13**: Restrict `Tab` field visibility, deprecate legacy fields *(done: 42 fields restricted)*

### Phase 5: Ongoing (as encountered)

- [x] **M9**: Add `thiserror` error types to `par-term-config` and `par-term-render` *(done)*
- [x] **M15**: `Arc<Vec<Cell>>` for cell cache *(done)*
- [x] **M12**: Convert ~678 `unwrap()` calls to `expect()` with context *(done except window_state area)*
- [x] **M7**: Document all 150 `try_lock()` calls in `src/app/` *(done: 85 documented, 1 FIXME)*
- [x] **H9**: Split major oversized files into focused sub-modules *(done for most; window_state pending C2)*
- [x] **L11**: Add doc-tests to key public API items in sub-crates *(done: 20+ doc-tests added)*
