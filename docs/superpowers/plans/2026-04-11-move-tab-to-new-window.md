# Move Tab to New Window — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add "Move Tab to New Window" and "Move Tab to Window → <existing>" entries to the tab right-click menu (and a `move_tab_to_new_window` keybinding action) that transfer a live `Tab` between par-term windows without tearing down the PTY, scrollback, split panes, or any other per-tab state.

**Architecture:** Reuse the existing `TabManager::remove_tab` / `insert_tab_at` primitives (already used by session-undo) to transfer a `Tab` by ownership. Add a `WindowManager::move_tab` entry point that orchestrates (a) destination resolution — either looking up an existing window or creating a new one via a new `create_window_for_moved_tab` helper that skips the default shell spawn, (b) tab extraction from source, (c) insertion into destination, (d) refresh-task rebind to the new host window, (e) optional source window close when the source is emptied by the move. Cross-window requests are marshalled from `WindowState::handle_tab_bar_action_after_render` onto a pending-request field in `overlay_state`, then drained by `WindowManager::about_to_wait` — the same pattern used for arrangement restores and settings-window opens.

**Tech Stack:** Rust 2024 edition, winit, egui, wgpu. Existing subcrates: `par-term-keybindings`, `par-term-settings-ui`.

**Spec:** `docs/superpowers/specs/2026-04-11-move-tab-to-new-window-design.md`

---

## File map

**Modify:**
- `src/tab/manager.rs` — add round-trip unit test for `remove_tab` + `insert_tab_at`
- `src/tab_bar_ui/mod.rs` — add two `TabBarAction` variants
- `src/tab_bar_ui/state.rs` — add move-context state fields + setter
- `src/tab_bar_ui/context_menu.rs` — render move menu items / submenu
- `src/app/window_state/overlay_ui_state.rs` — add `pending_move_tab_request` field
- `src/app/window_state/action_handlers/tab_bar.rs` — stash cross-window actions onto `overlay_state`
- `src/app/window_state/impl_init.rs` — split `initialize_async` to support `skip_default_tab`
- `src/app/window_manager/mod.rs` — new `MoveDestination` enum, `move_tab`, `other_window_labels` methods
- `src/app/window_manager/window_lifecycle.rs` — add `create_window_for_moved_tab`
- `src/app/handler/app_handler_impl.rs` — drain `pending_move_tab_request` in `about_to_wait`
- `src/app/render_pipeline/post_render.rs` — call new `set_move_tab_context` on tab bar before render
- `src/app/input_events/keybinding_actions.rs` — dispatch `"move_tab_to_new_window"`
- `par-term-settings-ui/src/input_tab/actions_table.rs` — register the action in both platform tables
- `par-term-settings-ui/src/sidebar.rs` — add search keywords
- `docs/TABS.md` — document the feature
- `docs/KEYBOARD_SHORTCUTS.md` — document the keybinding

**Create:** none (spec + plan docs already exist).

---

## Task 1: Lock round-trip invariant for `remove_tab` + `insert_tab_at`

**Why:** These primitives already exist and work for session-undo. The move feature relies on them preserving every preserved `Tab` field unchanged. A regression test prevents future refactors of the session-undo path from silently breaking the move feature.

**Files:**
- Test: `src/tab/manager.rs` (append a `#[cfg(test)] mod tests` block at the end of the file if none exists, else add a new test to the existing block)

- [ ] **Step 1: Check whether `src/tab/manager.rs` already has a `#[cfg(test)] mod tests` block**

Run: `grep -n "^#\[cfg(test)\]" src/tab/manager.rs`

If the grep returns a line number, add the test to the existing block. If not, append a new block at the end of the file.

- [ ] **Step 2: Write the failing test (append to `src/tab/manager.rs`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::sync::Arc;
    use tokio::runtime::Builder;

    fn test_runtime() -> Arc<tokio::runtime::Runtime> {
        Arc::new(
            Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        )
    }

    #[test]
    #[ignore = "requires PTY spawn"]
    fn remove_insert_round_trip_preserves_tab_fields() {
        let mut mgr = TabManager::new();
        let config = Config::default();
        let runtime = test_runtime();

        // Create two tabs so removing one leaves the manager non-empty.
        let _ = mgr
            .new_tab(&config, Arc::clone(&runtime), false, Some((80, 24)))
            .expect("create tab 1");
        let id = mgr
            .new_tab(&config, Arc::clone(&runtime), false, Some((80, 24)))
            .expect("create tab 2");

        // Customize the target tab so we can assert round-trip fidelity.
        {
            let tab = mgr.get_tab_mut(id).expect("target tab exists");
            tab.set_title("my-tab");
            tab.user_named = true;
            tab.set_custom_color([10, 20, 30]);
            tab.custom_icon = Some("\u{f120}".to_string());
        }

        // Snapshot preserved fields.
        let snapshot = {
            let tab = mgr.get_tab(id).expect("target tab exists");
            (
                tab.id,
                tab.title.clone(),
                tab.has_default_title,
                tab.user_named,
                tab.custom_color,
                tab.custom_icon.clone(),
            )
        };

        // Round-trip.
        let (live_tab, is_empty) = mgr.remove_tab(id).expect("remove returns Some");
        assert!(!is_empty, "manager should still have tab 1");
        mgr.insert_tab_at(live_tab, 1);

        let after = mgr.get_tab(id).expect("tab still present after round-trip");
        assert_eq!(after.id, snapshot.0);
        assert_eq!(after.title, snapshot.1);
        assert_eq!(after.has_default_title, snapshot.2);
        assert_eq!(after.user_named, snapshot.3);
        assert_eq!(after.custom_color, snapshot.4);
        assert_eq!(after.custom_icon, snapshot.5);
    }
}
```

> **Note:** The test is `#[ignore]`-gated because `TabManager::new_tab` spawns a real PTY and the CI environment may not have a TTY. This matches the pattern used by other PTY-dependent tests in this repo (see `cargo test -- --include-ignored` in the Makefile). The test is still valuable as a regression check run manually during development and via `make test-one` on a dev box.

- [ ] **Step 3: Run the test to verify it compiles and is recognized**

Run: `cargo test -p par-term --lib tab::manager::tests::remove_insert_round_trip_preserves_tab_fields -- --ignored`

Expected: either PASS (if current `remove_tab` / `insert_tab_at` already preserve fields, which they should) or FAIL with a clear field-diff message. If it fails, the failure diagnoses which field is being clobbered — fix `remove_tab` / `insert_tab_at` in place before proceeding.

