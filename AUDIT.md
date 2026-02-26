# par-term Project Audit

**Project**: par-term v0.23.0 -- Cross-platform GPU-accelerated terminal emulator
**Date**: 2026-02-26
**Scope**: Architecture, design patterns, security, code quality, documentation
**Codebase**: ~79,000 lines across 164 Rust source files, 13 workspace sub-crates

---

## Executive Summary

par-term is a well-architected Rust terminal emulator with a clean 13-crate workspace, GPU-accelerated rendering via wgpu, and comprehensive feature set including inline graphics, custom shaders, ACP agents, tmux integration, and split panes. The project demonstrates strong competence in GPU pipeline design, async I/O, and cross-platform development.

Rust's memory safety guarantees eliminate entire vulnerability classes. The codebase shows security awareness in several areas: proper shell quoting, HTML escaping, path canonicalization for ACP agents, credential leak prevention, and zip path traversal protection.

32 findings have been resolved across five phases. The most pressing remaining issues center around:
- **God Object**: `WindowState` (6,508 lines, ~90 fields, impl across 16 files)
- **Monolithic render function**: `render()` at 3,462 lines
- **Documentation gaps**: No centralized config reference, rustdoc coverage below target in several crates

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
| H11 | Docs | No unified configuration reference -- 386 config fields scattered across 30+ docs | `par-term-config/src/config/mod.rs` |
| H12 | Docs | `par-term-acp` rustdoc coverage ~32%; `par-term-config` coverage ~44% | Sub-crate source files |

### Medium

| # | Category | Finding | Location |
|---|----------|---------|----------|
| M1 | Security | Trigger `RunCommand` can be fired by malicious terminal output matching broad patterns | `src/app/triggers.rs` |
| M3 | Security | Config variable substitution resolves any `${VAR}` from environment -- info leak risk | `par-term-config/src/config/mod.rs` |
| M5 | Security | Unsafe cell pointer leak in split-pane render -- memory leak on panic | `src/app/window_state.rs` |
| M6 | Security | Scripting protocol defines `WriteText`/`RunCommand` (unimplemented) -- needs security model when added | `par-term-scripting/src/protocol.rs` |
| M7 | Architecture | Dual-mutex hierarchy with 150 `try_lock()` calls -- silent skip on contention, undocumented which are safe | `src/app/` (16 files) |
| M9 | Architecture | Missing typed errors -- only prettifier uses `thiserror`; rest relies on `anyhow` strings | `src/prettifier/traits.rs` |
| M10 | Architecture | Monolithic `Config` struct (~200+ fields, 3,157 lines) with no internal grouping | `par-term-config/src/config/mod.rs` |
| M12 | Code Quality | 680 `unwrap()` calls; `self.window.as_ref().unwrap()` in render path would crash with no context | `src/app/window_state.rs` |
| M13 | Code Quality | `Tab` struct has 40+ public fields including "legacy" fields that conflict with pane-based state | `src/tab/mod.rs` |
| M15 | Code Quality | Cell vector cloned on every render frame (cache hit + cache store) -- double allocation | `src/app/window_state.rs` |
| M21 | Docs | `par-term-terminal` rustdoc coverage ~56% | Sub-crate source |

### Low

| # | Category | Finding | Location |
|---|----------|---------|----------|
| L1 | Security | Clipboard paste without control character sanitization (standard terminal behavior) | `src/clipboard_history_ui.rs` |
| L3 | Security | MCP IPC files permissions not explicitly restrictive | `par-term-mcp/src/lib.rs` |
| L5 | Security | Session logger captures raw I/O including passwords at prompts | `src/session_logger.rs` |
| L11 | Code Quality | No doc-tests (0 documented examples in `cargo test` output) | Project-wide |

### Info (Positive Findings)

| # | Category | Finding | Location |
|---|----------|---------|----------|
| I1 | Architecture | 13-crate workspace is a clean DAG with no circular dependencies | `Cargo.toml` workspace |
| I2 | Architecture | Three-pass GPU pipeline (cells, graphics, egui) with dirty tracking is well-designed | `par-term-render/` |
| I3 | Architecture | Input pipeline cleanly layered: winit -> keybindings -> hardcoded shortcuts -> VT sequences | `par-term-input/`, `par-term-keybindings/` |
| I4 | Security | ACP agent write paths canonicalized and restricted to safe directories | `par-term-acp/src/agent.rs` |
| I5 | Security | Sensitive commands redacted from auto-context sent to AI agents | `src/app/window_state.rs` |
| I6 | Security | Dynamic profile fetcher refuses auth headers over plain HTTP | `src/profile/dynamic.rs` |
| I7 | Security | Zip extraction uses `enclosed_name()` for path traversal protection | `par-term-update/src/self_updater.rs` |
| I8 | Security | TLS uses platform certificate verifier (correct for production) | `par-term-update/src/http.rs`, `src/http.rs` |
| I9 | Security | Proper process cleanup via `Drop` for `ScriptProcess` and `ScriptManager` | `par-term-scripting/src/process.rs` |
| I10 | Code Quality | All tests pass; 0 clippy warnings | `cargo test`, `cargo clippy` |
| I11 | Code Quality | `parking_lot::Mutex` used project-wide (no poisoning) with documented lock discipline | CLAUDE.md, MEMORY.md |
| I12 | Docs | 38 markdown docs in `docs/` following consistent style guide with Mermaid diagrams | `docs/` |
| I13 | Docs | CLAUDE.md is exceptional developer documentation with workflows, gotchas, crate dependency graph | `CLAUDE.md` |
| I14 | Docs | All `unsafe` blocks have `// SAFETY:` comments | Project-wide |
| I15 | Docs | Only 1 TODO in entire codebase -- excellent code hygiene | `src/app/window_manager.rs` |

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

