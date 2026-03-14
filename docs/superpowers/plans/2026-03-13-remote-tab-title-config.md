# Remote Tab Title Format Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add two config fields (`remote_tab_title_format` and `remote_tab_title_osc_priority`) so users can control how the tab title is displayed when SSH-ed to a remote host via shell integration.

**Architecture:** A new `RemoteTabTitleFormat` enum is added to `par-term-config`; two new `Config` fields hold the format and priority flag. `Tab::update_title()` gains two new parameters and a new remote-host branch that reads hostname, username, and CWD in a single lock acquisition and formats the title accordingly. The settings UI gets a combo box and a checkbox directly below the existing "Tab title mode" control.

**Tech Stack:** Rust 2024 edition, serde for config serialization, egui for UI.

**Spec:** `docs/superpowers/specs/2026-03-13-remote-tab-title-config-design.md`

---

## Chunk 1: Config enum and fields

### Task 1: Add `RemoteTabTitleFormat` enum to `par-term-config`

**Files:**
- Modify: `par-term-config/src/types/tab_bar.rs` (after the `TabTitleMode` block, ~line 128)
- Modify: `par-term-config/src/types/mod.rs` (line 52–54, `pub use tab_bar::{...}`)
- Modify: `par-term-config/src/lib.rs` (line 60–72, `pub use types::{...}`)

- [ ] **Step 1: Write the failing test**

Add at the bottom of `par-term-config/src/types/tab_bar.rs`:

```rust
#[cfg(test)]
mod remote_format_tests {
    use super::*;

    #[test]
    fn all_returns_three_variants() {
        assert_eq!(RemoteTabTitleFormat::all().len(), 3);
    }

    #[test]
    fn display_name_covers_all_variants() {
        for v in RemoteTabTitleFormat::all() {
            assert!(!v.display_name().is_empty());
        }
    }

    #[test]
    fn default_is_user_at_host() {
        assert_eq!(RemoteTabTitleFormat::default(), RemoteTabTitleFormat::UserAtHost);
    }
}
```

- [ ] **Step 2: Run test to confirm it fails**

```bash
cargo test -p par-term-config remote_format_tests 2>&1 | head -20
```

Expected: compile error — `RemoteTabTitleFormat` not found.

- [ ] **Step 3: Add the enum and impls**

In `par-term-config/src/types/tab_bar.rs`, after the `TabTitleMode` block (after line 128):

```rust
// ============================================================================
// Remote Tab Title Format
// ============================================================================

/// Controls the tab title format when shell integration detects a remote host
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTabTitleFormat {
    /// "user@host" — SSH-style identifier (default)
    #[default]
    UserAtHost,
    /// "host" — remote hostname only
    Host,
    /// "host:~/dir" — hostname plus abbreviated CWD
    HostAndCwd,
}

impl RemoteTabTitleFormat {
    /// Display name for UI combo box
    pub fn display_name(&self) -> &'static str {
        match self {
            RemoteTabTitleFormat::UserAtHost => "user@host",
            RemoteTabTitleFormat::Host       => "host",
            RemoteTabTitleFormat::HostAndCwd => "host:~/cwd",
        }
    }

    /// All variants for UI iteration
    pub fn all() -> &'static [RemoteTabTitleFormat] {
        &[
            RemoteTabTitleFormat::UserAtHost,
            RemoteTabTitleFormat::Host,
            RemoteTabTitleFormat::HostAndCwd,
        ]
    }
}
```

- [ ] **Step 4: Export from `types/mod.rs`**

In `par-term-config/src/types/mod.rs`, extend line 52–54:

```rust
pub use tab_bar::{
    RemoteTabTitleFormat, StatusBarPosition, TabBarMode, TabBarPosition, TabStyle, TabTitleMode,
    WindowType,
};
```

- [ ] **Step 5: Re-export from `lib.rs`**

In `par-term-config/src/lib.rs`, append `RemoteTabTitleFormat` inside the **existing**
`pub use types::{...}` block (lines 60–72). Add it alphabetically after `PowerPreference`:

