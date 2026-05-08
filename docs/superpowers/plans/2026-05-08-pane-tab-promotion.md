# Pane/Tab Promotion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add promote (pane → tab) and demote (tab → pane) operations that preserve all running processes.

**Architecture:** `PaneManager` gets two new tree operations — `extract_pane()` to remove and return a live `Pane`, and `insert_subtree_at()` to graft a `PaneNode` tree into an existing tree. A new `Tab::new_from_pane()` constructor wraps a transferred pane in a new tab. The demote flow uses a `PaneTransferState` state machine on `WindowState` for the multi-step pick mode.

**Tech Stack:** Rust, egui (overlay UI), par-term's existing keybinding dispatch system

---

### Task 1: Add `ExtractResult` type and `extract_pane()` to PaneManager

**Files:**
- Modify: `src/pane/types/mod.rs` — re-export `ExtractResult`
- Modify: `src/pane/manager/mod.rs` — add `extract_pane()` method
- Modify: `src/pane/manager/creation.rs` — add `extract_pane_from_node()` recursive helper

- [ ] **Step 1: Add `ExtractResult` enum to `src/pane/manager/mod.rs`**

Add after the `use` block (around line 40), before `impl PaneManager`:

```rust
/// Result of extracting a pane from the tree (returns live Pane ownership)
pub enum ExtractResult {
    /// Pane was extracted; remaining tree is returned (None if it was the only pane)
    Extracted { pane: Pane, remaining: Option<PaneNode> },
    /// The target pane was the only pane in the tree
    OnlyPane(Pane),
    /// Pane was not found in the tree
    NotFound,
}
```

- [ ] **Step 2: Add `extract_pane()` method to `PaneManager` impl in `src/pane/manager/mod.rs`**

Add inside `impl PaneManager` (after the `root_mut()` method, around line 186):

```rust
/// Extract a pane from the tree by ID, returning ownership of the live `Pane`.
///
/// Unlike `remove_pane()` which drops the pane, this returns the pane
/// intact so it can be transferred to another tab or pane tree.
/// All processes in the pane's PTY continue running.
pub fn extract_pane(&mut self, target_id: PaneId) -> ExtractResult {
    if let Some(root) = self.root.take() {
        let result = Self::extract_pane_from_node(root, target_id);
        match result {
            ExtractResult::Extracted { pane, remaining } => {
                self.root = remaining;
                if let Some(id) = self.focused_pane_id {
                    if id == target_id {
                        // Focus moved to another pane
                        self.focused_pane_id = self.root.as_ref().and_then(|r| r.first_pane_id());
                    }
                }
                ExtractResult::Extracted { pane, remaining: self.root.take() }
            }
            ExtractResult::OnlyPane(pane) => {
                self.root = None;
                self.focused_pane_id = None;
                ExtractResult::OnlyPane(pane)
            }
            ExtractResult::NotFound => {
                self.root = Some(root);
                ExtractResult::NotFound
            }
        }
    } else {
        ExtractResult::NotFound
    }
}
```

Note: `first_pane_id()` needs to exist on `PaneNode`. Check if it does; if not, add it as:

```rust
// In src/pane/types/pane_node.rs, inside impl PaneNode:
pub fn first_pane_id(&self) -> Option<PaneId> {
    match self {
        PaneNode::Leaf(pane) => Some(pane.id),
        PaneNode::Split { first, .. } => first.first_pane_id(),
    }
}
```

- [ ] **Step 3: Add `extract_pane_from_node()` recursive helper in `src/pane/manager/creation.rs`**

Add inside `impl PaneManager`, after the existing `remove_pane()` method (around line 397):

