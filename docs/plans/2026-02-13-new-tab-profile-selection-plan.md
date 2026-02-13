# New Tab Profile Selection Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a split "+" button to the tab bar so users can quickly create a default tab (left click) or pick a profile from a dropdown (click chevron).

**Architecture:** The `+` button becomes two buttons side-by-side: `+` (default tab) and `▾` (profile dropdown). The dropdown is an egui popup showing "Default" + all profiles. A new config option `new_tab_shortcut_shows_profiles` controls whether Cmd+T also triggers the dropdown. The `TabBarUI.render()` signature gains a `&ProfileManager` parameter so it can list profiles.

**Tech Stack:** Rust, egui (popup/combo), existing ProfileManager

---

### Task 1: Add config option `new_tab_shortcut_shows_profiles`

**Files:**
- Modify: `src/config/mod.rs:1055` (after `show_profile_drawer_button`)
- Modify: `src/config/mod.rs` (Default impl, ~line 1771)

**Step 1: Add the field to Config struct**

In `src/config/mod.rs`, after the `show_profile_drawer_button` field (~line 1055), add:

```rust
    /// When true, the new-tab keyboard shortcut (Cmd+T / Ctrl+Shift+T) shows the
    /// profile selection dropdown instead of immediately opening a default tab
    #[serde(default = "defaults::bool_false")]
    pub new_tab_shortcut_shows_profiles: bool,
```

**Step 2: Add to Default impl**

In the `Default` impl for `Config` (~line 1771, after `show_profile_drawer_button`), add:

```rust
            new_tab_shortcut_shows_profiles: defaults::bool_false(),
```

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles (possibly with warnings about unused field)

**Step 4: Commit**

```bash
git add src/config/mod.rs
git commit -m "feat(config): add new_tab_shortcut_shows_profiles option"
```

---

### Task 2: Add new TabBarAction variants

**Files:**
- Modify: `src/tab_bar_ui.rs:28-43` (TabBarAction enum)

**Step 1: Add the new variants**

Add two new variants to the `TabBarAction` enum in `src/tab_bar_ui.rs`:

```rust
pub enum TabBarAction {
    /// No action
    None,
    /// Switch to a specific tab
    SwitchTo(TabId),
    /// Close a specific tab
    Close(TabId),
    /// Create a new tab
    NewTab,
    /// Create a new tab from a specific profile
    NewTabWithProfile(crate::profile::ProfileId),
    /// Show the new-tab profile selection menu (triggered by keyboard shortcut)
    ShowNewTabProfileMenu,
    /// Reorder a tab to a new position
    Reorder(TabId, usize),
    /// Set custom color for a tab
    SetColor(TabId, [u8; 3]),
    /// Clear custom color for a tab (revert to default)
    ClearColor(TabId),
}
```

**Step 2: Verify it compiles**

Run: `cargo build 2>&1 | head -30`
Expected: Compile errors in `window_state.rs` match arm — that's expected, we'll fix in Task 5.

**Step 3: Commit**

```bash
git add src/tab_bar_ui.rs
git commit -m "feat(tab_bar): add NewTabWithProfile and ShowNewTabProfileMenu actions"
```

---

### Task 3: Add profile dropdown state and UI to TabBarUI

**Files:**
- Modify: `src/tab_bar_ui.rs` (TabBarUI struct, new(), render_horizontal, render_vertical)

**Step 1: Add state fields to TabBarUI struct**

Add to the `TabBarUI` struct (after `scroll_offset: f32`):

```rust
    /// Whether the new-tab profile popup is open
    pub show_new_tab_profile_menu: bool,
```

And initialize in `new()`:

```rust
            show_new_tab_profile_menu: false,
```

**Step 2: Update `render()` signature to accept profiles**

Change the `render()` method signature from:

```rust
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        tabs: &TabManager,
        config: &Config,
    ) -> TabBarAction {
```

to:

```rust
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        tabs: &TabManager,
        config: &Config,
        profiles: &crate::profile::ProfileManager,
    ) -> TabBarAction {
```

Pass `profiles` through to `render_horizontal` and `render_vertical` (update their signatures too).

**Step 3: Replace the horizontal "+" button with a split button**

In `render_horizontal()`, replace the new tab button block (lines ~300-318) with a split button:

```rust
                // New tab split button: [+] [▾]
                ui.add_space(tab_spacing);

                // "+" button — creates default tab
                let plus_btn = ui.add(
                    egui::Button::new("+")
                        .min_size(egui::vec2(new_tab_btn_width - 14.0, config.tab_bar_height - 4.0))
                        .fill(egui::Color32::TRANSPARENT),
                );
                if plus_btn.clicked_by(egui::PointerButton::Primary) {
                    action = TabBarAction::NewTab;
                }
                if plus_btn.hovered() {
                    #[cfg(target_os = "macos")]
                    plus_btn.on_hover_text("New Tab (Cmd+T)");
                    #[cfg(not(target_os = "macos"))]
                    plus_btn.on_hover_text("New Tab (Ctrl+Shift+T)");
                }

                // "▾" chevron — opens profile dropdown
                let chevron_btn = ui.add(
                    egui::Button::new("▾")
                        .min_size(egui::vec2(14.0, config.tab_bar_height - 4.0))
                        .fill(egui::Color32::TRANSPARENT),
                );
                if chevron_btn.clicked_by(egui::PointerButton::Primary) {
                    self.show_new_tab_profile_menu = !self.show_new_tab_profile_menu;
                }
                if chevron_btn.hovered() {
                    chevron_btn.on_hover_text("New tab from profile");
                }
```

**Step 4: Do the same for `render_vertical()`**

Replace the vertical "+" button block (~lines 401-416) with the same split button pattern, using `ui.available_width() - 14.0` for the plus button width and `14.0` for the chevron, inside a `ui.horizontal()`.

**Step 5: Add the profile popup rendering**

Add a new method to `TabBarUI` that renders the popup. Call it at the end of both `render_horizontal` and `render_vertical`, after the context menu block, before returning `action`:

```rust
    /// Render the new-tab profile selection popup
    fn render_new_tab_profile_menu(
        &mut self,
        ctx: &egui::Context,
        profiles: &crate::profile::ProfileManager,
    ) -> TabBarAction {
        let mut action = TabBarAction::None;

        if !self.show_new_tab_profile_menu {
            return action;
        }

        let mut open = true;
        egui::Window::new("New Tab")
            .collapsible(false)
            .resizable(false)
            .fixed_size(egui::vec2(200.0, 0.0))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut open)
            .show(ctx, |ui| {
                // "Default" entry — always first
                if ui
                    .selectable_label(false, "  Default")
                    .on_hover_text("Open a new tab with default settings")
                    .clicked()
                {
                    action = TabBarAction::NewTab;
                    self.show_new_tab_profile_menu = false;
                }
                ui.separator();

                // Profile entries in display order
                for profile in profiles.profiles_ordered() {
                    let icon = profile.icon.as_deref().unwrap_or("  ");
                    let label = format!("{} {}", icon, profile.name);
                    if ui.selectable_label(false, &label).clicked() {
                        action = TabBarAction::NewTabWithProfile(profile.id);
                        self.show_new_tab_profile_menu = false;
                    }
                }
            });

        if !open {
            self.show_new_tab_profile_menu = false;
        }

        action
    }
```

Call at end of `render_horizontal` and `render_vertical`:

```rust
        // Render new-tab profile menu if open
        let menu_action = self.render_new_tab_profile_menu(ctx, profiles);
        if menu_action != TabBarAction::None {
            action = menu_action;
        }
```

**Step 6: Verify it compiles**

Run: `cargo build 2>&1 | head -30`
Expected: Compile error in `window_state.rs` at the `tab_bar_ui.render()` call site (missing `profiles` arg) and in the match arm — expected, fixed in Tasks 4-5.

**Step 7: Commit**

```bash
git add src/tab_bar_ui.rs
git commit -m "feat(tab_bar): split button UI with profile dropdown"
```

---

### Task 4: Update render call site to pass ProfileManager

**Files:**
- Modify: `src/app/window_state.rs:2204-2205`

**Step 1: Pass profile_manager to tab_bar_ui.render()**

Change line ~2205 from:

```rust
                    pending_tab_action =
                        self.tab_bar_ui.render(ctx, &self.tab_manager, &self.config);
```

to:

```rust
                    pending_tab_action =
                        self.tab_bar_ui.render(ctx, &self.tab_manager, &self.config, &self.profile_manager);
```

**Step 2: Verify it compiles**

Run: `cargo build 2>&1 | head -30`
Expected: Only remaining error is the match arm for new variants — fixed in Task 5.

**Step 3: Commit**

```bash
git add src/app/window_state.rs
git commit -m "feat(tab_bar): pass profile manager to tab bar render"
```

---

### Task 5: Handle new TabBarAction variants in window_state.rs

**Files:**
- Modify: `src/app/window_state.rs:2673-2739` (the pending_tab_action match)

