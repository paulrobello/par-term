# Project Audit Report

> **Project**: par-term
> **Date**: 2026-04-30
> **Stack**: Rust (Edition 2024), wgpu (GPU rendering), tokio (async runtime), egui (settings UI), 13 sub-crates
> **Audited by**: Claude Code Audit System

---

## Executive Summary

par-term is a well-engineered GPU-accelerated terminal emulator with a mature codebase (~300K+ LOC across 13 crates). The project demonstrates strong architectural decomposition progress, comprehensive documentation breadth, and disciplined security defaults. The most critical findings center on the self-update system bypassing macOS Gatekeeper verification (SEC-001), bypassable command denylist when auto-execution is enabled (SEC-002), and event-loop-blocking `thread::sleep` calls that freeze the entire UI (QA-002). The `WindowState` God Object (ARC-001) and `Config` struct monolith (ARC-002) remain the highest-value architectural targets, with active extraction already underway. Estimated remediation effort for critical and high issues is 2–3 sprints.

### Issue Count by Severity

| Severity | Architecture | Security | Code Quality | Documentation | Total |
|----------|:-----------:|:--------:|:------------:|:-------------:|:-----:|
| 🔴 Critical | 2 | 2 | 3 | 1 | **8** |
| 🟠 High     | 5 | 3 | 5 | 3 | **16** |
| 🟡 Medium   | 5 | 5 | 5 | 7 | **22** |
| 🔵 Low      | 4 | 3 | 3 | 5 | **15** |
| **Total**   | **16** | **13** | **16** | **16** | **61** |

---

## 🔴 Critical Issues (Resolve Immediately)

### [SEC-001] Self-Update Quarantine Removal Bypasses macOS Gatekeeper
- **Area**: Security
- **Location**: `par-term-update/src/install_methods.rs:131-154`
- **Description**: After extracting a downloaded zip, the self-updater runs `xattr -cr` on the entire `.app` bundle to remove the macOS quarantine attribute. This strips Gatekeeper's mechanism for verifying downloaded applications. If a MITM attacker serves a malicious binary (bypassing SHA256 checksum, which is also fetched from the same CDN), the malicious binary executes without OS-level verification.
- **Impact**: Supply-chain attack vector — malicious binary bypasses Gatekeeper entirely.
- **Remedy**: Verify code signature and notarization ticket (`codesign --verify --deep --strict` and `spctl --assess --type execute`) before removing quarantine. Alternatively, do not strip quarantine and let macOS handle first-launch verification.

### [SEC-002] Command Denylist is Bypassable (with `prompt_before_run: false`)
- **Area**: Security
- **Location**: `par-term-config/src/automation.rs:380-440`
- **Description**: The command denylist used by triggers/scripts is a substring-matching heuristic with documented bypasses (shell variable indirection, absolute paths, encoding tricks). When users configure `prompt_before_run: false` and `i_accept_the_risk: true`, terminal output can trigger arbitrary command execution with only this bypassable denylist as protection.
- **Impact**: Terminal output can trigger arbitrary command execution if denylist is bypassed.
- **Remedy**: Implement an allowlist model (`allowed_commands: [...]`) instead of a denylist. Canonicalize command paths with `std::fs::canonicalize` and match against allowlist before spawning.

### [ARC-001] WindowState God Object with 80+ Scattered impl Blocks
- **Area**: Architecture
- **Location**: `src/app/window_state/mod.rs` (245-line struct definition) and 37+ files
- **Description**: `WindowState` has 67 fields with 80+ `impl` blocks spread across the codebase. Any method can read or mutate any field, violating Single Responsibility. While 13 sub-state structs have already been extracted, the remaining fields still create a maintenance hazard.
- **Impact**: Adding features requires understanding the full 80-method surface area. Refactoring is blocked by risk of shared mutable state.
- **Remedy**: Continue extracting cohesive field groups into self-contained structs. Priority: `TmuxSubsystem` (already largely isolated in `src/app/tmux_handler/`).

### [ARC-002] Config Struct Monolith (1616 lines, 256+ fields)
- **Area**: Architecture
- **Location**: `par-term-config/src/config/config_struct/mod.rs`
- **Description**: The `Config` struct holds all terminal configuration in a single 1616-line file. The struct derives `Clone` and gets cloned per-window, meaning every window holds a full copy. Sub-struct extraction is in progress but ~200 fields remain inline.
- **Impact**: Expensive cloning on every settings change, all-or-nothing serialization, difficult navigation.
- **Remedy**: Continue `#[serde(flatten)]` extraction into 8-10 sub-structs. Introduce `Arc<Config>` for shared portions.