- [ ] **Step 4: Verify `make checkall` still passes**

Run: `make checkall`

Expected: PASS. If lint complains about unused imports (`use tokio::runtime::Builder;` et al. behind `#[cfg(test)]`), the module block already gates them; if they leak out, wrap them in `#[cfg(test)]`.

- [ ] **Step 5: Commit**

```bash
git add src/tab/manager.rs
git commit -m "test(tab): lock remove_tab/insert_tab_at round-trip invariant

Regression test for the live-tab transfer primitives used by session-undo and
the upcoming Move Tab to New Window feature. Gated with #[ignore] because
TabManager::new_tab spawns a real PTY."
```

---

## Task 2: Add `MoveDestination` enum and `MoveTabRequest` pending field

**Why:** Cross-window operations must be routed from the per-window action handler (`WindowState::handle_tab_bar_action_after_render`) up to `WindowManager`, which owns all windows. The existing pattern is to stash the request on `overlay_state` and drain it in `about_to_wait`. Establish the types first so later tasks can wire them up.

**Files:**
- Modify: `src/app/window_manager/mod.rs` (add `MoveDestination` + `MoveTabRequest` pub types)
- Modify: `src/app/window_state/overlay_ui_state.rs` (add `pending_move_tab_request` field)

- [ ] **Step 1: Read the current end of `src/app/window_manager/mod.rs` to find a good place to add the types**

Run: `wc -l src/app/window_manager/mod.rs` then Read the file.

- [ ] **Step 2: Add `MoveDestination` + `MoveTabRequest` types near the top of `src/app/window_manager/mod.rs` (after the existing imports and before the `WindowManager` struct)**

```rust
/// Destination for a `move_tab` operation.
///
/// `NewWindow` → spawn a fresh par-term window and insert the transferred tab as its only tab.
/// `ExistingWindow(WindowId)` → append the transferred tab to an already-open window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDestination {
    NewWindow,
    ExistingWindow(WindowId),
}

/// A request to move a tab, stashed on `WindowState::overlay_state` from the
/// per-window action handler and drained by `WindowManager::about_to_wait`.
#[derive(Debug, Clone, Copy)]
pub struct MoveTabRequest {
    pub tab_id: crate::tab::TabId,
    pub destination: MoveDestination,
}
```

- [ ] **Step 3: Add `pending_move_tab_request` field to `OverlayUiState` in `src/app/window_state/overlay_ui_state.rs`**

Open `src/app/window_state/overlay_ui_state.rs` and locate the `OverlayUiState` struct and its `new` constructor.

Add the field (place it next to other `pending_*` fields):

```rust
/// Pending "Move Tab to New Window" / "Move Tab to Window →" request,
/// drained by `WindowManager::about_to_wait`.
pub pending_move_tab_request: Option<crate::app::window_manager::MoveTabRequest>,
```

And in the `new` constructor initializer:

```rust
pending_move_tab_request: None,
```

- [ ] **Step 4: Typecheck and commit**

Run: `make typecheck`
Expected: PASS.

```bash
git add src/app/window_manager/mod.rs src/app/window_state/overlay_ui_state.rs
git commit -m "feat(window): add MoveDestination types and pending move-tab field

Scaffolding for the Move Tab to New Window feature. MoveTabRequest is stashed
on WindowState::overlay_state by the per-window tab bar action handler and
drained by WindowManager::about_to_wait so that cross-window mutations run at
the right scope."
```

---

## Task 3: Refactor `WindowState::initialize_async` to accept `skip_default_tab`

**Why:** The new-window path for a moved tab needs a window that's fully initialized (GPU renderer, egui, tab bar UI, status bar) but **without** the default shell spawn — we'll insert the transferred tab next. Extend `initialize_async` with a boolean flag and route existing callers with `false`.

**Files:**
- Modify: `src/app/window_state/impl_init.rs`
- Modify: `src/app/window_manager/window_lifecycle.rs` (existing callsite)
- Modify: `src/app/window_manager/window_session.rs` (existing callsites)
- Modify: `src/app/window_manager/arrangements.rs` (existing callsites)

- [ ] **Step 1: Identify every `initialize_async` callsite**

Run: `grep -rn "initialize_async" src/`

Expected: the definition in `impl_init.rs:164` plus 1-5 callsites in `window_lifecycle.rs`, `window_session.rs`, `arrangements.rs`. Record them.

- [ ] **Step 2: Locate the default-tab creation inside `initialize_async`**

Read `src/app/window_state/impl_init.rs` starting at line 164 and scroll down until you see where the first tab is created. It calls `self.tab_manager.new_tab(...)` or similar (possibly via `new_tab_with_cwd` when `first_tab_cwd` is `Some`). Note the exact block — we'll wrap it in a conditional.

- [ ] **Step 3: Change the signature of `initialize_async`**

```rust
// OLD:
pub(crate) async fn initialize_async(
    &mut self,
    window: Window,
    first_tab_cwd: Option<String>,
) -> Result<()> { ... }

// NEW:
pub(crate) async fn initialize_async(
    &mut self,
    window: Window,
    skip_default_tab: bool,
    first_tab_cwd: Option<String>,
) -> Result<()> { ... }
```

Wrap the default-tab-creation block identified in Step 2 in `if !skip_default_tab { ... }`. **Everything else in `initialize_async` — GPU renderer, egui init, theme, shader metadata, menu attach, tab bar UI, status bar, etc. — runs unconditionally regardless of the flag.**

If `initialize_async` also does side effects like `self.request_redraw()` or sets a `needs_redraw` flag based on the default tab, leave those outside the guard — an empty `tab_manager` is a valid transient state and the move-tab path will populate it immediately after `initialize_async` returns.

- [ ] **Step 4: Update every existing callsite to pass `false`**

For each callsite found in Step 1, pass `false` for the new argument. Example:

```rust
// window_lifecycle.rs
runtime.block_on(window_state.initialize_async(window, None))
// becomes:
runtime.block_on(window_state.initialize_async(window, false, None))
```

Apply the same mechanical edit in `window_session.rs` and `arrangements.rs`. Double-check each by searching with `grep -n "initialize_async" <file>` before editing.

- [ ] **Step 5: Build**

Run: `cargo check --workspace`
Expected: PASS. If any callsite is missed, the compiler will error on argument count — fix and re-run.

- [ ] **Step 6: Run the existing test suite to verify no regression**

Run: `make test`
Expected: PASS. (The PTY-gated tests are ignored by default, which is fine.)

