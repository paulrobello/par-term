# Project Audit Report

> **Project**: par-term
> **Date**: 2026-02-27
> **Stack**: Rust (Edition 2024), wgpu (GPU rendering), Tokio (async runtime), egui (settings UI)
> **Audited by**: Claude Code Audit System

---

## Executive Summary

par-term is a mature, feature-rich terminal emulator with excellent documentation and a well-organized workspace structure. The codebase demonstrates strong Rust practices including comprehensive error handling, proper async/sync boundaries, and thoughtful performance optimizations. However, the audit uncovered **4 critical issues** that require immediate attention: an exposed API token in local settings, and three architectural problems (God Object pattern in `WindowState`, complex `Arc<Mutex>` locking, and oversized settings UI files). The codebase has accumulated technical debt in the form of oversized configuration structs (1848 lines), dead code with `#[allow(dead_code)]` annotations, and incomplete features tracked via TODO comments. Estimated effort to remediate critical and high-priority issues: 2-3 sprints.

### Issue Count by Severity

| Severity | Architecture | Security | Code Quality | Documentation | Total |
|----------|:-----------:|:--------:|:------------:|:-------------:|:-----:|
| 🔴 Critical | 3 | 1 | 0 | 0 | **4** |
| 🟠 High     | 4 | 2 | 3 | 2 | **11** |
| 🟡 Medium   | 4 | 4 | 4 | 4 | **16** |
| 🔵 Low      | 3 | 3 | 3 | 3 | **12** |
| **Total**   | **14** | **10** | **10** | **9** | **43** |

---

## 🔴 Critical Issues (Resolve Immediately)

### [SEC-001] Exposed API Token in Local Settings File
- **Area**: Security
- **Location**: `.claude/settings.local.json:3`
- **Description**: The file `.claude/settings.local.json` contains a hardcoded `ANTHROPIC_AUTH_TOKEN` value. While backup files are in `.gitignore`, the main `settings.local.json` file is not explicitly ignored.
- **Impact**: If accidentally committed to a public repository, the API token could be used by attackers for unauthorized API calls at the project owner's expense.
- **Remedy**:
  1. Add `.claude/settings.local.json` explicitly to `.gitignore`
  2. Rotate the exposed token immediately via the Anthropic dashboard
  3. Verify the file has never been committed: `git log --all --full-history -- .claude/settings.local.json`

### [ARC-001] God Object: WindowState Struct Has 50+ Fields
- **Area**: Architecture
- **Location**: `src/app/window_state/mod.rs:91-270`
- **Description**: The `WindowState` struct contains an excessive number of fields (50+), including terminal state, UI state, caching, configuration, and feature-specific concerns all mixed together. This violates the Single Responsibility Principle.
- **Impact**: Makes the codebase difficult to maintain, test, and reason about. Changes to any subsystem risk unintended side effects. High cognitive load for developers.
- **Remedy**: Extract cohesive subsystems into dedicated state objects. Consider: `FocusState`, `OverlayState`, `TransferState`, `WatcherState`, `UIState`. The `render_pipeline/` subdirectory pattern is a good model.

### [ARC-002] Arc<Mutex<T>> Pattern Creates Locking Complexity
- **Area**: Architecture
- **Location**: `src/tab/mod.rs:69`
- **Description**: `Tab.terminal` uses `Arc<tokio::sync::Mutex<TerminalManager>>` which requires different access patterns from async vs sync contexts (`try_lock()` vs `blocking_lock()`). The codebase relies heavily on careful documentation of when to use which.
- **Impact**: Risk of deadlocks if `blocking_lock()` is called in async context, or busy-waiting if `try_lock()` misses updates. Thread safety depends on developers reading and following detailed access rules.
- **Remedy**: Consider using `tokio::sync::RwLock` for read-heavy workloads, or redesign to use message passing (MPSC channels) for terminal commands instead of shared mutable state.