### [QA-001] Config Cloning Propagates Full 256-Field Clone to Every Window
- **Area**: Code Quality
- **Location**: `src/app/window_manager/config_propagation.rs:50`
- **Description**: Every call to `apply_config_to_windows` clones the entire `Config` struct into every open `WindowState`. With N windows, this is N allocations per settings change. The code acknowledges this as "PROPAGATION TAX".
- **Impact**: Performance degradation proportional to open window count.
- **Remedy**: Replace `window_state.config = config.clone()` with `Arc<Config>` shared across all `WindowState` instances.

### [QA-002] Blocking `thread::sleep` in Event Loop Freezes Rendering
- **Area**: Code Quality
- **Location**: `src/app/input_events/snippet_actions.rs:357,490` and `src/app/triggers/mod.rs:602,742`
- **Description**: `std::thread::sleep` is called on the main event-loop thread during snippet sequence execution and trigger delay dispatch, blocking GPU rendering and window event processing for the sleep duration.
- **Impact**: Entire terminal UI freezes during delayed snippet/trigger execution. A repeat with 100 iterations and 50ms delay freezes UI for 5 seconds.
- **Remedy**: Move delayed execution to background Tokio tasks. Communicate completion via mpsc channel.

### [QA-003] Duplicated Shader-Option Branching in Render Path
- **Area**: Code Quality
- **Location**: `par-term-render/src/renderer/rendering.rs:61-450` and `:456-600`
- **Description**: `render_split_panes` and `take_screenshot` contain near-identical shader-chaining logic (4 combinations of custom+cursor shader). 12 `.expect()` calls across both functions duplicate the same invariants.
- **Impact**: Any shader pipeline change must be made in two places. Bugs fixed in one path are likely missed in the other.
- **Remedy**: Extract a `RenderPipeline` abstraction that encapsulates the 4 shader combinations into a single `render_composited()` method.

### [DOC-001] CLAUDE.md Version Stale at 0.30.4 (Current: 0.30.12)
- **Area**: Documentation
- **Location**: `CLAUDE.md:12`
- **Description**: The version field is 8 versions behind the actual project version. AI agents reading CLAUDE.md operate with stale version assumptions.
- **Impact**: AI agents may produce incorrect code or miss available APIs.
- **Remedy**: Update to `**Version**: 0.30.12` or remove the hardcoded version. Keep in sync during releases.

---

## 🟠 High Priority Issues

### [SEC-003] Shader Installer Downloads Without URL Validation
- **Area**: Security
- **Location**: `src/shader_installer.rs:178-195`, `src/http.rs`
- **Description**: `download_file()` does not enforce URL validation (no host allowlist, no HTTPS-only enforcement), unlike `par-term-update`'s `http.rs` which has strict validation.
- **Impact**: Tampered GitHub API response could redirect download to attacker-controlled server.
- **Remedy**: Apply the same URL validation pattern from `par-term-update/src/http.rs` to the shader installer.

### [SEC-004] Zip Slip Risk in Shader and Update Extraction
- **Area**: Security
- **Location**: `src/shader_installer.rs:258-286`, `par-term-update/src/install_methods.rs:80-128`
- **Description**: Both extractors use `enclosed_name()` but don't verify the final path remains within the target directory. A crafted zip could potentially write outside the target via symlink attacks or encoding tricks.
- **Impact**: Potential arbitrary file write via malicious zip archive.
- **Remedy**: Add `if !final_path.starts_with(target_dir) { continue; }` after computing `final_path`.

### [SEC-005] Shell Injection Surface in ACP Agent Spawning
- **Area**: Security
- **Location**: `par-term-acp/src/agent.rs:248-286`
- **Description**: When an ACP agent command contains shell metacharacters, it falls back to `$SHELL -lc <command>` without escaping the user-provided `run_command` string. SHELL validation exists but the command string has full shell power.
- **Impact**: Malicious agent config TOML could execute arbitrary commands.
- **Remedy**: Document agent TOML files as a trust boundary. Add warning when agents use shell fallback mode.

