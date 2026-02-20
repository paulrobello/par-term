# Tab Icon via Context Menu — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow users to set a custom icon on any tab via the right-click context menu, with persistence across sessions, layouts, and tab duplication.

**Architecture:** Add a `custom_icon: Option<String>` field to the `Tab` struct. Rendering uses `custom_icon.as_deref().or(profile_icon.as_deref())` so custom always wins over profile. The icon picker reuses the existing Nerd Font preset grid + free-text entry from the profile settings. Persistence is added to `SessionTab`, `TabSnapshot`, capture code, restore code, and tab duplication.

**Tech Stack:** Rust, egui, serde (YAML serialization)

---

### Task 1: Add `custom_icon` field to Tab struct

**Files:**
- Modify: `src/tab/mod.rs:352` (add field after `profile_icon`)

**Step 1: Add the field**

In `src/tab/mod.rs`, after line 352 (`pub profile_icon: Option<String>,`), add:

```rust
/// Custom icon set by user via context menu (takes precedence over profile_icon)
pub custom_icon: Option<String>,
```

**Step 2: Initialize in Tab::new()**

Find the `Tab { ... }` construction in `Tab::new()` and add `custom_icon: None,` alongside the existing `profile_icon: None,`.

**Step 3: Build to verify**

Run: `make build`
Expected: PASS (no compilation errors)

**Step 4: Commit**

```bash
git add src/tab/mod.rs
git commit -m "feat(tab): add custom_icon field for user-set tab icons"
```

---

### Task 2: Update icon rendering to prefer custom_icon

**Files:**
- Modify: `src/tab_bar_ui.rs:276,319,449` (3 call sites that pass `profile_icon`)

**Step 1: Change all 3 call sites**

At lines 276, 319, and 449, change:
```rust
tab.profile_icon.as_deref(),
```
to:
```rust
tab.custom_icon.as_deref().or(tab.profile_icon.as_deref()),
```

This ensures the custom icon takes precedence when set.

**Step 2: Build to verify**

Run: `make build`
Expected: PASS

**Step 3: Commit**

```bash
git add src/tab_bar_ui.rs
git commit -m "feat(tab-bar): prefer custom_icon over profile_icon in rendering"
```

---

### Task 3: Add `SetTabIcon` action and context menu UI

**Files:**
- Modify: `src/tab_bar_ui.rs:32-55` (TabBarAction enum)
- Modify: `src/tab_bar_ui.rs:57-97` (TabBarUI struct — add state fields)
- Modify: `src/tab_bar_ui.rs:99+` (TabBarUI::new — initialize new fields)
- Modify: `src/tab_bar_ui.rs:788-796,1231-1241` (right-click handlers — capture current icon)
- Modify: `src/tab_bar_ui.rs:1509-1678` (render_context_menu — add icon picker section)

**Step 1: Add action variant**

In the `TabBarAction` enum (around line 54), add before `ToggleAssistantPanel`:

```rust
/// Set custom icon for a tab (None = clear)
SetTabIcon(TabId, Option<String>),
```

**Step 2: Add state fields to TabBarUI**

After `context_menu_title` (line 92), add:

```rust
/// Whether the icon picker is active in the context menu
picking_icon: bool,
/// Buffer for the icon text field in the context menu
icon_buffer: String,
/// Current icon of the tab in the context menu (for pre-fill)
context_menu_icon: Option<String>,
```

**Step 3: Initialize in TabBarUI::new()**

Add to the constructor:

```rust
picking_icon: false,
icon_buffer: String::new(),
context_menu_icon: None,
```

**Step 4: Capture current icon on right-click**

We need to pass the tab's effective icon to the right-click handler. The `render_tab_with_width` and `render_vertical_tab` functions already receive `profile_icon: Option<&str>` — but now we also need the `custom_icon`. The simplest approach: add a `custom_icon: Option<&str>` parameter to the two rendering functions and the 3 call sites.

In `render_tab_with_width` (around line 900) and `render_vertical_tab` (around line 548), add parameter `custom_icon: Option<&str>` right after `profile_icon: Option<&str>`.

At the two right-click handlers (lines ~788, ~1231), after `self.context_menu_title = title.to_string();`, add:

```rust
self.context_menu_icon = custom_icon.map(|s| s.to_string());
self.icon_buffer = custom_icon.unwrap_or("").to_string();
self.picking_icon = false;
```

Update all 3 call sites (lines 276, 319, 449) to pass the additional argument:

```rust
tab.custom_icon.as_deref(),
```

(This is the raw custom_icon, separate from the combined icon already passed as `profile_icon`.)

**Step 5: Add icon picker to context menu**

In `render_context_menu()`, after the "Rename Tab" section (line ~1578) and before "Duplicate Tab" (line ~1580), add the icon picker section:

```rust
// Tab Icon section
ui.add_space(4.0);
ui.separator();
ui.add_space(4.0);

if self.picking_icon {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label("Icon:");
        let response = ui.add(
            egui::TextEdit::singleline(&mut self.icon_buffer)
                .desired_width(60.0)
                .hint_text("Icon"),
        );
        if !response.has_focus() {
            response.request_focus();
        }
        // Nerd Font picker button
        let picker_label = if self.icon_buffer.is_empty() {
            "\u{ea7b}"
        } else {
            &self.icon_buffer
        };
        let picker_btn = ui.button(picker_label);
        egui::Popup::from_toggle_button_response(&picker_btn)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_min_width(280.0);
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for (category, icons) in
                            crate::settings_ui::nerd_font::NERD_FONT_PRESETS
                        {
                            ui.label(
                                egui::RichText::new(*category)
                                    .small()
                                    .strong(),
                            );
                            ui.horizontal_wrapped(|ui| {
                                for (icon, label) in *icons {
                                    let btn = ui.add_sized(
                                        [28.0, 28.0],
                                        egui::Button::new(
                                            egui::RichText::new(*icon).size(16.0),
                                        )
                                        .frame(false),
                                    );
                                    if btn.on_hover_text(*label).clicked() {
                                        self.icon_buffer = icon.to_string();
                                        egui::Popup::close_all(ui.ctx());
                                    }
                                }
                            });
                            ui.add_space(2.0);
                        }
                        ui.add_space(4.0);
                        if ui.button("Clear icon").clicked() {
                            self.icon_buffer.clear();
                            egui::Popup::close_all(ui.ctx());
                        }
                    });
            });
    });
    // Submit on Enter
    if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        let icon = self.icon_buffer.trim().to_string();
        action = TabBarAction::SetTabIcon(
            tab_id,
            if icon.is_empty() { None } else { Some(icon) },
        );
        self.picking_icon = false;
        close_menu = true;
    }
} else if menu_item(ui, "Set Icon") {
    self.picking_icon = true;
}

// Clear Icon (only show when tab has a custom icon)
if self.context_menu_icon.is_some() && !self.picking_icon {
    if menu_item(ui, "Clear Icon") {
        action = TabBarAction::SetTabIcon(tab_id, None);
        close_menu = true;
    }
}
```

**Step 6: Reset picking_icon when menu closes**

In the close menu block (around line 1672-1675), add `self.picking_icon = false;`:

```rust
if close_menu {
    self.context_menu_tab = None;
    self.renaming_tab = false;
    self.picking_icon = false;
}
```

**Step 7: Build to verify**