### [ARC-003] Large Settings UI Files Exceed 1000 Lines
- **Area**: Architecture
- **Location**:
  - `par-term-settings-ui/src/input_tab.rs` (1542 lines)
  - `par-term-settings-ui/src/profile_modal_ui.rs` (1406 lines)
  - `par-term-settings-ui/src/terminal_tab.rs` (1356 lines)
  - `par-term-settings-ui/src/advanced_tab.rs` (1276 lines)
- **Description**: Multiple settings UI files significantly exceed the 500-line target and 800-line threshold. Files combine UI rendering, state management, and validation logic.
- **Impact**: Difficult to navigate, understand, and maintain. Testing is harder. Code review burden increases.
- **Remedy**: Extract logical sections into sub-modules. The `window_tab/` pattern (already split into `display.rs`, `tab_bar.rs`, `panes.rs`) is a good model to follow.

---

## 🟠 High Priority Issues

### [SEC-002] External Command Renderer Allows Arbitrary Command Execution
- **Area**: Security
- **Location**: `src/prettifier/custom_renderers.rs:72`
- **Description**: The `ExternalCommandRenderer::render()` method executes arbitrary commands from user configuration (`render_command` and `render_args`). There is no validation of the command being executed.
- **Impact**: A malicious config file shared with a user could include a custom renderer that executes destructive commands when triggered.
- **Remedy**: Consider implementing a command allowlist or requiring user confirmation for custom renderers that execute external commands. At minimum, document the security implications clearly.

### [SEC-003] Dynamic Profile Fetching from Remote URLs
- **Area**: Security
- **Location**: `src/profile/dynamic.rs:160-224`
- **Description**: The application fetches profile configurations from remote URLs via HTTP. While HTTPS is recommended, HTTP URLs are still permitted without auth headers. Remote profiles are YAML that gets deserialized into configuration affecting shell execution.
- **Impact**: A Man-in-the-Middle attacker on an untrusted network could intercept HTTP profile requests and inject malicious profiles.
- **Remedy**: Enforce HTTPS-only for profile URLs, or display a prominent security warning before applying profiles fetched over HTTP.

### [ARC-004] Legacy Fields on Tab Struct with Unclear Migration Path
- **Area**: Architecture
- **Location**: `src/tab/mod.rs:79-91`
- **Description**: Multiple fields on `Tab` are marked "Legacy field: each pane has its own... Will be removed in a future version" (`scroll_state`, `mouse`, `bell`, `cache`). No migration tracking issue or deprecation timeline exists.
- **Impact**: Code duplication between Tab and Pane, confusion about which state to use, potential for inconsistent state.
- **Remedy**: Create a tracked migration plan with specific versions for deprecation warnings and removal.

### [ARC-005] Duplicate Code in Tab Constructors
- **Area**: Architecture
- **Location**: `src/tab/mod.rs:193-386` and `405-626`
- **Description**: `Tab::new()` and `Tab::new_from_profile()` share approximately 80% identical code for terminal creation, coprocess setup, session logging, and field initialization.
- **Impact**: Changes must be made in two places, increasing maintenance burden and bug risk.
- **Remedy**: Extract common initialization into a private `Tab::new_internal()` or builder pattern.

### [ARC-006] Prettifier Module Lacks Clear Boundaries
- **Area**: Architecture
- **Location**: `src/prettifier/`
- **Description**: The prettifier has 15+ submodules with overlapping concerns (`detectors/`, `renderers/`, `pipeline/`, plus top-level files). The `config_bridge.rs` at 715 lines bridges config to prettifier but couples the two systems tightly.
- **Impact**: Difficult to understand the prettifier's architecture. Adding new content types requires touching multiple files.
- **Remedy**: Consolidate into a cleaner package structure: `prettifier::detect`, `prettifier::render`, `prettifier::pipeline` as main entry points.

