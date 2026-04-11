# Move Tab to New Window — Design

**Date:** 2026-04-11
**Status:** Approved — pending implementation plan
**Owner:** Paul Robello

## Problem

par-term has no way to move a tab to a different window. A user who wants to
split a working tab off into its own window must close it in place and re-open
a fresh shell, losing scrollback, running processes, session logger state,
prettifier pipeline state, profile history, and split-pane layout. iTerm2
supports this ("Move Tab to New Window" in the tab's context menu) by
transferring the live tab object between windows without touching the PTY or
session state.

## Goal

Let the user move a live tab to either a new window or an existing par-term
window via the tab right-click menu and a keybinding, preserving everything
inside the tab (PTY, scrollback, process tree, split panes, session logger,
prettifier, profile state, custom color/icon, user-set title).

## Non-goals

- Moving tmux gateway tabs (the gateway owns its host window's tmux control-mode
  PTY; moving it would orphan every display tab in the source window).
- Moving tmux display tabs (display→gateway link is currently intra-window;
  making it cross-window is a tmux-layer refactor, out of scope here).
- A keybinding to merge into a specific existing window (keybindings cannot
  parameterize on a window index).
- Drag-and-drop tab tear-off. Context menu only for the first iteration.
- Moving tabs between par-term *processes* (only between windows of the same
  process).

## User-facing surface

Three entry points:

1. **Tab context menu item "Move Tab to New Window"**
   - Always visible on the tab right-click menu.
   - Disabled when the tab is a tmux gateway, a tmux display tab, OR the source
     window has only one tab (solo-tab pop-out is visually a no-op).
2. **Tab context menu submenu "Move Tab to Window →"**
   - Lists every *other* par-term window, labeled
     `Window N — <active tab title>` with fallback `Window N` if the active tab
     has no meaningful title.
   - Source window excluded from the list.
   - Submenu hidden entirely when only one window exists.
   - Disabled for gateway tabs and display tabs.
   - **Enabled** for solo-tab source windows — this is the "merge into another
     window" path, and the source window closes cleanly after the move.
3. **Keybinding action `MoveTabToNewWindow`**
   - Registered in `par-term-keybindings` alongside existing tab actions.
   - Bound by the user through the settings UI; no default chord.
   - Discoverable via settings-search keywords:
     `move tab window detach popout`.
   - No keybinding for "merge into specific window" — submenu only.

## Architecture

### Transfer primitive

Add one method to `WindowManager`:

```rust
fn move_tab(
    &mut self,
    event_loop: &ActiveEventLoop,
    source_window: WindowId,
    tab_id: TabId,
    destination: MoveDestination,
)
```

where

```rust
enum MoveDestination {
    NewWindow,
    ExistingWindow(WindowId),
}
```

**Order of operations:**

1. **Pre-flight checks.** Verify source window exists, tab exists in source,
   tab is neither a tmux gateway nor a display tab. For
   `ExistingWindow(id)` verify `id` exists and `id != source_window`. For
   `NewWindow` + `tab_count == 1`, reject (solo-tab guard). On any failure, bail
   early without mutating state and log a warning.
2. **Resolve destination window.**
   - `ExistingWindow(id)` → look up `&mut WindowState` for it.
   - `NewWindow` → create it via the new helper
     `create_window_for_moved_tab(event_loop, size, position)`. The source
     window's outer position is offset `+30/+30` pixels and clamped to the
     source monitor's bounds so the new window cannot land off-screen. Window
     inner size matches the source. The helper returns the new `WindowId`.
3. **Extract from source.**
   `source.tab_manager.remove_tab(tab_id)` returns `(Tab, source_is_empty)`.
   Immediately call `tab.stop_refresh_task()` — its captured `Arc<Window>` still
   points at the *source* window.
4. **Insert into destination.**
   `dest.tab_manager.insert_tab_at(tab, dest.tab_manager.tab_count())` — append
   at the end; becomes active automatically.
5. **Rebind refresh task.** Start a new refresh task on the inserted tab using
   the destination window's `Arc<Window>` and the current config FPS values.
6. **Request redraw of the destination window.** The destination's normal
   resize path will reconcile the transferred terminal's cell grid to the
   destination window's dimensions on the next frame.
7. **Raise and focus the destination window** (`window.focus_window()` plus
   `window.set_visible(true)` if needed) so the user sees where the tab landed.
   On existing-window merges the destination may be occluded; on new-window
   creates it will already be visible.
8. **Close the source window if `source_is_empty`** via the existing
   `close_window(source_window)` path.

**Failure atomicity.** The only operation that can fail mid-flight is step 2
(new window creation). Destination is resolved *before* extracting the tab, so a
step-2 failure leaves source state untouched. Steps 3–8 are infallible local
state mutations. No rollback logic required.

### The `create_window_for_moved_tab` helper

`WindowManager::create_window()` today always spawns a default first tab inside
`WindowState::initialize_async()`. The helper refactors that path:

- Split `initialize_async(window, first_tab_cwd)` into:
  - `initialize_async(window, skip_default_tab: bool, first_tab_cwd: Option<String>)`
    — existing behavior when `skip_default_tab = false`. When `true`, creates
    the GPU renderer, egui, status bar, and tab bar UI, and leaves
    `tab_manager` empty.
- Add `create_window_for_moved_tab(event_loop, size: PhysicalSize<u32>, position: PhysicalPosition<i32>) -> Option<WindowId>`:
  - Reuses the title, icon, transparency, decorations, and first-mouse setup
    from `create_window`.
  - Overrides `.with_inner_size(size)` with the explicit source-matching size.
  - After `event_loop.create_window`, calls `window.set_outer_position(position)`
    with the clamped offset.
  - Calls `initialize_async(window, skip_default_tab = true, None)`.
  - **Skips** tmux auto-attach and "first-window-only" side effects (menu init,
    CLI timer start, update checker sync) — those are not appropriate for a
    secondary window that already inherits state from an existing session.
  - Returns the new `WindowId`.

**Offset clamp.** Compute `source_outer_pos + (30, 30)`, look up the source's
monitor bounds via `event_loop.available_monitors()`, and clamp so the new
window's full rect stays inside the monitor. If clamping would require moving
*back* across the source (i.e., the source already fills most of the monitor),
fall back to the source's exact outer position — the new window stacks
directly on top of the source, still visible as a separate OS window.

### Tab bar UI additions

Additions to `TabBarUI` state (`src/tab_bar_ui/state.rs`):

```rust
pub(crate) move_candidates: Vec<(WindowId, String)>,  // (window_id, display_label)
pub(crate) can_move_tab: bool,
pub(crate) is_solo_tab: bool,
```

**Frame prep.** The per-window update pass that currently prepares
`tab_bar_ui.context_menu_*` state also sets:

- `can_move_tab = is_gateway(tab) == false && is_display_tab(tab) == false`
  for the right-clicked tab. Concrete field / helper names to be decided in the
  implementation plan based on the existing `TabTmuxState` shape; the semantic
  is "not a tmux gateway" and "not a tmux display tab".
- `is_solo_tab = tab_manager.tab_count() == 1`.
- `move_candidates = window_manager.other_window_labels(current_window_id)` —
  a new `WindowManager` helper that walks `self.windows`, skips the current
  window, and returns `(WindowId, format!("Window {} — {}", ws.window_index, active_tab_title_or_default(ws)))`.

**Context menu render** (`src/tab_bar_ui/context_menu.rs`), after the existing
"Close Tab" item and before the color section:

```rust
ui.add_space(4.0);
ui.separator();
ui.add_space(4.0);

ui.add_enabled_ui(self.can_move_tab && !self.is_solo_tab, |ui| {
    if menu_item(ui, "Move Tab to New Window") {
        action = TabBarAction::MoveTabToNewWindow(tab_id);
        close_menu = true;
    }
});

if self.can_move_tab && !self.move_candidates.is_empty() {
    ui.menu_button("Move Tab to Window ▸", |ui| {
        for (win_id, label) in self.move_candidates.clone() {
            if ui.button(label).clicked() {
                action = TabBarAction::MoveTabToExistingWindow(tab_id, win_id);
                close_menu = true;
            }
        }
    });
}
```

egui's native `menu_button` handles submenu layout and dismissal, so the
existing `*_activated_frame` inline-mode frame-guard pattern does not apply.

### Action routing

New `TabBarAction` variants:

```rust
TabBarAction::MoveTabToNewWindow(TabId),
TabBarAction::MoveTabToExistingWindow(TabId, WindowId),
```

The existing action dispatcher grows two cases that call
`window_manager.move_tab(event_loop, current_window_id, tab_id, dest)`.

### Keybinding action

- Add `KeybindingAction::MoveTabToNewWindow` in `par-term-keybindings`.
- Dispatch in `src/app/input_events/keybinding_actions.rs`: resolve the
  currently focused tab's `TabId` and current window, call
  `window_manager.move_tab(event_loop, current_window, tab_id, MoveDestination::NewWindow)`.
- Register search keywords `move tab window detach popout` in
  `par-term-settings-ui/src/sidebar.rs → tab_search_keywords()`.

## Invariants preserved

1. **Tab PTY is never killed during a move.** `remove_tab` transfers ownership;
   `Drop` never runs on the success path. Every per-tab field (PTY, scrollback,
   prettifier, session logger, profile state, custom color/icon, split panes)
   travels inside the `Tab` struct automatically.
2. **No session-undo record on move.** The move path bypasses `close_tab`;
   nothing is pushed onto `overlay_state.closed_tabs`.
3. **Refresh task always points at the current host window.** Stopped before
   extract, restarted after insert with the destination window's `Arc<Window>`.
4. **Transferred terminal resizes to match the destination window** via the
   destination's normal resize path on its next frame. No explicit
   `terminal.resize()` call required.
5. **Source window closes cleanly if emptied.** Uses the existing
   `close_window` flow, which handles session save, `should_exit` logic, and
   menu teardown.
6. **Renumbering is automatic.** `remove_tab` and `insert_tab_at` both call
   `renumber_default_tabs()`; custom and user-named titles survive.

## Edge cases

| Case | Behavior |
|---|---|
| Solo tab + New Window | Menu item disabled. |
| Solo tab + Existing Window | Allowed; source window auto-closes after move. |
| Tmux gateway tab | Both entries disabled. |
| Tmux display tab | Both entries disabled. |
| Only one par-term window exists | "Move to Window →" submenu hidden; "New Window" entry still shown. |
| Window creation fails at step 2 | Source untouched; error logged; user sees no state change. |
| Active tab moved | Source picks new active via `remove_tab`; destination promotes moved tab to active via `insert_tab_at`. |
| Moved tab carries split panes | All panes travel with the `Tab`; focused pane is preserved. |
| Per-window custom shader | Moved tab adopts the destination window's shader (shaders are window-level, not tab-level). Documented as expected behavior. |
| AI assistant panel state | Per-window. Moved tab does not carry assistant state. Correct. |
| Multi-monitor | Offset clamp keeps the new window on the source's monitor. |
| Fast repeated moves | Each new window is clamped independently; cannot escape the monitor. |

## Logging

- `debug_info!("TAB", "Moving tab {} from window {:?} to {}", tab_id, src, dest_description)` at the start of `move_tab`.
- `debug_info!("TAB", "Move complete (source empty: {})", source_is_empty)` at the end.
- `debug_warn!("TAB", "Move rejected: {}", reason)` on pre-flight failure.

## Testing strategy

### Manual test plan (verification checklist)

1. Single window, two tabs: right-click non-active tab → Move to New Window →
   new window appears offset, both tabs preserved, PTY still alive
   (`echo hello`).
2. Single window, one tab: right-click → "Move to New Window" is grayed out;
   "Move to Window →" submenu absent.
3. Two windows, one tab each: right-click tab in window 1 → "Move to Window →"
   shows `Window 2 — <title>`. Click it → source window closes, destination
   gains the tab.
4. Window with tmux gateway: right-click gateway tab → both move entries
   disabled.
5. Window with tmux display tab: right-click display tab → both move entries
   disabled.
6. Move a tab running `top` → `top` keeps updating in the new window (refresh
   task rebind works).
7. Move a tab with split panes → all panes travel, focus preserved.
8. Multi-monitor: move source to monitor 2, right-click → Move to New Window →
   new window respects offset clamp (stays on monitor 2).
9. Keybinding: bind `MoveTabToNewWindow` to a chord, press it → active tab
   pops out.
10. Fast repeat: move 5 tabs in a row from the same window → no crashes, each
    new window positions sanely (clamped).

### Automated coverage

Add a pure-Rust unit test in `src/tab/manager.rs` asserting that
`remove_tab` → `insert_tab_at` round-trips a `Tab` without mutating its
preserved fields (`id`, `title`, `custom_color`, `custom_icon`,
`has_default_title`, `user_named`). This is the only cheaply unit-testable
piece without spinning up a winit event loop.

### CI

`make checkall` (fmt, lint, typecheck, test) must pass.

## Docs to update

- `docs/TABS.md` — add a "Moving tabs between windows" section describing both
  context menu entries and the keybinding.
- `docs/KEYBOARD_SHORTCUTS.md` — add `MoveTabToNewWindow` to the tab actions
  table.

## Open questions

None. All clarifications resolved during brainstorming:

- Scope → context menu + keybinding + merge-into-existing submenu.
- Solo-tab guard → New Window disabled, Merge-into-existing allowed.
- Tmux gateway/display → both disabled.
- New window size → match source, offset +30/+30, clamped to source monitor.
- Submenu labels → `Window N — <active tab title>`.