### [ARC-003] Layer Violation — par-term-config Re-exports par-term-emu-core-rust Types
- **Area**: Architecture
- **Location**: `par-term-config/src/lib.rs:132`
- **Description**: Foundation-layer config crate re-exports types from the external emulation core, creating upward dependency and forcing recompilation of all 9 dependent crates on emu-core changes.
- **Remedy**: Define native enum types in `par-term-config/src/types.rs`. Add `From` conversions in `par-term-terminal`.

### [ARC-004] Dual Logging System Creates Maintenance Burden
- **Area**: Architecture
- **Location**: `src/debug.rs` (513 lines)
- **Description**: Two parallel logging systems (custom `debug_info!` macros: 308 call sites; standard `log::*` macros: 1016 call sites) with separate filtering mechanisms (`DEBUG_LEVEL` vs `RUST_LOG`).
- **Remedy**: Unify under `tracing` with `tracing_subscriber` and `EnvFilter`. Add CI lint preventing new `log::debug!()` in rendering hot paths in the interim.

### [ARC-005] Settings-UI Files Exceeding 1700 Lines
- **Area**: Architecture
- **Location**: `par-term-settings-ui/src/background_tab/shader_settings.rs` (1719), `par-term-settings-ui/src/actions_tab.rs` (1642)
- **Description**: Two files far exceed the project's 800-line guideline with deeply nested egui UI code.
- **Remedy**: Extract per-control-type rendering into sub-modules. Extract action editor state machine from `actions_tab.rs`.

### [ARC-006] Custom Shader Renderer Approaching Maintainability Limit
- **Area**: Architecture
- **Location**: `par-term-render/src/custom_shader_renderer/mod.rs` (903), `transpiler.rs` (1242)
- **Description**: GLSL-to-WGSL transpilation, uniform management, texture loading, and rendering orchestration in two very large files.
- **Remedy**: Split transpiler into `glsl_parse.rs`, `pragma.rs`, `wgsl_emit.rs`. Extract uniform/texture management.

### [ARC-007] No Feature-Gated Dependency Isolation in Root Crate
- **Area**: Architecture
- **Location**: `Cargo.toml`
- **Description**: 50+ direct dependencies with only one feature flag. Heavy deps like `rodio`, `mermaid-rs-renderer`, `resvg`, `sysinfo`, `mdns-sd` are always compiled.
- **Remedy**: Introduce feature flags: `audio`, `mermaid`, `mdns`, `system-monitor`.

### [QA-004] 10 Files Exceed 800-Line Limit (Largest at 1719)
- **Area**: Code Quality
- **Location**: `shader_settings.rs` (1719), `shader_controls.rs` (1666), `actions_tab.rs` (1642), `config_struct/mod.rs` (1616), `snippets.rs` (1268), `transpiler.rs` (1242), `snippet_actions.rs` (1218), `box_drawing_data.rs` (1051), `custom_shader_renderer/mod.rs` (903), `pane_render/mod.rs` (815)
- **Remedy**: Continue extraction pattern. Priority: `shader_settings.rs`, `actions_tab.rs`, `snippet_actions.rs`.

### [QA-005] 363 `.unwrap()` Calls in Production Code
- **Area**: Code Quality
- **Location**: Concentrated in `par-term-mcp/src/lib.rs` (24), `par-term-render/src/renderer/rendering.rs` (12), `par-term-render/src/cell_renderer/atlas.rs` (8)
- **Description**: MCP module uses bare `.unwrap()` on JSON parsing and file I/O. Render module uses `.expect()` for checked invariants but lacks GPU device loss recovery.
- **Remedy**: MCP: replace with `?` or `.ok_or_else()`. Render: add recovery paths for GPU device loss.

### [QA-006] `build_pane_instance_buffers` — Single 550-Line Function
- **Area**: Code Quality
- **Location**: `par-term-render/src/cell_renderer/pane_render/mod.rs:261`
- **Description**: Handles viewport fill, cell iteration, half-block detection, cursor blending, RLE background merging, powerline fringe extension, and text instance building in one function with 15+ branch points.
- **Remedy**: Extract cursor cell handling, RLE merge loop body, and text instance building into separate helpers.

### [QA-007] `Config` Struct Has 256+ Fields (God Object)
- **Area**: Code Quality
- **Location**: `par-term-config/src/config/config_struct/mod.rs:139`
- **Description**: Approximately 200 fields remain inline despite ongoing sub-struct extraction. The `Default` impl has 256+ default functions.
- **Remedy**: Continue `#[serde(flatten)]` extraction. Prioritize: InputConfig, MouseConfig, SelectionConfig, CursorConfig, ThemeConfig.

