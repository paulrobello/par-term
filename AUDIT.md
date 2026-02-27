# par-term Code Audit

**Date**: 2026-02-27
**Version**: 0.24.0
**Scope**: Full codebase review covering architecture, security, performance, correctness, GPU resource management, error handling, concurrency, code quality, and test coverage.

---

## Summary

All Critical (2), High (12), and most Medium/Low issues have been addressed. The items below are deferred for future work due to requiring significant refactoring or API changes.

---

## Deferred Items

### M-1: Self-Update Uses HTTPS But No Certificate Pinning

**File**: `par-term-update/src/http.rs:17-28`
**Category**: Supply Chain Security

The update mechanism downloads binaries from GitHub over HTTPS using system root certificates. No certificate pinning for github.com endpoints.

**Fix**: Consider signing release binaries with a project-specific key and verifying signatures during update (independent of TLS).

---

### M-2: Trigger RunCommand Denylist is Bypassable

**File**: `par-term-config/src/automation.rs:252-333`
**Category**: Security

The denylist uses simple substring matching. When `require_user_action` is false, the denylist can be bypassed via: `/usr/bin/env rm -rf /`, `sh -c "rm -rf /"`, or commands not in the list.

**Fix**: Consider an allowlist approach when `require_user_action` is false. Document limitations prominently.

---

### M-7: `try_lock` Pattern May Silently Drop Operations

**Files**: Multiple files in `src/app/`
**Category**: Concurrency

The codebase uses `try_lock()` on `tokio::sync::Mutex<TerminalManager>`. Silently failing operations include resize, theme changes, and focus events.

**Fix**: Add metrics/counters for `try_lock` failures to detect contention issues.

---

### M-8: Config Saved from Multiple Locations Without Coordination

**Files**: Multiple locations across `src/app/`
**Category**: Data Integrity

Config is saved to disk from several independent locations. Near-simultaneous saves could overwrite each other's changes.

**Fix**: Centralize config saves to a single debounced location or use a file-level lock.

---

### M-9: Temporary Mutation of `window_opacity` in `render_to_texture`

**File**: `par-term-render/src/cell_renderer/render.rs:166-233`
**Category**: Correctness

The method saves `self.window_opacity`, sets it to `1.0`, renders, then restores. Early returns via `?` after mutation would skip restoration.

**Fix**: Pass opacity as parameter to uniform update function rather than mutating state. Or use a scoped guard.

---

### M-10: Massive Code Duplication in GLSL Transpiler

**File**: `par-term-render/src/custom_shader_renderer/transpiler.rs:83-403 vs 408-710`
**Category**: Maintainability

`transpile_glsl_to_wgsl()` and `transpile_glsl_to_wgsl_source()` duplicate ~300 lines of GLSL wrapper template and post-processing logic.

**Fix**: Extract shared logic into a private `transpile_impl()` function.

---

### M-11: Fragile String-Based WGSL Post-Processing in Transpiler

**File**: `par-term-render/src/custom_shader_renderer/transpiler.rs:346-366,653-673`
**Category**: Correctness / Robustness

Post-transpilation string replacements to inject `@builtin(position)` depend on exact whitespace patterns in naga's output. A naga version change could silently break these replacements.

**Fix**: Add validation that required code was actually injected. Return error if not.

---

### M-15: `display_lines()` Clones All Styled Lines Every Call

**File**: `src/prettifier/buffer.rs:47-58`
**Category**: Performance

Returns `Vec<StyledLine>` by cloning every line. In a rendering loop, this causes heap allocations proportional to rendered line count every frame.

**Fix**: Return `&[StyledLine]` for the rendered path, or provide a borrowing alternative.

---

### M-16: `section_matches` Function Duplicated 19 Times

**File**: All 19 files in `par-term-settings-ui/src/*_tab.rs`
**Category**: Code Duplication

Identical function copy-pasted into all 19 settings tab files.

**Fix**: Extract into the shared `section` module.

---

### M-17: Horizontal/Vertical Tab Rendering Code Duplication

**File**: `src/tab_bar_ui/mod.rs:947-1100+ vs 586-700+`
**Category**: Maintainability

`render_tab_with_width` and `render_vertical_tab` share ~35 lines of duplicated bg-color computation and similar patterns.

**Fix**: Extract shared logic into helper methods.

---

### M-18: Modifier Remapping Cannot Distinguish Left/Right When Both Pressed

**File**: `par-term-keybindings/src/matcher.rs:142-214`
**Category**: Correctness

When both left and right modifiers of the same type are pressed, the left mapping always wins.

**Fix**: Use the physical key code from the event to apply the correct side's remapping.

---

### M-20: `RenameArrangement` Uses Magic Sentinel Strings

**File**: `src/app/handler/app_handler_impl.rs:134-140`
**Category**: Code Quality

Overloads `new_name` with `"__move_up__"` / `"__move_down__"` sentinels instead of separate action variants.

**Fix**: Add dedicated `MoveArrangementUp(id)` and `MoveArrangementDown(id)` variants.

---

### L-3: Per-Frame Uniform Buffer Allocation for Pane Backgrounds