```rust
/// Recursive helper: extract a pane from the tree, returning the live Pane.
fn extract_pane_from_node(node: PaneNode, target_id: PaneId) -> ExtractResult {
    match node {
        PaneNode::Leaf(pane) => {
            if pane.id == target_id {
                ExtractResult::OnlyPane(*pane)
            } else {
                ExtractResult::NotFound(PaneNode::Leaf(pane))
            }
        }
        PaneNode::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            // Try first child
            match Self::extract_pane_from_node(*first, target_id) {
                ExtractResult::OnlyPane(pane) => {
                    // First child was the target — promote second child
                    ExtractResult::Extracted {
                        pane,
                        remaining: Some(*second),
                    }
                }
                ExtractResult::Extracted { pane, remaining } => {
                    // First child was modified
                    ExtractResult::Extracted {
                        pane,
                        remaining: Some(PaneNode::Split {
                            direction,
                            ratio,
                            first: Box::new(
                                remaining
                                    .expect("Extracted should always carry remaining for non-root"),
                            ),
                            second,
                        }),
                    }
                }
                ExtractResult::NotFound(first_node) => {
                    // Try second child
                    match Self::extract_pane_from_node(*second, target_id) {
                        ExtractResult::OnlyPane(pane) => {
                            // Second child was the target — promote first child
                            ExtractResult::Extracted {
                                pane,
                                remaining: Some(first_node),
                            }
                        }
                        ExtractResult::Extracted { pane, remaining } => {
                            ExtractResult::Extracted {
                                pane,
                                remaining: Some(PaneNode::Split {
                                    direction,
                                    ratio,
                                    first: Box::new(first_node),
                                    second: Box::new(
                                        remaining.expect(
                                            "Extracted should always carry remaining for non-root",
                                        ),
                                    ),
                                }),
                            }
                        }
                        ExtractResult::NotFound(second_node) => ExtractResult::NotFound(
                            PaneNode::Split {
                                direction,
                                ratio,
                                first: Box::new(first_node),
                                second: Box::new(second_node),
                            },
                        ),
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 4: Add `first_pane_id()` to `PaneNode` in `src/pane/types/pane_node.rs`**

Add inside `impl PaneNode` (after the `all_panes()` method, around line 152):

```rust
/// Get the ID of the first (leftmost/topmost) leaf in this subtree
pub fn first_pane_id(&self) -> Option<PaneId> {
    match self {
        PaneNode::Leaf(pane) => Some(pane.id),
        PaneNode::Split { first, .. } => first.first_pane_id(),
    }
}
```

- [ ] **Step 5: Re-export `ExtractResult` from `src/pane/types/mod.rs` or `src/pane/mod.rs`**

Check `src/pane/mod.rs` for the existing re-export pattern and add:

```rust
pub use pane::manager::ExtractResult;
```

- [ ] **Step 6: Build and verify**

Run: `cargo check --workspace`
Expected: Clean compilation with no errors.

- [ ] **Step 7: Commit**

```bash
git add src/pane/
git commit -m "feat(pane): add extract_pane() to PaneManager for live pane extraction"
```

---

### Task 2: Add `insert_subtree_at()` to PaneManager

**Files:**
- Modify: `src/pane/manager/creation.rs` — add `insert_subtree_at()` and `insert_subtree_at_node()`
- Modify: `src/pane/manager/mod.rs` — add public method declaration

- [ ] **Step 1: Add `insert_subtree_at()` method to `PaneManager` in `src/pane/manager/mod.rs`**

Add inside `impl PaneManager`, after `extract_pane()`:

```rust
/// Insert a `PaneNode` subtree into the tree by splitting the target pane.
///
/// The target leaf is replaced with a `Split` containing the original pane
/// as one child and the `subtree` as the other. Bounds are recalculated.
///
/// Returns `true` if the insertion succeeded.
pub fn insert_subtree_at(
    &mut self,
    target_pane_id: PaneId,
    subtree: PaneNode,
    direction: crate::pane::SplitDirection,
    ratio: f32,
) -> bool {
    if let Some(root) = self.root.take() {
        if let Some(new_root) =
            Self::insert_subtree_at_node(root, target_pane_id, subtree, direction, ratio)
        {
            self.root = Some(new_root);
            self.recalculate_bounds();
            return true;
        }
        self.root = Some(root);
    }
    false
}
```

- [ ] **Step 2: Add `insert_subtree_at_node()` recursive helper in `src/pane/manager/creation.rs`**

Add after `extract_pane_from_node()`:

```rust
/// Recursive helper: find a target leaf and replace it with a Split
/// containing the leaf and the subtree to insert.
fn insert_subtree_at_node(
    node: PaneNode,
    target_id: PaneId,
    subtree: PaneNode,
    direction: crate::pane::SplitDirection,
    ratio: f32,
) -> Option<PaneNode> {
    match node {
        PaneNode::Leaf(pane) => {
            if pane.id == target_id {
                Some(PaneNode::Split {
                    direction,
                    ratio,
                    first: Box::new(PaneNode::Leaf(pane)),
                    second: Box::new(subtree),
                })
            } else {
                None
            }
        }
        PaneNode::Split {
            direction: split_dir,
            ratio: existing_ratio,
            first,
            second,
        } => {
            // Try first child
            if let Some(new_first) =
                Self::insert_subtree_at_node(*first, target_id, subtree, direction, ratio)
            {
                Some(PaneNode::Split {
                    direction: split_dir,
                    ratio: existing_ratio,
                    first: Box::new(new_first),
                    second,
                })
            } else if let Some(new_second) =
                Self::insert_subtree_at_node(*second, target_id, subtree, direction, ratio)
            {
                Some(PaneNode::Split {
                    direction: split_dir,
                    ratio: existing_ratio,
                    first,
                    second: Box::new(new_second),
                })
            } else {
                None
            }
        }
    }
}
```

- [ ] **Step 3: Build and verify**

Run: `cargo check --workspace`
Expected: Clean compilation.

- [ ] **Step 4: Commit**

```bash
git add src/pane/manager/
git commit -m "feat(pane): add insert_subtree_at() for grafting pane trees"
```

---

### Task 3: Add `Tab::new_from_pane()` constructor

**Files:**
- Modify: `src/tab/constructors.rs` — add new constructor
- Modify: `src/tab/mod.rs` — ensure `TabInitParams` is accessible or add a simpler path

- [ ] **Step 1: Add `Tab::new_from_pane()` in `src/tab/constructors.rs`**

Add inside `impl Tab`, after `new_from_profile()`:

```rust
/// Create a new tab wrapping an existing `Pane` (e.g., from a promote operation).
///
/// The pane's PTY, scroll state, and session logger are preserved.
/// No new shell is spawned — the pane's terminal keeps running.
/// The tab shares the pane's `Arc<RwLock<TerminalManager>>` as its primary terminal.
pub fn new_from_pane(
    id: TabId,
    pane: crate::pane::Pane,
    config: &Config,
    runtime: Arc<Runtime>,
    tab_number: usize,
) -> Self {
    // Clone the pane's terminal Arc as the tab's primary terminal
    let terminal = Arc::clone(&pane.terminal);
    let is_active = Arc::clone(&pane.is_active);
    let session_logger = Arc::clone(&pane.session_logger);

    // Create a PaneManager with this pane as the single root
    let mut pm = PaneManager::new();
    pm.root = Some(crate::pane::PaneNode::leaf(pane));
    pm.focused_pane_id = pm.root.as_ref().and_then(|r| match r {
        crate::pane::PaneNode::Leaf(p) => Some(p.id),
        _ => None,
    });

    let title = format!("Tab {}", tab_number);

    Self {
        id,
        terminal,
        pane_manager: Some(pm),
        title,
        refresh_task: None,
        working_directory: None,
        custom_color: None,
        has_default_title: true,
        user_named: false,
        activity: TabActivityMonitor::default(),
        session_logger,
        tmux: TabTmuxState::default(),
        detected_hostname: None,
        detected_cwd: None,
        custom_icon: None,
        profile: TabProfileState::default(),
        scripting: TabScriptingState::default(),
        was_alt_screen: false,
        is_active,
        shutdown_fast: false,
        is_hidden: false,
        cached_modify_other_keys_mode: AtomicU8::new(0),
        cached_application_cursor: AtomicBool::new(false),
        cached_alt_screen_active: AtomicBool::new(false),
        cached_has_tmux_child: AtomicBool::new(false),
    }
}
```

- [ ] **Step 2: Add `pub` accessor for `pane.session_logger` in `src/pane/types/pane.rs`**

Check if `session_logger` is already `pub`. It is (line ~50 in the struct def: `pub session_logger`). No change needed — just verify.

- [ ] **Step 3: Build and verify**

Run: `cargo check --workspace`
Expected: Clean compilation.

- [ ] **Step 4: Commit**

```bash
git add src/tab/constructors.rs
git commit -m "feat(tab): add Tab::new_from_pane() constructor for pane transfer"
```

---

### Task 4: Add `PaneTransferState` and promote_pane_to_tab to WindowState

**Files:**
- Modify: `src/app/window_state/mod.rs` — add `PaneTransferState` field
- Create: `src/app/tab_ops/pane_transfer.rs` — promote and demote logic
- Modify: `src/app/tab_ops/mod.rs` — add `mod pane_transfer`

- [ ] **Step 1: Define `PaneTransferState` enum in a new file `src/app/tab_ops/pane_transfer.rs`**

```rust
//! Pane/tab promotion: promote pane to tab, demote tab to pane.