### [QA-008] `WindowState` — 67 pub(crate) Fields Across 19 impl Files
- **Area**: Code Quality
- **Location**: `src/app/window_state/mod.rs:132`
- **Description**: 19 implementation files create implicit coupling through shared `&mut self` access.
- **Remedy**: Continue decomposing into focused sub-structs with narrow interfaces.

### [DOC-002] Pub Enum Docstring Coverage at 1.5% (2 of 130)
- **Area**: Documentation
- **Location**: All workspace crates; worst in `par-term-config` (0/60), `par-term-acp` (0/4), `par-term-settings-ui` (0/7), `par-term-tmux` (0/7)
- **Description**: Only 2 of 130 public enums have `///` doc comments. `par-term-config` (foundation crate) has zero documented enums despite exporting 60.
- **Remedy**: Add doc comments to all public enums, starting with `par-term-config` (Layer 1).

### [DOC-003] Pub Struct Docstring Coverage at 29.6% (84 of 284)
- **Area**: Documentation
- **Location**: Worst in `par-term-config` (2/58, 3%), `par-term-acp` (2/43, 5%), `par-term-mcp` (0/9, 0%)
- **Description**: Most public structs lack doc comments, undermining `cargo doc` output and crates.io consumers.
- **Remedy**: Prioritize `par-term-config` (all 58), then `par-term-acp` (43), then remaining crates.

### [DOC-004] No Architecture Diagrams for Data Flow and Threading
- **Area**: Documentation
- **Location**: `docs/ARCHITECTURE.md`
- **Description**: Architecture document describes the render pipeline, threading model, and state lifecycle entirely in prose. No sequence diagrams, state diagrams, or flow diagrams.
- **Remedy**: Add Mermaid sequence diagrams for: (1) three-phase render pipeline, (2) PTY read to screen update, (3) split-pane state management. Add state diagrams for tab/pane lifecycle.

---

## 🟡 Medium Priority Issues

### Architecture
- **[ARC-008]** bitflags v1 and v2 duplication in transitive deps — monitor for `symphonia` update
- **[ARC-009]** `PostRenderActions` aggregates 13 unrelated action types — Open/Closed violation
- **[ARC-010]** Settings-UI crate is 28,644 lines (largest sub-crate) — compile-time bottleneck
- **[ARC-011]** Root crate `src/` is 70,587 lines across 310 files — extract self-contained feature modules
- **[ARC-012]** Config facade re-exports 60+ types — any change forces broad recompilation

### Security
- **[SEC-006]** `std::mem::zeroed()` for `KeyEvent` construction in test code — acceptable with documented justification
- **[SEC-007]** macOS `unsafe` FFI for window blur and space detection — signature mismatch risk from dlsym
- **[SEC-008]** MCP server trust boundary at stdin — no auth on IPC channel
- **[SEC-009]** Session logger heuristic password redaction can miss non-English prompts
- **[SEC-010]** 296 `unwrap()`/`expect()` calls across codebase — DoS risk in production paths

### Code Quality
- **[QA-009]** Mixed logging: 1016 `log::*` calls vs 308 custom debug macros — fragmented debug output
- **[QA-010]** `anyhow` in 53 files but only `par-term-render` uses typed errors (`RenderError`)
- **[QA-011]** macOS FFI `unsafe` blocks lack test coverage
- **[QA-012]** `par-term-mcp` heavy `.unwrap()` in production IPC handlers
- **[QA-013]** Config layer violation — `par-term-config` re-exports from higher-layer crates

### Documentation
- **[DOC-005]** README "What's New" section is ~900 lines — should link to CHANGELOG.md
- **[DOC-006]** CONTRIBUTING.md duplicates CLAUDE.md content — maintenance drift risk
- **[DOC-007]** docs/API.md is static index without CI validation — staleness risk
- **[DOC-008]** Sub-crate READMEs lack installation/usage sections for crates.io consumers
- **[DOC-009]** No CI badge for test coverage
- **[DOC-010]** CHANGELOG.md missing "Security" sections in some entries
- **[DOC-011]** docs/MIGRATION.md references removed "Prettifier" without context

---

## 🔵 Low Priority / Improvements

