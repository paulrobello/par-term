# Triggers, Trigger Actions & Coprocesses Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add frontend UI and event wiring for regex triggers, trigger action dispatch, and coprocess management (Issue #84).

**Architecture:** Config types persist trigger/coprocess definitions in config.yaml. A per-frame event loop handler polls the core library for trigger matches and action results, dispatching RunCommand/PlaySound/SendText. An "Automation" settings tab provides full CRUD for triggers and coprocesses. CoprocessManager lives per-tab alongside the terminal.

**Tech Stack:** Rust, egui (settings UI), par-term-emu-core-rust v0.31.0 (TriggerRegistry, CoprocessManager), rodio (audio), serde/serde_yaml (config persistence)

---

### Task 1: Config Automation Types

**Files:**
- Create: `src/config/automation.rs`
- Modify: `src/config/mod.rs:6-9` (add module declaration)
- Modify: `src/config/mod.rs:1322-1325` (add fields before closing brace on line 1325)
- Modify: `src/config/mod.rs:1565-1567` (add defaults in Default impl before closing braces)
- Test: `tests/automation_config_tests.rs`

**Step 1: Create `src/config/automation.rs` with types**

```rust
//! Configuration types for triggers and coprocesses.

use serde::{Deserialize, Serialize};

/// A trigger definition that matches terminal output and fires actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TriggerConfig {
    /// Human-readable name for this trigger
    pub name: String,
    /// Regex pattern to match against terminal output
    pub pattern: String,
    /// Whether this trigger is currently active
    #[serde(default = "super::defaults::bool_true")]
    pub enabled: bool,
    /// Actions to execute when the pattern matches
    #[serde(default)]
    pub actions: Vec<TriggerActionConfig>,
}

/// An action to perform when a trigger pattern matches.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerActionConfig {
    /// Highlight the matched text with colors
    Highlight {
        /// Foreground color [R, G, B]
        #[serde(default)]
        fg: Option<[u8; 3]>,
        /// Background color [R, G, B]
        #[serde(default)]
        bg: Option<[u8; 3]>,
        /// How long the highlight lasts (0 = permanent)
        #[serde(default = "default_highlight_duration")]
        duration_ms: u64,
    },
    /// Send a desktop notification
    Notify {
        title: String,
        message: String,
    },
    /// Mark the matched line in scrollback
    MarkLine {
        #[serde(default)]
        label: Option<String>,
    },
    /// Set a session variable
    SetVariable {
        name: String,
        value: String,
    },
    /// Run an external command
    RunCommand {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// Play a sound ("bell" = built-in tone, otherwise filename in sounds dir)
    PlaySound {
        #[serde(default)]
        sound_id: String,
        #[serde(default = "default_volume")]
        volume: u8,
    },
    /// Send text to the terminal
    SendText {
        text: String,
        #[serde(default)]
        delay_ms: u64,
    },
}

/// A coprocess definition for piped subprocess management.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoprocessDefConfig {
    /// Human-readable name
    pub name: String,
    /// Command to execute
    pub command: String,
    /// Command arguments
    #[serde(default)]
    pub args: Vec<String>,
    /// Start automatically when a tab opens
    #[serde(default)]
    pub auto_start: bool,
    /// Pipe terminal output to coprocess stdin
    #[serde(default = "super::defaults::bool_true")]
    pub copy_terminal_output: bool,
}

fn default_highlight_duration() -> u64 {
    5000
}

fn default_volume() -> u8 {
    50
}

/// Convert a TriggerActionConfig to the core library's TriggerAction type.
impl TriggerActionConfig {
    pub fn to_core_action(&self) -> par_term_emu_core_rust::terminal::TriggerAction {
        use par_term_emu_core_rust::terminal::TriggerAction;
        match self.clone() {
            Self::Highlight { fg, bg, duration_ms } => TriggerAction::Highlight {
                fg: fg.map(|c| (c[0], c[1], c[2])),
                bg: bg.map(|c| (c[0], c[1], c[2])),
                duration_ms,
            },
            Self::Notify { title, message } => TriggerAction::Notify { title, message },
            Self::MarkLine { label } => TriggerAction::MarkLine { label },
            Self::SetVariable { name, value } => TriggerAction::SetVariable { name, value },
            Self::RunCommand { command, args } => TriggerAction::RunCommand { command, args },
            Self::PlaySound { sound_id, volume } => TriggerAction::PlaySound { sound_id, volume },
            Self::SendText { text, delay_ms } => TriggerAction::SendText { text, delay_ms },
        }
    }
}
```

**Step 2: Register the module and add fields to Config**

In `src/config/mod.rs`:
- After line 9 (`mod types;`), add: `pub mod automation;`
- After line 22, add: `pub use automation::{CoprocessDefConfig, TriggerActionConfig, TriggerConfig};`
- Before line 1325 (closing `}` of Config struct), add:
```rust
    // ========================================================================
    // Triggers & Automation
    // ========================================================================
    /// Regex trigger definitions that match terminal output and fire actions
    #[serde(default)]
    pub triggers: Vec<automation::TriggerConfig>,

    /// Coprocess definitions for piped subprocess management
    #[serde(default)]
    pub coprocesses: Vec<automation::CoprocessDefConfig>,
```
- In `Default` impl, before line 1567 closing braces, add:
```rust
            triggers: Vec::new(),
            coprocesses: Vec::new(),
```