**File**: `par-term-render/src/cell_renderer/background.rs:542-612`
**Category**: GPU Performance

`create_pane_bg_bind_group()` creates a new buffer and bind group every frame per pane.

**Fix**: Pre-allocate buffers and reuse with `queue.write_buffer()`.

---

### L-5: Opaque 7-Element Tuple for Graphics Render Parameters

**File**: `par-term-render/src/renderer/mod.rs`, `par-term-render/src/graphics_renderer.rs:340`
**Category**: Code Quality

Graphics data passed as `Vec<(u64, isize, usize, usize, usize, f32, usize)>` with no named fields.

**Fix**: Define a named `GraphicRenderInfo` struct.

---

### L-6: Per-Frame Vec Allocations in Instance Buffer Building

**File**: `par-term-render/src/cell_renderer/instance_buffers.rs:21-22`
**Category**: Performance

For every dirty row, `Vec::with_capacity()` is called for `row_bg` and `row_text`.

**Fix**: Pre-allocate reusable scratch buffers as fields on `CellRenderer`.

---

### L-9: Regex Compilation in Hot Path for Trigger Prettify

**File**: `src/app/triggers.rs:464`
**Category**: Performance

`command_filter` and `block_end` regexes compiled fresh each time a prettify trigger fires.

**Fix**: Cache compiled regexes keyed by pattern string.

---

### L-12: Tab Bar File is 1895 Lines

**File**: `src/tab_bar_ui/mod.rs`
**Category**: Maintainability

Significantly exceeds the project's 500-line target and 800-line refactoring threshold.

**Fix**: Extract into sub-modules: `context_menu.rs`, `drag_drop.rs`, `profile_menu.rs`, `tab_rendering.rs`.

---

### L-13: Per-Frame String Allocations in `section_matches`

**Files**: All settings tab files
**Category**: Performance

Each call creates `to_lowercase()` allocations for title and all keywords.

**Fix**: Since query is already lowercased by callers, use `eq_ignore_ascii_case()` or pre-lowercase constants.

---

### L-14: Settings Window Tests Are Minimal

**File**: `tests/settings_window_tests.rs` (71 lines)
**Category**: Test Coverage

Only tests that enum variants exist and derive macros work.

**Fix**: Add tests for `section_matches`, setting validation ranges, and `has_changes` state machine.

---

### L-15: Tab Bar UI Tests Lack Interaction Coverage

**File**: `tests/tab_bar_ui_tests.rs` (530 lines)
**Category**: Test Coverage

No tests for drag-and-drop, context menu lifecycle, frame guards, or drop target calculation.

**Fix**: Test state machine logic (drag state transitions, context menu lifecycle) without rendering.

---

### L-17: Config File Watcher Uses PollWatcher at 500ms

**File**: `par-term-config/src/watcher.rs:120`
**Category**: Performance

Uses `PollWatcher` instead of native filesystem events. Polls every 500ms even when no changes occur.

**Fix**: Use `notify::RecommendedWatcher` (inotify/FSEvents/ReadDirectoryChanges). Fall back to `PollWatcher` on failure.

---

## Info (Positive Findings & Notes)

### I-1: No Unsafe Code in `src/app/` or Rendering Pipeline

The entire `src/app/` directory and `par-term-render` crate contain zero `unsafe` blocks. GPU interop is handled entirely through `bytemuck` derive macros and wgpu's safe API.

### I-2: Well-Structured Concurrency Model

The `try_lock` pattern for accessing `tokio::sync::Mutex<TerminalManager>` from the synchronous winit event loop is consistently applied with clear comments at every call site.

### I-3: Comprehensive Trigger Security Model

Three-layer defense: `require_user_action` flag (default true), command denylist, and rate limiting. Well-documented threat model.

### I-4: Good URL Injection Prevention

`expand_link_handler()` correctly parses command templates before URL substitution, preventing argument injection. Tested explicitly.

### I-5: ACP Agent Path Traversal Protection

Write path validation uses `canonicalize()` to resolve symlinks and `..` components. Non-absolute paths rejected. Tested.

### I-6: MCP IPC File Permissions

IPC files created with 0o600 permissions from creation (via `OpenOptionsExt::mode()`), avoiding TOCTOU race. Tested.

### I-7: Editor Command Shell Escaping

File paths shell-escaped before interpolation into editor commands. Tested.

### I-8: No Hardcoded Credentials

No API keys, tokens, passwords, or credentials found anywhere in the codebase.

### I-9: Clean Shutdown Design

`Drop` for `WindowState` signals shutdown first, hides window for instant feedback, spawns PTY cleanup on background threads.

### I-10: Good Dirty Flag Optimization

Row-level dirty tracking and frame-level dirty flags effectively skip unnecessary GPU work.

### I-11: Good Test Coverage in Prettifier and Snippets

Prettifier subsystem has excellent unit test coverage with well-structured mocks. Snippets/actions tests (1120 lines) are thorough.

### I-12: tmux Shell Escaping is Correct

Session names properly escaped using POSIX single-quote escaping pattern. Applied consistently.
