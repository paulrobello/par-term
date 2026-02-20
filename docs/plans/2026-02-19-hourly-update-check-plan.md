# Hourly Update Check + Status Bar Widget — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add an `Hourly` update check frequency and a clickable status bar widget that shows when an update is available, opening a dedicated update dialog.

**Architecture:** Extend existing `UpdateCheckFrequency` enum with `Hourly` variant. Add `UpdateAvailable` to `WidgetId` enum. Thread update state from `WindowManager` through `StatusBarUI` into the widget context. Add an egui update dialog overlay rendered in `window_state.rs`.

**Tech Stack:** Rust, egui, par-term-config, par-term-update, par-term-settings-ui

---

### Task 1: Add `Hourly` variant to `UpdateCheckFrequency`

**Files:**
- Modify: `par-term-config/src/types.rs:862-896`

**Step 1: Add the variant**

In the `UpdateCheckFrequency` enum, add `Hourly` between `Never` and `Daily`:

```rust
pub enum UpdateCheckFrequency {
    Never,
    Hourly,
    #[default]
    Daily,
    Weekly,
    Monthly,
}
```

**Step 2: Update `as_seconds()`**

Add `Hourly` arm returning `Some(3600)`:

```rust
UpdateCheckFrequency::Hourly => Some(3600),
```

**Step 3: Update `display_name()`**

Add arm:

```rust
UpdateCheckFrequency::Hourly => "Hourly",
```

**Step 4: Update test in `par-term-update/src/update_checker.rs`**

Add to `test_update_check_frequency_seconds`:

```rust
assert_eq!(UpdateCheckFrequency::Hourly.as_seconds(), Some(3600));
```

**Step 5: Run tests**

Run: `cargo test -p par-term-update`
Expected: All tests pass

**Step 6: Update Settings UI dropdown**

In `par-term-settings-ui/src/advanced_tab.rs:877-882`, add `Hourly` to the frequency list:

```rust
for freq in [
    UpdateCheckFrequency::Never,
    UpdateCheckFrequency::Hourly,
    UpdateCheckFrequency::Daily,
    UpdateCheckFrequency::Weekly,
    UpdateCheckFrequency::Monthly,
] {
```

**Step 7: Add search keywords**

In `par-term-settings-ui/src/sidebar.rs`, find the `Advanced` tab's keywords and add `"hourly"`.

**Step 8: Commit**

```bash
git add -A && git commit -m "feat(config): add Hourly update check frequency"
```

---

### Task 2: Add `UpdateAvailable` widget to status bar config

**Files:**
- Modify: `par-term-config/src/status_bar.rs:23-87` (WidgetId enum)
- Modify: `par-term-config/src/status_bar.rs:117-183` (default_widgets)

**Step 1: Add `UpdateAvailable` variant to `WidgetId`**

Add after `CurrentCommand` and before `Custom`:

```rust
/// Update available notification
UpdateAvailable,
```

**Step 2: Update `WidgetId::label()`**

Add arm:

```rust
WidgetId::UpdateAvailable => "Update Available",
```

**Step 3: Update `WidgetId::icon()`**

Add arm:

```rust
WidgetId::UpdateAvailable => "\u{2b06}",  // upwards arrow
```

**Step 4: Update `WidgetId::needs_system_monitor()`**

No change needed — `UpdateAvailable` doesn't need system monitor.

**Step 5: Add to `default_widgets()`**

Add at the end of the vec, before the closing `]`, with order 5 (after Clock at order 4):

```rust
StatusBarWidgetConfig {
    id: WidgetId::UpdateAvailable,
    enabled: true,
    section: StatusBarSection::Right,
    order: 5,
    format: None,
},
```

**Step 6: Run tests**

Run: `cargo test -p par-term`
Expected: Passes (status bar tests use their own widget configs)

**Step 7: Commit**

```bash
git add -A && git commit -m "feat(status-bar): add UpdateAvailable widget ID and default config"
```

---

### Task 3: Thread update result into widget context and render widget text

**Files:**
- Modify: `src/status_bar/widgets.rs:12-30` (WidgetContext)
- Modify: `src/status_bar/widgets.rs:36-93` (widget_text)
- Modify: `src/status_bar/mod.rs:292-475` (render method)

**Step 1: Add update version field to `WidgetContext`**

In `WidgetContext` struct, add:

```rust
/// Available update version string (e.g., "0.20.0"), None if up-to-date
pub update_available_version: Option<String>,
```

**Step 2: Handle `UpdateAvailable` in `widget_text()`**

Add arm in the `match id` block:

```rust
WidgetId::UpdateAvailable => {
    if let Some(ref version) = ctx.update_available_version {
        format!("\u{2b06} v{}", version)
    } else {
        String::new()
    }
}
```

This produces empty string when no update available, so the widget auto-hides (existing status bar rendering skips empty widgets).

**Step 3: Wire up update version in `StatusBarUI`**

In `src/status_bar/mod.rs`, add a public field to `StatusBarUI`:

```rust
/// Available update version (set by WindowManager when update is detected)
pub update_available_version: Option<String>,
```

Initialize it as `None` in `StatusBarUI::new()`.

In the `render()` method, when building `WidgetContext`, pass it through:

```rust
let widget_ctx = WidgetContext {
    // ... existing fields ...
    update_available_version: self.update_available_version.clone(),
};
```

**Step 4: Update WidgetContext construction in tests**

In `src/status_bar/widgets.rs` test helper `make_ctx()`, add:

```rust
update_available_version: None,
```

**Step 5: Run tests**

Run: `cargo test -p par-term`
Expected: All pass

**Step 6: Commit**

```bash
git add -A && git commit -m "feat(status-bar): render UpdateAvailable widget text from context"
```

---

### Task 4: Propagate update result from WindowManager to StatusBarUI

**Files:**
- Modify: `src/app/window_manager.rs` (after update check stores result)
- Modify: `src/app/window_state.rs` (sync update state to status bar)

**Step 1: Add helper to extract version from `UpdateCheckResult`**

In `src/app/window_manager.rs`, after the existing `to_settings_update_result` function, add:

```rust
/// Extract the available version string from an update result (None if not available).
fn update_available_version(result: &UpdateCheckResult) -> Option<String> {
    match result {
        UpdateCheckResult::UpdateAvailable(info) => {
            Some(info.version.strip_prefix('v').unwrap_or(&info.version).to_string())
        }
        _ => None,
    }
}
```

**Step 2: Sync update result to all window states**

After `self.last_update_result = Some(result);` in `check_for_updates()`, add:

```rust
// Sync update version to status bar widgets
let version = self.last_update_result.as_ref().and_then(update_available_version);
for ws in self.windows.values_mut() {
    ws.status_bar_ui.update_available_version = version.clone();
}
```

Do the same in `force_update_check()` after `self.last_update_result = Some(result);`.

**Step 3: Also sync on window creation**

In the window creation code, after a new `WindowState` is created, sync the existing update result:

```rust
ws.status_bar_ui.update_available_version = self.last_update_result.as_ref().and_then(update_available_version);
```

**Step 4: Build check**

Run: `cargo build`
Expected: Compiles successfully

**Step 5: Commit**

```bash
git add -A && git commit -m "feat(status-bar): propagate update version from WindowManager to StatusBarUI"
```

---

### Task 5: Make the update widget clickable and show update dialog

**Files:**
- Modify: `src/status_bar/mod.rs` (make widget interactive, return click action)
- Modify: `src/app/window_state.rs` (handle click, render dialog)

**Step 1: Change status bar render to return an action**

Currently `StatusBarUI::render()` returns `f32` (height). We need to also signal when the update widget is clicked.

Add a return type enum or use `Option<StatusBarAction>`:

In `src/status_bar/mod.rs`, add:

```rust
/// Actions that the status bar can request from the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusBarAction {
    /// User clicked the update-available widget.
    ShowUpdateDialog,
}
```

Change `render()` return type from `f32` to `(f32, Option<StatusBarAction>)`.

**Step 2: Make the update widget label clickable**

In the render method, when rendering right-section widgets, detect `WidgetId::UpdateAvailable` and render it as a clickable label instead of a plain label. Use `ui.add(egui::Label::new(rich_text).sense(egui::Sense::click()))`.

When clicked, set the action to `Some(StatusBarAction::ShowUpdateDialog)`.

For this widget, also use a highlight color (e.g., yellow/orange) to draw attention.

**Step 3: Update all callers of `render()`**

In `src/app/window_state.rs:3192`, update to capture the action:

```rust
let (_bar_height, status_bar_action) = self.status_bar_ui.render(
    ctx,
    &self.config,
    session_vars,
    self.is_fullscreen,
);
```

**Step 4: Add `show_update_dialog` flag to WindowState**

In `src/app/window_state.rs`, add field:

```rust
pub(crate) show_update_dialog: bool,
```

Initialize as `false`.

When `status_bar_action == Some(StatusBarAction::ShowUpdateDialog)`, set `self.show_update_dialog = true`.

**Step 5: Build check**

Run: `cargo build`
Expected: Compiles (dialog rendering comes next task)

