# Project Audit Report

> **Project**: par-term
> **Date**: 2026-02-27
> **Updated**: 2026-02-28 (post-remediation — completed issues removed)
> **Stack**: Rust (Edition 2024), wgpu (GPU rendering), Tokio (async runtime), egui (settings UI)
> **Audited by**: Claude Code Audit System

---

## Executive Summary

par-term is a mature, feature-rich terminal emulator with excellent documentation and a well-organized workspace structure. After an initial audit of 43 issues and a remediation pass, the remaining open issues are **25 items** requiring structural code changes. The critical security issue (exposed API token) and all documentation gaps have been resolved. What remains is primarily architectural debt: the `WindowState` God Object, oversized settings UI files, and the `Arc<Mutex>` locking pattern all require structural refactoring. Two security risks (external command renderer and HTTP profile fetching) require product decisions before they can be fully addressed.

### Remaining Issue Count by Severity

| Severity | Architecture | Security | Code Quality | Total |
|----------|:-----------:|:--------:|:------------:|:-----:|
| 🔴 Critical | 3 | 0 | 0 | **3** |
| 🟠 High     | 4 | 2 | 3 | **9** |
| 🟡 Medium   | 0 | 0 | 1 | **1** |
| 🔵 Low      | 3 | 3 | 3 | **9** |
| **Total**   | **10** | **5** | **7** | **22** |

> See `AUDIT-REMEDIATION.md` for the full record of what was resolved.

---

## 🔴 Critical Issues (Resolve Immediately)

### [ARC-001] God Object: WindowState Struct Has 50+ Fields
- **Area**: Architecture
- **Location**: `src/app/window_state/mod.rs:91-270`
- **Description**: The `WindowState` struct contains 50+ fields mixing terminal state, UI state, caching, configuration, and feature-specific concerns. Section grouping comments were added in remediation, but no structural decomposition was done.
- **Impact**: Makes the codebase difficult to maintain, test, and reason about. Changes to any subsystem risk unintended side effects.
- **Remedy**: Extract cohesive subsystems into dedicated state objects: `FocusState`, `OverlayState`, `TransferState`, `WatcherState`, `UIState`. The `render_pipeline/` subdirectory pattern is a good model.

### [ARC-002] Arc<Mutex<T>> Pattern Creates Locking Complexity
- **Area**: Architecture
- **Location**: `src/tab/mod.rs:69`
- **Description**: `Tab.terminal` uses `Arc<tokio::sync::Mutex<TerminalManager>>` requiring different access patterns from async vs sync contexts. Locking rules were documented in remediation, but the pattern itself was not changed.
- **Impact**: Risk of deadlocks if `blocking_lock()` is called in async context. Thread safety depends on developers following documented rules.
- **Remedy**: Consider `tokio::sync::RwLock` for read-heavy workloads, or redesign to use MPSC channels for terminal commands instead of shared mutable state.

### [ARC-003] Large Settings UI Files Exceed 1000 Lines (3 remaining)
- **Area**: Architecture
- **Location**:
  - `par-term-settings-ui/src/profile_modal_ui.rs` (~1406 lines)
  - `par-term-settings-ui/src/terminal_tab.rs` (~1356 lines)
  - `par-term-settings-ui/src/advanced_tab.rs` (~1276 lines)
- **Note**: `input_tab.rs` (1542 lines) was split into `par-term-settings-ui/src/input_tab/` in remediation. Three files remain.
- **Impact**: Difficult to navigate and maintain. Testing is harder. Code review burden increases.
- **Remedy**: Apply the same `input_tab/` subdirectory split pattern to the three remaining files.

---

## 🟠 High Priority Issues

### [SEC-002] External Command Renderer Allows Arbitrary Command Execution
- **Area**: Security
- **Location**: `src/prettifier/custom_renderers.rs:72`
- **Description**: `ExternalCommandRenderer::render()` executes arbitrary commands from user configuration. A security warning was documented in remediation, but no allowlist or confirmation prompt was implemented.
- **Impact**: A malicious shared config file could include a renderer that executes destructive commands.
- **Remedy**: Implement a command allowlist or require user confirmation before running custom renderers. Currently documented as a known risk only.

### [SEC-003] Dynamic Profile Fetching from Remote URLs
- **Area**: Security
- **Location**: `src/profile/dynamic.rs:160-224`
- **Description**: The application fetches profiles from remote URLs via HTTP. A `log::warn!` was added for HTTP URLs in remediation, but HTTP is not blocked.
- **Impact**: MITM attacker on an untrusted network could inject malicious profiles via HTTP.
- **Remedy**: Enforce HTTPS-only for profile URLs (requires product decision — currently only warns).

### [ARC-004] Legacy Fields on Tab Struct with Unclear Migration Path
- **Area**: Architecture
- **Location**: `src/tab/mod.rs:79-91`
- **Description**: Fields `scroll_state`, `mouse`, `bell`, `cache` are marked legacy but actively used in 7–17 files each. `TODO(migration)` comments were added in remediation, but no migration work was done.
- **Impact**: Code duplication between Tab and Pane, confusion about which state to use.
- **Remedy**: Execute the migration plan documented in `src/tab/mod.rs` — route callers to `PaneManager::active_pane()` equivalents, then remove the legacy fields.

### [ARC-005] Duplicate Code in Tab Constructors
- **Area**: Architecture
- **Location**: `src/tab/mod.rs:193-386` and `405-626`
- **Description**: `Tab::new()` and `Tab::new_from_profile()` share ~80% identical code. Refactoring plan was documented in remediation but not implemented.
- **Impact**: Changes must be made in two places, increasing maintenance burden and bug risk.
- **Remedy**: Extract shared initialization into a private `Tab::new_internal()` as documented in the `# REFACTOR` sections of each constructor.