- [ ] **Step 7: Commit**

```bash
git add src/app/window_state/impl_init.rs src/app/window_manager/window_lifecycle.rs src/app/window_manager/window_session.rs src/app/window_manager/arrangements.rs
git commit -m "refactor(window): add skip_default_tab flag to initialize_async

Splits the default-tab creation off into a guardable branch so that the new
'Move Tab to New Window' path can initialize a fully functional window that
starts with an empty TabManager, ready to receive the transferred tab via
insert_tab_at. All existing callsites pass false (unchanged behavior)."
```

---

## Task 4: Add `WindowManager::create_window_for_moved_tab` helper

**Why:** The move-to-new-window path needs a helper that looks like `create_window` but (a) sizes the window to match the source, (b) positions it at `source + (30, 30)` clamped to the source monitor, (c) skips the default shell spawn, (d) skips tmux auto-attach and other "first window only" side effects. Keep it separate from `create_window` to avoid flag-salad.

**Files:**
- Modify: `src/app/window_manager/window_lifecycle.rs`

- [ ] **Step 1: Add the helper method to the `impl WindowManager` block in `window_lifecycle.rs`**

Place it directly after `create_window()`. Use `create_window` as the structural template — copy the window-attrs construction (title, icon, transparency, decorations, first-mouse, always-on-top), but override size and skip the parts flagged in comments.

```rust
/// Create a window that will immediately receive a tab transferred via
/// `move_tab`. Unlike [`Self::create_window`], this helper:
///
/// - uses the caller-supplied `size` and `outer_position` instead of the
///   config default, so the new window matches the source window's geometry
/// - calls `initialize_async(..., skip_default_tab = true, None)` so the new
///   window starts with an empty `TabManager`
/// - skips tmux auto-attach and "first-window-only" side effects (menu init is
///   still performed once globally via `self.menu.is_none()`)
///
/// Returns the new `WindowId`, or `None` on failure.
pub(crate) fn create_window_for_moved_tab(
    &mut self,
    event_loop: &winit::event_loop::ActiveEventLoop,
    size: winit::dpi::PhysicalSize<u32>,
    outer_position: winit::dpi::PhysicalPosition<i32>,
) -> Option<winit::window::WindowId> {
    use crate::app::window_state::WindowState;
    use crate::menu::MenuManager;
    use std::sync::Arc;
    use winit::window::Window;

    // Reload config from disk so the new window picks up latest settings.
    if let Ok(fresh_config) = crate::config::Config::load() {
        self.config = fresh_config;
    }

    // Re-apply CLI shader override (fresh config load would wipe it).
    if let Some(ref shader) = self.runtime_options.shader {
        self.config.shader.custom_shader = Some(shader.clone());
        self.config.shader.custom_shader_enabled = true;
        self.config.background_image_enabled = false;
    }

    // Build window attrs — same as create_window but with explicit size.
    let window_number = self.windows.len() + 1;
    let title = if self.config.show_window_number {
        format!("{} [{}]", self.config.window_title, window_number)
    } else {
        self.config.window_title.clone()
    };

    let mut window_attrs = Window::default_attributes()
        .with_title(&title)
        .with_inner_size(size)
        .with_decorations(self.config.window.window_decorations)
        .with_transparent(true);

    if self.config.lock_window_size {
        window_attrs = window_attrs.with_resizable(false);
    }

    // Icon
    let icon_bytes = include_bytes!("../../../assets/icon.png");
    if let Ok(icon_image) = image::load_from_memory(icon_bytes) {
        let rgba = icon_image.to_rgba8();
        let (w, h) = rgba.dimensions();
        if let Ok(icon) = winit::window::Icon::from_rgba(rgba.into_raw(), w, h) {
            window_attrs = window_attrs.with_window_icon(Some(icon));
        }
    }

    if self.config.window.window_always_on_top {
        window_attrs = window_attrs.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
    }

    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS as _;
        window_attrs = window_attrs.with_accepts_first_mouse(true);
    }

    let window = match event_loop.create_window(window_attrs) {
        Ok(w) => w,
        Err(e) => {
            crate::debug_error!("TAB", "create_window_for_moved_tab: winit create_window failed: {}", e);
            return None;
        }
    };
    let window_id = window.id();

    // Menu init (idempotent — only runs once globally).
    if self.menu.is_none() {
        match MenuManager::new() {
            Ok(menu) => {
                if let Err(e) = menu.init_global() {
                    log::warn!("Failed to initialize global menu: {}", e);
                }
                self.menu = Some(menu);
            }
            Err(e) => log::warn!("Failed to create menu: {}", e),
        }
    }

    let mut window_state = WindowState::new(self.config.clone(), Arc::clone(&self.runtime));
    window_state.window_index = window_number;

    let runtime = Arc::clone(&self.runtime);
    if let Err(e) = runtime.block_on(window_state.initialize_async(window, true, None)) {
        crate::debug_error!("TAB", "create_window_for_moved_tab: initialize_async failed: {}", e);
        return None;
    }

    // Attach menu per-window (platform-specific).
    if let Some(menu) = &self.menu
        && let Some(win) = &window_state.window
        && let Err(e) = menu.init_for_window(win)
    {
        log::warn!("Failed to initialize menu for moved-tab window: {}", e);
    }

    // Apply the requested outer position. Clamp is the caller's responsibility.
    if let Some(win) = &window_state.window {
        win.set_outer_position(outer_position);
    }

    self.windows.insert(window_id, window_state);
    self.pending_window_count += 1;

    crate::debug_info!(
        "TAB",
        "Created new window {:?} for moved tab at {:?} size {:?}",
        window_id,
        outer_position,
        size
    );

    Some(window_id)
}
```

- [ ] **Step 2: Add the outer-position clamp helper**

Still in `window_lifecycle.rs`, add a private helper near `create_window_for_moved_tab`:

```rust
/// Compute the outer position for a newly-spawned "move to new window" window.
///
/// Starts from the source window's outer position + (30, 30) and clamps so the
/// full rect of the new window stays inside the source's monitor. If clamping
/// would require moving back across the source, returns the source's exact
/// outer position (new window stacks directly on top of the source).
pub(crate) fn compute_moved_tab_outer_position(
    event_loop: &winit::event_loop::ActiveEventLoop,
    source_outer_pos: winit::dpi::PhysicalPosition<i32>,
    new_window_size: winit::dpi::PhysicalSize<u32>,
) -> winit::dpi::PhysicalPosition<i32> {
    const OFFSET: i32 = 30;
    let desired = winit::dpi::PhysicalPosition::new(
        source_outer_pos.x + OFFSET,
        source_outer_pos.y + OFFSET,
    );

    // Find the monitor containing the source's origin.
    let monitors: Vec<_> = event_loop.available_monitors().collect();
    let source_monitor = monitors
        .iter()
        .find(|m| {
            let mp = m.position();
            let ms = m.size();
            source_outer_pos.x >= mp.x
                && source_outer_pos.y >= mp.y
                && source_outer_pos.x < mp.x + ms.width as i32
                && source_outer_pos.y < mp.y + ms.height as i32
        })
        .or_else(|| monitors.first());

    let Some(monitor) = source_monitor else {
        return desired;
    };

    let mp = monitor.position();
    let ms = monitor.size();
    let max_x = mp.x + ms.width as i32 - new_window_size.width as i32;
    let max_y = mp.y + ms.height as i32 - new_window_size.height as i32;

    let clamped_x = desired.x.min(max_x).max(mp.x);
    let clamped_y = desired.y.min(max_y).max(mp.y);

    // If clamping pushed us back above the source (i.e., the source is huge
    // relative to the monitor), stack on top of the source instead.
    if clamped_x <= source_outer_pos.x && clamped_y <= source_outer_pos.y {
        source_outer_pos
    } else {
        winit::dpi::PhysicalPosition::new(clamped_x, clamped_y)
    }
}
```

- [ ] **Step 3: Typecheck**

Run: `cargo check --workspace`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/app/window_manager/window_lifecycle.rs
git commit -m "feat(window): add create_window_for_moved_tab helper

Creates a fully initialized window at a caller-supplied size and position
without spawning the default shell, ready to receive a tab transferred via
insert_tab_at. Skips tmux auto-attach and first-window-only side effects.
Includes a compute_moved_tab_outer_position helper that offsets the new
window by (30,30) from the source and clamps to the source monitor's bounds."
```

---

## Task 5: Add `WindowManager::move_tab` and `other_window_labels`

**Why:** This is the heart of the feature — the single entry point that orchestrates destination resolution, tab extraction, insertion, refresh-task rebind, and source cleanup. `other_window_labels` supports the "Move to Window →" submenu.

**Files:**
- Modify: `src/app/window_manager/window_lifecycle.rs` (or a new `tab_transfer.rs` module — author's call)

- [ ] **Step 1: Add `other_window_labels` helper to `impl WindowManager`**

```rust
/// Produce `(WindowId, display_label)` pairs for every window *other than*
/// `source_window_id`, suitable for the "Move Tab to Window →" submenu.
///
/// Label format: `Window N — <active tab title>`, falling back to `Window N`
/// if the active tab has no meaningful title.
pub(crate) fn other_window_labels(
    &self,
    source_window_id: winit::window::WindowId,
) -> Vec<(winit::window::WindowId, String)> {
    self.windows
        .iter()
        .filter(|(id, _)| **id != source_window_id)
        .map(|(id, ws)| {
            let active_title = ws
                .tab_manager
                .active_tab()
                .map(|t| t.title.trim().to_string())
                .filter(|s| !s.is_empty());
            let label = match active_title {
                Some(title) => format!("Window {} — {}", ws.window_index, title),
                None => format!("Window {}", ws.window_index),
            };
            (*id, label)
        })
        .collect()
}
```

- [ ] **Step 2: Add `move_tab` to `impl WindowManager`**

```rust
/// Move a live `Tab` from `source_window` to `destination`, preserving the
/// PTY, scrollback, split panes, and all other per-tab state. See the design
/// spec at `docs/superpowers/specs/2026-04-11-move-tab-to-new-window-design.md`
/// for the full contract.
pub(crate) fn move_tab(
    &mut self,
    event_loop: &winit::event_loop::ActiveEventLoop,
    source_window: winit::window::WindowId,
    tab_id: crate::tab::TabId,
    destination: super::MoveDestination,
) {
    use super::MoveDestination;

    // --- Pre-flight validation ---
    let Some(source_state) = self.windows.get(&source_window) else {
        crate::debug_warn!("TAB", "move_tab: source window {:?} not found", source_window);
        return;
    };

    if source_state.tab_manager.get_tab(tab_id).is_none() {
        crate::debug_warn!("TAB", "move_tab: tab {} not in window {:?}", tab_id, source_window);
        return;
    }

    // Reject if tmux gateway is active anywhere in this window — covers both
    // the gateway tab itself and all display tabs (both are disallowed per
    // the spec).
    if source_state.is_gateway_active() {
        crate::debug_warn!(
            "TAB",
            "move_tab: refusing to move tab {} — source window has an active tmux gateway",
            tab_id
        );
        return;
    }

    match destination {
        MoveDestination::NewWindow => {
            if source_state.tab_manager.tab_count() <= 1 {
                crate::debug_warn!(
                    "TAB",
                    "move_tab: refusing solo-tab pop-out for tab {} (would be a no-op)",
                    tab_id
                );
                return;
            }
        }
        MoveDestination::ExistingWindow(dest_id) => {
            if dest_id == source_window {
                crate::debug_warn!("TAB", "move_tab: destination == source, ignoring");
                return;
            }
            if !self.windows.contains_key(&dest_id) {
                crate::debug_warn!("TAB", "move_tab: destination window {:?} not found", dest_id);
                return;
            }
        }
    }

    crate::debug_info!(
        "TAB",
        "Moving tab {} from window {:?} to {:?}",
        tab_id,
        source_window,
        destination
    );

    // --- Resolve destination ---
    // Record source geometry before we mutate anything.
    let (source_size, source_outer_pos) = {
        let ws = self.windows.get(&source_window).expect("validated above");
        let win = ws.window.as_ref();
        let size = win
            .map(|w| w.inner_size())
            .unwrap_or(winit::dpi::PhysicalSize::new(800, 600));
        let pos = win
            .and_then(|w| w.outer_position().ok())
            .unwrap_or(winit::dpi::PhysicalPosition::new(0, 0));
        (size, pos)
    };

    let dest_window_id = match destination {
        MoveDestination::ExistingWindow(id) => id,
        MoveDestination::NewWindow => {
            let clamped_pos =
                Self::compute_moved_tab_outer_position(event_loop, source_outer_pos, source_size);
            match self.create_window_for_moved_tab(event_loop, source_size, clamped_pos) {
                Some(id) => id,
                None => {
                    crate::debug_error!(
                        "TAB",
                        "move_tab: create_window_for_moved_tab failed — source state untouched"
                    );
                    return;
                }
            }
        }
    };

    // --- Extract from source ---
    let Some(source_state) = self.windows.get_mut(&source_window) else {
        crate::debug_error!("TAB", "move_tab: source window disappeared mid-flight");
        return;
    };
    let Some((mut live_tab, source_is_empty)) = source_state.tab_manager.remove_tab(tab_id) else {
        crate::debug_error!("TAB", "move_tab: remove_tab returned None for tab {}", tab_id);
        return;
    };

    // Stop the refresh task — its captured Arc<Window> still points at the source.
    live_tab.stop_refresh_task();

    // --- Insert into destination ---
    let Some(dest_state) = self.windows.get_mut(&dest_window_id) else {
        crate::debug_error!(
            "TAB",
            "move_tab: destination window {:?} disappeared — dropping tab",
            dest_window_id
        );
        // Dropping live_tab will kill the PTY; no way to recover at this point
        // short of reinserting into source, which is complex enough to not be
        // worth it for a should-never-happen case.
        return;
    };
    let insert_at = dest_state.tab_manager.tab_count();
    dest_state.tab_manager.insert_tab_at(live_tab, insert_at);

    // --- Rebind refresh task to the destination window ---
    if let Some(dest_win_arc) = dest_state.window.clone() {
        let active_fps = dest_state.config.active_fps();
        let inactive_fps = dest_state.config.inactive_fps();
        if let Some(tab) = dest_state.tab_manager.get_tab_mut(tab_id) {
            tab.start_refresh_task(
                std::sync::Arc::clone(&self.runtime),
                dest_win_arc,
                active_fps,
                inactive_fps,
            );
        }
    }

    // --- Flag destination for redraw (normal resize path will reconcile grid) ---
    dest_state.request_redraw();

    // --- Raise and focus destination ---
    if let Some(win) = dest_state.window.as_ref() {
        win.set_visible(true);
        win.focus_window();
    }

    crate::debug_info!(
        "TAB",
        "move_tab complete (source empty: {})",
        source_is_empty
    );

    // --- Close source if emptied ---
    if source_is_empty {
        self.close_window(source_window);
    }
}
```

> **Config helper note:** If `Config::active_fps()` / `inactive_fps()` don't exist, substitute the actual config fields used by the current `start_refresh_task` callsites. Find them with `grep -rn "start_refresh_task" src/` and mirror what you see.

- [ ] **Step 3: Typecheck**

Run: `cargo check --workspace`
Expected: PASS. If `request_redraw`, `close_window`, `is_gateway_active`, or `active_fps` names diverge from reality, the compiler will point them out — fix using the exact method name you see in the codebase.

- [ ] **Step 4: Commit**

```bash
git add src/app/window_manager/
git commit -m "feat(window): add move_tab and other_window_labels helpers