```rust
pub use types::{
    AlertEvent, AlertSoundConfig, BackgroundImageMode, BackgroundMode, CursorShaderConfig,
    CursorShaderMetadata, CursorStyle, DividerRect, DividerStyle, DownloadSaveLocation,
    DroppedFileQuoteStyle, FontRange, ImageScalingMode, InstallPromptState, IntegrationVersions,
    KeyBinding, LinkUnderlineStyle, LogLevel, ModifierRemapping, ModifierTarget, OptionKeyMode,
    PaneBackground, PaneBackgroundConfig, PaneId, PaneTitlePosition, PowerPreference,
    ProgressBarPosition, ProgressBarStyle, RemoteTabTitleFormat, SemanticHistoryEditorMode,
    SeparatorMark, SessionLogFormat, ShaderConfig, ShaderInstallPrompt, ShaderMetadata,
    ShellExitAction, ShellType, SmartSelectionPrecision, SmartSelectionRule, StartupDirectoryMode,
    StatusBarPosition, TabBarMode, TabBarPosition, TabId, TabStyle, TabTitleMode, ThinStrokesMode,
    UnfocusedCursorStyle, UpdateCheckFrequency, VsyncMode, WindowType,
    default_smart_selection_rules,
};
```

- [ ] **Step 6: Run test to confirm it passes**

```bash
cargo test -p par-term-config remote_format_tests 2>&1
```

Expected: 3 tests pass.

- [ ] **Step 7: Commit**

```bash
git add par-term-config/src/types/tab_bar.rs par-term-config/src/types/mod.rs par-term-config/src/lib.rs
git commit -m "feat(config): add RemoteTabTitleFormat enum with display_name and all()"
```

---

### Task 2: Add config fields to `Config`

**Files:**
- Modify: `par-term-config/src/config/config_struct/mod.rs` (use block ~line 96–104; struct body near `tab_title_mode` ~line 955)
- Modify: `par-term-config/src/config/config_struct/default_impl.rs` (near `tab_title_mode: TabTitleMode::default()` ~line 167)

- [ ] **Step 1: Add `RemoteTabTitleFormat` to the `use crate::types` import**

In `par-term-config/src/config/config_struct/mod.rs`, extend the `use crate::types::{...}`
block (lines 96–104) to include `RemoteTabTitleFormat`:

```rust
use crate::types::{
    BackgroundImageMode, BackgroundMode, CursorShaderConfig, CursorStyle, DividerStyle,
    DownloadSaveLocation, DroppedFileQuoteStyle, FontRange, ImageScalingMode, InstallPromptState,
    IntegrationVersions, KeyBinding, LogLevel, ModifierRemapping, OptionKeyMode, PaneTitlePosition,
    PowerPreference, ProgressBarPosition, ProgressBarStyle, RemoteTabTitleFormat,
    SemanticHistoryEditorMode, SessionLogFormat, ShaderConfig, ShaderInstallPrompt,
    ShellExitAction, SmartSelectionRule, StartupDirectoryMode, TabBarMode, TabBarPosition,
    TabStyle, TabTitleMode, ThinStrokesMode, UnfocusedCursorStyle, VsyncMode, WindowType,
};
```

- [ ] **Step 2: Add the two new fields to the `Config` struct**

In `par-term-config/src/config/config_struct/mod.rs`, directly after the `tab_title_mode`
field (~line 955):

```rust
    /// Format for tab title when shell integration detects a remote host
    #[serde(default)]
    pub remote_tab_title_format: RemoteTabTitleFormat,

    /// When true, explicit OSC title sequences override remote_tab_title_format
    #[serde(default = "crate::defaults::bool_true")]
    pub remote_tab_title_osc_priority: bool,
```

- [ ] **Step 3: Add `RemoteTabTitleFormat` to `default_impl.rs` import**

In `par-term-config/src/config/config_struct/default_impl.rs`, extend the
`use crate::types::{...}` block (lines 8–15) to include `RemoteTabTitleFormat`
(alphabetically between `ProgressBarStyle` and `SemanticHistoryEditorMode`):

```rust
use crate::types::{
    BackgroundImageMode, BackgroundMode, CursorStyle, DividerStyle, DroppedFileQuoteStyle,
    ImageScalingMode, InstallPromptState, IntegrationVersions, LogLevel, ModifierRemapping,
    OptionKeyMode, PaneTitlePosition, PowerPreference, ProgressBarPosition, ProgressBarStyle,
    RemoteTabTitleFormat, SemanticHistoryEditorMode, SessionLogFormat, ShaderInstallPrompt,
    ShellExitAction, TabBarMode, TabBarPosition, TabStyle, TabTitleMode, ThinStrokesMode,
    UnfocusedCursorStyle, VsyncMode, WindowType, default_smart_selection_rules,
};
```

- [ ] **Step 4: Add defaults to `impl Default for Config`**

In `par-term-config/src/config/config_struct/default_impl.rs`, directly after the
`tab_title_mode: TabTitleMode::default()` line (~line 167):

