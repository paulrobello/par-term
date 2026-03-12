# Design: SplitPane Trigger Action + prompt_before_run Security Model

**Date:** 2026-03-11
**Status:** Approved
**Scope:** par-term automation trigger system

---

## Overview

Add a new `SplitPane` trigger action type that allows automation triggers to open a new
horizontal or vertical pane and optionally run a command in it. Alongside this, redesign
the existing `require_user_action` security flag into `prompt_before_run`, which shows an
interactive confirmation dialog instead of silently suppressing dangerous actions.

---

## Background

Automation triggers in par-term match regex patterns against PTY output and fire configured
actions (Highlight, Notify, RunCommand, SendText, etc.). Two problems motivate this change:

1. **No pane creation action** — triggers cannot open new panes or run commands in them.
2. **Broken security UX** — `require_user_action: true` (the default) silently suppresses
   `RunCommand` and `SendText` actions. Users who configure a trigger command with the
   default flag get no feedback and the action never runs. The flag name implies "wait for
   a user action", but it actually means "permanently block".

---

## Goals

- Add `SplitPane` as a first-class trigger action type.
- Replace silent suppression with an interactive prompt dialog.
- Keep backward compatibility with existing `require_user_action` configs via alias.

---

## Section 1: Data Model / Config

### New types in `par-term-config/src/automation.rs`

```rust
/// Which pane to split when a SplitPane trigger fires.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SplitTarget {
    #[default]
    Active,   // Split the currently focused pane
    Source,   // Split the pane whose PTY output matched the trigger
}

/// How to run a command in the newly created pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SplitPaneCommand {
    /// Send text to the shell with a trailing newline (shell must already be running).
    SendText { text: String },
    /// Launch the pane with this as its initial command (replaces the shell).
    InitialCommand { command: String, args: Vec<String> },
}
```

### New variant in `TriggerActionConfig`

```rust
SplitPane {
    direction: SplitDirection,           // Horizontal | Vertical
    command: Option<SplitPaneCommand>,   // None = open empty pane
    #[serde(default = "default_true")]
    focus_new_pane: bool,                // Whether to move focus to the new pane
    #[serde(default)]
    target: SplitTarget,                 // Active (default) or Source
},
```

Capture group substitution (`$1`, `$2`, …) is supported in `SplitPaneCommand::SendText.text`
and `SplitPaneCommand::InitialCommand.command`/`args`, consistent with `SendText`/`RunCommand`.

### Rename `require_user_action` → `prompt_before_run` on `TriggerConfig`

```rust
/// When true, dangerous actions (RunCommand, SendText, SplitPane) show a
/// confirmation dialog before executing instead of running automatically.
#[serde(default = "default_true", alias = "require_user_action")]
pub prompt_before_run: bool,
```

The `alias` attribute provides backward compatibility — existing YAML configs using
`require_user_action` continue to work without modification.

### Example YAML config

```yaml
triggers:
  - name: "Open logs on build"
    pattern: "Build complete"
    prompt_before_run: false
    actions:
      - type: split_pane
        direction: horizontal
        command:
          type: send_text
          text: "tail -f build.log"
        focus_new_pane: true
        target: source
```

---

## Section 2: Action Dispatch & Prompt Dialog

### Pending action queue

App state gains a `Vec<PendingTriggerAction>` and a session-level `HashSet<u64>`
(`always_allow_trigger_ids`). Each frame:

1. Core library `ActionResult::SplitPane` (and existing `RunCommand`/`SendText`) events
   are polled from the active tab.
2. If `prompt_before_run: true` **and** trigger ID is not in `always_allow_trigger_ids`,
   the action is pushed onto the pending queue.
3. If `prompt_before_run: false` **or** trigger ID is in `always_allow_trigger_ids`,
   the action executes immediately (existing rate-limit + denylist guards still apply to
   `RunCommand`/`SendText`).
4. If the pending queue is non-empty and no dialog is open, an egui modal is shown for
   the first queued action.

### Confirmation dialog

```
┌─────────────────────────────────────────────────┐
│  Trigger: "Open logs on build"                  │
│                                                 │
│  Wants to: Open a horizontal pane and run       │
│  "tail -f build.log"                            │
│                                                 │
│  [Allow]    [Always Allow]    [Deny]            │
└─────────────────────────────────────────────────┘
```

- **Allow** — executes the action once, removes it from the queue.
- **Always Allow** — adds trigger ID to `always_allow_trigger_ids`, then executes.
  Persists for the session only. To make permanent, set `prompt_before_run: false` in
  config.