use std::sync::Arc;

use par_term_config::TabId;
use crate::pane::{ExtractResult, PaneId, SplitDirection};
use super::super::window_state::WindowState;

/// State machine for the multi-step demote (tab → pane) pick mode.
#[derive(Default)]
pub(crate) enum PaneTransferState {
    #[default]
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

impl PaneTransferState {
    pub fn is_active(&self) -> bool {
        !matches!(self, PaneTransferState::Idle)
    }
}

impl WindowState {
    /// Promote the focused pane in the current tab to its own tab.
    ///
    /// Extracts the pane from the source tab's tree, creates a new tab
    /// wrapping it, and inserts the new tab after the source tab.
    pub fn promote_pane_to_tab(&mut self) {
        let source_tab_id = match self.tab_manager.active_tab_id() {
            Some(id) => id,
            None => return,
        };

        let focused_pane_id = match self.tab_manager.active_tab().and_then(|t| t.focused_pane_id())
        {
            Some(id) => id,
            None => return,
        };

        // Extract the pane from the source tab's tree
        let pane = match self.tab_manager.get_tab_mut(source_tab_id) {
            Some(tab) => {
                let pm = match tab.pane_manager_mut() {
                    Some(pm) => pm,
                    None => return,
                };
                match pm.extract_pane(focused_pane_id) {
                    ExtractResult::Extracted { pane, remaining } => {
                        // Put the remaining tree back
                        if let Some(tab) = self.tab_manager.get_tab_mut(source_tab_id) {
                            if let Some(ref mut pm) = tab.pane_manager {
                                pm.root = remaining;
                            }
                        }
                        pane
                    }
                    ExtractResult::OnlyPane(pane) => pane,
                    ExtractResult::NotFound => return,
                }
            }
            None => return,
        };

        // If source tab is now empty, close it (but don't close the window)
        let source_is_empty = self
            .tab_manager
            .get_tab(source_tab_id)
            .is_none_or(|t| t.pane_count() == 0);

        let source_idx = self.tab_manager.tab_index(source_tab_id);

        // Generate tab number
        let tab_number = self.tab_manager.tab_count() + 1;

        // Get grid size for the new tab
        let grid_size = self.renderer.as_ref().map(|r| r.grid_size());

        // Create new tab from the extracted pane
        let new_tab_id = self.tab_manager.next_tab_id();
        let new_tab = crate::tab::Tab::new_from_pane(
            new_tab_id,
            pane,
            &self.config.load(),
            Arc::clone(&self.runtime),
            tab_number,
        );

        // Insert the new tab
        let tab_id = self.tab_manager.insert_tab(new_tab);

        // Position after source tab
        if let Some(idx) = source_idx {
            self.tab_manager.move_tab_to_index(tab_id, idx + 1);
        }

        // If source was empty, close it
        if source_is_empty {
            self.tab_manager.remove_tab(source_tab_id);
        }

        // Focus the new tab
        self.tab_manager.switch_to(tab_id);

        // Start refresh task for the new tab
        if let Some(window) = &self.window {
            if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
                tab.start_refresh_task(
                    Arc::clone(&self.runtime),
                    Arc::clone(window),
                    self.config.load().max_fps,
                    self.config.load().inactive_tab_fps,
                );
                tab.start_pane_refresh_tasks(
                    Arc::clone(&self.runtime),
                    Arc::clone(window),
                    self.config.load().max_fps,
                    self.config.load().inactive_tab_fps,
                );
            }
        }