**Step 3: Write tests**

Create `tests/automation_config_tests.rs`:
```rust
use par_term::config::{Config, TriggerConfig, TriggerActionConfig, CoprocessDefConfig};

#[test]
fn test_config_default_triggers_empty() {
    let config = Config::default();
    assert!(config.triggers.is_empty());
    assert!(config.coprocesses.is_empty());
}

#[test]
fn test_trigger_config_yaml_roundtrip() {
    let trigger = TriggerConfig {
        name: "Error monitor".to_string(),
        pattern: r"ERROR: (.+)".to_string(),
        enabled: true,
        actions: vec![
            TriggerActionConfig::Highlight {
                fg: Some([255, 0, 0]),
                bg: None,
                duration_ms: 5000,
            },
            TriggerActionConfig::Notify {
                title: "Error".to_string(),
                message: "Found: $1".to_string(),
            },
        ],
    };

    let yaml = serde_yaml::to_string(&trigger).unwrap();
    let deserialized: TriggerConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(trigger, deserialized);
}

#[test]
fn test_trigger_action_config_all_variants_serialize() {
    let actions = vec![
        TriggerActionConfig::Highlight { fg: Some([255, 0, 0]), bg: None, duration_ms: 5000 },
        TriggerActionConfig::Notify { title: "t".into(), message: "m".into() },
        TriggerActionConfig::MarkLine { label: Some("mark".into()) },
        TriggerActionConfig::SetVariable { name: "n".into(), value: "v".into() },
        TriggerActionConfig::RunCommand { command: "echo".into(), args: vec!["hi".into()] },
        TriggerActionConfig::PlaySound { sound_id: "bell".into(), volume: 80 },
        TriggerActionConfig::SendText { text: "hello".into(), delay_ms: 100 },
    ];

    for action in &actions {
        let yaml = serde_yaml::to_string(action).unwrap();
        let deserialized: TriggerActionConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(*action, deserialized);
    }
}

#[test]
fn test_trigger_action_to_core_action() {
    use par_term_emu_core_rust::terminal::TriggerAction;

    let config_action = TriggerActionConfig::Highlight {
        fg: Some([255, 0, 0]),
        bg: None,
        duration_ms: 3000,
    };
    let core_action = config_action.to_core_action();
    assert_eq!(core_action, TriggerAction::Highlight {
        fg: Some((255, 0, 0)),
        bg: None,
        duration_ms: 3000,
    });
}

#[test]
fn test_coprocess_config_yaml_roundtrip() {
    let coproc = CoprocessDefConfig {
        name: "Logger".to_string(),
        command: "grep".to_string(),
        args: vec!["--line-buffered".into(), "ERROR".into()],
        auto_start: true,
        copy_terminal_output: true,
    };

    let yaml = serde_yaml::to_string(&coproc).unwrap();
    let deserialized: CoprocessDefConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(coproc, deserialized);
}

#[test]
fn test_config_with_triggers_yaml_roundtrip() {
    let mut config = Config::default();
    config.triggers.push(TriggerConfig {
        name: "test".to_string(),
        pattern: "test".to_string(),
        enabled: true,
        actions: vec![],
    });
    config.coprocesses.push(CoprocessDefConfig {
        name: "test".to_string(),
        command: "cat".to_string(),
        args: vec![],
        auto_start: false,
        copy_terminal_output: true,
    });

    let yaml = serde_yaml::to_string(&config).unwrap();
    let deserialized: Config = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(config.triggers.len(), deserialized.triggers.len());
    assert_eq!(config.coprocesses.len(), deserialized.coprocesses.len());
}
```

**Step 4: Run tests**

Run: `cargo test --test automation_config_tests -v`
Expected: All 6 tests PASS

**Step 5: Commit**

```bash
git add src/config/automation.rs src/config/mod.rs tests/automation_config_tests.rs
git commit -m "feat(config): add trigger and coprocess config types"
```

---

### Task 2: Trigger Sync into Core Registry

**Files:**
- Modify: `src/terminal/mod.rs:1-11` (add imports)
- Modify: `src/terminal/mod.rs` (add sync method)

**Step 1: Add trigger sync method to TerminalManager**

In `src/terminal/mod.rs`, add a method after the existing public methods:

```rust
    /// Sync trigger configs from Config into the core TriggerRegistry.
    ///
    /// Clears existing triggers and re-adds from config. Called on startup
    /// and when settings are saved.
    pub fn sync_triggers(&self, triggers: &[crate::config::automation::TriggerConfig]) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();

        // Clear existing triggers by removing all
        let existing: Vec<u64> = term.list_triggers().iter().map(|t| t.id).collect();
        for id in existing {
            term.remove_trigger(id);
        }

        // Add triggers from config
        for trigger_config in triggers {
            let actions: Vec<par_term_emu_core_rust::terminal::TriggerAction> = trigger_config
                .actions
                .iter()
                .map(|a| a.to_core_action())
                .collect();

            match term.add_trigger(
                trigger_config.name.clone(),
                trigger_config.pattern.clone(),
                actions,
            ) {
                Ok(id) => {
                    if !trigger_config.enabled {
                        term.set_trigger_enabled(id, false);
                    }
                    log::info!("Trigger '{}' registered (id={})", trigger_config.name, id);
                }
                Err(e) => {
                    log::error!(
                        "Failed to register trigger '{}': {}",
                        trigger_config.name,
                        e
                    );
                }
            }
        }
    }
```