```rust
            remote_tab_title_format: RemoteTabTitleFormat::default(),
            remote_tab_title_osc_priority: true,
```

- [ ] **Step 5: Verify the config crate compiles cleanly**

```bash
cargo check -p par-term-config 2>&1
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add par-term-config/src/config/config_struct/mod.rs par-term-config/src/config/config_struct/default_impl.rs
git commit -m "feat(config): add remote_tab_title_format and remote_tab_title_osc_priority fields"
```

---

## Chunk 2: Logic — update_title

### Task 3: Update `Tab::update_title()` with remote-host branch

**Files:**
- Modify: `src/tab/profile_tracking.rs`
- Modify: `src/tab/manager.rs` (line 304–308)
- Modify: `src/app/render_pipeline/frame_setup.rs` (line 54–55)
- Modify: `src/app/window_state/action_handlers/tab_bar.rs` (line 75)

- [ ] **Step 1: Write failing tests for `format_remote_title()`**

Add at the bottom of `src/tab/profile_tracking.rs`:

```rust
#[cfg(test)]
mod format_remote_title_tests {
    use super::format_remote_title;
    use par_term_config::RemoteTabTitleFormat;

    #[test]
    fn user_at_host_with_both() {
        let result = format_remote_title(
            Some("server".into()),
            Some("alice".into()),
            None,
            RemoteTabTitleFormat::UserAtHost,
        );
        assert_eq!(result, "alice@server");
    }

    #[test]
    fn user_at_host_no_username_falls_back_to_host() {
        let result = format_remote_title(
            Some("server".into()),
            None,
            None,
            RemoteTabTitleFormat::UserAtHost,
        );
        assert_eq!(result, "server");
    }

    #[test]
    fn host_only() {
        let result = format_remote_title(
            Some("mybox".into()),
            Some("bob".into()),
            Some("/home/bob/projects".into()),
            RemoteTabTitleFormat::Host,
        );
        assert_eq!(result, "mybox");
    }

    #[test]
    fn host_and_cwd_abbreviates_linux_home() {
        let result = format_remote_title(
            Some("server".into()),
            Some("alice".into()),
            Some("/home/alice/projects/foo".into()),
            RemoteTabTitleFormat::HostAndCwd,
        );
        assert_eq!(result, "server:~/projects/foo");
    }

    #[test]
    fn host_and_cwd_abbreviates_macos_home() {
        let result = format_remote_title(
            Some("mac".into()),
            Some("alice".into()),
            Some("/Users/alice/dev".into()),
            RemoteTabTitleFormat::HostAndCwd,
        );
        assert_eq!(result, "mac:~/dev");
    }

    #[test]
    fn host_and_cwd_no_cwd_falls_back_to_host() {
        let result = format_remote_title(
            Some("server".into()),
            Some("alice".into()),
            None,
            RemoteTabTitleFormat::HostAndCwd,
        );
        assert_eq!(result, "server");
    }

    #[test]
    fn host_and_cwd_unknown_path_no_abbreviation() {
        let result = format_remote_title(
            Some("server".into()),
            Some("alice".into()),
            Some("/var/log".into()),
            RemoteTabTitleFormat::HostAndCwd,
        );
        assert_eq!(result, "server:/var/log");
    }
}
```

- [ ] **Step 2: Run test to confirm it fails**

```bash
cargo test -p par-term format_remote_title_tests 2>&1 | head -20
```

Expected: compile error — `format_remote_title` not found.

- [ ] **Step 3: Add `format_remote_title()` private helper**

In `src/tab/profile_tracking.rs`, after the `impl Tab { ... }` block (before the
`#[cfg(test)]` section):

```rust
/// Format a tab title for a remote host based on the configured format.
///
/// Uses the remote username to abbreviate the home directory in `HostAndCwd` mode
/// (e.g. `/home/alice/projects` → `~/projects`) rather than the local `$HOME`,
/// which never matches remote paths.
fn format_remote_title(
    hostname: Option<String>,
    username: Option<String>,
    cwd: Option<String>,
    format: par_term_config::RemoteTabTitleFormat,
) -> String {
    use par_term_config::RemoteTabTitleFormat;
    let host = hostname.unwrap_or_default();
    match format {
        RemoteTabTitleFormat::UserAtHost => {
            if let Some(user) = username {
                format!("{}@{}", user, host)
            } else {
                host
            }
        }
        RemoteTabTitleFormat::Host => host,
        RemoteTabTitleFormat::HostAndCwd => {
            if let Some(cwd) = cwd {
                let abbrev = if let Some(ref user) = username {
                    let linux_home = format!("/home/{}", user);
                    let macos_home = format!("/Users/{}", user);
                    if cwd.starts_with(&linux_home) {
                        cwd.replacen(&linux_home, "~", 1)
                    } else if cwd.starts_with(&macos_home) {
                        cwd.replacen(&macos_home, "~", 1)
                    } else {
                        cwd
                    }
                } else {
                    cwd
                };
                format!("{}:{}", host, abbrev)
            } else {
                host
            }
        }
    }
}
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cargo test -p par-term format_remote_title_tests 2>&1
```