Run: `make build`
Expected: PASS (the action is constructed but not yet handled — that's Task 4)

**Step 8: Commit**

```bash
git add src/tab_bar_ui.rs
git commit -m "feat(tab-bar): add icon picker to tab context menu"
```

---

### Task 4: Handle `SetTabIcon` action in window_state

**Files:**
- Modify: `src/app/window_state.rs:3936` (after `TabBarAction::Duplicate` match arm)

**Step 1: Add match arm**

After the `TabBarAction::Duplicate` arm (around line 3942), add:

```rust
TabBarAction::SetTabIcon(id, icon) => {
    if let Some(tab) = self.tab_manager.get_tab_mut(id) {
        tab.custom_icon = icon.clone();
        log::info!(
            "Set custom icon for tab {}: {:?}",
            id,
            icon
        );
    }
    if let Some(window) = &self.window {
        window.request_redraw();
    }
}
```

**Step 2: Build to verify**

Run: `make build`
Expected: PASS

**Step 3: Commit**

```bash
git add src/app/window_state.rs
git commit -m "feat(tab): handle SetTabIcon action to update custom_icon"
```

---

### Task 5: Add persistence to SessionTab

**Files:**
- Modify: `src/session/mod.rs:37-51` (SessionTab struct)
- Modify: `src/session/capture.rs:34-44` (capture code)
- Modify: `src/app/window_manager.rs:880-889` (restore code)

**Step 1: Add field to SessionTab**

In `src/session/mod.rs`, after `user_title` (line 47), add:

```rust
/// Custom icon set by user (persists across sessions)
#[serde(default, skip_serializing_if = "Option::is_none")]
pub custom_icon: Option<String>,
```

**Step 2: Capture custom_icon**

In `src/session/capture.rs`, in the `SessionTab { ... }` construction (around line 34-44), add after `pane_layout,`:

```rust
custom_icon: tab.custom_icon.clone(),
```

**Step 3: Restore custom_icon**

In `src/app/window_manager.rs`, in the session restore block (around line 880-889), after the `custom_color` restore, add:

```rust
if let Some(ref icon) = session_tab.custom_icon {
    tab.custom_icon = Some(icon.clone());
}
```

**Step 4: Build to verify**

Run: `make build`
Expected: PASS

**Step 5: Commit**

```bash
git add src/session/mod.rs src/session/capture.rs src/app/window_manager.rs
git commit -m "feat(session): persist custom_icon in session save/restore"
```

---

### Task 6: Add persistence to TabSnapshot (arrangements)

**Files:**
- Modify: `par-term-settings-ui/src/arrangements.rs:36-52` (TabSnapshot struct)
- Modify: `src/arrangements/capture.rs:80-89` (capture code)
- Modify: `src/app/window_manager.rs:2847-2861` (arrangement restore code)

**Step 1: Add field to TabSnapshot**

In `par-term-settings-ui/src/arrangements.rs`, after `user_title` (line 51), add:

```rust
/// Custom icon set by user
#[serde(default, skip_serializing_if = "Option::is_none")]
pub custom_icon: Option<String>,
```

**Step 2: Capture custom_icon**

In `src/arrangements/capture.rs`, in the `TabSnapshot { ... }` construction (around line 80-89), add after `user_title`:

```rust
custom_icon: tab.custom_icon.clone(),
```

**Step 3: Restore custom_icon**

In `src/app/window_manager.rs`, in the arrangement restore block (around line 2850-2861), after `custom_color` restore, add:

```rust
if let Some(ref icon) = snapshot.custom_icon {
    tab.custom_icon = Some(icon.clone());
}
```

**Step 4: Update test_serialization test**

In `par-term-settings-ui/src/arrangements.rs`, update the `test_serialization` test's `TabSnapshot` construction (around line 330-333) to include the new fields:

```rust
TabSnapshot {
    cwd: Some("/home/user".to_string()),
    title: "bash".to_string(),
    custom_color: None,
    user_title: None,
    custom_icon: None,
}
```

**Step 5: Build and test**

Run: `make build && make test`
Expected: PASS

**Step 6: Commit**

```bash
git add par-term-settings-ui/src/arrangements.rs src/arrangements/capture.rs src/app/window_manager.rs
git commit -m "feat(arrangements): persist custom_icon in saved layouts"
```

---

### Task 7: Copy custom_icon on tab duplication

**Files:**
- Modify: `src/tab/manager.rs:454-467` (duplicate_tab_by_id)

**Step 1: Capture and copy custom_icon**

In `duplicate_tab_by_id()`, after line 455 (`let custom_color = self.tabs[source_idx].custom_color;`), add:

```rust
let custom_icon = self.tabs[source_idx].custom_icon.clone();
```

After the custom_color copy block (line 467), add:

```rust
// Copy custom icon from source
tab.custom_icon = custom_icon;
```

**Step 2: Build to verify**

Run: `make build`
Expected: PASS

**Step 3: Commit**

```bash
git add src/tab/manager.rs
git commit -m "feat(tab): copy custom_icon when duplicating tabs"
```

---

### Task 8: Final validation

**Step 1: Run full checks**

Run: `make checkall`
Expected: All format, lint, typecheck, and test checks pass.

**Step 2: Final commit (if any formatting/lint fixes needed)**

```bash
git add -A
git commit -m "style: fix formatting from tab icon feature"
```