**Step 2: Add trigger sync import**

At the top of `src/terminal/mod.rs`, the existing imports already include `par_term_emu_core_rust::terminal::Terminal` — no new imports needed since we use full paths.

**Step 3: Run existing tests**

Run: `cargo test --test terminal_tests -v`
Expected: PASS (no regression)

**Step 4: Commit**

```bash
git add src/terminal/mod.rs
git commit -m "feat(terminal): add trigger config sync to core registry"
```

---

### Task 3: Tab Coprocess Support + Auto-start

**Files:**
- Modify: `src/tab/mod.rs:279-330` (add CoprocessManager field to Tab struct)
- Modify: `src/tab/mod.rs` (Tab::new — initialize coprocess manager, call sync_triggers, auto-start coprocesses)

**Step 1: Add CoprocessManager to Tab struct**

In `src/tab/mod.rs`, add import at top:
```rust
use par_term_emu_core_rust::coprocess::{CoprocessManager, CoprocessId};
```

In the `Tab` struct (after `badge_override` field at line ~329), add:
```rust
    /// Coprocess manager for this tab
    pub coprocess_manager: CoprocessManager,
    /// Mapping from config index to coprocess ID (for UI tracking)
    pub coprocess_ids: Vec<Option<CoprocessId>>,
```

**Step 2: Initialize in Tab::new**

In `Tab::new()`, after terminal creation and before the return, add:
```rust
        // Sync triggers from config into core registry
        terminal_manager.sync_triggers(&config.triggers);

        // Initialize coprocess manager and auto-start configured coprocesses
        let mut coprocess_manager = CoprocessManager::new();
        let mut coprocess_ids = Vec::with_capacity(config.coprocesses.len());
        for coproc_config in &config.coprocesses {
            if coproc_config.auto_start {
                let core_config = par_term_emu_core_rust::coprocess::CoprocessConfig {
                    command: coproc_config.command.clone(),
                    args: coproc_config.args.clone(),
                    cwd: None,
                    env: std::collections::HashMap::new(),
                    copy_terminal_output: coproc_config.copy_terminal_output,
                };
                match coprocess_manager.start(core_config) {
                    Ok(id) => {
                        log::info!("Auto-started coprocess '{}' (id={})", coproc_config.name, id);
                        coprocess_ids.push(Some(id));
                    }
                    Err(e) => {
                        log::error!("Failed to auto-start coprocess '{}': {}", coproc_config.name, e);
                        coprocess_ids.push(None);
                    }
                }
            } else {
                coprocess_ids.push(None);
            }
        }
```

And include `coprocess_manager` and `coprocess_ids` in the Tab struct initialization.

**Step 3: Run tests**

Run: `cargo build`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/tab/mod.rs
git commit -m "feat(tab): add coprocess manager with auto-start support"
```

---

### Task 4: Trigger Action Dispatch in Event Loop

**Files:**
- Create: `src/app/triggers.rs`
- Modify: `src/app/mod.rs:24` (add `mod triggers;` after `mod notifications;`)
- Modify: `src/app/handler.rs:697-698` (add call after check_bell)

**Step 1: Create `src/app/triggers.rs`**

```rust
//! Trigger action dispatch and sound playback.
//!
//! This module handles polling trigger action results from the core library
//! and executing frontend-handled actions: RunCommand, PlaySound, SendText.

use std::io::BufReader;
use std::path::PathBuf;

use super::window_state::WindowState;