Expected: all 7 tests pass.

- [ ] **Step 5: Update `update_title()` signature and body**

In `src/tab/profile_tracking.rs`, replace the existing `update_title` method (lines 22–55)
with the new three-parameter version:

```rust
    /// Update tab title from terminal OSC sequences or shell integration data.
    ///
    /// Priority when on a **remote** host (hostname detected via OSC 7):
    ///   1. Explicit OSC title (`\033]0;...\007`) if `remote_osc_priority` is true
    ///   2. `remote_format` — formatted from hostname/username/cwd
    ///
    /// Priority when **local**:
    ///   1. Explicit OSC title
    ///   2. Last CWD component (only in `TabTitleMode::Auto`)
    ///
    /// User-named tabs are never auto-updated.
    pub fn update_title(
        &mut self,
        title_mode: par_term_config::TabTitleMode,
        remote_format: par_term_config::RemoteTabTitleFormat,
        remote_osc_priority: bool,
    ) {
        // User-named tabs are static — never auto-update
        if self.user_named {
            return;
        }
        // try_lock: intentional — called every frame from the render path; blocking would
        // stall rendering. On miss: title is not updated this frame. No data loss.
        if let Ok(term) = self.terminal.try_write() {
            // Collect all values in a single lock acquisition
            let osc_title = term.get_title();
            let hostname  = term.shell_integration_hostname();
            let username  = term.shell_integration_username();
            let cwd       = term.shell_integration_cwd();
            drop(term); // release lock before mutating self

            let is_remote = hostname.is_some();

            if is_remote {
                if remote_osc_priority && !osc_title.is_empty() {
                    self.title = osc_title;
                    self.has_default_title = false;
                } else {
                    self.title = format_remote_title(hostname, username, cwd, remote_format);
                    self.has_default_title = false;
                }
            } else if !osc_title.is_empty() {
                self.title = osc_title;
                self.has_default_title = false;
            } else if title_mode == par_term_config::TabTitleMode::Auto {
                if let Some(cwd) = cwd {
                    // Abbreviate home directory to ~
                    let abbreviated = if let Some(home) = dirs::home_dir() {
                        cwd.replace(&home.to_string_lossy().to_string(), "~")
                    } else {
                        cwd
                    };
                    // Use just the last component for brevity (original pattern)
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
            // else: keep existing title
        }
    }
```

- [ ] **Step 6: Update `update_all_titles()` in `src/tab/manager.rs`**

Replace lines 303–308:

```rust
    /// Update titles for all tabs
    pub fn update_all_titles(
        &mut self,
        title_mode: par_term_config::TabTitleMode,
        remote_format: par_term_config::RemoteTabTitleFormat,
        remote_osc_priority: bool,
    ) {
        for tab in &mut self.tabs {
            tab.update_title(title_mode, remote_format, remote_osc_priority);
        }
    }
```

- [ ] **Step 7: Update call site in `src/app/render_pipeline/frame_setup.rs`**

Replace line 54–55:

```rust
        self.tab_manager.update_all_titles(
            self.config.tab_title_mode,
            self.config.remote_tab_title_format,
            self.config.remote_tab_title_osc_priority,
        );
```

- [ ] **Step 8: Update direct call site in `src/app/window_state/action_handlers/tab_bar.rs`**

Replace line 75:

```rust
                        tab.update_title(
                            self.config.tab_title_mode,
                            self.config.remote_tab_title_format,
                            self.config.remote_tab_title_osc_priority,
                        );
```

- [ ] **Step 9: Verify the workspace compiles**

```bash
cargo check --workspace 2>&1
```

Expected: no errors.

- [ ] **Step 10: Run all tests**

```bash
cargo test --workspace 2>&1 | tail -20
```

Expected: all tests pass (no regressions).

- [ ] **Step 11: Commit**

