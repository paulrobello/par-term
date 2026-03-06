# Replace Arrangement Button Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a "Replace" button to each saved arrangement row that overwrites the arrangement's captured layout with the current window state, after a confirmation dialog.

**Architecture:** Follows the exact pattern of the existing Delete confirmation flow — a new `arrangement_confirm_replace: Option<ArrangementId>` field on `SettingsUI` drives an inline confirmation dialog, which dispatches `SettingsWindowAction::ReplaceArrangement(id)` to a new `replace_arrangement()` method on `WindowManager` that captures the current layout and updates the arrangement in-place (preserving name and order).

**Tech Stack:** Rust 2024, egui (immediate-mode UI), winit, wgpu

---

### Task 1: Add `ReplaceArrangement` action variant

**Files:**
- Modify: `par-term-settings-ui/src/lib.rs:129-138`

**Step 1: Add the new action variant**

In `par-term-settings-ui/src/lib.rs`, find the `SettingsWindowAction` enum (around line 99). After the `MoveArrangementDown` variant, add:

```rust
    /// Replace a saved window arrangement with the current window layout
    ReplaceArrangement(ArrangementId),
```

The relevant section currently reads:
```rust
    /// Move a saved window arrangement one position down in the list
    MoveArrangementDown(ArrangementId),
    /// User requested an immediate update check
    ForceUpdateCheck,
```

After the change:
```rust
    /// Move a saved window arrangement one position down in the list
    MoveArrangementDown(ArrangementId),
    /// Replace a saved window arrangement with the current window layout
    ReplaceArrangement(ArrangementId),
    /// User requested an immediate update check
    ForceUpdateCheck,
```

**Step 2: Verify it compiles**

```bash
cargo check -p par-term-settings-ui
```
Expected: No errors (the new variant is just data, no match arms needed yet in the UI crate).

**Step 3: Commit**

```bash
git add par-term-settings-ui/src/lib.rs
git commit -m "feat(arrangements): add ReplaceArrangement action variant"
```

---

### Task 2: Add `arrangement_confirm_replace` state field to `SettingsUI`

**Files:**
- Modify: `par-term-settings-ui/src/settings_ui/mod.rs:381-391`
- Modify: `par-term-settings-ui/src/settings_ui/state.rs:219-225`

**Step 1: Add the field to the struct**

In `par-term-settings-ui/src/settings_ui/mod.rs`, find the arrangement-related fields (around line 381). After `arrangement_confirm_overwrite`, add:

```rust
    /// Arrangement pending replace confirmation (stores arrangement ID)
    pub arrangement_confirm_replace: Option<ArrangementId>,
```

The block currently reads:
```rust
    pub arrangement_confirm_restore: Option<ArrangementId>,
    // ...
    pub arrangement_confirm_delete: Option<ArrangementId>,
    // ...
    pub arrangement_confirm_overwrite: Option<String>,
    // ...
    pub arrangement_rename_id: Option<ArrangementId>,
```

Add the new field after `arrangement_confirm_overwrite`:
```rust
    pub arrangement_confirm_overwrite: Option<String>,

    /// Arrangement pending replace confirmation (stores arrangement ID)
    pub arrangement_confirm_replace: Option<ArrangementId>,
```

**Step 2: Initialize the field in the default state**

In `par-term-settings-ui/src/settings_ui/state.rs`, find the arrangement initialization block (around line 219). After `arrangement_confirm_overwrite: None,`, add:

```rust
            arrangement_confirm_replace: None,
```

The block should look like:
```rust
            arrangement_save_name: String::new(),
            arrangement_confirm_restore: None,
            arrangement_confirm_delete: None,
            arrangement_confirm_overwrite: None,
            arrangement_confirm_replace: None,
            arrangement_rename_id: None,
            arrangement_rename_text: String::new(),
```

**Step 3: Verify it compiles**

```bash
cargo check -p par-term-settings-ui
```
Expected: No errors.

**Step 4: Commit**

```bash
git add par-term-settings-ui/src/settings_ui/mod.rs par-term-settings-ui/src/settings_ui/state.rs
git commit -m "feat(arrangements): add arrangement_confirm_replace state field"
```

---

### Task 3: Add Replace button and confirmation dialog to the UI

**Files:**
- Modify: `par-term-settings-ui/src/arrangements_tab.rs`

**Step 1: Add the Replace button in the row**

In `show_arrangements_with_manager()` (around line 169), find the `ui.with_layout(right_to_left, ...)` block that renders the action buttons. The current order (right-to-left) is: `▼ ▲ Delete Rename Restore`.

Add `Replace` after `Delete` (it will appear to the left of Delete in right-to-left layout):

```rust
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Reorder buttons
                if i < arrangements.len() - 1 && ui.small_button("▼").clicked() {
                    settings
                        .pending_arrangement_actions
                        .push(SettingsWindowAction::MoveArrangementDown(id));
                }
                if i > 0 && ui.small_button("▲").clicked() {
                    settings
                        .pending_arrangement_actions
                        .push(SettingsWindowAction::MoveArrangementUp(id));
                }

                if ui.small_button("Delete").clicked() {
                    settings.arrangement_confirm_delete = Some(id);
                }
                if ui.small_button("Replace").clicked() {
                    settings.arrangement_confirm_replace = Some(id);
                }
                if ui.small_button("Rename").clicked() {
                    settings.arrangement_rename_id = Some(id);
                    settings.arrangement_rename_text = arr.name.clone();
                }
                if ui.small_button("Restore").clicked() {
                    settings.arrangement_confirm_restore = Some(id);
                }
            });
```

**Step 2: Add the confirmation dialog function**

At the end of the `// Confirmation Dialogs` section (after `show_rename_dialog`, around line 327), add a new function:

```rust
fn show_confirm_replace_dialog(ui: &mut egui::Ui, settings: &mut SettingsUI) {
    if let Some(id) = settings.arrangement_confirm_replace {
        // Look up the arrangement name for the dialog message
        let name = settings
            .arrangement_manager
            .get(&id)
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "this arrangement".to_string());

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "Replace \"{}\" with the current window layout?",
                    name
                ))
                .strong()
                .color(egui::Color32::from_rgb(255, 193, 7)),
            );
            ui.label("This cannot be undone.");
            ui.horizontal(|ui| {
                if ui.button("Replace").clicked() {
                    settings
                        .pending_arrangement_actions
                        .push(SettingsWindowAction::ReplaceArrangement(id));
                    settings.arrangement_confirm_replace = None;
                }
                if ui.button("Cancel").clicked() {
                    settings.arrangement_confirm_replace = None;
                }
            });
        });
    }
}
```

**Step 3: Call the new dialog from `show_arrangements_list`**

In `show_arrangements_list()` (around line 120), add the call after `show_confirm_delete_dialog`:

```rust
        |ui| {
            let manager = settings.arrangement_manager.clone();
            show_arrangements_with_manager(ui, settings, &manager);

            ui.add_space(4.0);

            // Show confirmation dialogs
            show_confirm_restore_dialog(ui, settings);
            show_confirm_delete_dialog(ui, settings);
            show_confirm_replace_dialog(ui, settings);
            show_rename_dialog(ui, settings);
        },
```

**Step 4: Verify it compiles**

```bash
cargo check -p par-term-settings-ui
```
Expected: No errors.

**Step 5: Commit**

```bash
git add par-term-settings-ui/src/arrangements_tab.rs
git commit -m "feat(arrangements): add Replace button and confirmation dialog to arrangement rows"
```

---

### Task 4: Implement `replace_arrangement` on `WindowManager`

**Files:**
- Modify: `src/app/window_manager/arrangements.rs`

**Step 1: Add the `replace_arrangement` method**

In `src/app/window_manager/arrangements.rs`, after `save_arrangement()` (around line 38), add:

```rust
    /// Replace the content of an existing arrangement with the current window layout.
    ///
    /// Preserves the arrangement's ID, name, and display order.
    pub fn replace_arrangement(&mut self, id: ArrangementId, event_loop: &ActiveEventLoop) {
        // Look up name before capturing so we can log it
        let name = match self.arrangement_manager.get(&id) {
            Some(a) => a.name.clone(),
            None => {
                log::error!("replace_arrangement: arrangement not found: {}", id);
                return;
            }
        };

        // Capture the current layout using the existing name
        let mut new_arrangement = crate::arrangements::capture::capture_arrangement(
            name.clone(),
            &self.windows,
            event_loop,
        );

        // Overwrite the ID and order from the original so it stays in place
        let original_order = self
            .arrangement_manager
            .get(&id)
            .map(|a| a.order)
            .unwrap_or(0);
        new_arrangement.id = id;
        new_arrangement.order = original_order;

        log::info!(
            "Replaced arrangement '{}' with current layout ({} windows)",
            name,
            new_arrangement.windows.len()
        );

        self.arrangement_manager.update(new_arrangement);
        if let Err(e) = crate::arrangements::storage::save_arrangements(&self.arrangement_manager) {
            log::error!("Failed to save arrangements after replace: {}", e);
        }
        self.sync_arrangements_to_settings();
    }
```

**Step 2: Verify it compiles**

```bash
cargo check
```
Expected: No errors.

**Step 3: Commit**

```bash
git add src/app/window_manager/arrangements.rs
git commit -m "feat(arrangements): add replace_arrangement method to WindowManager"
```

---

### Task 5: Dispatch `ReplaceArrangement` in the action handler

**Files:**
- Modify: `src/app/handler/app_handler_impl.rs:126-143`

**Step 1: Add the match arm**

In `src/app/handler/app_handler_impl.rs`, find the arrangement action match arms (around line 126). After `MoveArrangementDown`, add:

```rust
                    SettingsWindowAction::ReplaceArrangement(id) => {
                        self.replace_arrangement(id, event_loop);
                    }
```

The block should look like:
```rust
                    SettingsWindowAction::MoveArrangementUp(id) => {
                        self.move_arrangement_up(id);
                    }
                    SettingsWindowAction::MoveArrangementDown(id) => {
                        self.move_arrangement_down(id);
                    }
                    SettingsWindowAction::ReplaceArrangement(id) => {
                        self.replace_arrangement(id, event_loop);
                    }
                    SettingsWindowAction::ForceUpdateCheck => {
```

**Step 2: Verify full build**

```bash
make build
```
Expected: Builds cleanly with no warnings related to the new code.

**Step 3: Commit**

```bash
git add src/app/handler/app_handler_impl.rs
git commit -m "feat(arrangements): dispatch ReplaceArrangement action to window manager"
```

---

### Task 6: Manual verification

**Step 1: Run the app**

```bash
make run
```

**Step 2: Test the Replace flow**

1. Open Settings → Arrangements tab
2. Save at least one arrangement (use "Save Current Layout")
3. Open some new tabs or resize windows
4. In the saved arrangements list, click "Replace" on the saved arrangement
5. Verify: a confirmation dialog appears inline below the list with the arrangement's name and "This cannot be undone."
6. Click "Replace" in the dialog
7. Verify: the arrangement is updated (tab count/window count summary changes to reflect new state)
8. Click "Cancel" on a second replace attempt — verify the dialog dismisses without changes

**Step 3: Run tests**

```bash
make test
```
Expected: All tests pass.

**Step 4: Final commit if any cleanup needed**

```bash
git add -A
git commit -m "chore(arrangements): post-replace cleanup"
```