### Architecture
- **[ARC-013]** No benchmarks for rendering pipeline (Makefile `bench` target exists but empty)
- **[ARC-014]** Test organization lacks subdirectory grouping (41 files flat in `tests/`)
- **[ARC-015]** `RendererSizing` derives `Copy` with 40 bytes — monitor threshold

### Security
- **[SEC-011]** Test fixture uses `API_KEY: secret123` — false positive for secret scanners
- **[SEC-012]** Cargo.lock correctly committed (positive finding for binary crate)
- **[SEC-013]** No `.env` files or hardcoded secrets found

### Code Quality
- **[QA-014]** 30 `#[allow(dead_code)]`/`#[allow(clippy::...)]` suppressions — review those without ticket refs
- **[QA-015]** `TempDir::new().unwrap()` repeated across integration tests — expand `tests/common/mod.rs`
- **[QA-016]** 7 module-level `#![allow(...)]` in test files — consider `ConfigTestBuilder`

### Documentation
- **[DOC-012]** README sponsorship badge placement breaks badge flow
- **[DOC-013]** docs/ directory lacks subdirectory organization
- **[DOC-014]** Examples directory uses legacy config field names ("experimental" labels)
- **[DOC-015]** docs/README.md duplicates README.md documentation table

---

## Detailed Findings

### Architecture & Design

The project follows a well-structured layered crate architecture with 13 sub-crates organized in a documented dependency hierarchy (Layer 0–4). The workspace dependency centralization is excellent — all sub-crates use `workspace = true` for shared dependencies. The three-phase rendering pipeline (bg → text → cursor overlays) has strict ordering invariants enforced through `emit_three_phase_draw_calls()`.

Active decomposition is underway: `WindowState` has had 13 sub-state structs extracted (EguiState, FocusState, OverlayState, RenderLoopState, ShaderState, AgentState, CursorAnimState, OverlayUiState, TriggerState, WatcherState, UpdateState, DebugState). Files approaching the 800-line limit have ARC-009 TODO comments documenting the planned extraction.

Key concerns: the `WindowState` God Object (67 fields, 80+ impl blocks) and `Config` monolith (256+ fields, 1616 lines) remain the largest structural debt items. The root crate at 70,587 lines is a compile-time bottleneck that could benefit from extracting self-contained feature modules (ai_inspector, badges, session_logger, menus).

### Security Assessment

The project has a **Fair** overall security posture with strong defaults but critical gaps. Positive highlights include: HTTPS-only enforcement and host allowlist in the update system, `prompt_before_run: true` default for triggers, session files written with `0o600` permissions, command spawning that avoids shell interpretation, ACP agent SHELL validation, sensitive data redaction in agent context, and no hardcoded secrets.

Critical gaps: the self-updater's unconditional quarantine removal (SEC-001) strips macOS Gatekeeper verification, and the shader installer downloads files without URL validation (SEC-003). The command denylist bypass (SEC-002) is a design limitation acknowledged by developers but represents real risk when `prompt_before_run: false`.

### Code Quality

Code quality is **Fair** with active improvement tracking via ARC/QA issue IDs. The project has 1,634 test functions and well-structured error types in the render crate (`RenderError` with 14 variants). Technical debt is moderate: 11 TODO/FIXME comments, 30 lint suppressions, 10 files exceeding 800 lines.

The most impactful quality issues are: event-loop-blocking `thread::sleep` calls (QA-002), duplicated shader-chaining logic (QA-003), and Config cloning propagation (QA-001). Test coverage is moderate (30–70%) with key untested areas: GPU rendering pipeline, macOS FFI code, PTY management, split pane rendering, and session save/restore.

### Documentation Review

Documentation breadth is **exceptional** — 37 feature documentation files covering every major feature, plus 13 sub-crate READMEs, CHANGELOG.md, CONTRIBUTING.md, and comprehensive operational guides. The GETTING_STARTED.md is a model onboarding document.

Key gaps: pub enum docstring coverage at 1.5% and pub struct coverage at 29.6% undermine `cargo doc` utility. The CLAUDE.md version is stale. Architecture docs lack sequence/flow diagrams. The README "What's New" section at ~900 lines should delegate to CHANGELOG.md.

---

## Remediation Roadmap

### Immediate Actions (Before Next Deployment)
1. **SEC-001**: Add code signature verification before quarantine removal in self-updater
2. **SEC-003**: Add URL validation to shader installer downloads (copy pattern from `par-term-update/src/http.rs`)
3. **SEC-004**: Add `starts_with(target_dir)` containment check in zip extraction
4. **DOC-001**: Update CLAUDE.md version to 0.30.12