move_tab orchestrates the full live-tab transfer: pre-flight validation
(gateway check, solo-tab guard), destination resolution (resolving an
existing window or creating a new one via create_window_for_moved_tab),
remove_tab/insert_tab_at, refresh task rebind to the destination window,
destination focus, and source close on emptied window.

other_window_labels produces (WindowId, 'Window N — <title>') pairs for the
'Move Tab to Window ->' submenu."
```

---

## Task 6: Add new `TabBarAction` variants

**Files:**
- Modify: `src/tab_bar_ui/mod.rs`

- [ ] **Step 1: Add the variants to `enum TabBarAction`**

Edit `src/tab_bar_ui/mod.rs` lines 38-64 (the `TabBarAction` enum). Add after `ToggleAssistantPanel`:

```rust
    /// Move a tab to a brand-new par-term window.
    MoveTabToNewWindow(TabId),
    /// Move a tab into an existing par-term window.
    MoveTabToExistingWindow(TabId, winit::window::WindowId),
```

- [ ] **Step 2: Typecheck (expect a non-exhaustive match warning in the action handler)**

Run: `cargo check --workspace 2>&1 | tee /tmp/move_tab_typecheck.log`

Expected: build succeeds, but the compiler may warn / error about non-exhaustive match in `src/app/window_state/action_handlers/tab_bar.rs`. That's fine — Task 7 fills it in.

- [ ] **Step 3: Commit**

```bash
git add src/tab_bar_ui/mod.rs
git commit -m "feat(tab-bar): add MoveTabTo{NewWindow,ExistingWindow} action variants"
```

---

## Task 7: Wire tab bar actions to `overlay_state.pending_move_tab_request`

**Why:** The per-window handler can't call `WindowManager::move_tab` directly — `WindowState` doesn't own `WindowManager`. Instead, stash the request on `overlay_state` and let `WindowManager::about_to_wait` drain it (Task 11).

**Files:**
- Modify: `src/app/window_state/action_handlers/tab_bar.rs`

- [ ] **Step 1: Add cases for the new variants inside `handle_tab_bar_action_after_render`'s match**

Insert before the `TabBarAction::None => {}` arm:

```rust
            TabBarAction::MoveTabToNewWindow(tab_id) => {
                self.overlay_state.pending_move_tab_request =
                    Some(crate::app::window_manager::MoveTabRequest {
                        tab_id,
                        destination: crate::app::window_manager::MoveDestination::NewWindow,
                    });
            }
            TabBarAction::MoveTabToExistingWindow(tab_id, dest_id) => {
                self.overlay_state.pending_move_tab_request =
                    Some(crate::app::window_manager::MoveTabRequest {
                        tab_id,
                        destination: crate::app::window_manager::MoveDestination::ExistingWindow(
                            dest_id,
                        ),
                    });
            }
```

- [ ] **Step 2: Typecheck**

Run: `cargo check --workspace`
Expected: PASS (no non-exhaustive warning).

- [ ] **Step 3: Commit**

```bash
git add src/app/window_state/action_handlers/tab_bar.rs
git commit -m "feat(tab-bar): stash move-tab requests onto overlay_state