### [ARC-007] Three-Tier Configuration Resolution Complexity
- **Area**: Architecture
- **Location**: `par-term-config/src/`
- **Description**: Shader configuration uses a 3-tier resolution system (config → metadata → resolved) with caching. Similar patterns exist for profiles and prettifier config. Resolution logic is distributed across multiple files.
- **Impact**: Configuration bugs are hard to trace. Understanding effective configuration requires following multiple resolution steps.
- **Remedy**: Centralize resolution logic into dedicated resolver types with clear documentation of the resolution chain.

### [QA-001] Oversized Configuration Struct (1848 lines)
- **Area**: Code Quality
- **Location**: `par-term-config/src/config/config_struct/mod.rs`
- **Description**: The `Config` struct file is 1848 lines, far exceeding the 500-line target. Contains hundreds of configuration fields in a single file.
- **Impact**: Makes configuration difficult to navigate, understand, and maintain. Likely violates Single Responsibility Principle.
- **Remedy**: Split configuration into logical sub-structs (e.g., `WindowConfig`, `FontConfig`, `TerminalConfig`, `ShaderConfig`, `InputConfig`) with the main `Config` composing them.

### [QA-002] Excessive unwrap() Usage in Non-Test Code
- **Area**: Code Quality
- **Location**: Multiple files across `src/`
- **Description**: High count of `unwrap()` calls in production code paths that could panic. While many are in test modules, there are instances in production code.
- **Impact**: Runtime panics on unexpected conditions instead of graceful error handling.
- **Remedy**: Audit all `unwrap()` calls outside of tests. Replace with proper error propagation using `?` operator or `anyhow::Result`.

### [QA-003] Dead Code with #[allow(dead_code)] Annotations
- **Area**: Code Quality
- **Location**:
  - `src/app/config_updates.rs:29-119`
  - `src/app/file_transfers.rs:37-68`
  - `src/app/tmux_handler/notifications/flow_control.rs:101`
- **Description**: Multiple fields marked with `#[allow(dead_code)]` with comments indicating "future" or "planned" use. Some have become technical debt.
- **Impact**: Code bloat, confusion about which code paths are actually used, maintenance burden.
- **Remedy**: Either implement the planned functionality or remove these fields. Create tracking issues with deadlines.

### [DOC-001] Missing Environment Variables Reference Document
- **Area**: Documentation
- **Location**: Missing (should be `docs/ENVIRONMENT_VARIABLES.md`)
- **Description**: Environment variable usage is scattered throughout `par-term-config/src/config/env_vars.rs` and mentioned in `CONFIG_REFERENCE.md`, but there is no centralized reference document.
- **Impact**: Users and developers cannot easily discover what environment variables affect par-term behavior.
- **Remedy**: Create `docs/ENVIRONMENT_VARIABLES.md` with a table of all recognized environment variables (e.g., `DEBUG_LEVEL`, `RUST_LOG`, `SHELL`, `TERM`, XDG variables).

### [DOC-002] Missing rustdoc Generation Target in Makefile
- **Area**: Documentation
- **Location**: `Makefile`
- **Description**: While `make doc-open` exists for opening documentation, there is no `make doc` target that generates rustdoc without opening it.
- **Impact**: Developers cannot easily generate API documentation in CI pipelines or headless environments.
- **Remedy**: Add a `doc` target to the Makefile that runs `cargo doc --no-deps` without opening the result.

---

## 🟡 Medium Priority Issues

### Architecture

| ID | Title | Location | Description |
|----|-------|----------|-------------|
| ARC-008 | Re-exports Create Indirect Dependencies | `src/config/mod.rs`, `src/terminal.rs`, `src/renderer.rs` | Re-exports from sub-crates create unclear dependency boundaries |
| ARC-009 | Status Bar Widget System Lacks Registration | `src/status_bar/` | No formal registration mechanism or trait object pattern for widgets |
| ARC-010 | Session State Serialization Duplicate Logic | `src/session/`, `src/arrangements/` | Two forms of session persistence with similar serialization patterns |
| ARC-011 | TODO Comments Indicate Incomplete Features | Multiple files | Features like `WriteText`, `Notify`, `SetBadge` actions partially implemented |