### Short-term (Next 1–2 Sprints)
1. **QA-002**: Move `thread::sleep` to background Tokio tasks (fixes UI freezes)
2. **ARC-001**: Continue `WindowState` decomposition — extract `TmuxSubsystem`
3. **QA-003**: Extract shared `RenderPipeline` abstraction to deduplicate shader chaining
4. **SEC-002**: Implement command allowlist model for auto-execution triggers
5. **QA-005**: Replace MCP `.unwrap()` calls with proper error propagation

### Long-term (Backlog)
1. **ARC-002**: Complete Config sub-struct extraction + introduce `Arc<Config>`
2. **ARC-004**: Unify logging under `tracing`
3. **ARC-007**: Add feature flags for optional dependencies
4. **DOC-002/DOC-003**: Systematic docstring coverage improvement
5. **ARC-011**: Extract feature modules from root crate

---

## Positive Highlights

1. **Excellent workspace dependency centralization** — all 13 sub-crates use `workspace = true` for shared dependencies, eliminating version skew.

2. **Three-phase rendering pipeline with strict ordering** — `emit_three_phase_draw_calls()` enforces bg → text → cursor invariant with clear documentation.

3. **Active architectural decomposition** — 13 sub-state structs already extracted from `WindowState`; ARC/QA issue IDs track remaining work with planned extraction paths.

4. **Strong security defaults** — `prompt_before_run: true`, `i_accept_the_risk: false`, all script permissions default to `false`, HTTPS-only updates with host allowlist and SHA256 verification.

5. **Exceptional documentation breadth** — 37 feature docs, 13 sub-crate READMEs, model GETTING_STARTED.md, comprehensive CONTRIBUTING.md.

6. **Well-designed typed error in render crate** — `RenderError` with 14 variants, `#[source]` annotations, and convenience `From` impls is an exemplary pattern.

7. **Comprehensive test suite** — 1,634 test functions with mock traits (`TerminalAccess`, `UIElement`) for testing without live PTY/GPU contexts.

8. **Thoughtful lock contention telemetry** — `try_lock` tracking with per-site metrics and periodic reporting for diagnosing async mutex contention in production.

---

## Audit Confidence

| Area | Files Reviewed | Confidence |
|------|---------------|-----------|
| Architecture | 45+ (Cargo.toml, mod.rs, lib.rs files across all crates) | High |
| Security | 30+ (PTY, SSH, update, shader, MCP, session logger) | High |
| Code Quality | 40+ (rendering pipeline, config, MCP, FFI, tests) | High |
| Documentation | 50+ (all docs/ files, all READMEs, CLAUDE.md, CONTRIBUTING.md) | High |

---

## Remediation Plan

> This section is generated by the audit and consumed directly by `/fix-audit`.
> It pre-computes phase assignments and file conflicts so the fix orchestrator
> can proceed without re-analyzing the codebase.

### Phase Assignments

#### Phase 1 — Critical Security (Sequential, Blocking)
<!-- Issues that must be fixed before anything else. -->
| ID | Title | File(s) | Severity |
|----|-------|---------|----------|
| SEC-001 | Self-update quarantine removal bypasses Gatekeeper | `par-term-update/src/install_methods.rs` | Critical |
| SEC-003 | Shader installer downloads without URL validation | `src/shader_installer.rs`, `src/http.rs` | High |
| SEC-004 | Zip slip risk in shader/update extraction | `src/shader_installer.rs`, `par-term-update/src/install_methods.rs` | High |

#### Phase 2 — Critical Architecture (Sequential, Blocking)
<!-- Issues that restructure the codebase; must complete before Code Quality fixes. -->
| ID | Title | File(s) | Severity | Blocks |
|----|-------|---------|----------|--------|
| ARC-002 | Config struct monolith (1616 lines) | `par-term-config/src/config/config_struct/mod.rs` | Critical | QA-001, QA-007 |
| ARC-001 | WindowState God Object | `src/app/window_state/mod.rs` + 37 files | Critical | QA-008 |

#### Phase 3 — Parallel Execution
<!-- All remaining work, safe to run concurrently by domain. -->

