# H9 Module Extraction Plan — window_state.rs + Borderline Files

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Resolve AUDIT.md H9 finding by converting `window_state.rs` (6,461L) to a module with sub-files, and trim two borderline files.

**Architecture:** Three parallel tracks in Phase 1, then sequential Phase 2 sub-file extractions from `window_state/mod.rs`. Each sub-file contains an `impl WindowState { ... }` block following the existing pattern in `src/app/handler/window_state_impl/`.

**Tech Stack:** Rust 2024, `make build` (dev-release, ~30-40s), `make lint`, `make test`.

**Key rule:** When extracting methods to a sub-file, the file needs:
```rust
use crate::app::window_state::WindowState;
// + all types used by the methods being moved
```
And `window_state/mod.rs` needs `mod sub_file_name;` added (order matters: add before any methods that call into the sub-file, but Rust doesn't actually require ordering — just add alphabetically at the top of the `mod` declarations).

---

## PHASE 1 — Three parallel tasks (Tasks 1, 2, 3 run concurrently)

---

### Task 1: Trim `keybinding_actions.rs` (865L → ~557L)

**Files:**
- Modify: `src/app/input_events/keybinding_actions.rs`
- Create: `src/app/input_events/snippet_actions.rs`
- Modify: `src/app/input_events/mod.rs`

**What to move:** `execute_snippet` (lines 557-631, 75L) and `execute_custom_action` (lines 632-864, 233L) to a new file. These are helper functions called from `execute_keybinding_action`.

**Step 1:** Create `src/app/input_events/snippet_actions.rs`:

```rust
//! Snippet and custom action execution for WindowState keybindings.

use crate::app::window_state::WindowState;

impl WindowState {
    // Paste the exact bodies of execute_snippet and execute_custom_action here.
    // They start at line 557 and 632 respectively in keybinding_actions.rs.
}
```

**Step 2:** In `keybinding_actions.rs`, delete lines 557-864 (the two methods). The file should now end around line 556.

**Step 3:** Add `mod snippet_actions;` to `src/app/input_events/mod.rs` (append after the existing `mod keybinding_actions;`).

**Step 4:** Build and verify:
```bash
cd /Users/probello/Repos/par-term && make build 2>&1 | tail -5
```
Expected: `Compiling par-term` then `Finished`.

**Step 5:** Lint:
```bash
make lint 2>&1 | grep -E "^error|warning\[" | head -20
```

**Step 6:** Commit:
```bash
git add src/app/input_events/keybinding_actions.rs src/app/input_events/snippet_actions.rs src/app/input_events/mod.rs
git commit -m "refactor(input): extract snippet/custom action helpers to snippet_actions.rs — H9"
```

---

### Task 2: Trim `gateway.rs` (841L → ~611L)

**Files:**
- Modify: `src/app/tmux_handler/gateway.rs`
- Create: `src/app/tmux_handler/gateway_input.rs`
- Modify: `src/app/tmux_handler/mod.rs`

**What to move:** Five input-routing methods to a new file:
- `send_input_via_tmux` (lines 283-330, 48L)
- `format_send_keys_for_window` (lines 331-343, 13L)
- `send_input_via_tmux_window` (lines 344-383, 40L)
- `paste_via_tmux` (lines 384-416, 33L)
- `handle_tmux_prefix_key` (lines 745-840, 96L)

Total moved: ~230L. Remaining in gateway.rs: ~611L.

**Step 1:** Create `src/app/tmux_handler/gateway_input.rs`:

```rust
//! tmux input routing: send_input_via_tmux, paste_via_tmux, prefix key handling.

use crate::app::window_state::WindowState;

impl WindowState {
    // Paste the exact bodies of the 5 methods listed above.
    // Preserve all their doc comments and attributes.
}
```

**Step 2:** Delete those 5 methods from `gateway.rs`. Line numbers will shift as you delete — work top-to-bottom OR bottom-to-top (bottom-to-top is safer to avoid drift):
- Delete `handle_tmux_prefix_key` (lines 745-840) first
- Then delete `paste_via_tmux` (lines 384-416)
- Then delete `send_input_via_tmux_window` (lines 344-383)
- Then delete `format_send_keys_for_window` (lines 331-343)
- Then delete `send_input_via_tmux` (lines 283-330)

**Step 3:** Add `mod gateway_input;` to `src/app/tmux_handler/mod.rs`.

**Step 4:** Build:
```bash
cd /Users/probello/Repos/par-term && make build 2>&1 | tail -5
```

**Step 5:** Commit:
```bash
git add src/app/tmux_handler/gateway.rs src/app/tmux_handler/gateway_input.rs src/app/tmux_handler/mod.rs
git commit -m "refactor(tmux): extract input routing to gateway_input.rs — H9"
```

---

### Task 3: Convert `window_state.rs` → `window_state/mod.rs`

**Files:**
- Create: `src/app/window_state/` (directory)
- Rename: `src/app/window_state.rs` → `src/app/window_state/mod.rs`

This is a filesystem operation only — no code changes. The `mod window_state;` in `src/app/mod.rs` automatically resolves to the directory's `mod.rs`.

**Step 1:** Create directory and move file:
```bash
mkdir -p /Users/probello/Repos/par-term/src/app/window_state
mv /Users/probello/Repos/par-term/src/app/window_state.rs /Users/probello/Repos/par-term/src/app/window_state/mod.rs
```

**Step 2:** Build immediately to confirm nothing broke:
```bash
cd /Users/probello/Repos/par-term && make build 2>&1 | tail -5
```
Expected: identical output to before. If errors, check that no other code uses path-based imports that assumed `window_state.rs`.

**Step 3:** Commit:
```bash
git add src/app/window_state/
git add -u src/app/window_state.rs  # tells git the old file is gone
git commit -m "refactor(app): convert window_state.rs to window_state/ module — H9"
```

Note: `git add -u` stages the deletion of the old path. Or use `git mv src/app/window_state.rs src/app/window_state/mod.rs` (requires the directory to exist first — create it, then git mv, then build).

---

## PHASE 2 — Sequential sub-file extractions from `window_state/mod.rs`

**Wait for Task 3 to complete before starting Phase 2.**

After Phase 2, `window_state/mod.rs` should be ~1,200L (down from 6,461L).

Method line numbers in this phase reference `window_state/mod.rs` as it shrinks. **Always grep for the method name to find current line numbers before editing.**

---

### Task 4: Extract `render_pipeline.rs` (~2,800L)

This is the largest extraction. It covers the rendering phase methods plus the private helper structs they use.

**Files:**
- Create: `src/app/window_state/render_pipeline.rs`
- Modify: `src/app/window_state/mod.rs`

**Methods to move:**

| Method | Current approx. lines |
|--------|----------------------|
| `render` | ~24L |
| `should_render_frame` | ~18L |
| `update_frame_metrics` | ~14L |
| `update_animations` | ~26L |
| `sync_layout` | ~56L |
| `gather_render_data` | ~641L |
| `submit_gpu_frame` | ~1,454L |
| `update_post_render_state` | ~301L |
| `render_split_panes_with_data` | ~81L |

**Private structs to move** (currently near top of mod.rs, lines ~61-176):
- `RendererSizing`
- `PaneRenderData`
- `FrameRenderData`
- `PostRenderActions` + its `impl Default`

Also move `PreservedClipboardImage` and `ClipboardImageClickGuard` if they are only used by render methods (check with grep first).

**Step 1:** Grep to confirm current line numbers:
```bash
grep -n "^    pub(crate) fn render\b\|^    fn render\b\|^    fn should_render_frame\|^    fn update_frame_metrics\|^    fn update_animations\|^    fn sync_layout\b\|^    fn gather_render_data\|^    fn submit_gpu_frame\|^    fn update_post_render_state\|^    fn render_split_panes_with_data\|^struct RendererSizing\|^struct PaneRenderData\|^struct FrameRenderData\|^struct PostRenderActions\|^struct PreservedClipboard\|^pub.* struct Clipboard" src/app/window_state/mod.rs
```

**Step 2:** Create `src/app/window_state/render_pipeline.rs` starting with a module doc comment and the struct definitions, followed by the impl block containing all methods:

```rust
//! GPU render pipeline for WindowState.
//!
//! Contains:
//! - `render`: per-frame orchestration entry point
//! - `should_render_frame`, `update_frame_metrics`, `update_animations`, `sync_layout`: frame setup
//! - `gather_render_data`: snapshot terminal state into FrameRenderData
//! - `submit_gpu_frame`: egui + wgpu render pass, returns PostRenderActions
//! - `update_post_render_state`: dispatch post-render action queue
//! - `render_split_panes_with_data`: multi-pane layout rendering

use crate::app::window_state::WindowState;
// Add any additional `use` imports needed by the methods below.
// Check what types appear in method bodies and add them here.
// Common ones: crate::cell_renderer::*, crate::renderer::*, crate::tab::*,
//   par_term_emu_core_rust::*, winit::dpi::*, std::time::*, etc.

// --- Private render types (moved from mod.rs) ---

struct RendererSizing { /* ... exact fields ... */ }
struct PaneRenderData { /* ... exact fields ... */ }
struct FrameRenderData { /* ... exact fields ... */ }
struct PostRenderActions { /* ... exact fields ... */ }
impl Default for PostRenderActions { /* ... */ }

// PreservedClipboardImage and ClipboardImageClickGuard:
// Move here only if they are exclusively used by render_pipeline methods.
// Otherwise leave in mod.rs.

// --- impl block ---

impl WindowState {
    // Paste all 9 methods here, in the order listed above.
}
```

**Step 3:** In `mod.rs`:
- Delete the struct definitions (RendererSizing, PaneRenderData, FrameRenderData, PostRenderActions + Default impl) from the top of the file.
- Delete all 9 methods from the impl block.
- Add `mod render_pipeline;` near the top of `mod.rs` (with other `mod` declarations if any; otherwise add before the `use` imports or after them — Rust allows both).

**Step 4:** Build. Expect import errors in render_pipeline.rs — add missing `use` lines iteratively:
```bash
cd /Users/probello/Repos/par-term && make build 2>&1 | grep "^error" | head -20
```
Fix each "cannot find type/value" error by adding the appropriate `use` at the top of `render_pipeline.rs`.

**Step 5:** Once it compiles:
```bash
make lint 2>&1 | grep "^error\|^warning" | head -20
```

**Step 6:** Commit:
```bash
git add src/app/window_state/mod.rs src/app/window_state/render_pipeline.rs
git commit -m "refactor(render): extract render pipeline to render_pipeline.rs — H9"
```

---

### Task 5: Extract `agent_messages.rs` (~773L)

**Files:**
- Create: `src/app/window_state/agent_messages.rs`
- Modify: `src/app/window_state/mod.rs`

**Methods to move:**

| Method | Approx. lines |
|--------|---------------|
| `process_agent_messages_tick` | ~539L |
| `apply_agent_config_updates` | ~98L |
| `apply_single_config_update` | ~97L |
| `capture_terminal_screenshot_mcp_response` | ~39L |

**Step 1:** Grep for current line numbers:
```bash
grep -n "^    fn process_agent_messages_tick\|^    fn apply_agent_config_updates\|^    fn apply_single_config_update\|^    fn capture_terminal_screenshot" src/app/window_state/mod.rs
```

**Step 2:** Create `src/app/window_state/agent_messages.rs`:

```rust
//! ACP agent message processing and config-update application for WindowState.
//!
//! Contains:
//! - `process_agent_messages_tick`: drain agent message queue and update AI inspector
//! - `apply_agent_config_updates`: apply config changes from agent responses
//! - `apply_single_config_update`: dispatch a single config change
//! - `capture_terminal_screenshot_mcp_response`: respond to MCP screenshot requests

use crate::app::window_state::WindowState;
// Add additional imports as needed.

impl WindowState {
    // Paste all 4 methods here.
}
```

**Step 3:** Delete the 4 methods from `mod.rs`. Add `mod agent_messages;`.

**Step 4:** Build and fix import errors:
```bash
make build 2>&1 | grep "^error" | head -20
```

**Step 5:** Commit:
```bash
git add src/app/window_state/mod.rs src/app/window_state/agent_messages.rs
git commit -m "refactor(agent): extract agent message processing to agent_messages.rs — H9"
```

---

### Task 6: Extract `action_handlers.rs` (~740L)

**Files:**
- Create: `src/app/window_state/action_handlers.rs`
- Modify: `src/app/window_state/mod.rs`

**Methods to move:**

| Method | Approx. lines |
|--------|---------------|
| `handle_tab_bar_action_after_render` | ~122L |
| `handle_clipboard_history_action_after_render` | ~34L |
| `handle_inspector_action_after_render` | ~346L |
| `handle_integrations_response` | ~238L |

**Step 1:** Grep for current line numbers:
```bash
grep -n "^    fn handle_tab_bar_action\|^    fn handle_clipboard_history\|^    fn handle_inspector_action\|^    fn handle_integrations" src/app/window_state/mod.rs
```

**Step 2:** Create `src/app/window_state/action_handlers.rs`:

```rust
//! Post-render action dispatch for WindowState.
//!
//! Contains handlers for tab bar, clipboard history, AI inspector,
//! and external integrations UI actions — all dispatched after the
//! renderer borrow is released.

use crate::app::window_state::WindowState;
// Add additional imports as needed (e.g., TabBarAction, InspectorAction, IntegrationsResponse, etc.)

impl WindowState {
    // Paste all 4 methods here.
}
```

**Step 3:** Delete the 4 methods from `mod.rs`. Add `mod action_handlers;`.

**Step 4:** Build and fix import errors.

**Step 5:** Commit:
```bash
git add src/app/window_state/mod.rs src/app/window_state/action_handlers.rs
git commit -m "refactor(app): extract post-render action handlers to action_handlers.rs — H9"
```

---

### Task 7: Extract `renderer_ops.rs` (~529L)

**Files:**
- Create: `src/app/window_state/renderer_ops.rs`
- Modify: `src/app/window_state/mod.rs`

**Methods to move:**

| Method | Approx. lines |
|--------|---------------|
| `rebuild_renderer` | ~279L |
| `force_surface_reconfigure` | ~38L |
| `apply_tab_bar_offsets` | ~15L |
| `apply_tab_bar_offsets_for_position` | ~35L |
| `sync_ai_inspector_width` | ~59L |
| `sync_status_bar_inset` | ~28L |
| `update_cursor_blink` | ~75L |

**Step 1:** Grep for current line numbers:
```bash
grep -n "^    pub(crate) fn rebuild_renderer\|^    pub(crate) fn force_surface_reconfigure\|^    pub(crate) fn apply_tab_bar_offsets\b\|^    pub(crate) fn apply_tab_bar_offsets_for\|^    pub(crate) fn sync_ai_inspector\|^    pub(crate) fn sync_status_bar_inset\|^    pub(crate) fn update_cursor_blink" src/app/window_state/mod.rs
```

**Step 2:** Create `src/app/window_state/renderer_ops.rs`:

```rust
//! Renderer lifecycle and layout-sync operations for WindowState.
//!
//! Contains:
//! - `rebuild_renderer`: teardown and recreate the wgpu renderer (font changes, etc.)
//! - `force_surface_reconfigure`: force wgpu surface reconfiguration
//! - `apply_tab_bar_offsets` / `apply_tab_bar_offsets_for_position`: sync tab bar geometry
//! - `sync_ai_inspector_width`: propagate AI inspector panel width to renderer
//! - `sync_status_bar_inset`: propagate status bar height to renderer
//! - `update_cursor_blink`: advance cursor blink state

use crate::app::window_state::WindowState;
// Add additional imports.

impl WindowState {
    // Paste all 7 methods here.
}
```

**Step 3:** Delete the 7 methods from `mod.rs`. Add `mod renderer_ops;`.

**Step 4:** Build and fix import errors.

**Step 5:** Commit:
```bash
git add src/app/window_state/mod.rs src/app/window_state/renderer_ops.rs
git commit -m "refactor(renderer): extract renderer ops to renderer_ops.rs — H9"
```

---

### Task 8: Extract `shader_ops.rs` (~217L)

**Files:**
- Create: `src/app/window_state/shader_ops.rs`
- Modify: `src/app/window_state/mod.rs`

**Methods to move:**

| Method | Approx. lines |
|--------|---------------|
| `handle_shader_reload_event` | ~124L |
| `check_shader_reload` | ~16L |
| `init_shader_watcher` | ~58L |
| `reinit_shader_watcher` | ~19L |

**Step 1:** Grep for current line numbers:
```bash
grep -n "^    fn handle_shader_reload_event\|^    pub(crate) fn check_shader_reload\|^    pub(crate) fn init_shader_watcher\|^    pub(crate) fn reinit_shader_watcher" src/app/window_state/mod.rs
```

**Step 2:** Create `src/app/window_state/shader_ops.rs`:

```rust
//! Shader watcher lifecycle and hot-reload handling for WindowState.

use crate::app::window_state::WindowState;
// Add additional imports (ShaderReloadEvent, ShaderType, ShaderWatcher, etc.)

impl WindowState {
    // Paste all 4 methods here.
}
```

**Step 3:** Delete the 4 methods from `mod.rs`. Add `mod shader_ops;`.

**Step 4:** Build and fix import errors.

**Step 5:** Commit:
```bash
git add src/app/window_state/mod.rs src/app/window_state/shader_ops.rs
git commit -m "refactor(shader): extract shader ops to shader_ops.rs — H9"
```

---

### Task 9: Extract `config_watchers.rs` (~303L)

**Files:**
- Create: `src/app/window_state/config_watchers.rs`
- Modify: `src/app/window_state/mod.rs`

**Methods to move:**

| Method | Approx. lines |
|--------|---------------|
| `init_config_watcher` | ~21L |
| `init_config_update_watcher` | ~31L |
| `init_screenshot_request_watcher` | ~37L |
| `check_config_update_file` | ~47L |
| `check_screenshot_request_file` | ~65L |
| `check_config_reload` | ~102L |

**Step 1:** Grep for current line numbers:
```bash
grep -n "^    pub(crate) fn init_config_watcher\b\|^    pub(crate) fn init_config_update_watcher\|^    pub(crate) fn init_screenshot_request\|^    pub(crate) fn check_config_update_file\|^    pub(crate) fn check_screenshot_request\|^    pub(crate) fn check_config_reload" src/app/window_state/mod.rs
```

**Step 2:** Create `src/app/window_state/config_watchers.rs`:

```rust
//! Config file watcher setup and polling for WindowState.
//!
//! Handles live config reload (YAML changes), config update channel polling,
//! and MCP screenshot request polling.

use crate::app::window_state::WindowState;
// Add additional imports.

impl WindowState {
    // Paste all 6 methods here.
}
```

**Step 3:** Delete the 6 methods from `mod.rs`. Add `mod config_watchers;`.

**Step 4:** Build and fix import errors.

**Step 5:** Commit:
```bash
git add src/app/window_state/mod.rs src/app/window_state/config_watchers.rs
git commit -m "refactor(config): extract config watcher ops to config_watchers.rs — H9"
```

---

### Task 10: Final verification + AUDIT.md

**Step 1:** Count final line counts:
```bash
wc -l src/app/window_state/mod.rs src/app/window_state/*.rs src/app/input_events/keybinding_actions.rs src/app/input_events/snippet_actions.rs src/app/tmux_handler/gateway.rs src/app/tmux_handler/gateway_input.rs
```

**Step 2:** Run the full check suite:
```bash
make checkall
```
Expected: 0 errors, 0 new clippy warnings, all tests pass.

**Step 3:** Update `AUDIT.md` — mark H9 as resolved. Update the remaining findings table and roadmap.

```bash
# Example: replace the H9 finding with resolved status or remove it
```

**Step 4:** Commit:
```bash
git add AUDIT.md
git commit -m "docs(audit): resolve H9 — window_state.rs decomposed into module — H9"
```

---

## Expected Final State

| File | Before | After |
|------|--------|-------|
| `window_state.rs` → `window_state/mod.rs` | 6,461L | ~1,100L |
| `window_state/render_pipeline.rs` | — | ~2,800L |
| `window_state/agent_messages.rs` | — | ~773L |
| `window_state/action_handlers.rs` | — | ~740L |
| `window_state/renderer_ops.rs` | — | ~529L |
| `window_state/shader_ops.rs` | — | ~217L |
| `window_state/config_watchers.rs` | — | ~303L |
| `keybinding_actions.rs` | 865L | ~557L |
| `snippet_actions.rs` | — | ~308L |
| `gateway.rs` | 841L | ~611L |
| `gateway_input.rs` | — | ~230L |

Note: `render_pipeline.rs` at ~2,800L is still large because `submit_gpu_frame` (1,454L) is a single function with no obvious sub-extraction points without deep refactor. It is logically cohesive (one render pass). Accept this for now.

---

## Pitfalls

- **Line number drift**: After each Phase 2 task, mod.rs shrinks. Always grep to re-confirm line numbers before editing.
- **Visibility**: Moved methods may need `pub(super)` instead of `pub(crate)` for intra-module calls. Check if callers are in sibling sub-files vs. outside the module.
- **Private structs** (RendererSizing etc.): Once in render_pipeline.rs, they're private to that sub-module. The `render()` method also moves there (so `FrameRenderData` doesn't need to be visible from mod.rs).
- **`use super::` vs `use crate::`**: Sub-files of `window_state/` can use `super::SomeType` to access types defined in `mod.rs`, but since `WindowState` itself is public (`pub struct`), prefer `crate::app::window_state::WindowState` for clarity.
- **Missing imports**: The most common error. Read the first error message from `make build`, add the `use` line, repeat.
