# render() Extraction Plan — C1 Audit Finding

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce `render()` in `src/app/window_state.rs` from 2,482 lines to ~50 lines by extracting logical phases into focused helper methods.

**Architecture:** Define a `FrameRenderData` struct to carry computed per-frame state between phases. Extract each phase as a `&mut self` method (or method returning owned data). The final `render()` reads as a ~10-step orchestration.

**Tech Stack:** Rust 2024, `src/app/window_state.rs`, `src/app/debug_state.rs`. Use `make build` (dev-release) to verify after each task; `make lint` before committing.

---

## Target render() shape (~50 lines)

```rust
pub(crate) fn render(&mut self) {
    if self.is_shutting_down { return; }
    if !self.should_render_frame() { return; }
    self.update_frame_metrics();
    self.update_animations();
    self.sync_layout();
    let Some(frame_data) = self.gather_render_data() else { return; };
    self.process_agent_messages_tick();
    let actions = self.submit_gpu_frame(frame_data);
    self.update_post_render_state(actions);
}
```

---

## New types to define (in `window_state.rs`, near top with `PaneRenderData`)

### `FrameRenderData`

```rust
/// Data computed during gather_render_data() and consumed by submit_gpu_frame().
struct FrameRenderData {
    /// Processed terminal cells (URL underlines + search highlights applied)
    cells: Vec<crate::cell_renderer::Cell>,
    /// Cursor position on screen (col, row), None if hidden or scrolled away
    cursor_pos: Option<(usize, usize)>,
    /// Cursor glyph style from terminal or config overrides
    cursor_style: Option<par_term_emu_core_rust::TermCursorStyle>,
    /// Whether alternate screen is active (TUI apps like vim, htop)
    is_alt_screen: bool,
    /// Viewport scroll offset (0 = bottom / live view)
    scroll_offset: usize,
    /// Total scrollback lines
    scrollback_len: usize,
    /// Whether the scrollbar should be shown this frame
    show_scrollbar: bool,
    /// Visible grid rows
    visible_lines: usize,
    /// Visible grid columns
    grid_cols: usize,
    /// Inline prettifier graphics to composite alongside Sixel/Kitty/iTerm2
    prettifier_graphics: Vec<PrettifierGraphic>,
}

/// One rendered diagram/block graphic from the prettifier pipeline.
struct PrettifierGraphic {
    /// Stable texture ID (block_id offset to avoid collision with terminal graphic IDs)
    texture_id: u64,
    /// Screen row (viewport-relative) where the graphic starts
    screen_row: u32,
    /// RGBA pixel data
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}
```

### `PostRenderActions`

```rust
/// Actions collected inside the egui/GPU render pass that must be handled
/// after the renderer borrow is released.
struct PostRenderActions {
    clipboard:       ClipboardHistoryAction,
    command_history: CommandHistoryAction,
    paste_special:   PasteSpecialAction,
    session_picker:  SessionPickerAction,
    tab_action:      TabBarAction,
    shader_install:  ShaderInstallResponse,
    integrations:    IntegrationsResponse,
    search:          crate::search::SearchAction,
    inspector:       InspectorAction,
    profile_drawer:  ProfileDrawerAction,
    close_confirm:   CloseConfirmAction,
    quit_confirm:    QuitConfirmAction,
    remote_install:  RemoteShellInstallAction,
    ssh_connect:     SshConnectAction,
}

impl Default for PostRenderActions { /* all variants ::None / ::default() */ }
```

### `DebugState` addition

Add one field to `src/app/debug_state.rs`:

```rust
pub render_start: Option<std::time::Instant>,  // set by update_frame_metrics()
```

And initialise it to `None` in `DebugState::new()`.

---

## Task 1 — Extract `should_render_frame()`

**Files:**
- Modify: `src/app/window_state.rs:2346-2378` (inside `render()`)

**What to extract:** Lines 2347-2378 — the shutdown guard, FPS throttle calculation, early return, and resetting `needs_redraw` + updating `last_render_time`.

