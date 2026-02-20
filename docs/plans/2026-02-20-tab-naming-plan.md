# Tab Naming & Title Mode Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let users control tab title auto-updates and manually rename tabs, with persistence across sessions.

**Architecture:** Add `TabTitleMode` enum to config (auto/osc_only), `user_named` bool to `Tab`, rename UI in the context menu, and persist both user titles and custom colors in session/arrangement save/restore.

**Tech Stack:** Rust, egui, serde, par-term-config crate

---

### Task 1: Add `TabTitleMode` enum and config field

**Files:**
- Modify: `par-term-config/src/types.rs` (near line 356, after `TabBarMode`)
- Modify: `par-term-config/src/config.rs` (near line 1097, after `tab_bar_mode`)
- Modify: `par-term-config/src/config.rs` (Default impl, near line 2007)

**Step 1: Add enum to types.rs**

Add after the `TabBarMode` enum (after line 364):

```rust
/// Controls how tab titles are automatically updated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TabTitleMode {
    /// OSC title first, then CWD from shell integration, then keep default
    #[default]
    Auto,
    /// Only update from explicit OSC escape sequences; never auto-set from CWD
    OscOnly,
}
```

**Step 2: Add config field**

In `Config` struct, after `tab_bar_mode` (line 1097), add:

```rust
    /// Controls how tab titles are automatically updated (auto or osc_only)
    #[serde(default)]
    pub tab_title_mode: TabTitleMode,
```

**Step 3: Add to Default impl**

In the `Default` impl (near line 2007, after `tab_bar_mode`), add:

```rust
            tab_title_mode: TabTitleMode::default(),
```

**Step 4: Verify it compiles**

Run: `cargo check -p par-term-config`
Expected: success

**Step 5: Commit**

```bash
git add par-term-config/src/types.rs par-term-config/src/config.rs
git commit -m "feat(config): add TabTitleMode enum (auto/osc_only)"
```

---

### Task 2: Add `user_named` field to `Tab` and update `update_title()` logic

**Files:**
- Modify: `src/tab/mod.rs`

**Step 1: Add `user_named` field to `Tab` struct**

After line 322 (`pub has_default_title: bool,`), add:

```rust
    /// Whether the user has manually named this tab (makes title static)
    pub user_named: bool,
```

**Step 2: Initialize `user_named: false` in all constructors**

Add `user_named: false,` after each `has_default_title` init in:
- `Tab::new()` (around line 544)
- `Tab::new_from_profile()` — BUT set `user_named: true` when the profile has a `tab_name` set (around line 771). Specifically, after line 771 where it currently has `has_default_title: false`, add: `user_named: profile.tab_name.is_some(),`
- `Tab::new_stub()` (around line 1478)

**Step 3: Update `update_title()` to respect `user_named`**

Replace the `update_title()` method (lines 811-837) with:

```rust
    /// Update tab title from terminal OSC sequences
    pub fn update_title(&mut self, title_mode: par_term_config::types::TabTitleMode) {
        // User-named tabs are static — never auto-update
        if self.user_named {
            return;
        }
        if let Ok(term) = self.terminal.try_lock() {
            let osc_title = term.get_title();
            if !osc_title.is_empty() {
                self.title = osc_title;
                self.has_default_title = false;
            } else if title_mode == par_term_config::types::TabTitleMode::Auto {
                if let Some(cwd) = term.shell_integration_cwd() {
                    // Abbreviate home directory to ~
                    let abbreviated = if let Some(home) = dirs::home_dir() {
                        cwd.replace(&home.to_string_lossy().to_string(), "~")
                    } else {
                        cwd
                    };
                    // Use just the last component for brevity
                    if let Some(last) = abbreviated.rsplit('/').next() {
                        if !last.is_empty() {
                            self.title = last.to_string();
                        } else {
                            self.title = abbreviated;
                        }
                    } else {
                        self.title = abbreviated;
                    }
                    self.has_default_title = false;
                }
            }
            // Otherwise keep the existing title (e.g., "Tab N")
        }
    }
```

**Step 4: Update `update_all_titles()` in `src/tab/manager.rs`**

Change signature and body (lines 409-413):

```rust
    pub fn update_all_titles(&mut self, title_mode: par_term_config::types::TabTitleMode) {
        for tab in &mut self.tabs {
            tab.update_title(title_mode);
        }
    }
```