**Step 1: Add match arms for the new variants**

After the `TabBarAction::Reorder` arm (~line 2738), before `TabBarAction::None`, add:

```rust
            TabBarAction::NewTabWithProfile(profile_id) => {
                self.open_profile(profile_id);
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            TabBarAction::ShowNewTabProfileMenu => {
                self.tab_bar_ui.show_new_tab_profile_menu = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
```

**Step 2: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Clean compile (possibly warnings).

**Step 3: Commit**

```bash
git add src/app/window_state.rs
git commit -m "feat(tab_bar): handle profile selection actions"
```

---

### Task 6: Wire keyboard shortcut to respect config

**Files:**
- Modify: `src/app/input_events.rs:986-1002`

**Step 1: Update the new-tab shortcut handler**

Replace the block at ~line 998-1002:

```rust
        if is_new_tab {
            self.new_tab();
            log::info!("New tab created");
            return true;
        }
```

with:

```rust
        if is_new_tab {
            if self.config.new_tab_shortcut_shows_profiles
                && !self.profile_manager.is_empty()
            {
                self.tab_bar_ui.show_new_tab_profile_menu =
                    !self.tab_bar_ui.show_new_tab_profile_menu;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!("Toggled new-tab profile menu via shortcut");
            } else {
                self.new_tab();
                log::info!("New tab created");
            }
            return true;
        }
```

Also update the `"new_tab"` keybinding action handler (~line 1246) with the same conditional logic.

**Step 2: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Clean compile.

**Step 3: Commit**

```bash
git add src/app/input_events.rs
git commit -m "feat(input): new-tab shortcut respects profile menu config"
```

---

### Task 7: Add settings UI for the new option

**Files:**
- Modify: `src/settings_ui/window_tab.rs:995` (after `show_profile_drawer_button` checkbox)
- Modify: `src/settings_ui/sidebar.rs` (Window tab keywords)

**Step 1: Add checkbox to Window settings tab**

In `src/settings_ui/window_tab.rs`, after the `show_profile_drawer_button` checkbox block (after line ~995), add:

```rust
        if ui
            .checkbox(
                &mut settings.config.new_tab_shortcut_shows_profiles,
                "New tab shortcut shows profile picker",
            )
            .on_hover_text(
                "When enabled, the new tab keyboard shortcut (Cmd+T / Ctrl+Shift+T) shows a profile selection dropdown instead of immediately creating a default tab",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
```

**Step 2: Add search keywords**

In `src/settings_ui/sidebar.rs`, in the `SettingsTab::Window` keywords array, add these keywords (in the tab bar section):

```rust
            "new tab shortcut",
            "profile picker",
            "new tab profile",
```

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Clean compile.

**Step 4: Commit**

```bash
git add src/settings_ui/window_tab.rs src/settings_ui/sidebar.rs
git commit -m "feat(settings): add new tab profile picker toggle"
```

---

### Task 8: Final checks and cleanup

**Step 1: Run full quality checks**

Run: `make fmt && make lint && make test`
Expected: All pass.

**Step 2: Build release**

Run: `make release`
Expected: Clean release build.

**Step 3: Commit any formatting changes**

```bash
git add -A
git commit -m "style: format code"
```

---

### Task 9: Create PR

Run:

```bash
gh pr create --title "feat: profile selection when creating new tab (#129)" --body "$(cat <<'EOF'
## Summary

- Adds a split button to the tab bar: `+` (default tab) and `▾` (profile picker dropdown)
- Dropdown shows "Default" at top, then all profiles in display order with icons
- Works in both horizontal and vertical tab bar layouts
- New config option `new_tab_shortcut_shows_profiles` controls whether Cmd+T / Ctrl+Shift+T also shows the profile picker

Closes #129

## Test plan

- [ ] Click `+` button — creates default tab (unchanged behavior)
- [ ] Click `▾` chevron — dropdown appears with "Default" and all profiles
- [ ] Click "Default" in dropdown — creates default tab
- [ ] Click a profile in dropdown — creates tab with that profile's config
- [ ] Click outside dropdown — dismisses it
- [ ] Test in horizontal tab bar layout (top/bottom)
- [ ] Test in vertical tab bar layout (left)
- [ ] Enable `new_tab_shortcut_shows_profiles` in settings, press Cmd+T — dropdown appears
- [ ] Disable `new_tab_shortcut_shows_profiles`, press Cmd+T — default tab created immediately
- [ ] Settings checkbox appears in Window > Tab Behavior section
- [ ] Search "profile picker" in settings — Window tab highlighted
EOF
)"
```