### Security

| ID | Title | Location | Description |
|----|-------|----------|-------------|
| SEC-004 | Shell Command Execution via URL/File Handlers | `src/url_detection.rs:530-556` | `open_file_in_editor` executes shell commands with user-provided paths |
| SEC-005 | ACP Agent Permission System TOCTOU Risk | `par-term-acp/src/agent.rs:593-638` | `is_safe_write_path()` could have race conditions with symlinks |
| SEC-006 | Session Logging May Capture Credentials | `src/session_logger.rs:10-30` | Heuristic-based password redaction cannot guarantee all passwords are caught |
| SEC-007 | Trigger System Command Denylist is Bypassable | `par-term-config/src/automation.rs:283-365` | Substring matching denylist can be bypassed with encoding/obfuscation |

### Code Quality

| ID | Title | Location | Description |
|----|-------|----------|-------------|
| QA-004 | Large Settings UI Files (1400+ lines) | `par-term-settings-ui/src/` | Files exceed reasonable sizes, combine UI rendering and state management |
| QA-005 | Multiple Mutex Types Creating Deadlock Risk | Various files | Three different mutex implementations (tokio, parking_lot, std) used interchangeably |
| QA-006 | Inconsistent Error Handling Patterns | Throughout codebase | Mixes `anyhow::Result`, `Result<(), String>`, and custom error types |
| QA-007 | TODO Comments Without Tracking Issues | Multiple files | Some TODOs lack proper GitHub issue references |

### Documentation

| ID | Title | Location | Description |
|----|-------|----------|-------------|
| DOC-003 | Docstring Coverage Gap | Source files across `src/` and sub-crates | Approximately 1:1 coverage but uneven distribution |
| DOC-004 | Examples Directory Lacks Runnable Code | `examples/` | Contains YAML configs, not runnable Rust code examples |
| DOC-005 | Missing API Documentation Index | Missing `docs/API.md` | No overview document listing public types across sub-crates |
| DOC-006 | README Could Link to Quick Start More Prominently | `README.md` | Link to `docs/GETTING_STARTED.md` not immediately visible |

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

### Documentation
- **DOC-007**: Style Guide Violation: Emojis in Some Documents — `QUICK_START_FONTS.md` uses emojis
- **DOC-008**: Plan Documents Could Be Organized Separately — `docs/plans/` contains 26 internal design documents
- **DOC-009**: Architecture Document Could Include Error Handling Strategy — `ARCHITECTURE.md` lacks error handling patterns

---

## Detailed Findings

### Architecture & Design

The par-term architecture demonstrates strong foundations with a well-organized Cargo workspace separating concerns into 13 focused crates. The comprehensive `docs/ARCHITECTURE.md` (479 lines) with Mermaid diagrams, data flow descriptions, and threading model documentation is exemplary. The prettifier framework's trait-based design (`ContentDetector`, `ContentRenderer`) shows good separation of concerns and extensibility.

**Primary Concern**: The `WindowState` God Object combines too many responsibilities in a single struct. This is the most impactful architectural debt, making the codebase harder to maintain and test. Addressing this through composition-based decomposition would significantly improve maintainability.

**Key Strength**: The conditional dirty tracking, fast render path, and adaptive polling with exponential backoff demonstrate thoughtful performance engineering that is well-documented and measurable.

### Security Assessment

par-term has a generally fair security posture with several positive controls in place. The environment variable allowlist protection blocks exfiltration of sensitive variables (`AWS_SECRET_ACCESS_KEY`, `API_KEY`, `GITHUB_TOKEN`). Shell-aware command argument parsing prevents injection via crafted URLs. The self-updater is well-hardened with host allowlists, HTTPS enforcement, and SHA256 checksums.

