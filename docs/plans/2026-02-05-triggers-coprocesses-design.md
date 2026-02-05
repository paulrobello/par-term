# Triggers, Trigger Actions & Coprocesses — Frontend Design

**Issue:** #84
**Date:** 2026-02-05
**Status:** Approved

## Summary

Add frontend UI and event wiring for the three core-ready features in par-term-emu-core-rust v0.31.0: regex triggers, trigger action dispatch, and coprocess management.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Settings UI placement | Single "Automation" tab with collapsible sections | Thematically related, avoids sidebar bloat |
| Trigger persistence | Inline in config.yaml (`triggers` array) | Follows existing pattern, single config struct |
| Coprocess persistence | Config-defined templates + runtime ad-hoc | `auto_start` flag for persistent, plus UI for one-off |
| PlaySound behavior | `"bell"` or empty = built-in tone; else filename from sounds dir | Zero-config default with customization path |

## Config Data Model

```yaml
triggers:
  - name: "Error highlight"
    pattern: "ERROR: (.+)"
    enabled: true
    actions:
      - type: highlight
        fg: [255, 0, 0]
        duration_ms: 5000
      - type: notify
        title: "Error detected"
        message: "$1"

coprocesses:
  - name: "Log watcher"
    command: "grep"
    args: ["--line-buffered", "ERROR"]
    auto_start: false
    copy_terminal_output: true
```

### Rust Types

```rust
// src/config/automation.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerConfig {
    pub name: String,
    pub pattern: String,
    pub enabled: bool,
    pub actions: Vec<TriggerActionConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerActionConfig {
    Highlight { fg: Option<[u8; 3]>, bg: Option<[u8; 3]>, duration_ms: u64 },
    Notify { title: String, message: String },
    MarkLine { label: Option<String> },
    SetVariable { name: String, value: String },
    RunCommand { command: String, args: Vec<String> },
    PlaySound { sound_id: String, volume: u8 },
    SendText { text: String, delay_ms: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoprocessDefConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub auto_start: bool,
    pub copy_terminal_output: bool,
}
```

## Event Loop Integration

`check_trigger_actions()` called each frame after `check_bell()` in `handler.rs`:

1. `term.poll_action_results()` — drain frontend-handled actions
2. Dispatch: `RunCommand` spawns detached process, `PlaySound` plays tone or file, `SendText` writes to PTY
3. `term.get_trigger_highlights()` — overlay colors during cell rendering
4. Coprocess `read_buffered()` drains stdout lines each frame

Sound files resolved from `~/.config/par-term/sounds/`. Uses existing `rodio` dependency.

## Settings UI — Automation Tab

Three collapsible sections:

### Triggers
- List with name, pattern, enabled toggle, action count
- Add/edit inline form with regex validation
- Action builder with type-specific fields (color pickers, sliders, text inputs)
- Delete with confirmation

### Trigger Activity
- Recent matches display (last 5-10)
- Shows trigger name, matched text, timestamp

### Coprocesses
- List with name, command, auto_start toggle, running status indicator
- Add/edit inline form
- Start/Stop/Delete controls

Search keywords: trigger, regex, automation, coprocess, action, pattern, command, sound

## File Changes

### New Files
- `src/settings_ui/automation_tab.rs` — Automation settings tab
- `src/config/automation.rs` — Config types + conversion impls
- `src/app/triggers.rs` — Action dispatch + sound playback

### Modified Files
- `src/config/mod.rs` — Add triggers/coprocesses fields to Config
- `src/settings_ui/mod.rs` — Register automation tab, add runtime state
- `src/settings_ui/sidebar.rs` — Add Automation variant
- `src/app/handler.rs` — Add check_trigger_actions() call
- `src/app/mod.rs` — Add mod triggers
- `src/app/window_state.rs` — Add coprocess manager, trigger sync
- `src/tab/mod.rs` — Add CoprocessManager to tab, auto-start
- `src/terminal/mod.rs` — Sync TriggerConfig into core registry
- `src/cell_renderer/render.rs` — Apply trigger highlight colors