**New method signature:**

```rust
/// Returns true if we should proceed with rendering this frame.
/// Handles FPS throttle and updates last_render_time / needs_redraw.
fn should_render_frame(&mut self) -> bool {
    let target_fps = if self.config.pause_refresh_on_blur && !self.is_focused {
        self.config.unfocused_fps
    } else {
        self.config.max_fps
    };
    let frame_interval = std::time::Duration::from_millis(
        (1000 / target_fps.max(1)) as u64
    );
    if let Some(last_render) = self.last_render_time {
        if last_render.elapsed() < frame_interval {
            return false;
        }
    }
    self.last_render_time = Some(std::time::Instant::now());
    self.needs_redraw = false;
    true
}
```

**Step 1:** Read lines 2347-2378 carefully to confirm the exact code to move.

**Step 2:** Add the `should_render_frame` method body (above or below `render()` in the same impl block — place it near the other render helpers).

**Step 3:** Replace lines 2347-2378 in `render()` with:
```rust
if !self.should_render_frame() { return; }
```
Keep `if self.is_shutting_down { return; }` as the *first* line of `render()` (it's 1 line and belongs in render() per the target shape).

**Step 4:** Build and verify.
```bash
make build
```
Expected: compiles without errors.

**Step 5:** Commit.
```bash
git add src/app/window_state.rs
git commit -m "refactor(render): extract should_render_frame() — C1"
```

---

## Task 2 — Extract `update_frame_metrics()`

**Files:**
- Modify: `src/app/debug_state.rs` — add `render_start` field
- Modify: `src/app/window_state.rs` — extract method, replace in `render()`

**What to extract:** Lines 2374-2391 — `absolute_start`, frame timing push, `last_frame_start` update.
The end-of-function timing check (currently at lines 4817-4823) also moves here conceptually: store start in `self.debug.render_start` and check it at the end of `update_post_render_state`.

**Step 1:** Add to `DebugState` in `src/app/debug_state.rs`:
```rust
pub render_start: Option<std::time::Instant>,
```
Set it to `None` in `DebugState::new()`.

**Step 2:** Add the new method:
```rust
/// Record frame start time and update rolling frame-time metrics.
fn update_frame_metrics(&mut self) {
    self.debug.render_start = Some(std::time::Instant::now());
    let frame_start = self.debug.render_start.unwrap();
    if let Some(last_start) = self.debug.last_frame_start {
        let frame_time = frame_start.duration_since(last_start);
        self.debug.frame_times.push_back(frame_time);
        if self.debug.frame_times.len() > 60 {
            self.debug.frame_times.pop_front();
        }
    }
    self.debug.last_frame_start = Some(frame_start);
}
```

**Step 3:** In `render()`, replace lines 2374-2391 with:
```rust
self.update_frame_metrics();
```
Remove the local `absolute_start` variable. Update the end-of-function timing block (currently lines 4817-4823) to use `self.debug.render_start`:
```rust
if let Some(start) = self.debug.render_start {
    let total = start.elapsed();
    if total.as_millis() > 10 {
        log::debug!(
            "TIMING: AbsoluteTotal={:.2}ms",
            total.as_secs_f64() * 1000.0
        );
    }
}
```
(This timing block will move to `update_post_render_state()` in Task 6.)

**Step 4:** Build.
```bash
make build
```

**Step 5:** Commit.
```bash
git add src/app/debug_state.rs src/app/window_state.rs
git commit -m "refactor(render): extract update_frame_metrics(), add render_start to DebugState — C1"
```

---

## Task 3 — Extract `update_animations()`

**Files:**
- Modify: `src/app/window_state.rs:2393-2410`

**What to extract:** Scroll animation tick, tab title update from OSC, pending font rebuild.

```rust
/// Tick scroll animations, refresh tab titles, and rebuild renderer if fonts changed.
fn update_animations(&mut self) {
    // Tick scroll animation on active tab
    let animation_running = if let Some(tab) = self.tab_manager.active_tab_mut() {
        tab.scroll_state.update_animation()
    } else {
        false
    };

    // Update tab titles from terminal OSC sequences
    self.tab_manager.update_all_titles(self.config.tab_title_mode);

    // Rebuild renderer if font-related settings changed
    if self.pending_font_rebuild {
        if let Err(e) = self.rebuild_renderer() {
            log::error!("Failed to rebuild renderer after font change: {}", e);
        }
        self.pending_font_rebuild = false;
    }

    // Request another redraw if scroll animation is still running
    if animation_running {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
```

Note: The `animation_running` variable was previously used at line 2505 to request a redraw. Merge that check into this method (it currently happens after the cell-gather block, but logically belongs here).

**Step 1:** Verify the `animation_running` usage in `render()` — confirm lines 2394-2398 (animation) and 2505-2507 (redraw request). Both move into `update_animations()`.

**Step 2:** Add the method above.

**Step 3:** Replace lines 2393-2410 + 2504-2507 in `render()` with `self.update_animations();`.

**Step 4:** Build.
```bash
make build
```

**Step 5:** Commit.
```bash
git add src/app/window_state.rs
git commit -m "refactor(render): extract update_animations() — C1"
```

---

## Task 4 — Extract `sync_layout()`

**Files:**
- Modify: `src/app/window_state.rs:2412-2458`

**What to extract:** Tab bar offset sync with renderer and status bar inset sync. These are pure layout coordination steps.

```rust
/// Sync tab bar and status bar offsets with the renderer every frame.
/// Resizes terminal grids if tab bar geometry changed.
fn sync_layout(&mut self) {
    let tab_count = self.tab_manager.tab_count();
    let tab_bar_height = self.tab_bar_ui.get_height(tab_count, &self.config);
    let tab_bar_width = self.tab_bar_ui.get_width(tab_count, &self.config);
    crate::debug_trace!(
        "TAB_SYNC",
        "Tab count={}, tab_bar_height={:.0}, tab_bar_width={:.0}, position={:?}, mode={:?}",
        tab_count,
        tab_bar_height,
        tab_bar_width,
        self.config.tab_bar_position,
        self.config.tab_bar_mode
    );
    if let Some(renderer) = &mut self.renderer {
        let grid_changed = Self::apply_tab_bar_offsets_for_position(
            self.config.tab_bar_position,
            renderer,
            tab_bar_height,
            tab_bar_width,
        );
        if let Some((new_cols, new_rows)) = grid_changed {
            let cell_width = renderer.cell_width();
            let cell_height = renderer.cell_height();
            let width_px = (new_cols as f32 * cell_width) as usize;
            let height_px = (new_rows as f32 * cell_height) as usize;
            for tab in self.tab_manager.tabs_mut() {
                if let Ok(mut term) = tab.terminal.try_lock() {
                    term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                    let _ = term.resize_with_pixels(new_cols, new_rows, width_px, height_px);
                }
                tab.cache.cells = None;
            }
            crate::debug_info!(
                "TAB_SYNC",
                "Tab bar offsets changed (position={:?}), resized terminals to {}x{}",
                self.config.tab_bar_position,
                new_cols,
                new_rows
            );
        }
    }
    self.sync_status_bar_inset();
}
```

**Step 1:** Read lines 2412-2458 in `render()` and confirm they match the above.

**Step 2:** Add `sync_layout()` as an impl method on `WindowState`.

**Step 3:** Replace lines 2412-2458 in `render()` with `self.sync_layout();`.

**Step 4:** Build.
```bash
make build
```

**Step 5:** Commit.
```bash
git add src/app/window_state.rs
git commit -m "refactor(render): extract sync_layout() — C1"
```

---

## Task 5 — Define structs + extract `gather_render_data()`

**Files:**
- Modify: `src/app/window_state.rs` (add structs near top, add method)

This is the largest extraction. `gather_render_data()` covers:
- Lines 2460-2465: Get renderer_size/visible_lines/grid_cols (early return if no renderer)
- Lines 2467-2641: Snapshot active tab state, acquire terminal cells, cursor/style, is_alt_screen
- Lines 2643-2713: Prettifier alt-screen transition, cursor shader hides_cursor, cell cache update
- Lines 2714-2759: Scrollback length, terminal title, command history, shell lifecycle drain
- Lines 2761-2943: Prettifier pipeline feed (Claude Code detection, collapse markers, row feed)
- Lines 2945-2966: Block-count invalidation
- Lines 2967-3093: Cache scrollback, copy mode sync, trigger marks, scrollbar, window title, URL detection, search, cursor blink

**Step 1:** Add `FrameRenderData` and `PrettifierGraphic` structs near the top of `window_state.rs` (after `PaneRenderData`, before `PreservedClipboardImage`), using the definitions in the "New types" section above.

**Step 2:** Add `PostRenderActions` struct (after `FrameRenderData`).

**Step 3:** Move lines 2460-3093 into a new method:

```rust
/// Gather all data needed to render this frame.
/// Returns None if rendering should be skipped (e.g., no renderer or no active tab).
fn gather_render_data(&mut self) -> Option<FrameRenderData> {
    // ... (lines 2460-3093 moved here verbatim) ...
    // Replace `return;` with `return None;` where appropriate.
    // At the end, construct and return Some(FrameRenderData { ... })
}
```

**Important borrow checker notes:**
- `terminal.try_lock()` blocks are fine inside `&mut self` — no issue.
- The `cells` vec is declared and modified inside this scope; it will be moved into `FrameRenderData`.
- Replace each `return;` inside the extracted block with `return None;`.
- The `let (renderer_size, visible_lines, grid_cols)` block at line 2460: if `self.renderer.is_none()`, return `None`.

**Step 4:** Replace lines 2460-3093 in `render()` with:
```rust
let Some(frame_data) = self.gather_render_data() else { return; };
```

**Step 5:** Also declare `PostRenderActions` as a local in `render()` — it will be populated during `submit_gpu_frame()`. For now just add the struct definition; the wiring happens in Task 6.

**Step 6:** Build (expect compile errors; fix borrow checker issues iteratively).
```bash
make build 2>&1 | head -60
```

**Step 7:** Once it compiles cleanly, run the linter:
```bash
make lint
```

**Step 8:** Commit.
```bash
git add src/app/window_state.rs
git commit -m "refactor(render): define FrameRenderData/PostRenderActions, extract gather_render_data() — C1"
```

---

## Task 6 — Extract `submit_gpu_frame()` + `update_post_render_state()`

**Files:**
- Modify: `src/app/window_state.rs:3094-4824`

### 6a — Extract `submit_gpu_frame()`

This covers lines 3094-4546 (the main egui + GPU render block), adapted to take `FrameRenderData` and return `PostRenderActions`.

```rust
/// Run egui overlays and GPU render pass. Returns actions to handle post-frame.
fn submit_gpu_frame(&mut self, frame_data: FrameRenderData) -> PostRenderActions {
    let mut actions = PostRenderActions::default();

    // --- prettifier cell substitution (lines 3167-3295) ---
    // Uses frame_data.cells (already gathered), produces frame_data.prettifier_graphics
    // (already in frame_data from gather phase — no additional work needed here)

    // --- main renderer block (lines 3297-4546) ---
    // Replace all `pending_*` locals with `actions.*` fields.
    // e.g.:  let mut pending_tab_action = TabBarAction::None;
    //    ->  (removed; use actions.tab_action directly)
    // and:   pending_tab_action = tab_bar_result.action;
    //    ->  actions.tab_action = tab_bar_result.action;

    // ... (lines 3094-4546 moved here, pending_* vars replaced with actions.* fields) ...

    actions
}
```

**Note:** The prettifier cell substitution (lines 3167-3295) accesses `frame_data.cells` mutably to apply styled segments. Since `submit_gpu_frame` takes `frame_data` by value, this is fine.

### 6b — Extract `update_post_render_state()`

Lines 4547-4824 (post-render action dispatch).

```rust
/// Handle all actions collected during the egui/GPU render pass.
fn update_post_render_state(&mut self, actions: PostRenderActions) {
    // Sync AI Inspector panel width after render pass (avoids borrow conflict with renderer)
    // ... line 4547-4551 ...

    // Handle tab bar actions
    self.handle_tab_bar_action_after_render(actions.tab_action);

    // Handle clipboard actions
    self.handle_clipboard_history_action_after_render(actions.clipboard);

    // ... all other action handlers from lines 4552-4823 ...

    // End-of-frame timing check
    if let Some(start) = self.debug.render_start {
        let total = start.elapsed();
        if total.as_millis() > 10 {
            log::debug!(
                "TIMING: AbsoluteTotal={:.2}ms",
                total.as_secs_f64() * 1000.0
            );
        }
    }
}
```

**Step 1:** Add `submit_gpu_frame(&mut self, frame_data: FrameRenderData) -> PostRenderActions` to the impl block. Move lines 3094-4546 inside it, replacing `pending_*` locals with `actions.*` fields.

**Step 2:** Add `update_post_render_state(&mut self, actions: PostRenderActions)`. Move lines 4547-4823 inside it, plus the end-of-frame timing block.

**Step 3:** The `render()` body should now look like:
```rust
pub(crate) fn render(&mut self) {
    if self.is_shutting_down { return; }
    if !self.should_render_frame() { return; }
    self.update_frame_metrics();
    self.update_animations();
    self.sync_layout();
    let Some(frame_data) = self.gather_render_data() else { return; };
    self.process_agent_messages_tick();
    let actions = self.submit_gpu_frame(frame_data);
    self.update_post_render_state(actions);
}
```

**Step 4:** Build (expect borrow checker errors around the renderer block; fix iteratively).
```bash
make build 2>&1 | head -80
```

**Step 5:** Once clean:
```bash
make lint
make test
```

**Step 6:** Commit.
```bash
git add src/app/window_state.rs
git commit -m "refactor(render): extract submit_gpu_frame() and update_post_render_state() — C1"
```

---

## Task 7 — Verify and update AUDIT.md

**Step 1:** Count final render() lines.
```bash
# Find start and end of render()
grep -n "pub(crate) fn render\b\|^    fn " src/app/window_state.rs | head -20
```

**Step 2:** Check window_state.rs total size.
```bash
wc -l src/app/window_state.rs
```

**Step 3:** Run full check suite.
```bash
make checkall
```

**Step 4:** Update `AUDIT.md` — mark C1 as resolved if render() is ≤ 100 lines; update H9 with new line count for window_state.rs.

**Step 5:** Commit.
```bash
git add AUDIT.md
git commit -m "docs(audit): update C1/H9 line counts after render() extraction"
```

---

## Pitfalls & Borrow Checker Notes

- **`self.renderer` borrow**: The big renderer block borrows `&mut self.renderer`. Any other `self.*` access inside that closure must not conflict. When splitting, ensure `FrameRenderData` holds *owned copies* of everything from the gather phase so `submit_gpu_frame` doesn't need to re-borrow terminal/tab state.
- **`cells` mutability**: `gather_render_data()` mutates `cells` for URL underlines and search. `submit_gpu_frame()` also mutates it for prettifier substitution. Move prettifier substitution into `gather_render_data()` if easier, or keep in `submit_gpu_frame()` taking `cells` by value.
- **`terminal.try_lock()`**: Already async-safe. No special handling needed.
- **`return` vs `return None`**: Inside `gather_render_data()`, every bare `return;` becomes `return None;`.
- **Line-number drift**: After each task the file changes. Verify line ranges with `grep -n` before editing.