- **Deny** — drops the action, removes it from the queue.

Multiple pending actions queue up; they are shown one at a time.

### SplitPane execution path

After passing the prompt gate, `ActionResult::SplitPane` is handled in
`src/app/triggers/mod.rs`:

1. Resolve `target` → active pane ID or `source_pane_id` from the `ActionResult`.
2. Call `split_pane_horizontal()` / `split_pane_vertical()` in
   `src/app/tab_ops/pane_ops.rs` on the resolved pane.
3. If split fails (e.g. `max_panes` reached): log a debug entry and drop the action.
4. On success:
   - `InitialCommand` → pass command + args to the new pane's constructor.
   - `SendText` → send `text + "\n"` to the new pane's PTY after a 50 ms delay
     (allows the shell to initialize before receiving input).
5. Apply `focus_new_pane`: move focus to new pane, or return focus to the source pane.

---

## Section 3: Settings UI

### Trigger action editor (`par-term-settings-ui/`)

New `SplitPane` entry in the action type dropdown. When selected, shows:

| Field | Control |
|---|---|
| Direction | Segmented control: Horizontal / Vertical |
| Target | Segmented control: Active Pane / Source Pane |
| Focus new pane | Checkbox (default: checked) |
| Command type | Dropdown: None / Send Text / Initial Command |
| ↳ Send Text | Text field (hint: capture groups `$1`, `$2` supported) |
| ↳ Initial Command | Command field + args field |

### `prompt_before_run` field

The existing `require_user_action` checkbox is renamed to **"Prompt before running
dangerous actions"** with an updated tooltip:

> When enabled, a confirmation dialog is shown before RunCommand, SendText, or SplitPane
> actions execute. Disable to allow the trigger to run automatically.

Search keywords in `par-term-settings-ui/src/sidebar.rs` (`tab_search_keywords()`) updated
to include `prompt`, `confirm`, `dialog`, `split pane`, `split`, `pane`.

No new settings tab is needed — `SplitPane` lives inside the existing Triggers section of
the Automation tab.

---

## Section 4: Core Library (`par-term-emu-core-rust`)

A new `ActionResult::SplitPane` variant is added, carrying:

```rust
ActionResult::SplitPane {
    trigger_id: u64,
    direction: SplitDirection,
    command: Option<SplitPaneCommand>,
    focus_new_pane: bool,
    target: SplitTarget,
    source_pane_id: PaneId,   // ID of the pane whose output matched
}
```

`source_pane_id` is populated by the core library, which already tracks which terminal
instance fired the trigger. This enables the `SplitTarget::Source` behavior in the
frontend without additional bookkeeping.

The core library maps the `TriggerActionConfig::SplitPane` variant to this `ActionResult`
when a pattern match occurs, following the same pipeline as `RunCommand` and `SendText`.

---

## Migration

| Old config | Behavior |
|---|---|
| `require_user_action: true` (default) | Deserialized as `prompt_before_run: true`; now shows dialog instead of silently blocking |
| `require_user_action: false` | Deserialized as `prompt_before_run: false`; behavior unchanged (actions run automatically) |
| No field present | Defaults to `prompt_before_run: true` |

Users who previously set `require_user_action: false` to work around silent blocking now
have their actions run the same way. Users with `require_user_action: true` (or default)
will now see a dialog — an improvement over silent failure.

---

## Files Affected

| File | Change |
|---|---|
| `par-term-config/src/automation.rs` | Add `SplitTarget`, `SplitPaneCommand`, `SplitPane` variant; rename field with alias |
| `par-term-emu-core-rust` (external) | Add `ActionResult::SplitPane`; populate `source_pane_id` |
| `src/app/triggers/mod.rs` | Add pending queue, `always_allow_trigger_ids`, prompt dialog, `SplitPane` handler |
| `src/app/tab_ops/pane_ops.rs` | Wire `SplitPane` action to existing split functions |
| `par-term-settings-ui/src/` | New `SplitPane` action editor; rename `require_user_action` UI field |
| `par-term-settings-ui/src/sidebar.rs` | Add search keywords |
| `docs/AUTOMATION.md` | Document new action type and `prompt_before_run` |

---

## Out of Scope

- Persisting "Always Allow" choices across sessions (requires config write)
- Closing panes via triggers
- Targeting a pane by ID or name (only Active or Source)
- Rate limiting for `SplitPane` (pane creation is not a denylist concern)
