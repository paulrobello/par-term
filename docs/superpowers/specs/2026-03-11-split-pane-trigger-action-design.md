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

All new types are defined in `par-term-config` (not the main crate). `SplitDirection` is
added to `par-term-config` as well (currently it only exists in `src/pane/types/common.rs`
in the main crate). The core library (`par-term-emu-core-rust`) defines its own mirrored
`SplitDirection` for use in `ActionResult::SplitPane` — no cross-crate sharing of the type
is assumed. The frontend maps between them at the dispatch boundary.

```rust
/// Split orientation for a new pane.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SplitDirection {
    Horizontal, // new pane below (stacked vertically)
    Vertical,   // new pane to the right (side by side)
}

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
    /// Send text to the shell with a trailing newline. `delay_ms` is the time to
    /// wait after pane creation before sending — default 200ms, user-configurable.
    /// This is best-effort; no shell-ready signaling is available in this release.
    SendText {
        text: String,
        #[serde(default = "default_split_send_delay")]
        delay_ms: u64,
    },
    /// Launch the pane with this as its initial command (replaces the login shell).
    InitialCommand { command: String, #[serde(default)] args: Vec<String> },
}

fn default_split_send_delay() -> u64 { 200 }
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

`SplitPane` is a **dangerous action**. `TriggerActionConfig::is_dangerous()` must return
`true` for `SplitPane { .. }` and its doc comment updated to list all three dangerous
variants: `RunCommand`, `SendText`, `SplitPane`.

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

### trigger_security map and should_suppress_dangerous_action()

`TabScriptingState::trigger_security: HashMap<u64, bool>` currently maps
`trigger_id → require_user_action` and is consumed by `should_suppress_dangerous_action()`.
After this change:

- The field is renamed to `trigger_prompt_before_run: HashMap<u64, bool>` and maps
  `trigger_id → prompt_before_run`.
- `should_suppress_dangerous_action()` is **removed entirely** and replaced by the new
  pending queue logic in `src/app/triggers/mod.rs`.
- All call sites of `should_suppress_dangerous_action()` are replaced with the pending
  queue push / `always_allow_trigger_ids` check.

### Update `warn_require_user_action_false()`

The function `warn_require_user_action_false()` in `par-term-config/src/automation.rs`,
its re-export in `par-term-config/src/lib.rs`, its call site in
`par-term-config/src/config/persistence.rs`, and the wrapping `warn_insecure_triggers()`
comment block must all be renamed to `warn_prompt_before_run_false()` and updated to
reference `prompt_before_run` in all emitted messages. The semantics change: the warning
now says "actions will execute automatically" rather than "actions are blocked".

### Trigger ID stability

Trigger IDs (`u64`) are assigned at config-load time as a sequential counter index (0, 1, 2…)
based on the position in `config.triggers`. If the config is reloaded mid-session,
`always_allow_trigger_ids` must be cleared — session approval is tied to a specific load,
not a trigger name. This is already safe since the `HashSet` lives in app state and is not
persisted.

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
          delay_ms: 200
        focus_new_pane: true
        target: source
```

---

## Section 2: Action Dispatch & Prompt Dialog

### PendingTriggerAction struct

```rust
/// A dangerous trigger action waiting for user confirmation.
struct PendingTriggerAction {
    /// ID of the trigger (index into config.triggers at load time).
    trigger_id: u64,
    /// Human-readable name of the trigger (for the dialog title).
    trigger_name: String,
    /// The action result to execute if approved.
    action: ActionResult,
    /// Human-readable description of what the action will do (for the dialog body).
    /// Pre-formatted at enqueue time; e.g. "Open a horizontal pane and run 'tail -f build.log'".
    description: String,
}
```

### Pending action queue

App state gains:
- `pending_trigger_actions: Vec<PendingTriggerAction>`
- `always_allow_trigger_ids: HashSet<u64>` (session-level "always allow" set)
- `trigger_prompt_dialog_open: bool` (prevents stacking multiple dialogs)
- `trigger_prompt_activated_frame: u64` (flicker guard; see below)

Each frame:

1. Core library `ActionResult::SplitPane` (and existing `RunCommand`/`SendText`) events
   are polled from the active tab.
2. If `prompt_before_run: true` **and** trigger ID is not in `always_allow_trigger_ids`,
   the action is pushed onto `pending_trigger_actions` as a `PendingTriggerAction`.
