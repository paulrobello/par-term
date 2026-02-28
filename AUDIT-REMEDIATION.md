# Audit Remediation Report

> **Project**: par-term
> **Audit Date**: 2026-02-27
> **Remediation Date**: 2026-02-28
> **Severity Filter Applied**: all

---

## Execution Summary

| Phase | Status | Agent | Issues Targeted | Resolved | Partial | Manual |
|-------|--------|-------|----------------|----------|---------|--------|
| 1 — Critical Security | ⏭️ Skipped | — | 0 | 0 | 0 | 0 |
| 2 — Critical Architecture | ✅ Complete | fix-architecture | 3 | 1 | 0 | 2 |
| 3a — High+ Security | ✅ Complete | fix-security | 5 | 5 | 0 | 0 |
| 3b — High+ Architecture | ✅ Complete | fix-architecture | 6 | 6 | 0 | 0 |
| 3c — All Code Quality | ✅ Complete | fix-code-quality | 7 | 6 | 1 | 0 |
| 3d — All Documentation | ⏭️ Skipped | — | 0 | 0 | 0 | 0 |
| 4 — Verification | ✅ Pass | — | — | — | — | — |

**Overall**: 18 issues resolved, 1 partial (documented), 2 deferred as Backlog per audit roadmap.

---

## Resolved Issues ✅

### Architecture

- **[ARC-003]** Large Settings UI Files Exceed 1000 Lines — Split all three remaining oversized files into subdirectories following the `input_tab/` pattern:
  - `terminal_tab.rs` (1356 lines) → `terminal_tab/` (mod.rs + behavior, unicode, shell, startup, search, semantic_history)
  - `advanced_tab.rs` (1276 lines) → `advanced_tab/` (mod.rs + import_export, tmux, logging, system)
  - `profile_modal_ui.rs` (1406 lines) → `profile_modal_ui/` (mod.rs + form_helpers, list_view, edit_view)
- **[ARC-004]** Legacy Fields on Tab Struct — Added precise `LEGACY:` migration comments to all four legacy fields (`scroll_state`, `mouse`, `bell`, `cache`) naming exact call sites and step-by-step migration plans.
- **[ARC-005]** Duplicate Code in Tab Constructors — Extracted ~80% shared initialization into private `Tab::new_internal()` with `TabInitParams` struct; both constructors now contain only their unique logic.
- **[ARC-006]** Prettifier Module Lacks Clear Boundaries — Architecture doc comment was already present from prior remediation; confirmed complete.
- **[ARC-007]** Three-Tier Configuration Resolution Complexity — Added `# Role in the Three-Tier Resolution Chain` doc section to `shader_metadata.rs` cross-referencing the existing 3-tier documentation.
- **[ARC-012]** Makefile Has Duplicate Build Logic — Added `DEBUG_LOG` and `RUN_BASE` variables; de-duplicated 12 targets.
- **[ARC-013]** Test Organization Could Mirror Source Structure — Added documentation in `Cargo.toml` describing logical test groupings and future migration path.
- **[ARC-014]** Log Crate Bridge Complexity — Added `# Logging Quick Reference` table to `src/main.rs` comparing `crate::debug_*!()` vs `log::*!()` with when-to-use guidance.

### Security

- **[SEC-002]** External Command Renderer Allows Arbitrary Command Execution — Added `allowed_commands: Vec<String>` to `RendererConfig`, `PrettifierYamlConfig`, and `ResolvedPrettifierConfig`. `ExternalCommandRenderer::render()` now checks the command basename against the allowlist; if non-empty and command not listed, execution is refused. Empty allowlist (default) warns but allows for backward compatibility.
- **[SEC-003]** Dynamic Profile Fetching from Remote URLs — Added `allow_http_profiles: bool` (default `false`) to `Config`. HTTP profile URLs now return an error by default; opt-in via config with warning logged. Authentication headers over HTTP remain blocked unconditionally.
- **[SEC-008]** Unsafe Blocks for Platform-Specific Code — Expanded `// SAFETY:` comment in `macos_metal.rs` to full multi-line justification; other blocks were already adequately documented.
- **[SEC-009]** Test Code Uses Unsafe env::set_var — Added detailed `# Safety` doc comments to test helper functions in `config_tests.rs` and `par-term-mcp/src/lib.rs`.
- **[SEC-010]** HTTP Client for Self-Update Uses Hardcoded Hosts — Added comprehensive module-level security design doc to `par-term-update/src/http.rs` covering HTTPS enforcement, SSRF/DNS-rebinding prevention, response size caps, and binary validation.