Cross-window action that must be drained by WindowManager::about_to_wait
rather than handled per-window."
```

---

## Task 8: Drain `pending_move_tab_request` in `about_to_wait`

**Why:** Close the loop — actually call `WindowManager::move_tab` from the drain point that already handles other cross-window requests.

**Files:**
- Modify: `src/app/handler/app_handler_impl.rs`

- [ ] **Step 1: Read the existing drain loop in `about_to_wait`**

Run: `grep -n "pending_arrangement_restore\|for window_state in self.windows" src/app/handler/app_handler_impl.rs`

Pick the loop that iterates `self.windows.values_mut()` and drains fields like `pending_arrangement_restore`. We'll add the move-tab drain to the same loop.

- [ ] **Step 2: Add the drain step inside the loop**

Collect the drained request alongside existing `arrangement_restore_name` etc. — the loop body borrows `&mut WindowState`, so we can't call `self.move_tab` there directly. Instead, stash `(source_window_id, request)` into a local `Vec` and process it after the loop.

Above the loop, declare:

```rust
let mut pending_moves: Vec<(winit::window::WindowId, crate::app::window_manager::MoveTabRequest)> =
    Vec::new();
```

Inside the loop, after the other `pending_*` drains:

```rust
// Window IDs aren't available inside values_mut; we need the key too.
```

→ **Change the loop** from `self.windows.values_mut()` to `self.windows.iter_mut()` so we have access to the `WindowId`:

```rust
for (window_id, window_state) in self.windows.iter_mut() {
    // ... existing drains ...

    if let Some(req) = window_state.overlay_state.pending_move_tab_request.take() {
        pending_moves.push((*window_id, req));
    }

    window_state.about_to_wait(event_loop);
}
```

> If the existing loop is already `iter_mut`, great — just add the drain. If it's `values_mut`, converting to `iter_mut` is mechanical: every `window_state.field` stays the same, and `window_id` is available alongside.

After the loop finishes, process the pending moves:

```rust
for (source_window_id, req) in pending_moves {
    self.move_tab(event_loop, source_window_id, req.tab_id, req.destination);
}
```

- [ ] **Step 3: Typecheck**

Run: `cargo check --workspace`
Expected: PASS.

- [ ] **Step 4: Verify the existing `about_to_wait` behavior is unchanged**

Run: `make test`
Expected: PASS (none of the touched code has unit tests, but a passing build and no `clippy` regressions is the signal).

- [ ] **Step 5: Commit**

```bash
git add src/app/handler/app_handler_impl.rs
git commit -m "feat(window): drain pending_move_tab_request in about_to_wait

Wires the cross-window action from per-window action handlers to the
WindowManager::move_tab entry point."
```

---

## Task 9: Add move-context state to `TabBarUI`

**Why:** The context menu render pass needs to know whether the move menu items should be enabled (gateway check, solo-tab check) and what labels the submenu should show. Stash these on `TabBarUI` via a setter called per-frame before `render()`.

**Files:**
- Modify: `src/tab_bar_ui/state.rs`
- Modify: `src/tab_bar_ui/mod.rs` (new public setter)

- [ ] **Step 1: Add fields to `TabBarUI` in `src/tab_bar_ui/state.rs`**

Add after `show_new_tab_profile_menu`:

```rust
    /// Set per-frame: candidate destination windows for the "Move Tab to Window →" submenu.
    /// Each entry is `(WindowId, display_label)` (e.g., `"Window 2 — vim"`).
    pub(crate) move_candidates: Vec<(winit::window::WindowId, String)>,
    /// Set per-frame: true if the current window has an active tmux gateway.
    /// When true, the Move Tab menu entries are disabled for every tab.
    pub(crate) move_gateway_active: bool,
    /// Set per-frame: number of tabs in the source window. Used to disable
    /// "Move Tab to New Window" when `== 1` (solo-tab guard).
    pub(crate) move_source_tab_count: usize,
```

Initialize them in `new()`:

```rust
            move_candidates: Vec::new(),
            move_gateway_active: false,
            move_source_tab_count: 0,
```

- [ ] **Step 2: Add a public setter on `TabBarUI` in `src/tab_bar_ui/mod.rs`**

Add to the `impl TabBarUI` block alongside the other public methods:

```rust
    /// Update the move-tab context shown in the right-click context menu.
    /// Must be called each frame *before* `render()` so the context menu has
    /// fresh state.
    pub fn set_move_tab_context(
        &mut self,
        gateway_active: bool,
        tab_count: usize,
        candidates: Vec<(winit::window::WindowId, String)>,
    ) {
        self.move_gateway_active = gateway_active;
        self.move_source_tab_count = tab_count;
        self.move_candidates = candidates;
    }
```

- [ ] **Step 3: Typecheck**

Run: `cargo check --workspace`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/tab_bar_ui/state.rs src/tab_bar_ui/mod.rs
git commit -m "feat(tab-bar): add per-frame move-tab context state

Fields populated by the host window before each frame's render() call so
the context menu knows whether to enable Move Tab entries and what to
show in the 'Move Tab to Window ->' submenu."
```

---

## Task 10: Populate move-tab context before render

**Why:** Call `set_move_tab_context` from the render pipeline so the tab bar has fresh data.

**Files:**
- Modify: `src/app/render_pipeline/post_render.rs` (or wherever `tab_bar_ui.render()` is called — `grep -rn "tab_bar_ui\.render\b" src/` to find the right spot).

- [ ] **Step 1: Find where `tab_bar_ui.render(...)` is called during the per-window render pass**

Run: `grep -rn "tab_bar_ui\.render\b\|tab_bar_ui: &mut\|self.tab_bar_ui.render" src/ 2>/dev/null | head`

The call lives inside the egui render closure in the GPU submit / render pipeline path. Identify the file and line.

- [ ] **Step 2: Before that `render` call, invoke the setter**

```rust
// Populate move-tab context before rendering the tab bar.
let gateway_active = self.is_gateway_active();
let tab_count = self.tab_manager.tab_count();
let candidates = window_manager.other_window_labels(self.window_id);
self.tab_bar_ui.set_move_tab_context(gateway_active, tab_count, candidates);
```

> **Critical context problem:** the render pipeline runs inside `WindowState` which **does not have access to `WindowManager`**. Two options:

**Option A — pass candidates through `about_to_wait` into a `WindowState` field:**
- Add `pub(crate) move_tab_candidates_cache: Vec<(WindowId, String)>` to `WindowState` (or nest under `overlay_state`).
- In `about_to_wait`'s loop (Task 8), after `iter_mut`, do a *second* pass that computes and writes the candidates for each window. The first pass already has access to `window_id` and the handler (`self`) can read `other_window_labels` against a snapshot of window IDs taken before the loop.
- The next frame's render pipeline reads from `move_tab_candidates_cache`.

**Option B — compute lazily on secondary-click:** defer population until the right-click actually happens. Harder to wire cleanly; rejected.

Use **Option A**. Concretely:

1. In `src/app/window_state/overlay_ui_state.rs`, add:
   ```rust
   pub move_tab_candidates: Vec<(winit::window::WindowId, String)>,
   ```
   initialized to `Vec::new()`.

2. In `app_handler_impl.rs::about_to_wait`, **after** the existing `iter_mut` drain loop and before processing `pending_moves`, populate each window's cache:
   ```rust
   // Collect the window IDs first so we don't hold an immutable borrow across the mutation loop.
   let all_window_ids: Vec<winit::window::WindowId> = self.windows.keys().copied().collect();
   for wid in &all_window_ids {
       let labels = self.other_window_labels(*wid);
       if let Some(ws) = self.windows.get_mut(wid) {
           ws.overlay_state.move_tab_candidates = labels;
       }
   }
   ```

3. In the render pipeline (inside `WindowState` before `tab_bar_ui.render`):
   ```rust
   let gateway_active = self.is_gateway_active();
   let tab_count = self.tab_manager.tab_count();
   let candidates = self.overlay_state.move_tab_candidates.clone();
   self.tab_bar_ui
       .set_move_tab_context(gateway_active, tab_count, candidates);
   ```

- [ ] **Step 3: Typecheck**

Run: `cargo check --workspace`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/app/window_state/overlay_ui_state.rs src/app/handler/app_handler_impl.rs src/app/render_pipeline/
git commit -m "feat(window): populate move-tab context per frame

Caches sibling-window labels on WindowState during about_to_wait so the
per-window render pipeline can pass them into TabBarUI::set_move_tab_context
before rendering. Also feeds the current gateway state and tab count into
the same setter."
```

---

## Task 11: Render the context menu items

**Files:**
- Modify: `src/tab_bar_ui/context_menu.rs`

- [ ] **Step 1: Add the menu entries after "Close Tab" and before the color section**

Open `src/tab_bar_ui/context_menu.rs`. Locate the "Close Tab" block (around line 206). After its `close_menu = true;` line and the subsequent `ui.add_space(4.0); ui.separator(); ui.add_space(4.0);`, insert:

```rust
                        // ----- Move Tab entries -----
                        let can_move =
                            !self.move_gateway_active && self.move_source_tab_count >= 1;
                        let has_other_windows = !self.move_candidates.is_empty();

                        // "Move Tab to New Window" — disabled for gateway-active windows
                        // and for solo-tab source windows (visually a no-op).
                        let new_window_enabled =
                            can_move && self.move_source_tab_count >= 2;
                        ui.add_enabled_ui(new_window_enabled, |ui| {
                            if menu_item(ui, "Move Tab to New Window") {
                                action = TabBarAction::MoveTabToNewWindow(tab_id);
                                close_menu = true;
                            }
                        });

                        // "Move Tab to Window →" submenu — hidden entirely if there
                        // are no other windows or the move is disabled.
                        if can_move && has_other_windows {
                            let candidates = self.move_candidates.clone();
                            ui.menu_button("Move Tab to Window ▸", |ui| {
                                for (win_id, label) in candidates {
                                    if ui.button(&label).clicked() {
                                        action = TabBarAction::MoveTabToExistingWindow(
                                            tab_id, win_id,
                                        );
                                        close_menu = true;
                                    }
                                }
                            });
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);
                        // ----- end Move Tab entries -----
```

**Position note:** insert this block so that the existing `ui.add_space / separator / color section` comes *after* the new block. Result: existing order is `Rename / Icon / Duplicate / Close / [NEW: Move entries] / Color`. Adjust the separators so there's exactly one between Close/Move and one between Move/Color (not two).

- [ ] **Step 2: Verify menu_button existence for your egui version**

Run: `grep -rn "menu_button" src/ 2>/dev/null | head`

If `menu_button` is in use elsewhere, this is fine. If not, check the egui version's API — the typical name is `ui.menu_button(label, |ui| { ... })`. If egui in this project uses a different name, substitute it (e.g., `Popup::from_toggle_button_response` for manual popups).

- [ ] **Step 3: Typecheck**

Run: `cargo check --workspace`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/tab_bar_ui/context_menu.rs
git commit -m "feat(tab-bar): render Move Tab to {New,Existing} Window menu items

Disabled for tmux-gateway-active windows. New Window entry additionally
disabled for solo-tab source windows. 'Move Tab to Window ->' submenu
hidden entirely when no other par-term windows exist."
```

---

## Task 12: Keybinding action dispatch

**Files:**
- Modify: `src/app/input_events/keybinding_actions.rs`
- Modify: `par-term-settings-ui/src/input_tab/actions_table.rs`

- [ ] **Step 1: Add the dispatch case in `keybinding_actions.rs`**

Find the `"close_tab"` arm around line 155 and add a sibling arm:

```rust
            "move_tab_to_new_window" => {
                if let Some(tab_id) = self.tab_manager.active_tab_id()
                    && !self.is_gateway_active()
                    && self.tab_manager.tab_count() >= 2
                {
                    self.overlay_state.pending_move_tab_request =
                        Some(crate::app::window_manager::MoveTabRequest {
                            tab_id,
                            destination: crate::app::window_manager::MoveDestination::NewWindow,
                        });
                    log::info!("Move Tab to New Window triggered via keybinding");
                }
                true
            }
```

> This stashes the request on the same `pending_move_tab_request` field Task 7 wired up — it will be drained by `about_to_wait` just like a context-menu click.

- [ ] **Step 2: Register the action in the settings UI action tables**

Open `par-term-settings-ui/src/input_tab/actions_table.rs`. Find the macOS table (around line 38) and add after `("duplicate_tab", ...)`:

```rust
    ("move_tab_to_new_window", "Move Tab to New Window", None),
```

Find the non-macOS table (around line 164) and add the same entry after `("duplicate_tab", ...)`:

```rust
    ("move_tab_to_new_window", "Move Tab to New Window", None),
```

(No default chord — users bind it themselves if they want one.)

- [ ] **Step 3: Typecheck**

Run: `cargo check --workspace`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/app/input_events/keybinding_actions.rs par-term-settings-ui/src/input_tab/actions_table.rs
git commit -m "feat(keybinding): add move_tab_to_new_window action