**3a — Security (remaining)**
| ID | Title | File(s) | Severity |
|----|-------|---------|----------|
| SEC-002 | Bypassable command denylist | `par-term-config/src/automation.rs` | Critical |
| SEC-005 | Shell injection in ACP agent spawning | `par-term-acp/src/agent.rs` | High |
| SEC-006 | zeroed() KeyEvent in test code | `src/app/input_events/snippet_actions.rs` | Medium |
| SEC-007 | macOS unsafe FFI lack of signature verification | `src/macos_blur.rs`, `src/macos_space.rs` | Medium |
| SEC-008 | MCP stdin trust boundary | `par-term-mcp/src/lib.rs` | Medium |
| SEC-009 | Session logger password redaction gaps | `src/session_logger/core.rs` | Medium |

**3b — Architecture (remaining)**
| ID | Title | File(s) | Severity |
|----|-------|---------|----------|
| ARC-003 | Layer violation in par-term-config | `par-term-config/src/lib.rs` | High |
| ARC-004 | Dual logging system | `src/debug.rs` | High |
| ARC-005 | Settings-UI files >1700 lines | `par-term-settings-ui/src/background_tab/shader_settings.rs`, `par-term-settings-ui/src/actions_tab.rs` | High |
| ARC-006 | Custom shader renderer size | `par-term-render/src/custom_shader_renderer/mod.rs`, `transpiler.rs` | High |
| ARC-007 | No feature flags for optional deps | `Cargo.toml` | Medium |
| ARC-009 | PostRenderActions collect-all | `src/app/render_pipeline/types.rs` | Medium |
| ARC-010 | Settings-UI crate 28K lines | `par-term-settings-ui/` | Medium |
| ARC-011 | Root crate 70K lines | `src/` | Medium |

**3c — Code Quality (all)**
| ID | Title | File(s) | Severity |
|----|-------|---------|----------|
| QA-001 | Config cloning propagation | `src/app/window_manager/config_propagation.rs` | Critical |
| QA-002 | Blocking thread::sleep in event loop | `src/app/input_events/snippet_actions.rs`, `src/app/triggers/mod.rs` | Critical |
| QA-003 | Duplicated shader-chaining logic | `par-term-render/src/renderer/rendering.rs` | Critical |
| QA-004 | 10 files exceed 800-line limit | Multiple (see issue detail) | High |
| QA-005 | 363 .unwrap() in production code | `par-term-mcp/src/lib.rs`, `par-term-render/src/renderer/rendering.rs` | High |
| QA-006 | 550-line build_pane_instance_buffers | `par-term-render/src/cell_renderer/pane_render/mod.rs` | High |
| QA-009 | Mixed logging strategy | `par-term-render/src/`, `src/` | Medium |
| QA-010 | anyhow everywhere, typed errors only in render | 53 files | Medium |
| QA-011 | macOS FFI unsafe lacks tests | `src/macos_blur.rs`, `src/macos_space.rs` | Medium |
| QA-012 | MCP heavy .unwrap() in production | `par-term-mcp/src/lib.rs` | Medium |

**3d — Documentation (all)**
| ID | Title | File(s) | Severity |
|----|-------|---------|----------|
| DOC-001 | CLAUDE.md version stale | `CLAUDE.md` | Critical |
| DOC-002 | Pub enum docstring coverage 1.5% | All crates, esp. `par-term-config/` | High |
| DOC-003 | Pub struct docstring coverage 29.6% | All crates, esp. `par-term-config/` | High |
| DOC-004 | No architecture diagrams | `docs/ARCHITECTURE.md` | High |
| DOC-005 | README "What's New" too long | `README.md` | Medium |
| DOC-006 | CONTRIBUTING.md duplicates CLAUDE.md | `CONTRIBUTING.md` | Medium |
| DOC-007 | API.md static without CI check | `docs/API.md` | Medium |
| DOC-008 | Sub-crate READMEs lack install sections | All sub-crate READMEs | Medium |
| DOC-010 | CHANGELOG missing Security sections | `CHANGELOG.md` | Medium |
| DOC-011 | Migration doc lacks prettifier context | `docs/MIGRATION.md` | Medium |

### File Conflict Map
<!-- Files touched by issues in multiple domains. Fix agents must read current file state
     before editing — a prior agent may have already changed these. -->