**Primary Concern**: The exposed API token in `settings.local.json` requires immediate remediation. The external command renderer and remote profile fetching present significant attack surfaces that could be exploited through malicious configuration files.

**Recent Hardening**: Commit 5f56847 added path traversal validation, shader name validation, network URL allowlists, and binary content validation for updates.

### Code Quality

The codebase demonstrates mature Rust practices with proper error handling throughout. No critical issues were found that would cause production crashes or data loss. Documentation coverage is excellent with over 11,000 doc comment occurrences across 478 files. The project follows strong type safety with extensive use of newtypes and enums for domain concepts.

**Primary Concern**: The oversized configuration file (1848 lines) and accumulated dead code with `#[allow(dead_code)]` annotations represent the most impactful technical debt. The workspace structure with sub-crates enables independent versioning but requires coordination during updates.

**Test Coverage**: Estimated at 30-70% with appropriate PTY-dependent tests marked `#[ignore]`. Key untested areas include GPU rendering pipelines (require graphical environment) and window management operations.

### Documentation Review

par-term has exceptional documentation quality. The architecture documentation (`ARCHITECTURE.md`, `CONCURRENCY.md`, `STATE_LIFECYCLE.md`, `MUTEX_PATTERNS.md`) provides deep technical insight with Mermaid diagrams. The troubleshooting guide covers 35+ issues with consistent format. The changelog follows Keep a Changelog format meticulously.

**Primary Concern**: Missing centralized environment variables reference makes it difficult for users to discover runtime configuration options without reading source code.

**Outstanding Areas**: The getting started guide, configuration reference (200+ options documented), and contributing guide are all comprehensive and well-organized.

---

## Remediation Roadmap

### Immediate Actions (Before Next Deployment)
1. **SEC-001**: Add `.claude/settings.local.json` to `.gitignore` and rotate the exposed API token
2. **SEC-002**: Add security warning documentation for external command renderers

### Short-term (Next 1–2 Sprints)
1. **ARC-001**: Begin WindowState decomposition into focused state objects
2. **ARC-003**: Split large settings UI files into sub-modules
3. **QA-001**: Split Config struct into logical sub-structs
4. **QA-003**: Remove dead code with `#[allow(dead_code)]` annotations
5. **DOC-001**: Create environment variables reference document

### Long-term (Backlog)
1. **ARC-002**: Redesign terminal locking strategy toward message passing
2. **ARC-006**: Consolidate prettifier module structure
3. **ARC-007**: Centralize configuration resolution logic
4. **SEC-003**: Enforce HTTPS-only for profile URLs
5. **DOC-003**: Improve docstring coverage on high-traffic APIs

---

## Positive Highlights

1. **Exceptional Architecture Documentation**: The combination of `ARCHITECTURE.md`, `CONCURRENCY.md`, `STATE_LIFECYCLE.md`, and `MUTEX_PATTERNS.md` provides deep technical insight. Mermaid diagrams and state flow explanations are excellent.

2. **Outstanding Workspace Organization**: 13 focused crates with clear dependency boundaries documented in CLAUDE.md. The sub-crate dependency graph enables independent versioning.

3. **Comprehensive Troubleshooting Guide**: `TROUBLESHOOTING.md` covers 35+ issues with consistent symptom/cause/solution format, platform-specific advice, and debug logging instructions.

4. **Strong Type Safety**: Extensive use of newtypes and enums for domain concepts (`SettingsTab`, `AgentStatus`, `ShellExitAction`) prevents invalid states from being representable.

5. **Proper Async/Sync Boundaries**: Correct use of `tokio::sync::Mutex` for async contexts and `parking_lot::Mutex` for sync contexts, with documentation explaining the pattern.

6. **Comprehensive Makefile**: 70+ targets covering development workflows, testing, debugging, profiling, and deployment with clear help text.

7. **Excellent Changelog**: Follows Keep a Changelog format with detailed entries organized by category, making version history easy to understand.