### Code Quality

- **[QA-002]** Excessive unwrap() Usage — Full audit confirmed every remaining `.unwrap()` is inside `#[cfg(test)]` or test-only functions. Prior remediation (`expect()` for LazyLock regex) was sufficient; no production `unwrap()` calls remain.
- **[QA-003]** Dead Code with #[allow(dead_code)] — Confirmed all three locations have accurate `TODO(dead_code)` comments with v0.26 deadlines from prior remediation.
- **[QA-004]** Large Settings UI Files (Medium) — Resolved as part of ARC-003 (Phase 2).
- **[QA-008]** Excessive #[allow(clippy::too_many_arguments)] — Removed 2 false-positive exemptions; added explanatory comments to 11 genuine exemptions documenting the correct future fix for each.
- **[QA-009]** Magic Numbers in UI Code — Extracted 9 inline color/size literals in `sidebar.rs` into named constants (`COLOR_TAB_DIMMED`, `COLOR_TAB_SELECTED`, `TAB_BUTTON_WIDTH`, etc.).
- **[QA-010]** Test File Size — Reviewed 26 test files; determined acceptable as-is (largest is `config_tests.rs` at 1498 lines covering comprehensive config parsing).

---

## Requires Manual Intervention / Deferred 🔧

### [ARC-001] God Object: WindowState Struct Has 50+ Fields
- **Why deferred**: ARC-001 is classified as "Backlog" in the audit's own Remediation Roadmap. The struct is already 347 lines (below the 500-line threshold), has section headers, a field groups table, and a "Future decomposition candidates" list. Extracting sub-structs requires coordinated updates across 14+ files in `src/app/`.
- **Recommended approach**: Extract one sub-struct at a time, starting with `UpdateState` (3 files), then `FocusState` (6 files), then `TransientOverlayState` (4 files). Each extraction should be a separate PR with `make checkall` verification.
- **Estimated effort**: Large (multi-sprint)

### [ARC-002] Arc<Mutex<T>> Pattern Creates Locking Complexity
- **Why deferred**: ARC-002 is classified as "Backlog" in the audit's Remediation Roadmap. Locking rules are already comprehensively documented. `RwLock` conversion would only help if there were concurrent reads (there are not). MPSC redesign requires complete `TerminalManager` API overhaul.
- **Recommended approach**: `RwLock` conversion is the lower-risk path — audit each `terminal.lock().await` call site to distinguish read vs write access, then convert reads to `terminal.read().await`.
- **Estimated effort**: Large (architectural)

### [QA-001] Oversized Configuration Struct (1848 lines)
- **Why partial**: Full split into sub-structs using `#[serde(flatten)]` is non-trivial and requires careful YAML round-trip testing across all 40+ sections. A "How to safely split the struct" section was added to the module doc comment explaining the `#[serde(flatten)]` technique and mapping sections to candidate sub-structs.
- **Recommended approach**: Start with a low-risk section (e.g., `ScreenshotConfig` or `UpdateConfig`) and validate YAML serialization round-trips before tackling larger sections.
- **Estimated effort**: Medium per section

---

## Verification Results

- **Build**: ✅ Pass
- **Tests**: ✅ Pass (1033 tests across workspace)
- **Lint (clippy)**: ✅ Pass (0 warnings)
- **Format**: ✅ Pass

Two regressions introduced by Phase 3 agents were fixed before final commit:
1. `src/tab/mod.rs:371` — clippy `collapsible_if` lint from the `Tab::new_internal()` refactor (ARC-005) — collapsed nested `if let` blocks.
2. `src/prettifier/config_bridge.rs:289` — missing `allowed_commands` field in a test struct initializer after SEC-002 added the field to `ResolvedPrettifierConfig`.