| File | Domains | Issues | Risk |
|------|---------|--------|------|
| `src/app/input_events/snippet_actions.rs` | Security + Code Quality | SEC-006, QA-002, QA-004 | ⚠️ Read before edit |
| `par-term-update/src/install_methods.rs` | Security + Code Quality | SEC-001, SEC-004 | ⚠️ Read before edit |
| `src/shader_installer.rs` | Security + Code Quality | SEC-003, SEC-004 | ⚠️ Read before edit |
| `src/http.rs` | Security + Code Quality | SEC-003 | Low |
| `par-term-mcp/src/lib.rs` | Security + Code Quality | SEC-008, QA-012 | ⚠️ Read before edit |
| `src/macos_blur.rs` | Security + Code Quality | SEC-007, QA-011 | ⚠️ Read before edit |
| `src/macos_space.rs` | Security + Code Quality | SEC-007, QA-011 | ⚠️ Read before edit |
| `par-term-config/src/config/config_struct/mod.rs` | Architecture + Code Quality | ARC-002, QA-007 | ⚠️ Read before edit |
| `src/app/window_state/mod.rs` | Architecture + Code Quality | ARC-001, QA-008 | ⚠️ Read before edit |
| `par-term-config/src/lib.rs` | Architecture + Code Quality | ARC-003, QA-013 | ⚠️ Read before edit |
| `par-term-render/src/custom_shader_renderer/mod.rs` | Architecture + Code Quality | ARC-006, QA-004 | ⚠️ Read before edit |
| `par-term-settings-ui/src/background_tab/shader_settings.rs` | Architecture + Code Quality | ARC-005, QA-004 | ⚠️ Read before edit |
| `par-term-settings-ui/src/actions_tab.rs` | Architecture + Code Quality | ARC-005, QA-004 | ⚠️ Read before edit |
| `par-term-render/src/renderer/rendering.rs` | Code Quality (internal) | QA-003, QA-005 | ⚠️ Read before edit |
| `par-term-render/src/cell_renderer/pane_render/mod.rs` | Code Quality (internal) | QA-004, QA-006 | ⚠️ Read before edit |
| `src/debug.rs` | Architecture + Code Quality | ARC-004, QA-009 | ⚠️ Read before edit |
| `src/app/window_manager/config_propagation.rs` | Architecture + Code Quality | QA-001 (depends on ARC-002) | ⚠️ Read before edit |
| `src/session_logger/core.rs` | Security + Documentation | SEC-009, DOC (logging docs) | Low |

### Blocking Relationships
<!-- Explicit dependency declarations from audit agents.
     Format: [blocker issue] → [blocked issue] — reason -->
- ARC-002 → QA-001: Config cloning fix requires Arc<Config> architecture from ARC-002 decomposition
- ARC-002 → QA-007: Config field extraction must complete before targeted sub-struct work
- ARC-001 → QA-008: WindowState decomposition must precede sub-struct interface narrowing
- ARC-001 → QA-004 (window_state): WindowState file extraction blocks size reduction
- ARC-003 → QA-013: Layer violation fix resolves the same re-export issue
- ARC-004 → QA-009: Logging unification resolves mixed logging strategy
- ARC-006 → QA-003: Shader renderer extraction should precede render pipeline deduplication
- SEC-001 → SEC-004: Install methods file touched by both — quarantine fix first
- QA-002 → (none): Blocking sleep fix is independent but needs Tokio task infrastructure

### Dependency Diagram

```mermaid
graph TD
    P1["Phase 1: Critical Security"]
    P2["Phase 2: Critical Architecture"]
    P3a["Phase 3a: Security (remaining)"]
    P3b["Phase 3b: Architecture (remaining)"]
    P3c["Phase 3c: Code Quality"]
    P3d["Phase 3d: Documentation"]
    P4["Phase 4: Verification"]

    P1 --> P2
    P2 --> P3a & P3b & P3c & P3d
    P3a & P3b & P3c & P3d --> P4

    ARC002["ARC-002 Config"] -->|blocks| QA001["QA-001 Config Clone"]
    ARC002["ARC-002 Config"] -->|blocks| QA007["QA-007 Config Fields"]
    ARC001["ARC-001 WindowState"] -->|blocks| QA008["QA-008 WindowState Fields"]
    ARC006["ARC-006 Shader Renderer"] -->|blocks| QA003["QA-003 Shader Dupe"]
    ARC004["ARC-004 Logging"] -->|blocks| QA009["QA-009 Mixed Logging"]
    SEC001["SEC-001 Quarantine"] -->|blocks| SEC004["SEC-004 Zip Slip"]