#### 1.3 Design Patterns Identified

| Pattern | Where Used | Assessment |
|---------|-----------|------------|
| Workspace/Crate Module | 13 sub-crates | Excellent -- clean DAG |
| Observer | `par-term-scripting`, shell lifecycle | Appropriate |
| State Machine | Copy mode, tmux prefix | Good fit |
| Command | `TabBarAction`, `StatusBarAction`, `MenuAction` | Good decoupling |
| Builder (partial) | `RendererInitParams`, `PaneManager` | Incomplete -- should be extended |
| Proxy/Facade | Re-export modules | Appropriate for migration |
| LRU Cache | Glyph atlas, text shaping cache | Performance-appropriate |
| RAII/Drop | Tab, Pane, SessionLogger, SystemMonitor | Well-implemented with fast-shutdown flag |
| Tree Structure | `PaneNode` for pane splitting | Appropriate for arbitrary nesting |

#### 1.4 Missing Typed Errors (M9)

Only the prettifier subsystem defines `thiserror` error types. The rest relies on `anyhow::Result`. Sub-crates (`par-term-config`, `par-term-render`) would benefit from typed errors for config parsing failures, GPU errors, and terminal failures so callers can match on specific variants.

---

### 2. Security Assessment

#### 2.1 Positive Security Findings

- ACP agent file writes are canonicalized and restricted to safe root directories (`/tmp`, `shaders_dir`, `config_dir`)
- Sensitive commands (password, token, secret, key, apikey, auth, credential) are redacted from AI agent context
- Dynamic profile fetcher blocks auth headers over plain HTTP
- Zip extraction uses `enclosed_name()` for path traversal protection
- TLS uses platform certificate verifier
- `ScriptProcess` and `ScriptManager` implement `Drop` for proper cleanup

---

### 3. Code Quality

#### 3.1 Monolithic Render Function (C1)

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

#### 3.2 Production unwrap() Calls (M12)

680 `unwrap()` calls across the codebase. Most concerning: `self.window.as_ref().unwrap()` in the render path would crash with no useful error message. Either make `window` non-optional (require at construction) or use early-return patterns.

#### 3.3 Cell Cloning in Render Path (M15)

The cell vector (cols * rows elements) is cloned on every cache hit and cache store, doubling allocation cost. Consider `Arc<Vec<Cell>>` or a double-buffer pattern.

---

### 4. Documentation

#### 4.1 Overall Assessment: A-

The project has exceptionally strong documentation for a Rust project of its size: 38 markdown files in `docs/`, a comprehensive README, detailed ARCHITECTURE.md with Mermaid diagrams, a documentation style guide, CONTRIBUTING.md, SECURITY.md, GETTING_STARTED.md, and TROUBLESHOOTING.md. All `unsafe` blocks have SAFETY comments. Only 1 TODO in the entire codebase.

#### 4.2 Rustdoc Coverage by Crate

| Crate | Coverage | Assessment |
|-------|----------|------------|
| par-term (main) | ~84% | Good |
| par-term-scripting | ~89% | Good |
| par-term-tmux | ~88% | Good |
| par-term-ssh | ~85% | Good |
| par-term-settings-ui | ~73% | Acceptable |
| par-term-update | ~72% | Acceptable |
| par-term-input | ~71% | Acceptable |
| par-term-render | ~69% | Needs improvement |
| par-term-mcp | ~60% | Needs improvement |
| par-term-keybindings | ~58% | Needs improvement |
| par-term-terminal | ~56% | Needs improvement |
| par-term-fonts | ~52% | Needs improvement |
| par-term-config | ~44% | Poor -- this is the user-facing config crate |
| par-term-acp | ~32% | Poor -- architecturally complex crate |

#### 4.3 Remaining Documentation Gaps

- **Centralized config reference** (H11): 386 config fields scattered across 30+ docs with no single reference.
- **Rustdoc coverage** (H12, M21): Several crates below 70% target.

---

## Remediation Roadmap

### Phase 3: Structural Refactoring (2-4 weeks)

- [ ] **C2**: Extract `AgentState`, `ShaderState`, `TmuxState`, `OverlayUiState` from `WindowState`
- [ ] **C1**: Break `render()` into coordinator calling focused sub-methods
- [ ] **M13**: Restrict `Tab` field visibility, deprecate legacy fields

### Phase 4: Documentation (1-2 weeks)

- [ ] **H11**: Create `docs/CONFIG_REFERENCE.md` or add rustdoc to all Config fields
- [ ] **H12**: Improve rustdoc coverage for `par-term-acp` (32% -> 70%+) and `par-term-config` (44% -> 70%+)
- [ ] **M21**: Improve `par-term-terminal` rustdoc to 70%+

### Phase 5: Ongoing (as encountered)

- [ ] **M9**: Add `thiserror` error types to `par-term-config` and `par-term-render`
- [ ] **M12**: Audit `unwrap()` calls in storage/I/O paths; make `window` non-optional
- [ ] **M15**: Investigate `Arc<Vec<Cell>>` or double-buffering for cell cache
- [ ] **L11**: Add doc-tests to key public API items in sub-crates

---

## Testing & Tooling Status

| Metric | Value | Assessment |
|--------|-------|------------|
| Total tests | 1,507 | Strong |
| Test pass rate | 100% | Excellent |
| Clippy warnings | 0 | Excellent |
| Unit tests | 999 | Good coverage |
| Integration tests | 508+ across 26 files | Good |
| Doc-tests | 0 | Gap |
| `unsafe` blocks documented | 100% | Excellent |
| TODO/FIXME count | 1 | Excellent hygiene |