impl WindowState {
    /// Check for trigger action results and dispatch them.
    ///
    /// Called each frame after check_bell(). Polls the core library for
    /// ActionResult events and executes the appropriate frontend action.
    pub(crate) fn check_trigger_actions(&mut self) {
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        // Poll action results from core terminal
        let action_results = if let Ok(mut term) = tab.terminal.try_lock() {
            term.poll_action_results()
        } else {
            return;
        };

        if action_results.is_empty() {
            return;
        }

        for action in action_results {
            match action {
                par_term_emu_core_rust::terminal::ActionResult::RunCommand {
                    trigger_id,
                    command,
                    args,
                } => {
                    log::info!(
                        "Trigger {} firing RunCommand: {} {:?}",
                        trigger_id,
                        command,
                        args
                    );
                    // Spawn detached — don't block the event loop
                    match std::process::Command::new(&command).args(&args).spawn() {
                        Ok(_) => {
                            log::debug!("RunCommand spawned successfully");
                        }
                        Err(e) => {
                            log::error!("RunCommand failed to spawn '{}': {}", command, e);
                        }
                    }
                }
                par_term_emu_core_rust::terminal::ActionResult::PlaySound {
                    trigger_id,
                    sound_id,
                    volume,
                } => {
                    log::info!(
                        "Trigger {} firing PlaySound: '{}' at volume {}",
                        trigger_id,
                        sound_id,
                        volume
                    );
                    if sound_id == "bell" || sound_id.is_empty() {
                        // Reuse the built-in bell tone
                        if let Some(tab) = self.tab_manager.active_tab() {
                            if let Some(ref audio_bell) = tab.bell.audio {
                                audio_bell.play(volume);
                            }
                        }
                    } else {
                        // Play sound file from config sounds directory
                        Self::play_sound_file(&sound_id, volume);
                    }
                }
                par_term_emu_core_rust::terminal::ActionResult::SendText {
                    trigger_id,
                    text,
                    delay_ms,
                } => {
                    log::info!(
                        "Trigger {} firing SendText: '{}' (delay={}ms)",
                        trigger_id,
                        text,
                        delay_ms
                    );
                    if let Some(tab) = self.tab_manager.active_tab() {
                        if delay_ms == 0 {
                            if let Err(e) = tab.terminal.try_lock().map(|t| t.write(text.as_bytes())) {
                                log::error!("SendText write failed: {:?}", e);
                            }
                        } else {
                            // Delayed send — spawn a thread to handle the delay
                            let terminal = std::sync::Arc::clone(&tab.terminal);
                            let text_owned = text;
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                                if let Ok(term) = terminal.try_lock() {
                                    if let Err(e) = term.write(text_owned.as_bytes()) {
                                        log::error!("Delayed SendText write failed: {}", e);
                                    }
                                }
                            });
                        }
                    }
                }
            }
        }
    }

    /// Play a sound file from the par-term sounds directory.
    ///
    /// Looks for the file in `~/.config/par-term/sounds/<sound_id>`.
    /// Supports WAV, OGG, FLAC, and MP3 via rodio.
    fn play_sound_file(sound_id: &str, volume: u8) {
        let sounds_dir = Self::sounds_dir();
        let path = sounds_dir.join(sound_id);

        if !path.exists() {
            log::warn!("Sound file not found: {}", path.display());
            return;
        }

        let volume_f32 = (volume as f32 / 100.0).clamp(0.0, 1.0);

        // Spawn thread to avoid blocking event loop
        std::thread::spawn(move || {
            let file = match std::fs::File::open(&path) {
                Ok(f) => f,
                Err(e) => {
                    log::error!("Failed to open sound file '{}': {}", path.display(), e);
                    return;
                }
            };

            let (_stream, handle) = match rodio::OutputStream::try_default() {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to open audio output: {}", e);
                    return;
                }
            };

            let sink = match rodio::Sink::try_new(&handle) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to create audio sink: {}", e);
                    return;
                }
            };

            let source = match rodio::Decoder::new(BufReader::new(file)) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to decode sound file '{}': {}", path.display(), e);
                    return;
                }
            };

            sink.set_volume(volume_f32);
            sink.append(source);
            sink.sleep_until_end();
        });
    }

    /// Get the sounds directory path.
    fn sounds_dir() -> PathBuf {
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join("par-term").join("sounds")
        } else {
            PathBuf::from("sounds")
        }
    }
}
```

**Step 2: Register module in `src/app/mod.rs`**

After line 24 (`mod notifications;`), add:
```rust
mod triggers;
```

**Step 3: Add call in handler.rs event loop**

In `src/app/handler.rs`, after line 697 (`self.check_bell();`), add:
```rust

        // Check for trigger action results and dispatch
        self.check_trigger_actions();
```

**Step 4: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 5: Commit**

```bash
git add src/app/triggers.rs src/app/mod.rs src/app/handler.rs
git commit -m "feat(app): add trigger action dispatch in event loop"
```

---

### Task 5: Settings UI — Automation Tab (Sidebar + Tab Shell)

**Files:**
- Modify: `src/settings_ui/sidebar.rs:10-22` (add Automation variant)
- Modify: `src/settings_ui/sidebar.rs:26-38` (add display name)
- Modify: `src/settings_ui/sidebar.rs:42-55` (add icon)
- Modify: `src/settings_ui/sidebar.rs:58-71` (add to all() array)
- Modify: `src/settings_ui/sidebar.rs:148-574` (add search keywords)
- Modify: `src/settings_ui/sidebar.rs:578-591` (add tooltip)
- Create: `src/settings_ui/automation_tab.rs`
- Modify: `src/settings_ui/mod.rs:11-24` (add module declaration)
- Modify: `src/settings_ui/mod.rs:997-1029` (add match arm)
- Modify: `src/settings_ui/mod.rs:52-207` (add UI state fields to SettingsUI)
- Modify: `src/settings_ui/mod.rs:209-298` (initialize state in new())

**Step 1: Add Automation to SettingsTab enum in sidebar.rs**

Add `Automation` variant after `Integrations` and before `Advanced` in:
- The enum definition (after line 20)
- `display_name()` match (after Integrations arm)
- `icon()` match — use `"⚡"`
- `all()` array (after Integrations)
- `tab_search_keywords()` — add automation keywords
- `tab_contents_summary()` — add summary

**Step 2: Add SettingsUI state fields for automation tab**

In `src/settings_ui/mod.rs`, add fields to `SettingsUI` struct (before `show_reset_defaults_dialog`):
```rust
    // Automation tab state
    /// Trigger being edited (index into config.triggers, None = not editing)
    pub(crate) editing_trigger_index: Option<usize>,
    /// Temp state for trigger name being edited/added
    pub(crate) temp_trigger_name: String,
    /// Temp state for trigger pattern being edited/added
    pub(crate) temp_trigger_pattern: String,
    /// Temp state for trigger actions being edited/added
    pub(crate) temp_trigger_actions: Vec<crate::config::automation::TriggerActionConfig>,
    /// Whether we're adding a new trigger (vs editing existing)
    pub(crate) adding_new_trigger: bool,
    /// Pattern validation error message
    pub(crate) trigger_pattern_error: Option<String>,
    /// Coprocess being edited (index into config.coprocesses, None = not editing)
    pub(crate) editing_coprocess_index: Option<usize>,
    /// Temp state for coprocess name
    pub(crate) temp_coprocess_name: String,
    /// Temp state for coprocess command
    pub(crate) temp_coprocess_command: String,
    /// Temp state for coprocess args (space-separated)
    pub(crate) temp_coprocess_args: String,
    /// Whether we're adding a new coprocess
    pub(crate) adding_new_coprocess: bool,
    /// Flag to request trigger resync with core registry
    pub trigger_resync_requested: bool,