8. **Security-Conscious Design**: Environment variable allowlists, shell escaping, command denylists, and recent hardening demonstrate attention to security.

---

## Audit Confidence

| Area | Files Reviewed | Confidence |
|------|---------------|-----------|
| Architecture | 35+ | High |
| Security | 25+ | High |
| Code Quality | 40+ | High |
| Documentation | 30+ | High |

*All areas have high confidence due to comprehensive agent analysis across the codebase.*

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
| SEC-001 | Exposed API Token in Local Settings File | `.claude/settings.local.json`, `.gitignore` | Critical |

#### Phase 2 — Critical Architecture (Sequential, Blocking)
<!-- Issues that restructure the codebase; must complete before Code Quality fixes. -->
| ID | Title | File(s) | Severity | Blocks |
|----|-------|---------|----------|--------|
| ARC-001 | God Object: WindowState Struct Has 50+ Fields | `src/app/window_state/mod.rs` | Critical | QA work on window_state |
| ARC-002 | Arc<Mutex<T>> Pattern Creates Locking Complexity | `src/tab/mod.rs` | Critical | New terminal access patterns |
| ARC-003 | Large Settings UI Files Exceed 1000 Lines | `par-term-settings-ui/src/input_tab.rs`, `terminal_tab.rs`, `profile_modal_ui.rs`, `advanced_tab.rs` | Critical | QA-004 (settings UI splits) |

#### Phase 3 — Parallel Execution
<!-- All remaining work, safe to run concurrently by domain. -->

**3a — Security (remaining)**
| ID | Title | File(s) | Severity |
|----|-------|---------|----------|
| SEC-002 | External Command Renderer Allows Arbitrary Command Execution | `src/prettifier/custom_renderers.rs` | High |
| SEC-003 | Dynamic Profile Fetching from Remote URLs | `src/profile/dynamic.rs` | High |
| SEC-004 | Shell Command Execution via URL/File Handlers | `src/url_detection.rs` | Medium |
| SEC-005 | ACP Agent Permission System TOCTOU Risk | `par-term-acp/src/agent.rs` | Medium |
| SEC-006 | Session Logging May Capture Credentials | `src/session_logger.rs` | Medium |
| SEC-007 | Trigger System Command Denylist is Bypassable | `par-term-config/src/automation.rs` | Medium |

**3b — Architecture (remaining)**
| ID | Title | File(s) | Severity |
|----|-------|---------|----------|
| ARC-004 | Legacy Fields on Tab Struct with Unclear Migration Path | `src/tab/mod.rs` | High |
| ARC-005 | Duplicate Code in Tab Constructors | `src/tab/mod.rs` | High |
| ARC-006 | Prettifier Module Lacks Clear Boundaries | `src/prettifier/` | High |
| ARC-007 | Three-Tier Configuration Resolution Complexity | `par-term-config/src/` | High |
| ARC-008 | Re-exports Create Indirect Dependencies | `src/config/mod.rs`, `src/terminal.rs`, `src/renderer.rs` | Medium |
| ARC-009 | Status Bar Widget System Lacks Registration | `src/status_bar/` | Medium |
| ARC-010 | Session State Serialization Duplicate Logic | `src/session/`, `src/arrangements/` | Medium |
| ARC-011 | TODO Comments Indicate Incomplete Features | Multiple | Medium |

**3c — Code Quality (all)**
| ID | Title | File(s) | Severity |
|----|-------|---------|----------|
| QA-001 | Oversized Configuration Struct (1848 lines) | `par-term-config/src/config/config_struct/mod.rs` | High |
| QA-002 | Excessive unwrap() Usage in Non-Test Code | Multiple files in `src/` | High |
| QA-003 | Dead Code with #[allow(dead_code)] Annotations | `src/app/config_updates.rs`, `src/app/file_transfers.rs`, `src/app/tmux_handler/notifications/flow_control.rs` | High |
| QA-004 | Large Settings UI Files (1400+ lines) | `par-term-settings-ui/src/` | Medium |
| QA-005 | Multiple Mutex Types Creating Deadlock Risk | Various files | Medium |
| QA-006 | Inconsistent Error Handling Patterns | Throughout codebase | Medium |
| QA-007 | TODO Comments Without Tracking Issues | Multiple files | Medium |