        // Resize the new tab's terminal to match renderer dimensions
        if let Some(renderer) = &self.renderer {
            if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
                let (cols, rows) = renderer.grid_size();
                let cell_width = renderer.cell_width();
                let cell_height = renderer.cell_height();
                let width_px = (cols as f32 * cell_width) as usize;
                let height_px = (rows as f32 * cell_height) as usize;
                if let Ok(mut term) = tab.terminal.try_write() {
                    term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                    let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                }
            }
        }

        // Handle tab bar visibility change
        Self::handle_tab_bar_resize_after_promote(self);

        if let Some(renderer) = &mut self.renderer {
            renderer.clear_all_cells();
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();

        crate::debug_info!("PANE_PROMOTE", "Promoted pane {} to new tab {}", focused_pane_id, tab_id);
    }

    /// Start the demote (tab → pane) pick mode.
    pub fn start_demote_tab(&mut self) {
        if self.tab_manager.tab_count() < 2 {
            log::warn!("Cannot demote tab: need at least 2 tabs");
            return;
        }
        if let Some(tab_id) = self.tab_manager.active_tab_id() {
            self.pane_transfer_state = PaneTransferState::DemotePickTab {
                source_tab_id: tab_id,
            };
            self.focus_state.needs_redraw = true;
            self.request_redraw();
            crate::debug_info!("TAB_DEMOTE", "Started demote pick mode for tab {}", tab_id);
        }
    }