```

Initialize all fields in `new()`:
```rust
            editing_trigger_index: None,
            temp_trigger_name: String::new(),
            temp_trigger_pattern: String::new(),
            temp_trigger_actions: Vec::new(),
            adding_new_trigger: false,
            trigger_pattern_error: None,
            editing_coprocess_index: None,
            temp_coprocess_name: String::new(),
            temp_coprocess_command: String::new(),
            temp_coprocess_args: String::new(),
            adding_new_coprocess: false,
            trigger_resync_requested: false,
```

**Step 3: Create `src/settings_ui/automation_tab.rs`**

```rust
//! Automation settings tab.
//!
//! Contains:
//! - Trigger management (add/edit/delete regex triggers with actions)
//! - Trigger activity monitor (recent matches)
//! - Coprocess management (add/edit/delete/start/stop coprocesses)

use crate::config::automation::{CoprocessDefConfig, TriggerActionConfig, TriggerConfig};

use super::SettingsUI;
use super::section::{SLIDER_WIDTH, collapsing_section};

const SLIDER_HEIGHT: f32 = 18.0;

/// Show the automation tab content.
pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    let query = settings.search_query.trim().to_lowercase();

    // Triggers section
    if section_matches(&query, "Triggers", &["trigger", "regex", "pattern", "match"]) {
        show_triggers_section(ui, settings, changes_this_frame);
    }

    // Coprocesses section
    if section_matches(&query, "Coprocesses", &["coprocess", "pipe", "subprocess"]) {
        show_coprocesses_section(ui, settings, changes_this_frame);
    }
}

fn section_matches(query: &str, title: &str, keywords: &[&str]) -> bool {
    if query.is_empty() {
        return true;
    }
    if title.to_lowercase().contains(query) {
        return true;
    }
    keywords.iter().any(|k| k.to_lowercase().contains(query))
}

// ============================================================================
// Triggers Section
// ============================================================================

fn show_triggers_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Triggers", "automation_triggers", true, |ui| {
        ui.label("Define regex patterns to match terminal output and trigger actions.");
        ui.add_space(4.0);

        // Show existing triggers as a list
        let mut trigger_to_remove: Option<usize> = None;
        let mut trigger_to_toggle: Option<(usize, bool)> = None;
        let mut trigger_to_edit: Option<usize> = None;

        if settings.config.triggers.is_empty() && !settings.adding_new_trigger {
            ui.label(egui::RichText::new("No triggers configured.").italics());
        } else {
            for (i, trigger) in settings.config.triggers.iter().enumerate() {
                ui.horizontal(|ui| {
                    // Enable/disable toggle
                    let mut enabled = trigger.enabled;
                    if ui.checkbox(&mut enabled, "").changed() {
                        trigger_to_toggle = Some((i, enabled));
                    }

                    // Name and pattern
                    ui.label(
                        egui::RichText::new(&trigger.name).strong(),
                    );
                    ui.label(
                        egui::RichText::new(format!("/{}/", truncate_str(&trigger.pattern, 30)))
                            .monospace()
                            .color(egui::Color32::from_rgb(150, 150, 180)),
                    );

                    // Action count badge
                    ui.label(
                        egui::RichText::new(format!("{} action(s)", trigger.actions.len()))
                            .small()
                            .color(egui::Color32::from_rgb(120, 120, 120)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Delete").clicked() {
                            trigger_to_remove = Some(i);
                        }
                        if ui.small_button("Edit").clicked() {
                            trigger_to_edit = Some(i);
                        }
                    });
                });
                ui.separator();
            }
        }

        // Apply deferred mutations
        if let Some((idx, enabled)) = trigger_to_toggle {
            settings.config.triggers[idx].enabled = enabled;
            settings.has_changes = true;
            settings.trigger_resync_requested = true;
            *changes_this_frame = true;
        }
        if let Some(idx) = trigger_to_remove {
            settings.config.triggers.remove(idx);
            settings.has_changes = true;
            settings.trigger_resync_requested = true;
            *changes_this_frame = true;
        }
        if let Some(idx) = trigger_to_edit {
            let trigger = &settings.config.triggers[idx];
            settings.editing_trigger_index = Some(idx);
            settings.temp_trigger_name = trigger.name.clone();
            settings.temp_trigger_pattern = trigger.pattern.clone();
            settings.temp_trigger_actions = trigger.actions.clone();
            settings.adding_new_trigger = false;
            settings.trigger_pattern_error = None;
        }

        ui.add_space(4.0);

        // Add new trigger button
        if !settings.adding_new_trigger && settings.editing_trigger_index.is_none() {
            if ui.button("+ Add Trigger").clicked() {
                settings.adding_new_trigger = true;
                settings.editing_trigger_index = None;
                settings.temp_trigger_name = String::new();
                settings.temp_trigger_pattern = String::new();
                settings.temp_trigger_actions = Vec::new();
                settings.trigger_pattern_error = None;
            }
        }

        // Inline edit/add form
        if settings.adding_new_trigger || settings.editing_trigger_index.is_some() {
            show_trigger_edit_form(ui, settings, changes_this_frame);
        }
    });
}