---

## Files Changed

### Phase 2 — Critical Architecture
**Created**:
- `par-term-settings-ui/src/terminal_tab/mod.rs`
- `par-term-settings-ui/src/terminal_tab/behavior.rs`
- `par-term-settings-ui/src/terminal_tab/unicode.rs`
- `par-term-settings-ui/src/terminal_tab/shell.rs`
- `par-term-settings-ui/src/terminal_tab/startup.rs`
- `par-term-settings-ui/src/terminal_tab/search.rs`
- `par-term-settings-ui/src/terminal_tab/semantic_history.rs`
- `par-term-settings-ui/src/advanced_tab/mod.rs`
- `par-term-settings-ui/src/advanced_tab/import_export.rs`
- `par-term-settings-ui/src/advanced_tab/tmux.rs`
- `par-term-settings-ui/src/advanced_tab/logging.rs`
- `par-term-settings-ui/src/advanced_tab/system.rs`
- `par-term-settings-ui/src/profile_modal_ui/mod.rs`
- `par-term-settings-ui/src/profile_modal_ui/form_helpers.rs`
- `par-term-settings-ui/src/profile_modal_ui/list_view.rs`
- `par-term-settings-ui/src/profile_modal_ui/edit_view.rs`

**Deleted**:
- `par-term-settings-ui/src/terminal_tab.rs`
- `par-term-settings-ui/src/advanced_tab.rs`
- `par-term-settings-ui/src/profile_modal_ui.rs`

### Phase 3a — Security
**Modified**:
- `src/prettifier/custom_renderers.rs`
- `src/prettifier/traits.rs`
- `src/prettifier/config_bridge.rs`
- `par-term-config/src/config/prettifier.rs`
- `par-term-config/src/config/config_struct/mod.rs`
- `par-term-config/src/config/config_struct/default_impl.rs`
- `par-term-config/src/profile.rs`
- `src/profile/dynamic.rs`
- `src/app/window_manager/mod.rs`
- `src/app/handler/app_handler_impl.rs`
- `src/macos_metal.rs`
- `tests/config_tests.rs`
- `par-term-mcp/src/lib.rs`
- `par-term-update/src/http.rs`

### Phase 3b — Architecture
**Modified**:
- `src/tab/mod.rs`
- `par-term-config/src/shader_metadata.rs`
- `src/main.rs`
- `Makefile`
- `Cargo.toml`

### Phase 3c — Code Quality
**Modified**:
- `par-term-config/src/config/config_struct/mod.rs`
- `par-term-render/src/cell_renderer/background.rs`
- `par-term-render/src/cell_renderer/block_chars/snapping.rs`
- `par-term-render/src/cell_renderer/mod.rs`
- `par-term-render/src/cell_renderer/pane_render.rs`
- `par-term-render/src/custom_shader_renderer/mod.rs`
- `par-term-render/src/graphics_renderer.rs`
- `par-term-render/src/renderer/shaders.rs`
- `par-term-render/src/scrollbar.rs`
- `par-term-settings-ui/src/background_tab/pane_backgrounds.rs`
- `par-term-settings-ui/src/sidebar.rs`
- `src/app/window_state/render_pipeline/pane_render.rs`
- `src/tab_bar_ui/tab_rendering.rs`

### Phase 4 — Verification Fixes
**Modified**:
- `src/tab/mod.rs` (collapsible_if lint)
- `src/prettifier/config_bridge.rs` (missing field in test initializer)

---

## Next Steps

1. Review **Requires Manual Intervention** items above and assign to team members:
   - ARC-001 (WindowState decomposition) — multi-sprint architectural effort
   - ARC-002 (Arc<Mutex> → RwLock) — architectural decision required
   - QA-001 (Config struct split) — start with one section, validate YAML round-trips
2. Re-run `/audit` to get an updated AUDIT.md reflecting current state (22 → ~3 open issues)
3. Merge `fix/audit-remediation` branch via squash merge: `gh pr merge --squash --delete-branch`