### [ARC-006] Prettifier Module Lacks Clear Boundaries
- **Area**: Architecture
- **Location**: `src/prettifier/`
- **Description**: 15+ submodules with overlapping concerns. A module overview was documented in remediation, but the structure was not consolidated.
- **Impact**: Difficult to understand the prettifier's architecture. Adding new content types requires touching multiple files.
- **Remedy**: Consolidate into a cleaner package structure: `prettifier::detect`, `prettifier::render`, `prettifier::pipeline` as main entry points.

### [ARC-007] Three-Tier Configuration Resolution Complexity
- **Area**: Architecture
- **Location**: `par-term-config/src/`
- **Description**: Shader config uses a 3-tier resolution system (config → metadata → resolved) with caching. The resolution chain was documented in remediation, but logic remains distributed.
- **Impact**: Configuration bugs are hard to trace. Understanding effective configuration requires following multiple resolution steps.
- **Remedy**: Centralize resolution logic into dedicated resolver types.

### [QA-001] Oversized Configuration Struct (1848 lines)
- **Area**: Code Quality
- **Location**: `par-term-config/src/config/config_struct/mod.rs`
- **Description**: The `Config` struct file is 1848 lines. Section grouping comments and a refactoring plan were added in remediation, but the struct was not split.
- **Impact**: Configuration is difficult to navigate, understand, and maintain.
- **Remedy**: Split into logical sub-structs (`WindowConfig`, `FontConfig`, `TerminalConfig`, `ShaderConfig`, `InputConfig`) as documented in the module-level comment.

### [QA-002] Excessive unwrap() Usage in Non-Test Code
- **Area**: Code Quality
- **Location**: Multiple files across `src/`
- **Description**: 2 `LazyLock` regex `unwrap()` calls were converted to `expect()` in remediation. Many other production `unwrap()` calls remain.
- **Impact**: Runtime panics on unexpected conditions instead of graceful error handling.
- **Remedy**: Continue auditing `unwrap()` calls outside test modules. Replace with `?`, `unwrap_or_default()`, or `expect("reason")`.

### [QA-003] Dead Code with #[allow(dead_code)] Annotations
- **Area**: Code Quality
- **Location**:
  - `src/app/config_updates.rs:29-119`
  - `src/app/file_transfers.rs:37-68`
  - `src/app/tmux_handler/notifications/flow_control.rs:101`
- **Description**: Fields marked with `#[allow(dead_code)]` received `TODO(dead_code)` tracking comments with a v0.26 deadline in remediation, but the code was not removed or implemented.
- **Impact**: Code bloat, confusion about which code paths are actually used.
- **Remedy**: By v0.26, either implement the planned functionality or remove these fields.

---

## 🟡 Medium Priority Issues

### [QA-004] Large Settings UI Files (3 remaining, 1400+ lines)
- **Area**: Code Quality
- **Location**: `par-term-settings-ui/src/profile_modal_ui.rs`, `terminal_tab.rs`, `advanced_tab.rs`
- **Description**: Three settings UI files still exceed 1000 lines. Section headers were added where missing in remediation. `input_tab.rs` was fully split (no longer an issue).
- **Impact**: Difficult to navigate. Code review burden.
- **Remedy**: Apply the `input_tab/` subdirectory split pattern (see ARC-003).

---

## 🔵 Low Priority / Improvements

### Architecture
- **ARC-012**: Makefile Has Duplicate Build Logic — similar patterns in `build`, `build-full`, `build-debug` targets
- **ARC-013**: Test Organization Could Mirror Source Structure — integration tests are flat in `tests/` directory
- **ARC-014**: Log Crate Bridge Complexity — custom debug macros alongside standard `log` crate

### Security
- **SEC-008**: Unsafe Blocks for Platform-Specific Code — macOS FFI calls in `macos_metal.rs`, `macos_space.rs`, `macos_blur.rs`
- **SEC-009**: Test Code Uses Unsafe env::set_var/remove_var — acceptable in test code but should be documented
- **SEC-010**: HTTP Client for Self-Update Uses Hardcoded Hosts — well-hardened with allowlist and HTTPS enforcement

### Code Quality
- **QA-008**: Excessive `#[allow(clippy::too_many_arguments)]` Usage — 20+ occurrences in rendering and UI code
- **QA-009**: Magic Numbers in UI Code — color values without named constants in `sidebar.rs`
- **QA-010**: Test File Size — large test files but acceptable for comprehensive testing

---

## Remediation Roadmap

### Next Sprint
1. **ARC-003 / QA-004**: Split `profile_modal_ui.rs`, `terminal_tab.rs`, `advanced_tab.rs` using the `input_tab/` pattern
2. **QA-003**: Implement or remove dead code fields by v0.26 deadline
3. **SEC-003**: Product decision — block HTTP profile URLs or keep warn-only

### Backlog
1. **ARC-001**: WindowState decomposition into focused state objects
2. **ARC-002**: Consider MPSC channel redesign for terminal commands
3. **ARC-004**: Execute Tab legacy field migration plan
4. **ARC-005**: Extract `Tab::new_internal()` from duplicate constructors
5. **ARC-006**: Consolidate prettifier module structure
6. **ARC-007**: Centralize configuration resolution logic
7. **QA-001**: Split Config struct into logical sub-structs
8. **QA-002**: Continue auditing `unwrap()` in production code
9. **SEC-002**: Implement command allowlist or confirmation for external renderers