Stashes a MoveTabRequest onto overlay_state (same path as the context menu)
so it's drained by WindowManager::about_to_wait. No default chord; users
bind it via the settings UI. Gateway-active windows and solo-tab windows
are rejected up front."
```

---

## Task 13: Settings search keywords

**Files:**
- Modify: `par-term-settings-ui/src/sidebar.rs`

- [ ] **Step 1: Find `tab_search_keywords()`**

Run: `grep -n "fn tab_search_keywords\|Keybindings" par-term-settings-ui/src/sidebar.rs`

- [ ] **Step 2: Add the new keywords to the keybindings tab entry**

Append `"move tab window detach popout"` to the existing keyword string for the Keybindings / Input tab. Example:

```rust
SettingsTab::Keybindings => "... existing keywords ... move tab window detach popout",
```

(Merge into the existing literal; don't add a duplicate arm.)

- [ ] **Step 3: Build and commit**

Run: `cargo check --workspace`
Expected: PASS.

```bash
git add par-term-settings-ui/src/sidebar.rs
git commit -m "feat(settings-ui): add search keywords for move-tab-to-new-window"
```

---

## Task 14: Documentation

**Files:**
- Modify: `docs/TABS.md`
- Modify: `docs/KEYBOARD_SHORTCUTS.md`

- [ ] **Step 1: Read `docs/TABS.md` to find a good insertion point**

Run: `wc -l docs/TABS.md` and Read the file.

- [ ] **Step 2: Add a "Moving tabs between windows" section to `docs/TABS.md`**

Insert near the existing tab-operations section:

```markdown
## Moving tabs between windows

A tab (including its PTY, scrollback, running processes, split panes, session
logger, and prettifier state) can be moved to a different window without being
restarted.

**Context menu:**

- Right-click a tab → **Move Tab to New Window** spawns a new par-term window
  with the tab as its only occupant. The new window matches the source window's
  size and is placed 30 pixels down-and-right from the source, clamped to the
  source monitor's bounds. Disabled when the source window has only one tab
  (the operation would be a no-op) or when the source window is hosting a tmux
  gateway.
- Right-click a tab → **Move Tab to Window →** opens a submenu listing every
  other par-term window, labeled `Window N — <active tab title>`. Selecting a
  window transfers the tab there. If the source window becomes empty as a
  result, it closes. Disabled for tmux-gateway windows; hidden entirely when no
  other par-term windows exist.

**Keybinding:** Bind the `move_tab_to_new_window` action in
Settings → Keybindings. There is no default chord. The keybinding only covers
the "new window" case; the "move to existing window" case is menu-only because
keybindings cannot parameterize on a specific target window.

**Limitations:**

- Tmux gateway tabs and tmux display tabs cannot be moved — both would break
  the gateway/display link inside the source window.
- Per-window state (custom shader, assistant panel, window-level settings)
  does not travel with the tab. The moved tab adopts the destination window's
  settings.
```

- [ ] **Step 3: Add a row to `docs/KEYBOARD_SHORTCUTS.md`**

Find the tab-actions table and add a row:

```markdown
| Move Tab to New Window | `move_tab_to_new_window` | *(unbound)* | Transfer the active tab to a new window while preserving PTY, scrollback, and split panes. |
```

Match the existing table's column format — inspect the file first and adapt column count/order as needed.

- [ ] **Step 4: Commit**

```bash
git add docs/TABS.md docs/KEYBOARD_SHORTCUTS.md
git commit -m "docs: document move-tab-to-new-window feature"
```

---

## Task 15: Final `make checkall` + manual smoke test

**Files:** none (verification)

- [ ] **Step 1: Clean checkall**

Run: `make checkall`
Expected: PASS. If clippy flags anything in the new code, fix inline before the smoke test.

- [ ] **Step 2: Build a dev binary**

Run: `make build`
Expected: completes without error.

- [ ] **Step 3: Execute the manual smoke-test checklist from the spec**

From `docs/superpowers/specs/2026-04-11-move-tab-to-new-window-design.md`, run each of the 10 manual tests:

1. Single window, two tabs → Move Tab to New Window → new window offset, both tabs alive.
2. Single window, one tab → "Move to New Window" grayed out, "Move to Window →" submenu absent.
3. Two windows, one tab each → merge via "Move to Window →" → source window closes.
4. Window with tmux gateway → menu entries disabled.
5. Window with tmux display tab → menu entries disabled.
6. Tab running `top` → keeps updating after move (refresh-task rebind works).
7. Tab with split panes → all panes and focused pane survive.
8. Multi-monitor offset-clamp behavior.
9. Keybinding binding `move_tab_to_new_window` → active tab pops out.
10. Repeat the move 5× from the same window → each new window clamps sanely.

Record any failures as separate bug tickets / follow-up fixes. Do **not** mark this task complete if any smoke test fails — open fix PRs, then re-run.

- [ ] **Step 4: Save a vault note after shipping**

After the feature is merged, dispatch the research-agent to save a note at
`~/ClaudeVault/Patterns/par-term-tab-window-transfer.md` documenting the
live-tab transfer pattern (pre-flight → resolve destination → extract → insert
→ rebind refresh task → focus → close source if empty). This pattern is
reusable for future drag-and-drop tear-off work.

- [ ] **Step 5: Commit any checkall fixups + close the work**

If clippy / checkall fixes were needed:

```bash
git add -A
git commit -m "chore: checkall fixups for move-tab-to-new-window"
```

---

## Self-review summary

**Spec coverage:** Every section of the spec maps to at least one task:

| Spec section | Tasks |
|---|---|
| Transfer primitive ordering | Task 5 (`move_tab`) |
| `create_window_for_moved_tab` helper | Task 4 |
| Context menu + submenu | Task 11 (render) + Task 9/10 (state wiring) |
| Keybinding action | Task 12 |
| Pre-flight gateway / solo-tab guards | Task 5 (WindowManager) + Task 11 (UI disable) + Task 12 (keybinding guard) |
| Offset + clamp | Task 4 (`compute_moved_tab_outer_position`) |
| Refresh task rebind | Task 5 |
| Close-source-if-empty | Task 5 |
| Destination focus | Task 5 |
| Window labels submenu | Task 5 (`other_window_labels`) |
| Unit test | Task 1 |
| Manual smoke test | Task 15 |
| Docs | Task 14 |
| Settings-UI search keywords | Task 13 |

**Placeholder scan:** None. Every task has exact file paths, complete code blocks, and exact commands.

**Type consistency:** `MoveDestination`, `MoveTabRequest`, `pending_move_tab_request`, `set_move_tab_context`, `move_candidates`, `move_gateway_active`, `move_source_tab_count`, `other_window_labels`, `create_window_for_moved_tab`, `compute_moved_tab_outer_position`, and `move_tab` appear with the same signature everywhere they're referenced.