**Step 5: Update caller in `src/app/window_state.rs`**

At line 2082, change:

```rust
        self.tab_manager.update_all_titles(self.config.tab_title_mode);
```

**Step 6: Verify it compiles**

Run: `cargo check`
Expected: success

**Step 7: Commit**

```bash
git add src/tab/mod.rs src/tab/manager.rs src/app/window_state.rs
git commit -m "feat(tab): add user_named field and title_mode to update_title()"
```

---

### Task 3: Add rename UI to tab context menu

**Files:**
- Modify: `src/tab_bar_ui.rs`

**Step 1: Add `RenameTab` variant to `TabBarAction`**

After `Duplicate(TabId),` (line 50), add:

```rust
    /// Rename a specific tab
    RenameTab(TabId, String),
```

**Step 2: Add rename state fields to `TabBarUI` struct**

After `editing_color` (line 82), add:

```rust
    /// Whether the rename text field is active in the context menu
    renaming_tab: bool,
    /// Buffer for the rename text field
    rename_buffer: String,
```

**Step 3: Initialize new fields in `TabBarUI::new()`**

After `editing_color` init (line 105), add:

```rust
            renaming_tab: false,
            rename_buffer: String::new(),
```

**Step 4: Add "Rename Tab" item to context menu**

In `render_context_menu()` (around line 1524), add before "Duplicate Tab":

```rust
                        // Rename Tab
                        if self.renaming_tab {
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                let response = ui.add(
                                    egui::TextEdit::singleline(&mut self.rename_buffer)
                                        .desired_width(140.0)
                                        .hint_text("Tab name"),
                                );
                                // Auto-focus on first frame
                                if !response.has_focus() && response.gained_focus() == false {
                                    response.request_focus();
                                }
                                // Submit on Enter
                                if response.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    let name = self.rename_buffer.trim().to_string();
                                    action = TabBarAction::RenameTab(tab_id, name);
                                    self.renaming_tab = false;
                                    close_menu = true;
                                }
                                // Cancel on Escape
                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    self.renaming_tab = false;
                                    close_menu = true;
                                }
                            });
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("Leave blank to use auto title")
                                        .weak()
                                        .small(),
                                );
                            });
                        } else if menu_item(ui, "Rename Tab") {
                            // Pre-fill with current title — caller passes it via tab info
                            self.renaming_tab = true;
                            // rename_buffer will be set by the caller before rendering
                        }

                        ui.add_space(2.0);
```

**Step 5: Pass current title to pre-fill rename buffer**

In `render_context_menu()`, at the top of the function (after `let mut close_menu = false;`), the rename buffer needs to be pre-filled when entering rename mode. We need to accept the current tab title. Change the method signature:

```rust
    fn render_context_menu(
        &mut self,
        ctx: &egui::Context,
        tab_id: TabId,
        current_title: &str,
    ) -> TabBarAction {
```

When `renaming_tab` becomes true (the `else if menu_item` branch), set:
```rust
                            self.rename_buffer = current_title.to_string();
```

**Step 6: Update all callers of `render_context_menu`**

Search for calls to `self.render_context_menu(ctx, context_tab_id)` and add the `current_title` parameter. The callers need access to tab titles — the render methods already receive tab info. Update the two call sites (around lines 381 and 513) to pass the tab title. The tab title comes from the tab data passed to the render methods.

Update the horizontal and vertical render methods to track the context menu tab's title when opening the menu, and pass it through. Add a field:

```rust
    /// Title of the tab in the context menu (for rename pre-fill)
    context_menu_title: String,
```

Initialize with `String::new()`. Set it when opening the context menu (where `context_menu_tab` is set). Pass `&self.context_menu_title` to `render_context_menu`.

**Step 7: Reset rename state when context menu closes**

In the `if close_menu` block (line 1609), add:

```rust
            self.renaming_tab = false;
```

**Step 8: Verify it compiles**

Run: `cargo check`
Expected: success

**Step 9: Commit**

```bash
git add src/tab_bar_ui.rs
git commit -m "feat(tab-bar): add rename tab UI to context menu"
```

---

### Task 4: Handle `RenameTab` action in window state

**Files:**
- Modify: `src/app/window_state.rs`

**Step 1: Add handler for `RenameTab`**

In the `match` on `TabBarAction` (around line 3908, after `Duplicate`), add:

```rust
            TabBarAction::RenameTab(id, name) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(id) {
                    if name.is_empty() {
                        // Blank name: revert to auto title mode
                        tab.user_named = false;
                        tab.has_default_title = true;
                        // Trigger immediate title update
                        tab.update_title(self.config.tab_title_mode);
                    } else {
                        tab.title = name;
                        tab.user_named = true;
                        tab.has_default_title = false;
                    }
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: success

**Step 3: Commit**

```bash
git add src/app/window_state.rs
git commit -m "feat(tab): handle RenameTab action with blank-reverts-to-auto"
```

---

### Task 5: Persist user titles and custom colors in session save/restore

**Files:**
- Modify: `src/session/mod.rs`
- Modify: `src/session/capture.rs`
- Modify: `src/app/window_manager.rs` (restore path)

**Step 1: Add fields to `SessionTab`**

In `SessionTab` (lines 37-45), add after `title`:

```rust
    /// Custom tab color (only saved when user set a color)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_color: Option<[u8; 3]>,

    /// User-set tab title (present only when user manually named the tab)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_title: Option<String>,
```

**Step 2: Capture user title and custom color**

In `capture.rs`, update the `SessionTab` construction (around line 34):

```rust
                SessionTab {
                    cwd: tab.get_cwd(),
                    title: tab.title.clone(),
                    custom_color: tab.custom_color,
                    user_title: if tab.user_named {
                        Some(tab.title.clone())
                    } else {
                        None
                    },
                    pane_layout,
                }
```

**Step 3: Restore user titles and custom colors**

In `window_manager.rs` `restore_session()`, after the pane layout restore loop (around line 877), add a new loop:

```rust
                // Restore user titles and custom colors
                for (tab_idx, session_tab) in session_window.tabs.iter().enumerate() {
                    if let Some(tab) = tabs.get_mut(tab_idx) {
                        if let Some(ref user_title) = session_tab.user_title {
                            tab.title = user_title.clone();
                            tab.user_named = true;
                            tab.has_default_title = false;
                        }
                        if let Some(color) = session_tab.custom_color {
                            tab.set_custom_color(color);
                        }
                    }
                }
```

Note: the `tabs` mutable reference is already available from the existing loop at line 870.

**Step 4: Update session storage tests**

In `src/session/storage.rs`, update test `SessionTab` constructions to include the new fields (set to `None`).

**Step 5: Verify it compiles and tests pass**

Run: `cargo check && cargo test`
Expected: success

**Step 6: Commit**

```bash
git add src/session/mod.rs src/session/capture.rs src/app/window_manager.rs src/session/storage.rs
git commit -m "feat(session): persist user tab titles and custom colors"
```

---

### Task 6: Persist in arrangements (TabSnapshot)

**Files:**
- Modify: `par-term-settings-ui/src/arrangements.rs`
- Modify: `src/arrangements/capture.rs`
- Modify: `src/app/window_manager.rs` (arrangement restore path)

**Step 1: Add fields to `TabSnapshot`**

In `TabSnapshot` (around line 36), add after `title`:

```rust
    /// Custom tab color
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_color: Option<[u8; 3]>,

    /// User-set tab title
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_title: Option<String>,
```

**Step 2: Capture in arrangements**

In `src/arrangements/capture.rs`, update `TabSnapshot` construction (around line 80):

```rust
            .map(|tab| TabSnapshot {
                cwd: tab.get_cwd(),
                title: tab.title.clone(),
                custom_color: tab.custom_color,
                user_title: if tab.user_named {
                    Some(tab.title.clone())
                } else {
                    None
                },
            })
```

**Step 3: Restore from arrangements**

In `window_manager.rs` `restore_arrangement()` (around line 2830), after `create_window_with_overrides`, add a restore loop similar to session restore. Find the window just created and apply user titles and colors:

```rust
            // Restore user titles and custom colors from arrangement
            if let Some((_window_id, window_state)) = self.windows.iter_mut().last() {
                let tabs = window_state.tab_manager.tabs_mut();
                for (tab_idx, snapshot) in window_snapshot.tabs.iter().enumerate() {
                    if let Some(tab) = tabs.get_mut(tab_idx) {
                        if let Some(ref user_title) = snapshot.user_title {
                            tab.title = user_title.clone();
                            tab.user_named = true;
                            tab.has_default_title = false;
                        }
                        if let Some(color) = snapshot.custom_color {
                            tab.set_custom_color(color);
                        }
                    }
                }
            }