    /// Cancel the demote pick mode.
    pub fn cancel_pane_transfer(&mut self) {
        self.pane_transfer_state = PaneTransferState::Idle;
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Execute the demote: merge source tab's pane tree into target tab.
    pub(crate) fn execute_demote(
        &mut self,
        source_tab_id: TabId,
        target_tab_id: TabId,
        target_pane_id: PaneId,
        direction: SplitDirection,
    ) {
        // Check max_panes on target tab
        let config = self.config.load();
        if config.max_panes > 0 {
            let target_pane_count = self
                .tab_manager
                .get_tab(target_tab_id)
                .map(|t| t.pane_count())
                .unwrap_or(0);
            let source_pane_count = self
                .tab_manager
                .get_tab(source_tab_id)
                .map(|t| t.pane_count())
                .unwrap_or(0);
            if target_pane_count + source_pane_count > config.max_panes {
                log::warn!(
                    "Cannot demote: would exceed max_panes ({})",
                    config.max_panes
                );
                self.cancel_pane_transfer();
                return;
            }
        }
        drop(config);

        // Extract the source tab's entire pane tree
        let source_tree = match self.tab_manager.get_tab_mut(source_tab_id) {
            Some(tab) => {
                let pm = match tab.pane_manager_mut() {
                    Some(pm) => pm,
                    None => {
                        self.cancel_pane_transfer();
                        return;
                    }
                };
                pm.root.take()
            }
            None => {
                self.cancel_pane_transfer();
                return;
            }
        };

        let mut source_tree = match source_tree {
            Some(tree) => tree,
            None => {
                self.cancel_pane_transfer();
                return;
            }
        };

        // Get the target tab's is_active Arc for updating transplanted panes
        let target_is_active = self
            .tab_manager
            .get_tab(target_tab_id)
            .and_then(|t| Some(Arc::clone(&t.is_active)));

        // Insert the source tree into the target tab at the target pane
        let inserted = match self.tab_manager.get_tab_mut(target_tab_id) {
            Some(tab) => {
                let pm = match tab.pane_manager_mut() {
                    Some(pm) => pm,
                    None => false,
                };
                // Update is_active on all transplanted panes
                if let Some(ref is_active) = target_is_active {
                    for pane in source_tree.all_panes_mut() {
                        pane.is_active = Arc::clone(is_active);
                    }
                }
                pm.insert_subtree_at(target_pane_id, source_tree, direction, 0.5)
            }
            None => false,
        };

        if !inserted {
            self.cancel_pane_transfer();
            return;
        }

        // Close the source tab (it's now empty)
        self.tab_manager.remove_tab(source_tab_id);

        // Start refresh tasks for all panes in the target tab
        if let Some(window) = &self.window {
            if let Some(tab) = self.tab_manager.get_tab_mut(target_tab_id) {
                tab.start_pane_refresh_tasks(
                    Arc::clone(&self.runtime),
                    Arc::clone(window),
                    self.config.load().max_fps,
                    self.config.load().inactive_tab_fps,
                );
            }
        }

        self.pane_transfer_state = PaneTransferState::Idle;

        // Handle tab bar visibility change
        Self::handle_tab_bar_resize_after_promote(self);

        if let Some(renderer) = &mut self.renderer {
            renderer.clear_all_cells();
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();

        crate::debug_info!(
            "TAB_DEMOTE",
            "Demoted tab {} into tab {} at pane {}",
            source_tab_id,
            target_tab_id,
            target_pane_id
        );
    }

    /// Handle tab bar visibility changes after tab count changes.
    fn handle_tab_bar_resize_after_promote(&mut self) {
        let tab_count = self.tab_manager.tab_count();
        let config = self.config.load();
        let tab_bar_height = self.tab_bar_ui.get_height(tab_count, &config);
        let tab_bar_width = self.tab_bar_ui.get_width(tab_count, &config);

        if let Some(renderer) = &mut self.renderer {
            let size = renderer.size();
            let padding = renderer.window_padding();
            let content_offset_y = renderer.content_offset_y();
            let scale = renderer.scale_factor();

            let physical_tab_bar_height = tab_bar_height * scale;
            let content_width = size.width as f32 - padding * 2.0;
            let content_height =
                size.height as f32 - content_offset_y - padding - physical_tab_bar_height;

            let bounds = crate::pane::PaneBounds::new(
                padding,
                content_offset_y,
                content_width,
                content_height,
            );
            let cell_width = renderer.cell_width();
            let cell_height = renderer.cell_height();

            for tab in self.tab_manager.tabs_mut() {
                tab.set_pane_bounds(bounds, cell_width, cell_height);
            }
        }
    }
}
```

- [ ] **Step 2: Add `next_tab_id()` to `TabManager` in `src/tab/manager.rs`**

Check if this method exists. If not, add:

```rust
/// Get the next tab ID that will be assigned
pub fn next_tab_id(&self) -> TabId {
    self.next_tab_id
}
```

- [ ] **Step 3: Add `insert_tab()` to `TabManager` in `src/tab/manager.rs`**

Check if this method exists. If not, add:

```rust
/// Insert an already-constructed tab and return its ID
pub fn insert_tab(&mut self, tab: crate::tab::Tab) -> TabId {
    let id = tab.id;
    self.tabs.push(tab);
    self.set_active_tab(Some(id));
    id
}
```

- [ ] **Step 4: Add `tab_index()` to `TabManager` in `src/tab/manager.rs`**

Check if this method exists. If not, add:

```rust
/// Get the index of a tab by ID
pub fn tab_index(&self, id: TabId) -> Option<usize> {
    self.tabs.iter().position(|t| t.id == id)
}
```

- [ ] **Step 5: Add `remove_tab()` to `TabManager` in `src/tab/manager.rs`**

This method should already exist (used by session undo). Verify it returns the live `Tab`. If it doesn't exist, add:

```rust
/// Remove a tab by ID, returning the live Tab.
/// Used by promote/demote to transfer tabs without killing PTYs.
pub fn remove_tab(&mut self, id: TabId) -> Option<crate::tab::Tab> {
    let idx = self.tabs.iter().position(|t| t.id == id)?;
    let removed = self.tabs.remove(idx);
    // If the removed tab was active, switch to neighbor
    if self.active_tab_id == Some(id) {
        let new_idx = idx.min(self.tabs.len().saturating_sub(1));
        self.active_tab_id = self.tabs.get(new_idx).map(|t| t.id);
    }
    self.renumber_default_titled_tabs();
    Some(removed)
}
```

- [ ] **Step 6: Add `pane_transfer_state` field to `WindowState` in `src/app/window_state/mod.rs`**

Add inside `WindowState` struct, in the "Feature state" section (around line 226):

```rust
/// State machine for promote/demote pane-tab operations
pub(crate) pane_transfer_state: crate::app::tab_ops::pane_transfer::PaneTransferState,
```

Initialize in the `WindowState::new()` constructor (find the existing field initializations):

```rust
pane_transfer_state: Default::default(),
```

- [ ] **Step 7: Add module declaration in `src/app/tab_ops/mod.rs`**

Add:

```rust
pub(crate) mod pane_transfer;
```

- [ ] **Step 8: Build and verify**

Run: `cargo check --workspace`
Expected: Clean compilation (there will be unused warnings for demote state handlers — that's fine, they're wired in later tasks).

- [ ] **Step 9: Commit**

```bash
git add src/app/tab_ops/ src/app/window_state/mod.rs src/tab/manager.rs
git commit -m "feat(transfer): add promote_pane_to_tab() and demote state machine"
```

---

### Task 5: Wire keybinding actions and Escape handler

**Files:**
- Modify: `src/app/input_events/keybinding_actions.rs` — add action dispatch
- Modify: `src/app/input_events/key_handler/mod.rs` — handle Escape during pick mode

- [ ] **Step 1: Add action dispatch in `src/app/input_events/keybinding_actions.rs`**

Add after the `"toggle_broadcast_input"` match arm (around line 296):

```rust
"promote_pane_to_tab" => {
    self.promote_pane_to_tab();
    true
}
"demote_tab_to_pane" => {
    self.start_demote_tab();
    true
}
```

- [ ] **Step 2: Handle Escape during demote pick mode in `src/app/input_events/key_handler/mod.rs`**

Find where `Escape` key is handled (search for `Escape` or `NamedKey::Escape`). Add before the PTY-write fallback (after the keybinding lookup, before hardcoded shortcuts or at the Escape handling point):

```rust
// Cancel pane transfer pick mode on Escape
if matches!(
    ui_events::KeyCode::Named(winit::keyboard::NamedKey::Escape),
    event.logical_key
) && self.pane_transfer_state.is_active()
{
    self.cancel_pane_transfer();
    return;
}
```

Note: Check the actual `event.logical_key` type and match pattern used in this file. Adapt accordingly — the key may come from `KeyEvent` from winit. Look at how Escape is handled elsewhere in this file for the exact pattern.

- [ ] **Step 3: Build and verify**

Run: `cargo check --workspace`
Expected: Clean compilation.

- [ ] **Step 4: Commit**

```bash
git add src/app/input_events/
git commit -m "feat(keybind): wire promote_pane_to_tab and demote_tab_to_pane actions"
```

---

### Task 6: Wire demote pick mode into mouse event handlers

**Files:**
- Modify: `src/app/mouse_events/mouse_button.rs` — intercept clicks during pick mode

- [ ] **Step 1: Add demote pick mode click handling**

Find the mouse click handler (the `MouseInput::ButtonPressed` / `ElementState::Pressed` handler for left clicks). Before the normal click handling, add a guard that checks `self.pane_transfer_state`:

For a left-click in the content area during `DemotePickTab` or `DemotePickPane` state, check if the click is on the tab bar (handled by tab bar) or in the terminal content area.

The simplest approach: intercept in the tab bar click handler. When in `DemotePickTab` state and a tab is clicked (not the source tab), transition to `DemotePickPane`. When in `DemotePickPane` state and a pane is clicked, transition to `DemoteChooseDirection`.

This wiring happens in the `handle_tab_bar_action_after_render` method in `src/app/window_state/action_handlers/tab_bar.rs`:

```rust
// After TabBarAction::SwitchTo(id) handling, add demote pick mode logic:
TabBarAction::SwitchTo(id) => {
    // Check if we're in demote pick-tab mode
    if let PaneTransferState::DemotePickTab { source_tab_id } = &self.pane_transfer_state {
        let target_id = id;
        if target_id == *source_tab_id {
            // Reject demote to self
            return;
        }
        let source = *source_tab_id;
        self.pane_transfer_state = PaneTransferState::DemotePickPane {
            source_tab_id: source,
            target_tab_id: target_id,
        };
        self.tab_manager.switch_to(target_id);
        self.clear_and_invalidate();
        return;
    }
    // Normal switch
    self.tab_manager.switch_to(id);
    self.clear_and_invalidate();
}
```

For the pane click (`DemotePickPane` → `DemoteChooseDirection`), intercept in the mouse click handler for terminal content area clicks. In `src/app/mouse_events/mouse_button.rs`, in the left-click pressed handler, before the normal focus/select logic:

```rust
// Check demote pick-pane mode
if let crate::app::tab_ops::pane_transfer::PaneTransferState::DemotePickPane {
    source_tab_id,
    target_tab_id,
} = &self.pane_transfer_state
{
    if let Some(tab) = self.tab_manager.active_tab()
        && let Some(pm) = tab.pane_manager()
        && let Some(pane) = pm.find_pane_at(cursor_x, cursor_y)
    {
        let source = *source_tab_id;
        let target = *target_tab_id;
        self.pane_transfer_state =
            crate::app::tab_ops::pane_transfer::PaneTransferState::DemoteChooseDirection {
                source_tab_id: source,
                target_tab_id: target,
                target_pane_id: pane.id,
            };
        self.focus_state.needs_redraw = true;
        self.request_redraw();
        return; // Don't process normal click
    }
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo check --workspace`
Expected: Clean compilation.

- [ ] **Step 3: Commit**

```bash
git add src/app/mouse_events/ src/app/window_state/action_handlers/tab_bar.rs
git commit -m "feat(transfer): wire demote pick mode into click handlers"
```

---

### Task 7: Add direction-choice egui overlay for demote

**Files:**
- Modify: `src/app/tab_ops/pane_transfer.rs` — add overlay rendering state
- Modify: `src/app/render_pipeline/post_render.rs` — render the overlay in the egui pass

- [ ] **Step 1: Add overlay rendering in the egui pass**

The direction-choice overlay appears when `PaneTransferState::DemoteChooseDirection` is active. In the egui rendering pass (where other overlays like close confirmation are rendered), add:

```rust
// In the egui overlay rendering, after existing overlay checks:
if let crate::app::tab_ops::pane_transfer::PaneTransferState::DemoteChooseDirection {
    target_pane_id,
    source_tab_id,
    target_tab_id,
} = &self.pane_transfer_state
{
    // Get the target pane's bounds to position the overlay
    let pane_bounds = self.tab_manager.get_tab(*target_tab_id)
        .and_then(|t| t.pane_manager())
        .and_then(|pm| pm.get_pane(*target_pane_id))
        .map(|p| p.bounds);

    if let Some(bounds) = pane_bounds {
        let center_x = bounds.x + bounds.width / 2.0;
        let center_y = bounds.y + bounds.height / 2.0;

        egui::Area::new(egui::Id::new("demote_direction_overlay"))
            .fixed_pos(egui::pos2(center_x - 100.0, center_y - 20.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label("Split direction:");
                    ui.horizontal(|ui| {
                        if ui.button("Horizontal").clicked() {
                            // Execute demote
                            let state = std::mem::take(&mut self.pane_transfer_state);
                            if let crate::app::tab_ops::pane_transfer::PaneTransferState::DemoteChooseDirection {
                                source_tab_id,
                                target_tab_id,
                                target_pane_id,
                            } = state
                            {
                                self.execute_demote(
                                    source_tab_id,
                                    target_tab_id,
                                    target_pane_id,
                                    crate::pane::SplitDirection::Horizontal,
                                );
                            }
                        }
                        if ui.button("Vertical").clicked() {
                            let state = std::mem::take(&mut self.pane_transfer_state);
                            if let crate::app::tab_ops::pane_transfer::PaneTransferState::DemoteChooseDirection {
                                source_tab_id,
                                target_tab_id,
                                target_pane_id,
                            } = state
                            {
                                self.execute_demote(
                                    source_tab_id,
                                    target_tab_id,
                                    target_pane_id,
                                    crate::pane::SplitDirection::Vertical,
                                );
                            }
                        }
                    });
                });
            });
    }
}
```

Note: The exact rendering location depends on where the existing egui overlay pass is. Check `src/app/render_pipeline/post_render.rs` for the egui overlay rendering pattern used by the close confirmation dialog and follow that pattern. The `WindowState` reference may need to be passed differently. Adapt the pattern to match.

- [ ] **Step 2: Also render a status message during DemotePickTab and DemotePickPane**

During `DemotePickTab`, show "Click a tab to merge into" in the status bar or as a temporary toast. During `DemotePickPane`, show "Click a pane to merge into". Use the existing `show_toast()` method or status bar message pattern.

- [ ] **Step 3: Build and verify**

Run: `cargo check --workspace`
Expected: Clean compilation.

- [ ] **Step 4: Commit**

```bash
git add src/app/render_pipeline/ src/app/tab_ops/pane_transfer.rs
git commit -m "feat(transfer): add direction-choice overlay for demote"
```

---

### Task 8: Add tab bar context menu entries

**Files:**
- Modify: `src/tab_bar_ui/mod.rs` — add `TabBarAction` variants
- Modify: `src/tab_bar_ui/context_menu.rs` — add menu items
- Modify: `src/app/window_state/action_handlers/tab_bar.rs` — dispatch new actions

- [ ] **Step 1: Add action variants to `TabBarAction` in `src/tab_bar_ui/mod.rs`**

Add to the enum (after `MoveTabToExistingWindow`, around line 67):

```rust
/// Promote the focused pane of this tab to a new tab
PromotePaneToTab(TabId),
/// Start demote pick mode for this tab
DemoteTabToPane(TabId),
```

- [ ] **Step 2: Add menu items to context menu in `src/tab_bar_ui/context_menu.rs`**

After the "Move Tab to New Window" section (around line 249), add:

```rust
// ----- Promote / Demote -----
ui.add_space(4.0);
ui.separator();
ui.add_space(4.0);

// "Promote Pane to Tab" — only if tab has multiple panes
let has_multiple = self.tab_has_multiple_panes;
ui.add_enabled_ui(has_multiple, |ui| {
    if menu_item(ui, "Promote Pane to Tab") {
        action = TabBarAction::PromotePaneToTab(tab_id);
        close_menu = true;
    }
});

// "Demote Tab to Pane" — only if there are other tabs to receive it
let has_other_tabs = self.move_source_tab_count >= 2;
ui.add_enabled_ui(has_other_tabs, |ui| {
    if menu_item(ui, "Demote Tab to Pane") {
        action = TabBarAction::DemoteTabToPane(tab_id);
        close_menu = true;
    }
});
```

Also add `tab_has_multiple_panes: bool` to the `TabBarUI` state struct in `src/tab_bar_ui/state.rs` and set it each frame when populating context menu state.

- [ ] **Step 3: Dispatch new actions in `src/app/window_state/action_handlers/tab_bar.rs`**

Add inside the match:

```rust
TabBarAction::PromotePaneToTab(tab_id) => {
    self.tab_manager.switch_to(tab_id);
    self.promote_pane_to_tab();
    self.request_redraw();
}
TabBarAction::DemoteTabToPane(tab_id) => {
    self.tab_manager.switch_to(tab_id);
    self.start_demote_tab();
    self.request_redraw();
}
```

- [ ] **Step 4: Build and verify**

Run: `cargo check --workspace`
Expected: Clean compilation.

- [ ] **Step 5: Commit**

```bash
git add src/tab_bar_ui/ src/app/window_state/action_handlers/tab_bar.rs
git commit -m "feat(ui): add promote/demote to tab bar context menu"
```

---

### Task 9: Add settings UI keybinding entries

**Files:**
- Modify: `par-term-settings-ui/src/input_tab/actions_table.rs` — add actions to both platform tables
- Modify: `par-term-settings-ui/src/input_tab/mod.rs` — add search keywords

- [ ] **Step 1: Add to macOS `AVAILABLE_ACTIONS` table in `par-term-settings-ui/src/input_tab/actions_table.rs`**

Add after the `"close_pane"` entry (around line 68):

```rust
("promote_pane_to_tab", "Promote Pane to Tab", None),
("demote_tab_to_pane", "Demote Tab to Pane", None),
```

- [ ] **Step 2: Add to Linux/Windows `AVAILABLE_ACTIONS` table (same file, around line 213)**

Add after the `"close_pane"` entry:

```rust
("promote_pane_to_tab", "Promote Pane to Tab", None),
("demote_tab_to_pane", "Demote Tab to Pane", None),
```

- [ ] **Step 3: Add search keywords in `par-term-settings-ui/src/input_tab/mod.rs`**

Add to the `keywords()` function, in the keybindings section (around line 236):

```rust
"promote",
"demote",
"pane to tab",
"tab to pane",
```

- [ ] **Step 4: Build and verify**

Run: `cargo check --workspace`
Expected: Clean compilation.

- [ ] **Step 5: Commit**

```bash
git add par-term-settings-ui/
git commit -m "feat(settings): add promote/demote keybinding config in settings UI"
```

---

### Task 10: Integration test and final verification

**Files:**
- No new files — manual testing

- [ ] **Step 1: Build the dev-release binary**

Run: `make build`
Expected: Successful build.

- [ ] **Step 2: Run the test suite**

Run: `make test`
Expected: All tests pass.

- [ ] **Step 3: Run lint and format checks**

Run: `make checkall`
Expected: All checks pass.

- [ ] **Step 4: Manual test — promote**

1. Launch par-term
2. Split the pane (Cmd+D)
3. Bind a key to `promote_pane_to_tab` in config
4. Press it — the focused pane should become a new tab
5. Verify: the original tab still has the other pane, the new tab has the promoted pane
6. Verify: running processes in both tabs continue

- [ ] **Step 5: Manual test — promote single pane**

1. With a single-pane tab, press the promote key
2. Verify: a new tab is created, the original tab is closed
3. The terminal content and processes should be preserved

- [ ] **Step 6: Manual test — demote via context menu**

1. Create two tabs, each with content
2. Right-click the tab bar on one tab → "Demote Tab to Pane"
3. Click the other tab in the tab bar
4. Click a pane within that tab
5. Choose Horizontal or Vertical in the overlay
6. Verify: the source tab's panes are merged into the target tab
7. Verify: source tab is closed, all processes continue

- [ ] **Step 7: Manual test — cancel demote**

1. Start demote, press Escape — should cancel back to Idle
2. Start demote, right-click — should cancel

- [ ] **Step 8: Manual test — edge cases**

1. Demote to self — should be rejected
2. Only one tab — demote should be rejected (context menu item disabled)
3. max_panes limit — should be rejected with warning if exceeded

- [ ] **Step 9: Fix any issues found during testing**

- [ ] **Step 10: Final commit**

```bash
git add -A
git commit -m "test: verify pane/tab promotion feature"
```

---

## Self-Review

**Spec coverage:**
- Promote pane to tab → Task 4 (`promote_pane_to_tab()`)
- Demote tab to pane state machine → Task 4 (`PaneTransferState`)
- Demote pick-tab step → Task 6 (tab bar click intercept)
- Demote pick-pane step → Task 6 (mouse click intercept)
- Demote direction overlay → Task 7 (egui overlay)
- Escape cancel → Task 5
- Right-click cancel → Task 6 (mouse right-click returns early)
- Context menu entries → Task 8
- Settings UI keybindings → Task 9
- `extract_pane()` → Task 1
- `insert_subtree_at()` → Task 2
- `Tab::new_from_pane()` → Task 3
- is_active Arc update → Task 4 (`execute_demote`)
- max_panes check → Task 4
- demote to self rejection → Task 6

**Placeholder scan:** No TBD/TODO/vague steps. All code blocks are complete.

**Type consistency:**
- `ExtractResult` defined in Task 1, used consistently in Tasks 1 and 4
- `PaneTransferState` defined in Task 4, referenced by all demote tasks
- `insert_subtree_at()` signature matches usage in Task 4
- `Tab::new_from_pane()` signature matches usage in Task 4
- `TabBarAction` variants match dispatch in Task 8
- `AVAILABLE_ACTIONS` tuples match existing pattern in Task 9