**3d — Documentation (all)**
| ID | Title | File(s) | Severity |
|----|-------|---------|----------|
| DOC-001 | Missing Environment Variables Reference Document | `docs/ENVIRONMENT_VARIABLES.md` (create) | High |
| DOC-002 | Missing rustdoc Generation Target in Makefile | `Makefile` | High |
| DOC-003 | Docstring Coverage Gap | Source files across `src/` | Medium |
| DOC-004 | Examples Directory Lacks Runnable Code | `examples/` | Medium |
| DOC-005 | Missing API Documentation Index | `docs/API.md` (create) | Medium |
| DOC-006 | README Could Link to Quick Start More Prominently | `README.md` | Medium |

### File Conflict Map
<!-- Files touched by issues in multiple domains. Fix agents must read current file state
     before editing — a prior agent may have already changed these. -->

| File | Domains | Issues | Risk |
|------|---------|--------|------|
| `par-term-settings-ui/src/input_tab.rs` | Architecture + Code Quality | ARC-003, QA-004 | ⚠️ Read before edit |
| `par-term-settings-ui/src/terminal_tab.rs` | Architecture + Code Quality | ARC-003, QA-004 | ⚠️ Read before edit |
| `par-term-settings-ui/src/profile_modal_ui.rs` | Architecture + Code Quality | ARC-003, QA-004 | ⚠️ Read before edit |
| `par-term-settings-ui/src/advanced_tab.rs` | Architecture + Code Quality | ARC-003, QA-004 | ⚠️ Read before edit |
| `src/tab/mod.rs` | Architecture (multiple) | ARC-002, ARC-004, ARC-005 | ⚠️ Read before edit |
| `par-term-acp/src/agent.rs` | Security + Code Quality | SEC-005, QA-004 | ⚠️ Read before edit |
| `src/app/tmux_handler/notifications/flow_control.rs` | Architecture + Code Quality | ARC-011, QA-003 | ⚠️ Read before edit |
| `src/app/window_manager/scripting.rs` | Architecture + Code Quality | ARC-011, QA-007 | ⚠️ Read before edit |
| `src/app/input_events/snippet_actions.rs` | Architecture + Code Quality | ARC-011, QA-007 | ⚠️ Read before edit |
| `Makefile` | Architecture + Documentation | ARC-012, DOC-002 | ⚠️ Read before edit |
| `par-term-config/src/` | Architecture + Security + Code Quality | ARC-007, SEC-007, QA-001 | ⚠️ Read before edit |

### Blocking Relationships
<!-- Explicit dependency declarations from audit agents.
     Format: [blocker issue] → [blocked issue] — reason -->
- ARC-001 → QA work on window_state: WindowState decomposition must complete before code quality improvements to window_state/mod.rs
- ARC-002 → New terminal access patterns: Terminal locking strategy changes must complete before adding new terminal access code
- ARC-003 → QA-004: Settings UI file splits must complete before code quality work on those files
- QA-001 → QA-004: Config struct refactor should complete before settings UI refactoring since settings UI depends on config types
- QA-003 → Downstream dependency checks: Dead code removal requires confirming no sub-crate dependencies first
- SEC-001 → All other work: API token rotation must happen immediately

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

    %% Explicit blocker edges
    SEC001["SEC-001"] -->|blocks all| ARC001["ARC-001"]
    ARC001["ARC-001"] -->|blocks| QA_WS["QA work on window_state"]
    ARC002["ARC-002"] -->|blocks| NewTerm["New terminal access patterns"]
    ARC003["ARC-003"] -->|blocks| QA004["QA-004"]
    QA001["QA-001"] -->|blocks| QA004
