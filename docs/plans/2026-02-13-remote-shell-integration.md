# Remote Shell Integration Install Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a "Shell" menu with an "Install Shell Integration on Remote Host..." item that shows a confirmation dialog and injects a curl command into the active terminal's PTY.

**Architecture:** New `MenuAction` variant + new "Shell" submenu in menu bar + new egui confirmation dialog (`RemoteShellInstallUI`) following the `CloseConfirmationUI` pattern. When user confirms, writes the curl command to the active tab's PTY via `write_str()`.

**Tech Stack:** Rust, muda (menus), egui (dialog), wgpu terminal

---

### Task 1: Add MenuAction variant

**Files:**
- Modify: `src/menu/actions.rs:93` (add new variant before keybinding actions)

**Step 1: Add the variant**

In `src/menu/actions.rs`, add after the `SaveArrangement` variant (line 92) and before the keybinding actions comment (line 94):

```rust
    // Shell menu
    /// Install shell integration on a remote host via curl
    InstallShellIntegrationRemote,
```

**Step 2: Verify it compiles**

Run: `cargo check 2>&1 | head -20`
Expected: compiles cleanly (variant is unused but that's fine for now)

**Step 3: Commit**

```bash
git add src/menu/actions.rs
git commit -m "feat(menu): add InstallShellIntegrationRemote menu action variant"
```

---

### Task 2: Add Shell submenu to menu bar

**Files:**
- Modify: `src/menu/mod.rs` (add Shell submenu between View menu and Window/Help menus)

**Step 1: Add the Shell submenu**

In `src/menu/mod.rs`, after the View menu block (after `menu.append(&view_menu)?;` at line 392) and before the Window menu block (line 394), add:

```rust
        // Shell menu
        let shell_menu = Submenu::new("Shell", true);

        let install_remote_integration = MenuItem::with_id(
            "install_remote_shell_integration",
            "Install Shell Integration on Remote Host...",
            true,
            None,
        );
        action_map.insert(
            install_remote_integration.id().clone(),
            MenuAction::InstallShellIntegrationRemote,
        );
        shell_menu.append(&install_remote_integration)?;

        menu.append(&shell_menu)?;
```

**Step 2: Verify it compiles**

Run: `cargo check 2>&1 | head -20`
Expected: compiles cleanly

**Step 3: Commit**

```bash
git add src/menu/mod.rs
git commit -m "feat(menu): add Shell submenu with remote integration install item"
```

---

### Task 3: Create RemoteShellInstallUI dialog

**Files:**
- Create: `src/remote_shell_install_ui.rs`

**Step 1: Create the dialog module**

Create `src/remote_shell_install_ui.rs` with the confirmation dialog. Follow the `CloseConfirmationUI` pattern from `src/close_confirmation_ui.rs`:

```rust
//! Remote shell integration install confirmation dialog.
//!
//! Shows a confirmation dialog when the user selects "Install Shell Integration
//! on Remote Host" from the Shell menu. Displays the exact curl command that will
//! be sent to the active terminal and lets the user confirm or cancel.

/// The install command URL
const INSTALL_URL: &str = "https://paulrobello.github.io/par-term/install-shell-integration.sh";

/// Action returned by the remote shell install dialog
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteShellInstallAction {
    /// User confirmed - send the install command to the active terminal
    Install,
    /// User cancelled
    Cancel,
    /// No action yet (dialog still showing or not visible)
    None,
}

/// State for the remote shell integration install dialog
pub struct RemoteShellInstallUI {
    /// Whether the dialog is visible
    visible: bool,
}

impl Default for RemoteShellInstallUI {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteShellInstallUI {
    /// Create a new remote shell install UI
    pub fn new() -> Self {
        Self { visible: false }
    }

    /// Check if the dialog is currently visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Show the confirmation dialog
    pub fn show_dialog(&mut self) {
        self.visible = true;
    }

    /// Hide the dialog
    fn hide(&mut self) {
        self.visible = false;
    }

    /// Get the install command string
    pub fn install_command() -> String {
        format!("curl -sSL {} | sh", INSTALL_URL)
    }

    /// Render the dialog and return any action
    pub fn show(&mut self, ctx: &egui::Context) -> RemoteShellInstallAction {
        if !self.visible {
            return RemoteShellInstallAction::None;
        }

        let mut action = RemoteShellInstallAction::None;
        let command = Self::install_command();

        egui::Window::new("Install Shell Integration on Remote Host")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);

                    ui.label(
                        egui::RichText::new("Send Install Command to Terminal")
                            .size(16.0)
                            .strong(),
                    );
                    ui.add_space(8.0);

                    ui.label("This will send the following command to the active terminal:");
                    ui.add_space(8.0);

                    // Command preview in a highlighted code block
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 220))
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .corner_radius(4.0)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(&command)
                                    .color(egui::Color32::LIGHT_GREEN)
                                    .monospace()
                                    .size(13.0),
                            );
                        });

                    ui.add_space(10.0);

                    // Warning
                    ui.label(
                        egui::RichText::new(
                            "Only use this when SSH'd into a remote host that needs shell integration.",
                        )
                        .color(egui::Color32::YELLOW)
                        .size(12.0),
                    );

                    ui.add_space(15.0);

                    // Buttons
                    ui.horizontal(|ui| {
                        let install_button = egui::Button::new(
                            egui::RichText::new("Install").color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(50, 120, 50));

                        if ui.add(install_button).clicked() {
                            action = RemoteShellInstallAction::Install;
                        }

                        ui.add_space(10.0);

                        if ui.button("Cancel").clicked() {
                            action = RemoteShellInstallAction::Cancel;
                        }
                    });
                    ui.add_space(10.0);
                });
            });

        // Handle escape key to cancel
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            action = RemoteShellInstallAction::Cancel;
        }

        // Hide dialog on any action
        if !matches!(action, RemoteShellInstallAction::None) {
            self.hide();
        }

        action
    }
}
```

**Step 2: Register the module in `src/lib.rs`**

Add after the `quit_confirmation_ui` line (line 39):

```rust
pub mod remote_shell_install_ui;
```

**Step 3: Verify it compiles**

Run: `cargo check 2>&1 | head -20`
Expected: compiles cleanly

**Step 4: Commit**

```bash
git add src/remote_shell_install_ui.rs src/lib.rs
git commit -m "feat: add RemoteShellInstallUI confirmation dialog"
```

---

### Task 4: Wire dialog into WindowState

**Files:**
- Modify: `src/app/window_state.rs` (add field, import, init, render, handle action)

**Step 1: Add import**

In `src/app/window_state.rs`, add after the `quit_confirmation_ui` import (line 25):

```rust
use crate::remote_shell_install_ui::{RemoteShellInstallAction, RemoteShellInstallUI};
```

**Step 2: Add field to WindowState struct**

After the `quit_confirmation_ui` field (around line 138), add:

```rust
    /// Remote shell integration install dialog UI
    pub(crate) remote_shell_install_ui: RemoteShellInstallUI,
```

**Step 3: Initialize in `WindowState::new()`**

In the `Self { ... }` block, after `quit_confirmation_ui: QuitConfirmationUI::new(),` (around line 324), add:

```rust
            remote_shell_install_ui: RemoteShellInstallUI::new(),
```

**Step 4: Add pending action variable in render_egui method**

Find the line `let mut pending_quit_confirm_action = QuitConfirmAction::None;` (around line 1767) and add after it:

```rust
        let mut pending_remote_install_action = RemoteShellInstallAction::None;
```

**Step 5: Render the dialog in the egui block**

Find where `pending_quit_confirm_action = self.quit_confirmation_ui.show(ctx);` is called (around line 2252) and add after it:

```rust
                    // Show remote shell install dialog if visible
                    pending_remote_install_action = self.remote_shell_install_ui.show(ctx);
```

**Step 6: Handle the action after rendering**

Find the quit confirmation action handler block (starts around line 2820 with `match pending_quit_confirm_action`) and add AFTER that entire match block:

```rust
        // Handle remote shell integration install action
        match pending_remote_install_action {
            RemoteShellInstallAction::Install => {
                // Send the install command to the active terminal
                let command = RemoteShellInstallUI::install_command();
                if let Some(tab) = self.tab_manager.active_tab() {
                    if let Ok(term) = tab.terminal.try_lock() {
                        let _ = term.write_str(&format!("{}\r", command));
                    }
                }
            }
            RemoteShellInstallAction::Cancel => {
                // Nothing to do - dialog already hidden
            }
            RemoteShellInstallAction::None => {}
        }
```

**Step 7: Verify it compiles**

Run: `cargo check 2>&1 | head -20`
Expected: compiles cleanly

**Step 8: Commit**

```bash
git add src/app/window_state.rs
git commit -m "feat: wire RemoteShellInstallUI into WindowState render loop"
```

---

### Task 5: Handle menu action in WindowManager

**Files:**
- Modify: `src/app/window_manager.rs` (add handler for `InstallShellIntegrationRemote`)

**Step 1: Add the menu action handler**

In `src/app/window_manager.rs`, find the `handle_menu_action` method's match block. After the `MenuAction::SaveArrangement` arm (around line 1169), add:

```rust
            MenuAction::InstallShellIntegrationRemote => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.remote_shell_install_ui.show_dialog();
                    window_state.needs_redraw = true;
                }
            }
```

**Step 2: Verify full build compiles**

Run: `cargo check 2>&1 | head -20`
Expected: compiles cleanly

**Step 3: Commit**

```bash
git add src/app/window_manager.rs
git commit -m "feat: handle InstallShellIntegrationRemote menu action in window manager"
```

---

### Task 6: Add tests

**Files:**
- Modify: `src/remote_shell_install_ui.rs` (add unit tests)

**Step 1: Add tests to the dialog module**

Append to `src/remote_shell_install_ui.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_command_format() {
        let cmd = RemoteShellInstallUI::install_command();
        assert!(cmd.starts_with("curl"));
        assert!(cmd.contains("paulrobello.github.io/par-term"));
        assert!(cmd.contains("install-shell-integration.sh"));
        assert!(cmd.ends_with("| sh"));
    }

    #[test]
    fn test_dialog_initial_state() {
        let ui = RemoteShellInstallUI::new();
        assert!(!ui.is_visible());
    }

    #[test]
    fn test_dialog_show_hide() {
        let mut ui = RemoteShellInstallUI::new();
        assert!(!ui.is_visible());

        ui.show_dialog();
        assert!(ui.is_visible());

        ui.hide();
        assert!(!ui.is_visible());
    }

    #[test]
    fn test_default_impl() {
        let ui = RemoteShellInstallUI::default();
        assert!(!ui.is_visible());
    }
}
```

**Step 2: Run the tests**

Run: `cargo test remote_shell -- --nocapture`
Expected: all 4 tests pass

**Step 3: Commit**

```bash
git add src/remote_shell_install_ui.rs
git commit -m "test: add unit tests for RemoteShellInstallUI"
```

---

### Task 7: Run full quality checks

**Step 1: Format**

Run: `make fmt`

**Step 2: Lint**

Run: `make lint`
Expected: no warnings

**Step 3: Run all tests**

Run: `make test`
Expected: all tests pass

**Step 4: Commit any formatting changes**

```bash
git add -A
git commit -m "style: format remote shell integration code"
```

(Skip if no changes)

---

### Task 8: Create PR

**Step 1: Push branch and create PR**

```bash
git push -u origin feat/install-remote-shell-integration
gh pr create --title "feat(shell-integration): add menu option to install shell integration on remote hosts" --body "$(cat <<'EOF'
## Summary
- Adds new "Shell" menu to the menu bar with "Install Shell Integration on Remote Host..." item
- Shows an egui confirmation dialog with the exact curl command before sending
- Sends `curl -sSL https://paulrobello.github.io/par-term/install-shell-integration.sh | sh` to the active terminal's PTY

Closes #135

## Test plan
- [x] Unit tests for dialog state management and command format
- [ ] Manual: verify Shell menu appears in menu bar
- [ ] Manual: verify dialog shows when clicking the menu item
- [ ] Manual: verify Install button sends command to terminal
- [ ] Manual: verify Cancel and Escape dismiss the dialog
- [ ] Manual: verify command works when SSH'd to a remote host
EOF
)"
```