```bash
git add src/tab/profile_tracking.rs src/tab/manager.rs \
        src/app/render_pipeline/frame_setup.rs \
        src/app/window_state/action_handlers/tab_bar.rs
git commit -m "feat(tab): add remote-host tab title formatting with configurable format and OSC priority"
```

---

## Chunk 3: UI — settings controls and keywords

### Task 4: Add UI controls in settings and update search keywords

**Files:**
- Modify: `par-term-settings-ui/src/window_tab/tab_bar_behavior.rs` (after "Tab title mode" combo box, ~line 147)
- Modify: `par-term-settings-ui/src/window_tab/mod.rs` (keywords slice ~line 117; section_matches Tab Bar call ~line 114)

- [ ] **Step 1: Add `RemoteTabTitleFormat` to the imports in `tab_bar_behavior.rs`**

In `par-term-settings-ui/src/window_tab/tab_bar_behavior.rs`, line 5:

```rust
use par_term_config::{RemoteTabTitleFormat, TabBarMode, TabBarPosition, TabStyle, TabTitleMode};
```

- [ ] **Step 2: Add the remote format combo box and OSC priority checkbox**

In `par-term-settings-ui/src/window_tab/tab_bar_behavior.rs`, directly after the closing
`});` of the "Tab title mode" combo box block (~line 147), insert:

```rust
        ui.horizontal(|ui| {
            ui.label("Remote tab title format:");
            egui::ComboBox::from_id_salt("window_remote_tab_title_format")
                .selected_text(settings.config.remote_tab_title_format.display_name())
                .show_ui(ui, |ui| {
                    for &fmt in RemoteTabTitleFormat::all() {
                        if ui
                            .selectable_value(
                                &mut settings.config.remote_tab_title_format,
                                fmt,
                                fmt.display_name(),
                            )
                            .on_hover_text(match fmt {
                                RemoteTabTitleFormat::UserAtHost =>
                                    "Show username and hostname (e.g. paul@server)",
                                RemoteTabTitleFormat::Host =>
                                    "Show hostname only",
                                RemoteTabTitleFormat::HostAndCwd =>
                                    "Show hostname and current directory (e.g. server:~/projects)",
                            })
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        if ui
            .checkbox(
                &mut settings.config.remote_tab_title_osc_priority,
                "OSC title takes priority on remote hosts",
            )
            .on_hover_text(
                "When checked, explicit OSC title sequences (\\033]0;) \
                 override the remote tab title format",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
```

- [ ] **Step 3: Add search keywords in `par-term-settings-ui/src/window_tab/mod.rs`**

Two places need updating in `mod.rs`:

**A) `keywords()` function (~line 266–279, under "// Tab bar" comment)**

Add after `"cwd title"` (line 273):

```rust
        "remote tab title",
        "ssh title",
        "remote host",
        "user at host",
        "remote format",
        "osc priority",
```

**B) `section_matches` for "Tab Bar" (~line 114–131)**

Add the same keywords to the `section_matches` "Tab Bar" slice:

```rust
    if section_matches(
        &query,
        "Tab Bar",
        &[
            "tab",
            "tabs",
            "bar",
            "index",
            "close button",
            "profile drawer",
            "stretch",
            "html titles",
            "inherit directory",
            "max tabs",
            "remote tab title",
            "ssh title",
            "remote host",
            "user at host",
            "remote format",
            "osc priority",
        ],
    ) {
```

- [ ] **Step 4: Verify the full workspace builds**

```bash
cargo check --workspace 2>&1
```

Expected: no errors.

- [ ] **Step 5: Build in dev-release mode**

```bash
make build 2>&1 | tail -5
```

Expected: `Finished` with no errors.

- [ ] **Step 6: Commit**

```bash
git add par-term-settings-ui/src/window_tab/tab_bar_behavior.rs \
        par-term-settings-ui/src/window_tab/mod.rs
git commit -m "feat(ui): add remote tab title format controls to tab bar settings"
```

---

## Manual Verification

After all tasks are complete, verify end-to-end:

1. SSH to a remote host that has par-term shell integration installed
2. Confirm tab title shows `user@host` (default `UserAtHost` format)
3. Open Settings → Window → Tab Bar
4. Change "Remote tab title format" to `host` → tab updates to hostname only
5. Change to `host:~/cwd` → tab shows `server:~/some/path`
6. Uncheck "OSC title takes priority" → if remote shell emits an OSC title, the format now wins
7. Re-check → OSC title takes priority again
8. Clear tab rename (set blank name) → tab reverts to remote format immediately
