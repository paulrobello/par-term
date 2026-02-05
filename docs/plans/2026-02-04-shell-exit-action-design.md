# Shell Exit Action Feature Design

**Date:** 2026-02-04
**Status:** Approved
**Effort:** Low-Medium (1-2 days)

## Overview

Replace the boolean `exit_on_shell_exit` config option with a more flexible `shell_exit_action` enum that provides Close/Keep/Restart options, matching iTerm2's functionality.

## Motivation

Currently par-term has a simple boolean:
- `exit_on_shell_exit: true` → Close the tab/pane when shell exits
- `exit_on_shell_exit: false` → Keep the pane open showing the dead shell

iTerm2 offers additional restart options that are useful for:
- Development workflows where you want shells to auto-restart
- Server monitoring where you want to see the exit message before restarting
- Long-running processes that should restart on failure

## Design

### Enum Definition

New enum in `src/config/types.rs`:

```rust
/// Action to take when the shell process exits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShellExitAction {
    /// Close the tab/pane when shell exits
    #[default]
    Close,
    /// Keep the pane open showing the terminated shell
    Keep,
    /// Immediately restart the shell
    RestartImmediately,
    /// Show a prompt message and wait for Enter before restarting
    RestartWithPrompt,
    /// Restart the shell after a 1 second delay
    RestartAfterDelay,
}
```

Serializes to YAML as: `close`, `keep`, `restart_immediately`, `restart_with_prompt`, `restart_after_delay`

### Config Changes

In `src/config/mod.rs`:

1. Replace the field:
```rust
// Old (remove):
pub exit_on_shell_exit: bool,

// New:
#[serde(default, deserialize_with = "deserialize_shell_exit_action")]
pub shell_exit_action: ShellExitAction,
```

2. Custom deserializer for backward compatibility:
```rust
fn deserialize_shell_exit_action<'de, D>(deserializer: D) -> Result<ShellExitAction, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrAction {
        Bool(bool),
        Action(ShellExitAction),
    }

    match BoolOrAction::deserialize(deserializer)? {
        BoolOrAction::Bool(true) => Ok(ShellExitAction::Close),
        BoolOrAction::Bool(false) => Ok(ShellExitAction::Keep),
        BoolOrAction::Action(action) => Ok(action),
    }
}
```

3. Update Default impl to use `ShellExitAction::Close`

### Restart State Tracking

For `RestartWithPrompt` and `RestartAfterDelay` modes, add state to track restart status:

```rust
/// State for shell restart behavior
#[derive(Debug, Clone)]
pub enum RestartState {
    /// Waiting for user to press Enter
    AwaitingInput,
    /// Waiting for delay timer (stores exit timestamp)
    AwaitingDelay(std::time::Instant),
}
```

This can be added to the pane's terminal manager or tracked at the tab level.

### Handler Logic

In `src/app/handler.rs`, refactor the exit handling:

```rust
match self.config.shell_exit_action {
    ShellExitAction::Close => {
        // Current behavior: close the pane/tab
    }
    ShellExitAction::Keep => {
        // Do nothing, leave pane showing terminated shell
    }
    ShellExitAction::RestartImmediately => {
        // Spawn new shell in the same pane immediately
    }
    ShellExitAction::RestartWithPrompt => {
        // Write "[Process exited. Press Enter to restart...]" to terminal
        // Set pane state to AwaitingInput
        // On Enter keypress in that pane, spawn new shell
    }
    ShellExitAction::RestartAfterDelay => {
        // Record exit timestamp (AwaitingDelay)
        // On next frame check, if 1 second elapsed, spawn new shell
    }
}
```

### Settings UI

In `src/settings_ui/terminal_tab.rs`, replace checkbox with dropdown:

```rust
ui.horizontal(|ui| {
    ui.label("Shell exit action:");
    egui::ComboBox::from_id_salt("shell_exit_action")
        .selected_text(match settings.config.shell_exit_action {
            ShellExitAction::Close => "Close tab/pane",
            ShellExitAction::Keep => "Keep open",
            ShellExitAction::RestartImmediately => "Restart immediately",
            ShellExitAction::RestartWithPrompt => "Restart with prompt",
            ShellExitAction::RestartAfterDelay => "Restart after 1s delay",
        })
        .show_ui(ui, |ui| {
            for action in [
                ShellExitAction::Close,
                ShellExitAction::Keep,
                ShellExitAction::RestartImmediately,
                ShellExitAction::RestartWithPrompt,
                ShellExitAction::RestartAfterDelay,
            ] {
                let label = match action {
                    ShellExitAction::Close => "Close tab/pane",
                    ShellExitAction::Keep => "Keep open",
                    ShellExitAction::RestartImmediately => "Restart immediately",
                    ShellExitAction::RestartWithPrompt => "Restart with prompt",
                    ShellExitAction::RestartAfterDelay => "Restart after 1s delay",
                };
                if ui.selectable_label(
                    settings.config.shell_exit_action == action,
                    label
                ).clicked() {
                    settings.config.shell_exit_action = action;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            }
        });
});
```

## Files to Modify

| File | Changes |
|------|---------|
| `src/config/types.rs` | Add `ShellExitAction` enum |
| `src/config/mod.rs` | Replace field, add deserializer, update Default |
| `src/app/handler.rs` | Refactor exit logic to match on enum |
| `src/terminal/mod.rs` | Add `RestartState` and restart methods |
| `src/settings_ui/terminal_tab.rs` | Replace checkbox with ComboBox |
| `MATRIX.md` | Update status from 🔶 to ✅ |

## Backward Compatibility

- Old configs with `exit_on_shell_exit: true` → `ShellExitAction::Close`
- Old configs with `exit_on_shell_exit: false` → `ShellExitAction::Keep`
- Custom deserializer handles both boolean and enum string formats
- No user action required for migration

## Testing

1. Test each exit action mode manually
2. Test backward compatibility with old boolean config
3. Test Settings UI dropdown functionality
4. Test restart with tmux tabs (should be skipped, as currently)