3. If `prompt_before_run: false` **or** trigger ID is in `always_allow_trigger_ids`,
   the action executes immediately. For `RunCommand`/`SendText`, the existing rate-limit
   and denylist guards still apply. Actions that received explicit user approval via
   the dialog ("Allow" / "Always Allow") **bypass the rate limiter** — the user just
   clicked Allow right now; rate-limiting would silently fail the action with no feedback.
   The denylist still applies even to user-approved actions (see security note below).
4. If `pending_trigger_actions` is non-empty and `!trigger_prompt_dialog_open`,
   open the dialog for the first queued action.

If the config is reloaded mid-session, `always_allow_trigger_ids` is cleared.

**Denylist for `SplitPane`:** `SplitPaneCommand::InitialCommand.command` and `args` are
checked against `check_command_denylist()` before execution (same as `RunCommand`). If
a denylist match occurs — even after the user approved via dialog — the action is dropped
and a visible error is shown (not a silent drop). `SplitPaneCommand::SendText.text` is
not denylist-checked (consistent with the existing `SendText` action).

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

- **Allow** — executes the action once, removes it from the queue, clears
  `trigger_prompt_dialog_open`.
- **Always Allow** — adds trigger ID to `always_allow_trigger_ids`, then executes.
  Persists for the session only. To make permanent, set `prompt_before_run: false` in
  config.
- **Deny** — drops the action, removes it from the queue, clears
  `trigger_prompt_dialog_open`.

Multiple pending actions queue up; they are shown one at a time. The dialog title uses
`trigger_name`; the body uses `description` (pre-formatted at enqueue time).

**SplitTarget::Source demotion in dialog text:** When `SplitTarget::Source` is demoted to
`SplitTarget::Active` (because per-pane polling is not yet wired up), the `description`
field must reflect the actual behavior: "Open a horizontal pane (in active pane) and run…"
not "Open a horizontal pane (in source pane)…". The demotion happens at enqueue time so
the description is always accurate.

**Flicker guard:** Store `trigger_prompt_activated_frame = ctx.cumulative_frame_nr()` when
the dialog opens. Guard any click-outside dismiss with
`current_frame > trigger_prompt_activated_frame` to prevent spurious immediate dismissal
on the first frame — consistent with the pattern used for tab bar context menus in
`TabBarUI`.

### SplitPane execution path

After passing the prompt gate, `ActionResult::SplitPane` is handled in
`src/app/triggers/mod.rs`:

1. **Resolve target pane:**
   - `SplitTarget::Active` → use the currently focused pane ID.
   - `SplitTarget::Source` → use `source_pane_id` from the `ActionResult` (`Option<PaneId>`).
     If `None` (per-pane polling not yet wired), degrade to `SplitTarget::Active` and emit
     a debug log entry. Demotion is reflected in the pre-enqueued `description` string.

2. **Check tmux mode:** `split_pane_horizontal()` / `split_pane_vertical()` in
   `src/app/tab_ops/pane_ops.rs` already delegate to `split_pane_via_tmux()` when tmux is
   connected. Trigger-initiated splits follow the same delegation. The `command` field is
   ignored in tmux mode (tmux manages its own shell startup); a debug log entry is emitted
   if a command was specified.

3. **Full split call chain — all signatures change to add `focus_new: bool`:**

   ```
   check_trigger_actions()                    [src/app/triggers/mod.rs]
     → split_pane_horizontal(focus_new: bool) [src/app/tab_ops/pane_ops.rs]
       → Tab::split_horizontal(focus_new)     [src/tab/pane_ops.rs]
         → Tab::split(dir, ratio, focus_new)  [src/tab/pane_ops.rs]
           → PaneManager::split(dir, ratio, focus_new) [src/pane/manager/creation.rs]
   ```

   `split_pane_vertical()` follows the same chain.

4. **If split fails** (e.g. `max_panes` reached): log a debug entry and drop the action
   silently.

5. **On success, execute command:**
   - `None` → no command; pane opens with the configured login shell.
   - `InitialCommand` → call `Pane::new_with_command()` (see below) instead of `Pane::new()`.
   - `SendText` → after `delay_ms` milliseconds, send `text + "\n"` to the new pane's PTY.
     This is best-effort; shell startup time varies. There is no shell-ready acknowledgement
     in this release.

6. **Apply focus:** `focus_new: bool` is passed through the entire call chain to
   `PaneManager::split()`. When `false`, `focused_pane_id` is left unchanged after split
   (not set to the new pane ID). When `true` (default), behavior is unchanged from today.