fn show_trigger_edit_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    ui.add_space(8.0);
    ui.group(|ui| {
        let title = if settings.adding_new_trigger {
            "New Trigger"
        } else {
            "Edit Trigger"
        };
        ui.heading(title);
        ui.add_space(4.0);

        // Name field
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut settings.temp_trigger_name);
        });

        // Pattern field with validation
        ui.horizontal(|ui| {
            ui.label("Pattern:");
            let response = ui.text_edit_singleline(&mut settings.temp_trigger_pattern);
            if response.changed() {
                // Validate regex
                match regex::Regex::new(&settings.temp_trigger_pattern) {
                    Ok(_) => settings.trigger_pattern_error = None,
                    Err(e) => {
                        settings.trigger_pattern_error = Some(format!("Invalid regex: {}", e))
                    }
                }
            }
        });
        if let Some(ref error) = settings.trigger_pattern_error {
            ui.label(
                egui::RichText::new(error)
                    .color(egui::Color32::from_rgb(255, 80, 80))
                    .small(),
            );
        }

        // Actions list
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Actions:").strong());

        let mut action_to_remove: Option<usize> = None;
        for (i, action) in settings.temp_trigger_actions.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("  {}. {}", i + 1, action_summary(action)));
                if ui.small_button("Remove").clicked() {
                    action_to_remove = Some(i);
                }
            });
        }
        if let Some(idx) = action_to_remove {
            settings.temp_trigger_actions.remove(idx);
        }

        // Add action dropdown
        ui.horizontal(|ui| {
            if ui.button("+ Add Action").clicked() {
                ui.memory_mut(|mem| mem.toggle_popup(ui.make_persistent_id("add_action_popup")));
            }
        });

        let popup_id = ui.make_persistent_id("add_action_popup");
        egui::popup_below_widget(ui, popup_id, &ui.button(""), egui::PopupCloseBehavior::CloseOnClick, |ui| {
            if ui.button("Highlight").clicked() {
                settings.temp_trigger_actions.push(TriggerActionConfig::Highlight {
                    fg: Some([255, 0, 0]),
                    bg: None,
                    duration_ms: 5000,
                });
            }
            if ui.button("Notify").clicked() {
                settings.temp_trigger_actions.push(TriggerActionConfig::Notify {
                    title: "Trigger Match".into(),
                    message: "$0".into(),
                });
            }
            if ui.button("Run Command").clicked() {
                settings.temp_trigger_actions.push(TriggerActionConfig::RunCommand {
                    command: String::new(),
                    args: Vec::new(),
                });
            }
            if ui.button("Play Sound").clicked() {
                settings.temp_trigger_actions.push(TriggerActionConfig::PlaySound {
                    sound_id: "bell".into(),
                    volume: 50,
                });
            }
            if ui.button("Send Text").clicked() {
                settings.temp_trigger_actions.push(TriggerActionConfig::SendText {
                    text: String::new(),
                    delay_ms: 0,
                });
            }
            if ui.button("Mark Line").clicked() {
                settings.temp_trigger_actions.push(TriggerActionConfig::MarkLine {
                    label: None,
                });
            }
            if ui.button("Set Variable").clicked() {
                settings.temp_trigger_actions.push(TriggerActionConfig::SetVariable {
                    name: String::new(),
                    value: String::new(),
                });
            }
        });

        ui.add_space(8.0);

        // Save/Cancel buttons
        ui.horizontal(|ui| {
            let can_save = !settings.temp_trigger_name.is_empty()
                && !settings.temp_trigger_pattern.is_empty()
                && settings.trigger_pattern_error.is_none();

            if ui
                .add_enabled(can_save, egui::Button::new("Save"))
                .clicked()
            {
                let trigger = TriggerConfig {
                    name: settings.temp_trigger_name.clone(),
                    pattern: settings.temp_trigger_pattern.clone(),
                    enabled: true,
                    actions: settings.temp_trigger_actions.clone(),
                };

                if settings.adding_new_trigger {
                    settings.config.triggers.push(trigger);
                } else if let Some(idx) = settings.editing_trigger_index {
                    // Preserve enabled state from existing trigger
                    let was_enabled = settings.config.triggers[idx].enabled;
                    settings.config.triggers[idx] = trigger;
                    settings.config.triggers[idx].enabled = was_enabled;
                }

                settings.has_changes = true;
                settings.trigger_resync_requested = true;
                *changes_this_frame = true;
                settings.adding_new_trigger = false;
                settings.editing_trigger_index = None;
            }

            if ui.button("Cancel").clicked() {
                settings.adding_new_trigger = false;
                settings.editing_trigger_index = None;
            }
        });
    });
}

// ============================================================================
// Coprocesses Section
// ============================================================================