```

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: success

**Step 5: Commit**

```bash
git add par-term-settings-ui/src/arrangements.rs src/arrangements/capture.rs src/app/window_manager.rs
git commit -m "feat(arrangements): persist user tab titles and custom colors"
```

---

### Task 7: Add settings UI dropdown for `tab_title_mode`

**Files:**
- Modify: `par-term-settings-ui/src/window_tab.rs`
- Modify: `par-term-settings-ui/src/sidebar.rs`

**Step 1: Add dropdown to Window > Tab Bar section**

In `window_tab.rs`, after the "Show tab bar" dropdown (after line 965), add:

```rust
        ui.horizontal(|ui| {
            ui.label("Tab title mode:");
            let current = match settings.config.tab_title_mode {
                TabTitleMode::Auto => 0,
                TabTitleMode::OscOnly => 1,
            };
            let mut selected = current;
            egui::ComboBox::from_id_salt("window_tab_title_mode")
                .selected_text(match current {
                    0 => "Auto (OSC + CWD)",
                    1 => "OSC only",
                    _ => "Unknown",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, 0, "Auto (OSC + CWD)")
                        .on_hover_text("Use OSC title, fall back to working directory");
                    ui.selectable_value(&mut selected, 1, "OSC only")
                        .on_hover_text("Only use titles set by OSC escape sequences");
                });
            if selected != current {
                settings.config.tab_title_mode = match selected {
                    0 => TabTitleMode::Auto,
                    1 => TabTitleMode::OscOnly,
                    _ => TabTitleMode::Auto,
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
```

Add `use par_term_config::types::TabTitleMode;` to the imports at the top of the file if not already present.

**Step 2: Add search keywords**

In `sidebar.rs`, in the `SettingsTab::Window` keywords (around line 297-334), add in the "Tab bar" section:

```rust
            "tab title mode",
            "tab title",
            "osc only",
            "cwd title",
            "rename tab",
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: success

**Step 4: Commit**

```bash
git add par-term-settings-ui/src/window_tab.rs par-term-settings-ui/src/sidebar.rs
git commit -m "feat(settings): add tab title mode dropdown to Window tab"
```

---

### Task 8: Update documentation

**Files:**
- Modify: `docs/TABS.md`

**Step 1: Add Tab Title Mode section**

After the "Tab Bar" section and before "Tab Appearance", add a new section:

```markdown
## Tab Title Mode

Control how tab titles are automatically updated:

| Mode | Description |
|------|-------------|
| `auto` | OSC title first, then working directory from shell integration, then keep default "Tab N" (default) |
| `osc_only` | Only update from explicit OSC escape sequences; never auto-set from CWD |

```yaml
tab_title_mode: auto
```

**Settings UI:** Settings > Window > Tab Bar > "Tab title mode"

### Renaming Tabs

Right-click any tab and select **Rename Tab** to set a custom name. Manually named tabs are static — they are never auto-updated regardless of the title mode setting.

To revert a renamed tab to automatic title updates, right-click and rename with a blank name.

**Session persistence:** User-set tab names and custom colors are preserved across session save/restore and in window arrangements.
```

**Step 2: Add `tab_title_mode` to the Configuration reference**

In the config YAML block at the bottom, add after `tab_bar_mode`:

```yaml
# Tab title mode: "auto", "osc_only"
tab_title_mode: auto
```

**Step 3: Commit**

```bash
git add docs/TABS.md
git commit -m "docs: add tab title mode and rename tab documentation"
```

---

### Task 9: Full build and manual test

**Step 1: Run full checks**

Run: `make all`
Expected: format, lint, test, and build all pass

**Step 2: Manual smoke test**

Run: `make run`

Test:
1. Right-click a tab → "Rename Tab" appears in context menu
2. Enter a name → tab title changes and stays static
3. Rename with blank text → tab reverts to auto behavior
4. Settings > Window > Tab Bar > "Tab title mode" → dropdown with Auto/OSC only
5. Switch to "OSC only" → CWD no longer updates tab titles
6. Close and reopen → user-named tabs and custom colors are restored

**Step 3: Final commit if any fixes needed**

```bash
git add -A
git commit -m "fix: address issues from manual testing"
```
