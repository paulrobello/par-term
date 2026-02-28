# Project Audit Report

> **Project**: par-term
> **Date**: 2026-02-27
> **Updated**: 2026-02-28 (post-remediation #2 — PR #206 resolved issues removed)
> **Stack**: Rust (Edition 2024), wgpu (GPU rendering), Tokio (async runtime), egui (settings UI)
> **Audited by**: Claude Code Audit System

---

## Executive Summary

par-term is a mature, feature-rich terminal emulator with excellent documentation and a well-organized workspace structure. After two remediation passes (PRs #205 and #206), **4 open issues remain** — all architectural debt requiring multi-sprint structural refactoring. All security issues have been resolved. The large settings UI files have been split. The remaining work is the `WindowState` God Object decomposition, the `Arc<Mutex>` locking pattern redesign, and the Config struct split — all classified as Backlog items requiring coordinated effort.

### Remaining Issue Count by Severity

| Severity | Architecture | Security | Code Quality | Total |
|----------|:-----------:|:--------:|:------------:|:-----:|
| 🔴 Critical | 2 | 0 | 0 | **2** |
| 🟠 High     | 1 | 0 | 1 | **2** |
| **Total**   | **3** | **0** | **1** | **4** |

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

---

## 🟠 High Priority Issues

### [ARC-005] Duplicate Code in Tab Constructors
- **Area**: Architecture
- **Location**: `src/tab/mod.rs:193-386` and `405-626`
- **Description**: `Tab::new()` and `Tab::new_from_profile()` share ~80% identical code. Refactoring plan was documented in remediation but not implemented.
- **Impact**: Changes must be made in two places, increasing maintenance burden and bug risk.
- **Remedy**: Extract shared initialization into a private `Tab::new_internal()` as documented in the `# REFACTOR` sections of each constructor.

### [QA-001] Oversized Configuration Struct (1848 lines)
- **Area**: Code Quality
- **Location**: `par-term-config/src/config/config_struct/mod.rs`
- **Description**: The `Config` struct file is 1848 lines. Section grouping comments and a refactoring plan were added in remediation, but the struct was not split.
- **Impact**: Configuration is difficult to navigate, understand, and maintain.
- **Remedy**: Split into logical sub-structs (`WindowConfig`, `FontConfig`, `TerminalConfig`, `ShaderConfig`, `InputConfig`) as documented in the module-level comment.

---

## Remediation Roadmap

### Backlog (all remaining issues require multi-sprint coordinated effort)
1. **ARC-001**: WindowState decomposition — extract `UpdateState`, `FocusState`, `TransientOverlayState` one sub-struct at a time; start with smallest blast radius
2. **ARC-002**: Arc<Mutex> locking — audit call sites for read vs write, convert reads to `RwLock::read()` as a first step
3. **ARC-005**: Extract `Tab::new_internal()` — implementation plan documented in `# REFACTOR` sections of each constructor
4. **QA-001**: Config struct split — use `#[serde(flatten)]` technique documented in module-level comment; start with `ScreenshotConfig` or `UpdateConfig` section
