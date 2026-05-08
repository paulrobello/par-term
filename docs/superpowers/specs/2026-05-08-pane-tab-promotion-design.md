# Pane/Tab Promotion Design

Date: 2026-05-08

## Overview

Add the ability to promote a split pane to its own tab and demote a tab to become a pane (or pane tree) in another tab. All running processes in moved panes are preserved — only the terminal grid dimensions change via a resize event.

## Promote Pane to Tab

**Trigger:** Keybinding action `"promote_pane_to_tab"` (instant, single keypress)

**Default keybinding:** None — user configures via config YAML or settings UI.

**Flow:**
1. Get the focused pane from the active tab's `PaneManager`
2. Call `PaneManager::extract_pane(focused_id)` — takes ownership of the live `Pane` without dropping it
3. If the source tab's tree is now empty, close the source tab
4. Create a new `Tab` via `Tab::new_from_pane(pane, ...)` — wraps the existing `Pane` with its live PTY, scroll state, session logger
5. Insert the new tab after the source tab's position (NewTabPosition::AfterActive behavior)
6. Focus the new tab
7. Resize the pane's terminal to match the new tab's grid dimensions

**Edge cases:**
- Single-pane tab: allowed — creates a new tab and closes the original (effectively repositioning the tab)
- No running process check needed — processes survive the transfer

## Demote Tab to Pane

**Trigger:** Keybinding action `"demote_tab_to_pane"` or context menu option.

**Default keybinding:** None — user configures via config YAML or settings UI.

### State Machine

```rust
enum PaneTransferState {
    Idle,
    DemotePickTab {
        source_tab_id: TabId,
    },
    DemotePickPane {
        source_tab_id: TabId,
        target_tab_id: TabId,
    },
    DemoteChooseDirection {
        source_tab_id: TabId,
        target_tab_id: TabId,
        target_pane_id: PaneId,
    },
}
```

Stored on `WindowState`.

### Flow

**Step 1 — DemotePickTab:**
- Visual indicator (status bar message: "Click a tab to merge into")
- User clicks a tab in the tab bar (must be different from source tab)
- Reject demote to self
- Transitions to `DemotePickPane`

**Step 2 — DemotePickPane:**
- Switches to the target tab
- Status message: "Click a pane to merge into"
- User clicks a pane within the target tab
- Transitions to `DemoteChooseDirection`

**Step 3 — DemoteChooseDirection:**
- Inline overlay near the clicked pane showing "Horizontal | Vertical" buttons
- User picks direction
- Execute the merge (see below)
- Return to `Idle`

**Cancellation:** Escape key or right-click cancels back to `Idle` at any step.

### Merge Execution

1. Extract the source tab's entire `PaneManager::root` tree (take ownership)
2. Call `PaneManager::insert_subtree_at(target_pane_id, subtree, direction, ratio)` on the target tab — replaces the target leaf with a `Split` containing the target pane and the transplanted subtree
3. Update `is_active` Arc on all transplanted panes to point to the target tab's `Arc<AtomicBool>`
4. Close the source tab (now empty)
5. Recalculate bounds in the target tab, resize all terminals

## Core Primitives

### `PaneManager::extract_pane(id) -> ExtractResult`

```rust
enum ExtractResult {
    Extracted { pane: Pane, remaining: Option<PaneNode> },
    OnlyPane(Pane),
    NotFound,
}
```

Walks the tree, removes the target leaf, promotes its sibling. Returns ownership of the live `Pane` and the remaining tree. Similar to `remove_pane()` but returns the pane instead of dropping it.

### `PaneManager::insert_subtree_at(target_pane_id, subtree, direction, ratio)`

Walks the tree, finds the target leaf, replaces it with:
```rust
PaneNode::Split {
    direction,
    ratio,
    first: target_leaf,
    second: subtree,  // the transplanted tree
}
```
Calls `recalculate_bounds()` to scale the transplanted tree to fit the new split area.

### `Tab::new_from_pane(pane: Pane, config, runtime, ...) -> Self`

New constructor:
- Takes ownership of an existing `Pane`
- Clones the pane's `Arc<RwLock<TerminalManager>>` as the tab's primary terminal
- Creates a `PaneManager` with this pane as the root leaf
- Sets up refresh task, session logger (from pane's existing logger), title, activity monitor
- Does NOT spawn a new shell — pane's PTY keeps running

## Settings UI Integration

**Location:** `par-term-settings-ui/src/` keybindings tab

Both actions appear as configurable keybindings:
- `promote_pane_to_tab` — "Promote Pane to Tab"
- `demote_tab_to_pane` — "Demote Tab to Pane"

Search keywords added to `sidebar.rs` → `tab_search_keywords()`: "promote", "demote", "pane to tab", "tab to pane".

Context menu entries added to tab bar context menu and/or pane context menu.

## Edge Cases

1. **Demote into a tab with max_panes reached** — reject with a brief visual indication
2. **No running process check needed** — all processes survive the transfer
3. **Single-pane tab promote** — allowed; creates new tab, closes original
4. **Demote to self** — rejected at step 1 (must pick a different tab)
5. **Tab closes or window closes mid-pick** — state resets to `Idle`
6. **Transplanted tree `is_active` Arcs** — all panes in the transplanted tree get their `is_active` updated to point to the target tab's `Arc<AtomicBool>`
7. **Escape during pick mode** — cancels to `Idle` at any step
8. **Right-click during pick mode** — cancels to `Idle` at any step

## Files to Modify

| File | Change |
|------|--------|
| `src/pane/manager/creation.rs` | Add `extract_pane()`, `insert_subtree_at()` |
| `src/pane/manager/mod.rs` | Add `ExtractResult` type, method declarations |
| `src/tab/constructors.rs` | Add `Tab::new_from_pane()` |
| `src/app/tab_ops/pane_ops.rs` | Add `promote_pane_to_tab()`, demote state machine methods |
| `src/app/window_state/mod.rs` | Add `PaneTransferState` field |
| `src/app/input_events/keybinding_actions.rs` | Add `"promote_pane_to_tab"`, `"demote_tab_to_pane"` dispatch |
| `src/app/input_events/key_handler/mod.rs` | Handle Escape during pick mode |
| `src/app/mouse_events/` | Handle clicks during demote pick mode |
| `src/app/render_pipeline/` | egui inline overlay for direction-choice prompt (same overlay approach as existing egui popups) |
| `par-term-settings-ui/src/` | Keybinding config UI + search keywords |
| `par-term-config/src/defaults/misc.rs` | Action name documentation (no default binding) |