fn show_coprocesses_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Coprocesses", "automation_coprocesses", true, |ui| {
        ui.label("Define subprocesses that receive terminal output and can interact with sessions.");
        ui.add_space(4.0);

        let mut coproc_to_remove: Option<usize> = None;
        let mut coproc_to_edit: Option<usize> = None;

        if settings.config.coprocesses.is_empty() && !settings.adding_new_coprocess {
            ui.label(egui::RichText::new("No coprocesses configured.").italics());
        } else {
            for (i, coproc) in settings.config.coprocesses.iter().enumerate() {
                ui.horizontal(|ui| {
                    // Status indicator
                    let status_text = if coproc.auto_start { "auto" } else { "manual" };
                    let status_color = if coproc.auto_start {
                        egui::Color32::from_rgb(80, 200, 80)
                    } else {
                        egui::Color32::from_rgb(120, 120, 120)
                    };
                    ui.label(
                        egui::RichText::new(format!("[{}]", status_text))
                            .small()
                            .color(status_color),
                    );

                    // Name and command
                    ui.label(egui::RichText::new(&coproc.name).strong());
                    ui.label(
                        egui::RichText::new(format!(
                            "$ {} {}",
                            coproc.command,
                            coproc.args.join(" ")
                        ))
                        .monospace()
                        .color(egui::Color32::from_rgb(150, 150, 180)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Delete").clicked() {
                            coproc_to_remove = Some(i);
                        }
                        if ui.small_button("Edit").clicked() {
                            coproc_to_edit = Some(i);
                        }
                    });
                });
                ui.separator();
            }
        }

        // Apply deferred mutations
        if let Some(idx) = coproc_to_remove {
            settings.config.coprocesses.remove(idx);
            settings.has_changes = true;
            *changes_this_frame = true;
        }
        if let Some(idx) = coproc_to_edit {
            let coproc = &settings.config.coprocesses[idx];
            settings.editing_coprocess_index = Some(idx);
            settings.temp_coprocess_name = coproc.name.clone();
            settings.temp_coprocess_command = coproc.command.clone();
            settings.temp_coprocess_args = coproc.args.join(" ");
            settings.adding_new_coprocess = false;
        }

        ui.add_space(4.0);

        // Add new coprocess button
        if !settings.adding_new_coprocess && settings.editing_coprocess_index.is_none() {
            if ui.button("+ Add Coprocess").clicked() {
                settings.adding_new_coprocess = true;
                settings.editing_coprocess_index = None;
                settings.temp_coprocess_name = String::new();
                settings.temp_coprocess_command = String::new();
                settings.temp_coprocess_args = String::new();
            }
        }

        // Inline edit/add form
        if settings.adding_new_coprocess || settings.editing_coprocess_index.is_some() {
            show_coprocess_edit_form(ui, settings, changes_this_frame);
        }
    });
}

fn show_coprocess_edit_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    ui.add_space(8.0);
    ui.group(|ui| {
        let title = if settings.adding_new_coprocess {
            "New Coprocess"
        } else {
            "Edit Coprocess"
        };
        ui.heading(title);
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut settings.temp_coprocess_name);
        });

        ui.horizontal(|ui| {
            ui.label("Command:");
            ui.text_edit_singleline(&mut settings.temp_coprocess_command);
        });

        ui.horizontal(|ui| {
            ui.label("Arguments:");
            ui.text_edit_singleline(&mut settings.temp_coprocess_args);
        });

        // Auto-start and copy_terminal_output will use defaults for new,
        // or preserve existing values when editing.

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let can_save = !settings.temp_coprocess_name.is_empty()
                && !settings.temp_coprocess_command.is_empty();

            if ui
                .add_enabled(can_save, egui::Button::new("Save"))
                .clicked()
            {
                let args: Vec<String> = settings
                    .temp_coprocess_args
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();

                let coproc = CoprocessDefConfig {
                    name: settings.temp_coprocess_name.clone(),
                    command: settings.temp_coprocess_command.clone(),
                    args,
                    auto_start: false,
                    copy_terminal_output: true,
                };

                if settings.adding_new_coprocess {
                    settings.config.coprocesses.push(coproc);
                } else if let Some(idx) = settings.editing_coprocess_index {
                    // Preserve auto_start and copy_terminal_output from existing
                    let existing = &settings.config.coprocesses[idx];
                    let mut updated = coproc;
                    updated.auto_start = existing.auto_start;
                    updated.copy_terminal_output = existing.copy_terminal_output;
                    settings.config.coprocesses[idx] = updated;
                }

                settings.has_changes = true;
                *changes_this_frame = true;
                settings.adding_new_coprocess = false;
                settings.editing_coprocess_index = None;
            }

            if ui.button("Cancel").clicked() {
                settings.adding_new_coprocess = false;
                settings.editing_coprocess_index = None;
            }
        });
    });
}

// ============================================================================
// Helpers
// ============================================================================

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

fn action_summary(action: &TriggerActionConfig) -> String {
    match action {
        TriggerActionConfig::Highlight { duration_ms, .. } => {
            format!("Highlight ({}ms)", duration_ms)
        }
        TriggerActionConfig::Notify { title, .. } => format!("Notify: {}", title),
        TriggerActionConfig::MarkLine { label } => {
            format!("Mark line{}", label.as_ref().map(|l| format!(": {}", l)).unwrap_or_default())
        }
        TriggerActionConfig::SetVariable { name, value } => {
            format!("Set {} = {}", name, value)
        }
        TriggerActionConfig::RunCommand { command, .. } => format!("Run: {}", command),
        TriggerActionConfig::PlaySound { sound_id, volume } => {
            format!("Play: {} ({}%)", if sound_id.is_empty() { "bell" } else { sound_id }, volume)
        }
        TriggerActionConfig::SendText { text, .. } => {
            format!("Send: {}", truncate_str(text, 20))
        }
    }
}
```

**Step 4: Register module and add match arm in mod.rs**

In `src/settings_ui/mod.rs`:
- After line 17 (`pub mod notifications_tab;`), add: `pub mod automation_tab;`
- In `show_tab_content()` at line 997, add match arm after Integrations (before Advanced):
```rust
            SettingsTab::Automation => {
                automation_tab::show(ui, self, changes_this_frame);
            }