### PaneManager API change

`PaneManager::split()` in `src/pane/manager/creation.rs` currently always sets
`self.focused_pane_id = Some(new_id)`. Signature changes to:

```rust
pub fn split(
    &mut self,
    direction: SplitDirection,
    ratio: f32,
    focus_new: bool,
    config: &Config,
    runtime: &Runtime,
) -> Result<PaneId, PaneError>
```

When `focus_new` is `false`, the `self.focused_pane_id = Some(new_id)` line is skipped.

### Pane::new_with_command() API

`Pane::new_with_command()` uses the existing `terminal.spawn_custom_shell_with_dir(command, args, dir, env)`
pattern already available in `par-term-terminal`. No new entry-point is needed in
`par-term-terminal`. The implementation follows the same structure as `Pane::new()` but
passes `command`/`args` to `spawn_custom_shell_with_dir()` instead of the configured shell.

```rust
impl Pane {
    pub fn new_with_command(
        id: PaneId,
        config: &Config,
        runtime: &Runtime,
        working_dir: Option<PathBuf>,
        command: String,
        args: Vec<String>,
    ) -> Result<Self, PaneError>;
}
```

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
| ↳ Send Text | Text field + `delay_ms` number field (hint: capture groups `$1`, `$2` supported) |
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

A new `ActionResult::SplitPane` variant is added:

```rust
ActionResult::SplitPane {
    trigger_id: u64,
    direction: SplitDirection,           // core-local type, mirrored from config
    command: Option<SplitPaneCommand>,   // core-local type, mirrored from config
    focus_new_pane: bool,
    target: SplitTarget,                 // core-local type, mirrored from config
    /// The PaneId of the pane whose PTY output matched the trigger.
    /// `None` if per-pane trigger polling is not yet wired (current state).
    source_pane_id: Option<PaneId>,
}
```

`source_pane_id` is `Option<PaneId>` (where `PaneId = u64`). `None` is emitted when the
core cannot identify which pane fired the trigger (the current case before per-pane polling
is wired up). The frontend degrades to `SplitTarget::Active` when `None` is received.

All types in `ActionResult::SplitPane` are core-local. The frontend maps them to its local
equivalents at the dispatch boundary in `src/app/triggers/mod.rs`.

The core library maps `TriggerActionConfig::SplitPane` to this `ActionResult` when a
pattern match occurs, following the same pipeline as `RunCommand` and `SendText`.

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
| `par-term-config/src/automation.rs` | Add `SplitDirection`, `SplitTarget`, `SplitPaneCommand`, `SplitPane` variant; rename field with alias; update `is_dangerous()`; rename `warn_require_user_action_false()` |
| `par-term-config/src/lib.rs` | Update re-export of renamed warning function |
| `par-term-config/src/config/persistence.rs` | Update call site and `warn_insecure_triggers()` comment; rename to `warn_prompt_before_run_false()` |
| `par-term-emu-core-rust` (external) | Add `ActionResult::SplitPane`; `source_pane_id: Option<PaneId>` |
| `src/app/triggers/mod.rs` | Remove `should_suppress_dangerous_action()`; add pending queue, `always_allow_trigger_ids`, `PendingTriggerAction`, prompt dialog, `SplitPane` handler; clear `always_allow_trigger_ids` on config reload |
| `src/app/tab_ops/pane_ops.rs` | Add `focus_new: bool` param; handle tmux-mode SplitPane |
| `src/tab/pane_ops.rs` | Add `focus_new: bool` to `split_horizontal()`, `split_vertical()`, `split()` |
| `src/pane/manager/creation.rs` | Add `focus_new: bool` to `PaneManager::split()`; add `Pane::new_with_command()` |
| `par-term-settings-ui/src/` | New `SplitPane` action editor; rename `require_user_action` UI field; add `delay_ms` field |
| `par-term-settings-ui/src/sidebar.rs` | Add search keywords |
| `docs/AUTOMATION.md` | Document new action type and `prompt_before_run` |

---

## Out of Scope

- Persisting "Always Allow" choices across sessions (requires config write)
- Closing panes via triggers
- Targeting a pane by ID or name (only Active or Source)
- Rate limiting for `SplitPane` (pane creation is not a denylist concern)
- Full per-pane trigger polling (`SplitTarget::Source` degrades to `Active` until implemented)
- Shell-ready signaling for `SendText` (best-effort delay only in this release)