**Step 6: Commit**

```bash
git add -A && git commit -m "feat(status-bar): make update widget clickable with StatusBarAction"
```

---

### Task 6: Render the update dialog overlay

**Files:**
- Create: `src/update_dialog.rs`
- Modify: `src/app/window_state.rs` (call dialog render)
- Modify: `src/main.rs` or `src/lib.rs` (add `mod update_dialog;`)

**Step 1: Create the update dialog module**

Create `src/update_dialog.rs` with a function that renders an egui modal window:

```rust
use crate::update_checker::{UpdateCheckResult, UpdateInfo};
use par_term_config::Config;

/// Result of rendering the update dialog.
pub enum UpdateDialogAction {
    /// User dismissed the dialog (close it).
    Dismiss,
    /// User wants to skip this version.
    SkipVersion(String),
    /// User wants to install the update.
    InstallUpdate(String),
    /// Dialog is still open, no action taken.
    None,
}

/// Render the update dialog.
///
/// Returns the action taken by the user, if any.
pub fn render_update_dialog(
    ctx: &egui::Context,
    update_result: &UpdateCheckResult,
    config: &Config,
    current_version: &str,
    installation_type: par_term_settings_ui::InstallationType,
) -> UpdateDialogAction {
    // ... egui::Window with modal overlay
}
```

The dialog should:
- Be an `egui::Window` with a title like "Update Available"
- Show current version vs available version
- Show release notes if available (in a scrollable area)
- Show a clickable release URL link
- Show "Install Update" button for standalone/bundle, or command text for Homebrew/Cargo
- Show "Skip Version" and "Dismiss" buttons
- Return the appropriate action

**Step 2: Register the module**

Add `mod update_dialog;` in the appropriate place (look at existing module declarations).

**Step 3: Call from window_state render**

After the status bar render in `window_state.rs`, add:

```rust
if self.show_update_dialog {
    if let Some(ref update_result) = /* get update result */ {
        let action = crate::update_dialog::render_update_dialog(
            ctx,
            update_result,
            &self.config,
            env!("CARGO_PKG_VERSION"),
            /* installation_type */,
        );
        match action {
            UpdateDialogAction::Dismiss => self.show_update_dialog = false,
            UpdateDialogAction::SkipVersion(v) => {
                self.config.skipped_version = Some(v);
                self.show_update_dialog = false;
                // trigger config save
            }
            UpdateDialogAction::InstallUpdate(v) => {
                // trigger install (reuse existing logic)
                self.show_update_dialog = false;
            }
            UpdateDialogAction::None => {}
        }
    }
}
```

**Step 4: Store update result on WindowState**

Add `pub(crate) last_update_result: Option<UpdateCheckResult>` to WindowState.
Sync it from WindowManager alongside the version string (in Task 4 code).

Also store `installation_type` on WindowState — either detect once at startup or pass from WindowManager.

**Step 5: Build check**

Run: `cargo build`
Expected: Compiles

**Step 6: Commit**

```bash
git add -A && git commit -m "feat: add update dialog overlay triggered from status bar widget"
```

---

### Task 7: Handle install and skip actions from the dialog

**Files:**
- Modify: `src/app/window_state.rs` (handle dialog actions)
- Modify: `src/app/window_manager.rs` (process install requests from window state)

**Step 1: Wire up SkipVersion action**

When `SkipVersion(v)` is returned:
- Set `self.config.skipped_version = Some(v)`
- Clear `status_bar_ui.update_available_version = None` (hides widget)
- Save config

**Step 2: Wire up InstallUpdate action**

Reuse the existing self-update mechanism from `par-term-update::self_updater::perform_update()`.
Spawn the update in a background thread, similar to how the settings UI does it.

**Step 3: Build and manual test**

Run: `cargo build && cargo run`

Manual test:
1. Verify "Hourly" appears in Settings > Advanced > Updates dropdown
2. If an update is available, verify the status bar shows the update widget
3. Click the widget, verify dialog opens with version info
4. Test Dismiss, Skip Version buttons

**Step 4: Commit**

```bash
git add -A && git commit -m "feat: wire up install and skip actions in update dialog"
```

---

### Task 8: Final integration testing and cleanup

**Files:**
- Various (cleanup)

**Step 1: Run full test suite**

Run: `make test`
Expected: All tests pass

**Step 2: Run lint and format**

Run: `make fmt && make lint`
Fix any warnings

**Step 3: Build check**

Run: `make build`
Expected: Clean build

**Step 4: Final commit**

```bash
git add -A && git commit -m "chore: lint and format cleanup for update check feature"
```