```

**Step 5: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 6: Commit**

```bash
git add src/settings_ui/automation_tab.rs src/settings_ui/sidebar.rs src/settings_ui/mod.rs
git commit -m "feat(settings): add Automation tab with triggers and coprocesses UI"
```

---

### Task 6: Trigger Resync on Settings Save

**Files:**
- Modify: `src/app/config_updates.rs` or wherever settings save is handled (wire trigger_resync_requested)

**Step 1: Find and modify the settings save handler**

Search for where `settings_ui.has_changes` is checked and config is saved. Add trigger resync:

```rust
// After config is saved and applied, resync triggers if needed
if settings_ui.trigger_resync_requested {
    if let Some(tab) = self.tab_manager.active_tab() {
        if let Ok(term) = tab.terminal.try_lock() {
            term.sync_triggers(&self.config.triggers);
        }
    }
    settings_ui.trigger_resync_requested = false;
}
```

**Step 2: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 3: Commit**

```bash
git add src/app/config_updates.rs
git commit -m "feat(app): resync triggers when settings are saved"
```

---

### Task 7: Trigger Highlight Rendering

**Files:**
- Modify: `src/terminal/rendering.rs` (overlay trigger highlights on cells)

**Step 1: Add trigger highlight overlay**

In `get_cells_with_scrollback()`, after building cells but before returning, query trigger highlights from the core terminal and apply them:

```rust
        // Apply trigger highlights on top of cell colors
        let highlights = term.get_trigger_highlights();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        for highlight in &highlights {
            // Check if highlight is still active
            if highlight.expiry != u64::MAX && now_ms > highlight.expiry {
                continue;
            }

            // Map highlight row to screen row (accounting for scroll offset)
            let abs_row = highlight.row;
            if abs_row < start_line || abs_row >= end_line {
                continue;
            }
            let screen_row = abs_row - start_line;

            for col in highlight.col_start..highlight.col_end.min(cols) {
                let cell_idx = screen_row * cols + col;
                if cell_idx < cells.len() {
                    if let Some((r, g, b)) = highlight.fg {
                        cells[cell_idx].fg = [r, g, b, 255];
                    }
                    if let Some((r, g, b)) = highlight.bg {
                        cells[cell_idx].bg = [r, g, b, 255];
                    }
                }
            }
        }

        // Clean up expired highlights
        term.clear_expired_highlights();
```

Note: The terminal lock (`term`) is already held in this method, so we can call these methods directly. The `term` binding may need to be changed from immutable to mutable (`let mut term = terminal.lock()`) if not already.

**Step 2: Build and verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 3: Commit**

```bash
git add src/terminal/rendering.rs
git commit -m "feat(renderer): apply trigger highlight colors to cells"
```

---

### Task 8: Final Integration Test + Polish

**Files:**
- Test: `tests/automation_config_tests.rs` (already created, verify still passes)
- Modify: `Makefile` (verify test target includes new tests)

**Step 1: Run full test suite**

Run: `make test`
Expected: All tests PASS (including new automation_config_tests)

**Step 2: Run full quality checks**

Run: `make all`
Expected: Format, lint, test, and build all PASS

**Step 3: Verify the Makefile test target**

Check that `make test` runs `cargo test` which automatically discovers all test files in `tests/`. No Makefile changes should be needed since cargo test discovers tests automatically.

**Step 4: Final commit if any polish needed**

```bash
git add -A
git commit -m "chore: polish automation feature integration"
```

---

### Task 9: Create Pull Request

**Step 1: Push branch and create PR**

```bash
gh pr create --title "feat: Add triggers, trigger actions, and coprocesses UI" --body "$(cat <<'EOF'
## Summary

- Add Automation settings tab with full CRUD for regex triggers and coprocesses
- Wire trigger action dispatch (RunCommand, PlaySound, SendText) in the event loop
- Add trigger highlight rendering overlay on matched cells
- Add coprocess management with auto-start support
- Persist trigger and coprocess definitions in config.yaml

Closes #84

## Test plan

- [ ] Verify triggers section in Automation settings tab shows empty state
- [ ] Add a trigger with regex pattern, verify regex validation works
- [ ] Add Highlight action to trigger, verify matched text gets colored
- [ ] Add Notify action, verify desktop notification fires on match
- [ ] Add RunCommand action, verify command spawns on match
- [ ] Add PlaySound action with "bell", verify tone plays on match
- [ ] Add SendText action, verify text is sent to terminal on match
- [ ] Enable/disable triggers via toggle, verify behavior changes
- [ ] Delete a trigger, verify it's removed
- [ ] Add a coprocess definition, verify it saves to config
- [ ] Set auto_start on coprocess, restart app, verify it starts
- [ ] Run `make test` — all tests pass
- [ ] Run `make all` — format, lint, test, build all pass
EOF
)"
```
