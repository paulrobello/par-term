# SplitPane Trigger Action Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `SplitPane` trigger action type so automation triggers can open a new horizontal/vertical pane and optionally run a command in it, replacing the silent `require_user_action` suppression with an interactive `prompt_before_run` confirmation dialog.

**Architecture:** New types added to `par-term-config`; core library (`par-term-emu-core-rust`) extended via local path override with `TriggerAction::SplitPane` and `ActionResult::SplitPane`; existing pane split call chain extended with `focus_new: bool`; pending action queue + egui dialog added to `TriggerState`.

**Tech Stack:** Rust 2024 edition, serde/serde_yaml, egui, wgpu, tokio, par-term-emu-core-rust (local fork during development)

**Spec:** `docs/superpowers/specs/2026-03-11-split-pane-trigger-action-design.md`

---

## Chunk 1: Config Types

### Task 1: Add SplitDirection, SplitTarget, SplitPaneCommand, SplitPane variant

**Files:**
- Modify: `par-term-config/src/automation.rs`
- Test: `par-term-config/src/automation.rs` (inline `#[cfg(test)]` block)

- [ ] **Add new types before `TriggerActionConfig`** in `par-term-config/src/automation.rs` (after line 131, before the `RestartPolicy` section):

```rust
/// Split orientation for a new pane created by a trigger action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSplitDirection {
    Horizontal, // new pane below (panes stacked vertically)
    Vertical,   // new pane to the right (side by side)
}

/// Which pane to split when a SplitPane trigger fires.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSplitTarget {
    #[default]
    Active,   // split the currently focused pane
    Source,   // split the pane whose PTY output matched (degrades to Active for now)
}

/// How to run a command in the newly created pane.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SplitPaneCommand {
    /// Send text to the shell with a trailing newline. Best-effort; shell must be running.
    SendText {
        text: String,
        #[serde(default = "default_split_send_delay")]
        delay_ms: u64,
    },
    /// Launch the pane with this command instead of the login shell.
    InitialCommand {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

fn default_split_send_delay() -> u64 {
    200
}
```

- [ ] **Add `SplitPane` variant to `TriggerActionConfig`** (after the `Prettify` variant closing brace):

```rust
    /// Open a new pane (horizontal or vertical split) and optionally run a command in it.
    SplitPane {
        direction: TriggerSplitDirection,
        #[serde(default)]
        command: Option<SplitPaneCommand>,
        #[serde(default = "crate::defaults::bool_true")]
        focus_new_pane: bool,
        #[serde(default)]
        target: TriggerSplitTarget,
    },
```

- [ ] **Update `is_dangerous()`** to include `SplitPane` and update the doc comment:

```rust
    /// Returns true if this action is considered dangerous when triggered by
    /// passive terminal output (i.e., without explicit user interaction).
    ///
    /// Dangerous actions: `RunCommand`, `SendText`, `SplitPane`
    /// Safe actions: `Highlight`, `Notify`, `MarkLine`, `SetVariable`, `PlaySound`, `Prettify`
    pub fn is_dangerous(&self) -> bool {
        matches!(
            self,
            Self::RunCommand { .. } | Self::SendText { .. } | Self::SplitPane { .. }
        )
    }
```

- [ ] **Add serde deserialization tests** at the bottom of `par-term-config/src/automation.rs`:

```rust
#[cfg(test)]
mod split_pane_tests {
    use super::*;

    #[test]
    fn test_split_pane_config_deserialize_send_text() {
        let yaml = r#"
type: split_pane
direction: horizontal
command:
  type: send_text
  text: "tail -f build.log"
  delay_ms: 300
focus_new_pane: true
target: active
"#;
        let action: TriggerActionConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(action, TriggerActionConfig::SplitPane { .. }));
        assert!(action.is_dangerous());
    }

    #[test]
    fn test_split_pane_config_deserialize_initial_command() {
        let yaml = r#"
type: split_pane
direction: vertical
command:
  type: initial_command
  command: htop
  args: []
focus_new_pane: false
target: source
"#;
        let action: TriggerActionConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(
            action,
            TriggerActionConfig::SplitPane {
                direction: TriggerSplitDirection::Vertical,
                focus_new_pane: false,
                target: TriggerSplitTarget::Source,
                ..
            }
        ));
    }

    #[test]
    fn test_split_pane_defaults() {
        let yaml = r#"
type: split_pane
direction: horizontal
"#;
        let action: TriggerActionConfig = serde_yaml::from_str(yaml).unwrap();
        if let TriggerActionConfig::SplitPane {
            command,
            focus_new_pane,
            target,
            ..
        } = action
        {
            assert!(command.is_none());
            assert!(focus_new_pane); // defaults true
            assert_eq!(target, TriggerSplitTarget::Active); // defaults Active
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn test_send_text_default_delay() {
        let yaml = r#"type: send_text
text: "hello"
"#;
        let cmd: SplitPaneCommand = serde_yaml::from_str(yaml).unwrap();
        if let SplitPaneCommand::SendText { delay_ms, .. } = cmd {
            assert_eq!(delay_ms, 200);
        }
    }
}
```

- [ ] **Run tests:**
```bash
cd /Users/probello/Repos/par-term
cargo test -p par-term-config split_pane -- --nocapture
```
Expected: 4 passing tests.

- [ ] **Commit:**
```bash
git add par-term-config/src/automation.rs
git commit -m "feat(config): add SplitPane trigger action config types"
```

---

### Task 2: Rename require_user_action → prompt_before_run + update warning function

**Files:**
- Modify: `par-term-config/src/automation.rs`
- Modify: `par-term-config/src/lib.rs`
- Modify: `par-term-config/src/config/persistence.rs`
- Modify: `par-term-terminal/src/terminal/triggers.rs` — rename field access + update doc comment
- Modify: `src/tab/scripting_state.rs` — rename `trigger_security` → `trigger_prompt_before_run`
- Modify: `src/tab/constructors.rs` — update field initializer
- Modify: `src/app/window_manager/config_propagation.rs` — update field assignment

- [ ] **Rename field in `TriggerConfig`** in `par-term-config/src/automation.rs`. Replace:

```rust
    #[serde(default = "crate::defaults::bool_true")]
    pub require_user_action: bool,
```

With:

```rust
    /// When true (default), dangerous actions show a confirmation dialog before executing.
    /// When false, they execute automatically (with rate-limit + denylist guards still applied).
    ///
    /// Previously named `require_user_action`. The old name is accepted as an alias for
    /// backward compatibility with existing config files.
    #[serde(
        default = "crate::defaults::bool_true",
        alias = "require_user_action"
    )]
    pub prompt_before_run: bool,
```

- [ ] **Rename `warn_require_user_action_false()`** in `par-term-config/src/automation.rs`:

```rust
/// Emit a security warning when a trigger is configured with `prompt_before_run: false`.
///
/// Called during config load for any trigger with `prompt_before_run: false` that contains
/// dangerous actions. With `prompt_before_run: false`, dangerous actions execute automatically
/// without user confirmation; only the rate-limiter and denylist provide protection.
pub fn warn_prompt_before_run_false(trigger_name: &str) {
    eprintln!(
        "[par-term SECURITY WARNING] Trigger '{trigger_name}' has `prompt_before_run: false`.\n\
         This allows terminal output to directly trigger RunCommand/SendText/SplitPane actions\n\
         without confirmation. The command denylist provides only limited protection.\n\
         Only use this setting if you fully trust the configured commands and environment.\n\
         Recommendation: set `prompt_before_run: true` (the default) to require confirmation."
    );
}
```

- [ ] **Update re-export in `par-term-config/src/lib.rs`** — change the `warn_require_user_action_false` export to `warn_prompt_before_run_false`:

```rust
pub use automation::{
    CoprocessDefConfig, RestartPolicy, SplitPaneCommand, TriggerActionConfig, TriggerConfig,
    TriggerRateLimiter, TriggerSplitDirection, TriggerSplitTarget,
    check_command_denylist, warn_prompt_before_run_false,
};
```

- [ ] **Update call site in `par-term-config/src/config/persistence.rs`** — find `warn_insecure_triggers()` and update:

```rust
    pub(crate) fn warn_insecure_triggers(&mut self) {
        self.insecure_trigger_names.clear();
        for trigger in &self.triggers {
            if !trigger.prompt_before_run && trigger.actions.iter().any(|a| a.is_dangerous()) {
                crate::automation::warn_prompt_before_run_false(&trigger.name);
                self.insecure_trigger_names.push(trigger.name.clone());
            }
        }
    }
```

- [ ] **Update `par-term-terminal/src/terminal/triggers.rs`** — replace the Rust field access (line 48: `trigger_config.require_user_action`) and update the function doc comment. Note: `#[serde(alias)]` only affects YAML deserialization; the Rust struct field name itself changes to `prompt_before_run`, so this file must be updated or it will fail to compile:

```rust
    /// Sync trigger configs from Config into the core TriggerRegistry.
    ///
    /// Returns a map of `trigger_id -> prompt_before_run` for each
    /// successfully registered trigger, so the frontend can decide whether
    /// to show a confirmation dialog for dangerous actions.
    pub fn sync_triggers(
        &self,
        triggers: &[par_term_config::TriggerConfig],
    ) -> std::collections::HashMap<u64, bool> {
```

And inside the loop:
```rust
                    security_map.insert(id, trigger_config.prompt_before_run);
                    log::info!(
                        "Trigger '{}' registered (id={}, prompt_before_run={})",
                        trigger_config.name,
                        id,
                        trigger_config.prompt_before_run,
                    );
```

- [ ] **Rename `trigger_security` → `trigger_prompt_before_run` in `src/tab/scripting_state.rs`**:

```rust
    /// Security metadata: maps trigger_id -> prompt_before_run flag.
    /// When true, dangerous actions show a confirmation dialog instead of executing automatically.
    pub(crate) trigger_prompt_before_run: std::collections::HashMap<u64, bool>,
```

Update the `Default` impl and any field initializers in that file.

- [ ] **Update `src/tab/constructors.rs`** — change the struct field initializer:
```rust
            scripting: TabScriptingState {
                coprocess_ids,
                trigger_prompt_before_run: trigger_security,  // local var still named trigger_security from sync_triggers return
                ..TabScriptingState::default()
            },
```

- [ ] **Update `src/app/window_manager/config_propagation.rs`**:
```rust
                    tab.scripting.trigger_prompt_before_run = term.sync_triggers(&config.triggers);
```

- [ ] **Run `cargo check --workspace` to catch any remaining `require_user_action` or `trigger_security` references:**
```bash
cd /Users/probello/Repos/par-term
cargo check --workspace 2>&1 | grep "require_user_action\|trigger_security\|error"
```

- [ ] **Run tests:**
```bash
make test
```

- [ ] **Commit:**
```bash
git add par-term-config/src/automation.rs par-term-config/src/lib.rs \
        par-term-config/src/config/persistence.rs \
        par-term-terminal/src/terminal/triggers.rs \
        src/tab/scripting_state.rs src/tab/constructors.rs \
        src/app/window_manager/config_propagation.rs
git commit -m "feat(config): rename require_user_action to prompt_before_run with backward-compat alias"
```

---

## Chunk 2: Core Library — ActionResult::SplitPane

### Task 3: Set up local path override for par-term-emu-core-rust

**Files:**
- Modify: `/Users/probello/Repos/par-term/Cargo.toml`

- [ ] **Verify the local repo exists before touching Cargo.toml:**
```bash
ls /Users/probello/Repos/par-term-emu-core-rust/Cargo.toml
```
Expected: file found. If not found, clone or locate the repo before proceeding.

- [ ] **Add `[patch.crates-io]` section** to the bottom of `Cargo.toml` (or update it if it exists):

```toml
[patch.crates-io]
par-term-emu-core-rust = { path = "../par-term-emu-core-rust" }
```

- [ ] **Verify it resolves before committing:**
```bash
cd /Users/probello/Repos/par-term
cargo metadata --no-deps 2>&1 | grep "par-term-emu-core-rust" | head -3
```
Expected: should show the local path version being used. If this fails, do NOT commit — investigate the path.

- [ ] **Commit only after successful verification:**
```bash
git add Cargo.toml
git commit -m "chore: patch par-term-emu-core-rust to local path for SplitPane development"
```

---

### Task 4: Add TriggerAction::SplitPane and ActionResult::SplitPane to core library

**Files:**
- Modify: `/Users/probello/Repos/par-term-emu-core-rust/src/terminal/trigger.rs`

- [ ] **Add new types before `TriggerAction`** enum in `trigger.rs`:

```rust
/// Split direction for SplitPane trigger actions (mirrors par-term-config::TriggerSplitDirection)
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerSplitDirection {
    Horizontal,
    Vertical,
}

/// Which pane to split (mirrors par-term-config::TriggerSplitTarget)
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TriggerSplitTarget {
    #[default]
    Active,
    Source,
}

/// Command to run in the new pane (mirrors par-term-config::SplitPaneCommand)
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerSplitCommand {
    SendText { text: String, delay_ms: u64 },
    InitialCommand { command: String, args: Vec<String> },
}
```

- [ ] **Add `SplitPane` variant to `TriggerAction`** enum (after `SendText`):

```rust
    /// Open a new pane (frontend-handled, emitted as ActionResult::SplitPane)
    SplitPane {
        direction: TriggerSplitDirection,
        command: Option<TriggerSplitCommand>,
        focus_new_pane: bool,
        target: TriggerSplitTarget,
    },
```

- [ ] **Add `SplitPane` variant to `ActionResult`** enum:

```rust
    /// Frontend should open a new split pane and optionally run a command
    SplitPane {
        trigger_id: TriggerId,
        direction: TriggerSplitDirection,
        command: Option<TriggerSplitCommand>,
        focus_new_pane: bool,
        target: TriggerSplitTarget,
        /// Pane ID that generated the match. None = per-pane polling not yet available.
        source_pane_id: Option<u64>,
    },
```

- [ ] **Add match arm in the trigger execution loop** (where `TriggerAction::RunCommand` is handled). Find the match on `TriggerAction` and add after the `SendText` arm:

```rust
                TriggerAction::SplitPane {
                    direction,
                    command,
                    focus_new_pane,
                    target,
                } => {
                    self.trigger_action_results.push(ActionResult::SplitPane {
                        trigger_id: trigger.id,
                        direction,
                        command,
                        focus_new_pane,
                        target,
                        source_pane_id: None, // per-pane polling not yet wired
                    });
                }
```

- [ ] **Verify `StopPropagation` arm exists** — it is already handled in the trigger execution loop; do not add a duplicate. Just confirm the exhaustiveness check passes with the new `SplitPane` arm added.

- [ ] **Verify the core library compiles:**
```bash
cd /Users/probello/Repos/par-term-emu-core-rust
cargo check
```

- [ ] **Verify par-term picks up the changes:**
```bash
cd /Users/probello/Repos/par-term
cargo check --workspace 2>&1 | grep "error" | head -20
```

- [ ] **Commit (in core library repo — DO NOT push):**
```bash
cd /Users/probello/Repos/par-term-emu-core-rust
git add src/terminal/trigger.rs
git commit -m "feat(trigger): add TriggerAction::SplitPane and ActionResult::SplitPane"
# Do NOT push — keep changes local until par-term-emu-core-rust is ready for a new release
```

---

### Task 5: Wire SplitPane through to_core_action() and sync_triggers

**Files:**
- Modify: `par-term-config/src/automation.rs` (`to_core_action()`)
- Test: `par-term-config/src/automation.rs` (inline tests)

- [ ] **Add `SplitPane` arm to `to_core_action()`** in `par-term-config/src/automation.rs`. In the `match self.clone()` block, add after the `Prettify` arm. Use fully-qualified paths throughout to avoid name collisions with the config-side types that share the same base name:

```rust
            Self::SplitPane {
                direction,
                command,
                focus_new_pane,
                target,
            } => {
                // Use fully-qualified core paths to avoid shadowing the config-side types.
                let core_direction = match direction {
                    crate::automation::TriggerSplitDirection::Horizontal => {
                        par_term_emu_core_rust::terminal::TriggerSplitDirection::Horizontal
                    }
                    crate::automation::TriggerSplitDirection::Vertical => {
                        par_term_emu_core_rust::terminal::TriggerSplitDirection::Vertical
                    }
                };
                let core_command = command.map(|c| match c {
                    crate::automation::SplitPaneCommand::SendText { text, delay_ms } => {
                        par_term_emu_core_rust::terminal::TriggerSplitCommand::SendText {
                            text,
                            delay_ms,
                        }
                    }
                    crate::automation::SplitPaneCommand::InitialCommand { command, args } => {
                        par_term_emu_core_rust::terminal::TriggerSplitCommand::InitialCommand {
                            command,
                            args,
                        }
                    }
                });
                let core_target = match target {
                    crate::automation::TriggerSplitTarget::Active => {
                        par_term_emu_core_rust::terminal::TriggerSplitTarget::Active
                    }
                    crate::automation::TriggerSplitTarget::Source => {
                        par_term_emu_core_rust::terminal::TriggerSplitTarget::Source
                    }
                };
                TriggerAction::SplitPane {
                    direction: core_direction,
                    command: core_command,
                    focus_new_pane,
                    target: core_target,
                }
            }
```

- [ ] **Run `cargo check --workspace`:**
```bash
cd /Users/probello/Repos/par-term
cargo check --workspace 2>&1 | grep "error"
```

- [ ] **Run tests:**
```bash
make test
```

- [ ] **Commit:**
```bash
git add par-term-config/src/automation.rs
git commit -m "feat(config): wire SplitPane through to_core_action()"
```

---

## Chunk 3: Pane API Changes

### Task 6: Add focus_new: bool to PaneManager::split() call chain

**Files:**
- Modify: `src/pane/manager/creation.rs`
- Modify: `src/tab/pane_ops.rs`
- Modify: `src/app/tab_ops/pane_ops.rs`
- Test: (compile test — split is used in keyboard handlers, verified by `cargo check`)

- [ ] **Update `PaneManager::split()`** in `src/pane/manager/creation.rs`. Change signature and body:

```rust
    pub fn split(
        &mut self,
        direction: SplitDirection,
        focus_new: bool,     // <-- new param
        config: &Config,
        runtime: Arc<Runtime>,
    ) -> Result<Option<PaneId>> {
```

At the end of the function, replace:
```rust
        // Focus the new pane
        self.focused_pane_id = Some(new_id);
```
With:
```rust
        // Focus the new pane only if requested
        if focus_new {
            self.focused_pane_id = Some(new_id);
        }
```

- [ ] **Update `Tab::split()`** (private method) in `src/tab/pane_ops.rs`. Add `focus_new: bool` parameter:

```rust
    fn split(
        &mut self,
        direction: SplitDirection,
        focus_new: bool,     // <-- new param
        config: &Config,
        runtime: Arc<Runtime>,
        dpi_scale: f32,
    ) -> anyhow::Result<Option<crate::pane::PaneId>> {
```

Pass it through to `pm.split()`:
```rust
            let new_pane_id = pm.split(direction, focus_new, config, Arc::clone(&runtime))?;
```

- [ ] **Update `Tab::split_horizontal()` and `Tab::split_vertical()`** to add `focus_new: bool` and pass through:

```rust
    pub fn split_horizontal(
        &mut self,
        focus_new: bool,
        config: &Config,
        runtime: Arc<Runtime>,
        dpi_scale: f32,
    ) -> anyhow::Result<Option<crate::pane::PaneId>> {
        self.split(SplitDirection::Horizontal, focus_new, config, runtime, dpi_scale)
    }

    pub fn split_vertical(
        &mut self,
        focus_new: bool,
        config: &Config,
        runtime: Arc<Runtime>,
        dpi_scale: f32,
    ) -> anyhow::Result<Option<crate::pane::PaneId>> {
        self.split(SplitDirection::Vertical, focus_new, config, runtime, dpi_scale)
    }
```

- [ ] **Update callers in `src/app/tab_ops/pane_ops.rs`**. Both `split_pane_horizontal()` and `split_pane_vertical()` call `tab.split_horizontal(...)` / `tab.split_vertical(...)`. Add `true` as the first argument (keyboard-initiated splits always focus the new pane):

```rust
            match tab.split_horizontal(true, &self.config, Arc::clone(&self.runtime), dpi_scale) {
```
```rust
            match tab.split_vertical(true, &self.config, Arc::clone(&self.runtime), dpi_scale) {
```

- [ ] **Refactor `split_pane_horizontal()` and `split_pane_vertical()` in `src/app/tab_ops/pane_ops.rs`** — extract the shared bounds-setup + split logic into a new private helper to avoid duplication and to support trigger-initiated splits:

```rust
    /// Shared implementation for trigger- and keyboard-initiated pane splits.
    ///
    /// Handles renderer bounds query, tmux delegation, and the split call.
    /// Returns the new pane ID on success, None on failure.
    fn split_pane_direction(
        &mut self,
        direction: crate::pane::SplitDirection,
        focus_new: bool,
    ) -> Option<crate::pane::PaneId> {
        // Extract the ~70-line bounds-setup logic shared between split_pane_horizontal
        // and split_pane_vertical here. Call tab.split_horizontal/split_vertical with
        // the appropriate focus_new value.
        // ...
    }
```

Then rewrite `split_pane_horizontal()` and `split_pane_vertical()` to call this helper with `focus_new: true`, and update `execute_trigger_split_pane()` (Task 10) to call it with the trigger's `focus_new_pane` value.

> **Note:** This refactor is required because `split_pane_horizontal()` and `split_pane_vertical()` each contain ~70 lines of renderer-query, bounds-calculation, and status-bar-height logic that cannot simply be "wrapped" — it must be shared. Read both methods in full before extracting.

- [ ] **Search for any other callers and fix them:**
```bash
grep -rn "split_horizontal\|split_vertical\|pm\.split(" /Users/probello/Repos/par-term/src/ | grep -v "pane_ops\|test\|\.rs:#"
```

- [ ] **Verify compilation:**
```bash
cd /Users/probello/Repos/par-term
cargo check --workspace 2>&1 | grep "error"
```

- [ ] **Commit:**
```bash
git add src/pane/manager/creation.rs src/tab/pane_ops.rs src/app/tab_ops/pane_ops.rs
git commit -m "feat(pane): add focus_new param to split call chain; extract split_pane_direction helper"
```

---

### Task 7: Add Pane::new_with_command()

**Files:**
- Modify: `src/pane/types/pane.rs`
- Test: (compile + manual smoke test via `make run-debug`)

- [ ] **Read `Pane::new()` in full** (`src/pane/types/pane.rs` lines 85–end of `new()`) to understand the exact initialization sequence before writing anything.

- [ ] **Add `Pane::new_with_command()`** immediately after `Pane::new()`. The implementation must be a complete copy of `new()` with a single substitution: instead of calling `get_shell_command(config)` to build the shell path, use the caller-supplied `command` and `args` directly. Do NOT call `Self::new()` and kill/respawn — that approach has no valid API on `Arc<RwLock<TerminalManager>>` and would cause a brief flicker.

  The signature:
  ```rust
      /// Create a pane that launches `command args` instead of the configured login shell.
      ///
      /// Identical to `Pane::new()` except the PTY is started with the given command.
      /// All other fields (scroll state, cache, refresh task, etc.) are the same as `new()`.
      pub fn new_with_command(
          id: PaneId,
          config: &Config,
          runtime: Arc<Runtime>,
          working_directory: Option<String>,
          command: String,
          args: Vec<String>,
      ) -> anyhow::Result<Self> {
          // Copy the full body of Pane::new() here, then replace the
          // `get_shell_command(config)` / `apply_login_shell_flag` block with:
          //
          //   let shell_env = build_shell_env(config.shell_env.as_ref());
          //   let work_dir = working_directory.as_deref();
          //   terminal.spawn_custom_shell_with_dir(
          //       &command,
          //       Some(args.as_slice()),
          //       work_dir,
          //       shell_env,
          //   )?;
          //
          // Everything else in new() stays identical.
          todo!("copy Pane::new() body, substitute shell spawn with command/args")
      }
  ```

- [ ] **Verify compilation:**
```bash
cargo check -p par-term 2>&1 | grep "error"
```

- [ ] **Commit:**
```bash
git add src/pane/types/pane.rs
git commit -m "feat(pane): add Pane::new_with_command() for trigger-initiated pane spawning"
```

---

## Chunk 4: Trigger Dispatch Refactor

### Task 8: Add PendingTriggerAction struct and state fields

**Files:**
- Modify: `src/app/window_state/trigger_state.rs`
- Modify: `src/app/triggers/mod.rs`
- Modify: `src/tab/scripting_state.rs`

- [ ] **Add new types and fields to `TriggerState`** in `src/app/window_state/trigger_state.rs`:

```rust
use par_term_emu_core_rust::terminal::ActionResult;
use std::collections::HashSet;

/// A dangerous trigger action awaiting user confirmation in the prompt dialog.
pub(crate) struct PendingTriggerAction {
    /// Trigger index/ID (assigned at config-load time)
    pub(crate) trigger_id: u64,
    /// Human-readable trigger name (for dialog title)
    pub(crate) trigger_name: String,
    /// The action to execute if approved
    pub(crate) action: ActionResult,
    /// Pre-formatted description of the action (for dialog body)
    pub(crate) description: String,
}

/// State for managing terminal triggers and their spawned processes.
#[derive(Default)]
pub(crate) struct TriggerState {
    /// PIDs of spawned trigger commands with their spawn time, for resource management
    pub(crate) trigger_spawned_processes: std::collections::HashMap<u32, std::time::Instant>,
    /// Compiled regex cache for prettify trigger patterns
    pub(crate) trigger_regex_cache: std::collections::HashMap<String, regex::Regex>,
    /// Queue of dangerous actions waiting for user confirmation
    pub(crate) pending_trigger_actions: Vec<PendingTriggerAction>,
    /// Trigger IDs the user has approved for auto-execution this session
    pub(crate) always_allow_trigger_ids: HashSet<u64>,
    /// Whether the confirmation dialog is currently open (prevents stacking)
    pub(crate) trigger_prompt_dialog_open: bool,
    /// Frame number when the dialog opened (flicker guard). None = dialog not open.
    /// Set to Some(ctx.cumulative_frame_nr()) when the dialog is first shown.
    /// Dismissed only when current_frame > activated_frame.
    pub(crate) trigger_prompt_activated_frame: Option<u64>,
}
```

- [ ] **Rename `trigger_security` → `trigger_prompt_before_run`** in `src/tab/scripting_state.rs`:

```rust
    /// Security metadata: maps trigger_id -> prompt_before_run flag.
    /// When true, dangerous actions show a confirmation dialog instead of executing automatically.
    pub(crate) trigger_prompt_before_run: std::collections::HashMap<u64, bool>,
```

- [ ] **Fix all references to `trigger_security`** across the codebase:
```bash
grep -rn "trigger_security" /Users/probello/Repos/par-term/src/
```
Update each occurrence to `trigger_prompt_before_run`.

- [ ] **Update `src/app/triggers/mod.rs`** — change the snapshot variable name and remove `should_suppress_dangerous_action()`:
  - Rename the local `trigger_security` variable to `trigger_prompt_before_run`
  - Delete the `should_suppress_dangerous_action()` function (lines 72-98)
  - Update the module-level doc comment to describe the new dialog-based model

- [ ] **Verify compilation:**
```bash
cargo check --workspace 2>&1 | grep "error"
```

- [ ] **Commit:**
```bash
git add src/app/window_state/trigger_state.rs src/tab/scripting_state.rs \
        src/tab/constructors.rs src/app/triggers/mod.rs \
        src/app/window_manager/config_propagation.rs
git commit -m "feat(triggers): add PendingTriggerAction queue, rename trigger_security field"
```

---

### Task 9: Replace silent suppression with pending queue for RunCommand/SendText

**Files:**
- Modify: `src/app/triggers/mod.rs`

- [ ] **Replace the security check for `RunCommand`** — instead of calling `should_suppress_dangerous_action()`, enqueue if `prompt_before_run` is true:

```rust
                ActionResult::RunCommand {
                    trigger_id,
                    command,
                    args,
                } => {
                    let command = expand_tilde(&command);
                    let args: Vec<String> = args.iter().map(|a| expand_tilde(a)).collect();

                    // If prompt_before_run and not in always_allow list, enqueue for dialog
                    let prompt = trigger_prompt_before_run
                        .get(&trigger_id)
                        .copied()
                        .unwrap_or(true);
                    if prompt
                        && !self
                            .trigger_state
                            .always_allow_trigger_ids
                            .contains(&trigger_id)
                    {
                        let trigger_name = trigger_names
                            .get(&trigger_id)
                            .cloned()
                            .unwrap_or_else(|| format!("trigger #{}", trigger_id));
                        let description = format!(
                            "Run command: {} {}",
                            command,
                            args.join(" ")
                        );
                        self.trigger_state
                            .pending_trigger_actions
                            .push(PendingTriggerAction {
                                trigger_id,
                                trigger_name,
                                action: ActionResult::RunCommand { trigger_id, command, args },
                                description,
                            });
                        continue;
                    }

                    // Security check: command denylist (always applied)
                    if let Some(denied_pattern) = check_command_denylist(&command, &args) {
                        log::error!(/* ... */);
                        continue;
                    }

                    // Rate limiting (skipped for dialog-approved actions — user just clicked Allow)
                    if !approved_this_frame.contains(&trigger_id) {
                        if let Some(tab) = self.tab_manager.active_tab_mut()
                            && !tab.scripting.trigger_rate_limiter.check_and_update(trigger_id)
                        {
                            log::warn!(/* rate-limited */);
                            continue;
                        }
                    }

                    // ... rest of RunCommand spawn logic unchanged
```

> **Note:** `approved_this_frame: HashSet<u64>` is a local variable initialized at the top of `check_trigger_actions()` containing trigger IDs just approved via the dialog. `trigger_names: HashMap<u64, String>` must also be fetched from tab data (from the trigger registry or config). See below.

- [ ] **Add `trigger_names` snapshot** — after the existing `trigger_prompt_before_run` snapshot in `check_trigger_actions()`:

```rust
        // Snapshot trigger names for use in dialog descriptions
        let trigger_names: std::collections::HashMap<u64, String> =
            if let Some(t) = self.tab_manager.active_tab()
                && let Ok(term) = t.terminal.try_read()
            {
                term.list_triggers()
                    .iter()
                    .map(|t| (t.id, t.name.clone()))
                    .collect()
            } else {
                std::collections::HashMap::new()
            };
```

- [ ] **Apply the same pattern to `SendText`** — enqueue instead of suppress when `prompt_before_run` is true.

- [ ] **Commit:**
```bash
git add src/app/triggers/mod.rs
git commit -m "feat(triggers): replace silent suppression with pending queue for RunCommand/SendText"
```

---

### Task 10: Add ActionResult::SplitPane handler

**Files:**
- Modify: `src/app/triggers/mod.rs`
- Modify: `src/app/tab_ops/pane_ops.rs`

- [ ] **Add `SplitPane` arm to the `for action in action_results` loop** in `check_trigger_actions()`:

```rust
                ActionResult::SplitPane {
                    trigger_id,
                    direction,
                    command,
                    focus_new_pane,
                    target,
                    source_pane_id: _,  // not yet used (per-pane polling not wired)
                } => {
                    // Enqueue for dialog if prompt_before_run
                    let prompt = trigger_prompt_before_run
                        .get(&trigger_id)
                        .copied()
                        .unwrap_or(true);
                    if prompt
                        && !self
                            .trigger_state
                            .always_allow_trigger_ids
                            .contains(&trigger_id)
                    {
                        let trigger_name = trigger_names
                            .get(&trigger_id)
                            .cloned()
                            .unwrap_or_else(|| format!("trigger #{}", trigger_id));
                        let dir_str = match direction {
                            par_term_emu_core_rust::terminal::TriggerSplitDirection::Horizontal => "horizontal",
                            par_term_emu_core_rust::terminal::TriggerSplitDirection::Vertical => "vertical",
                        };
                        let cmd_str = match &command {
                            Some(par_term_emu_core_rust::terminal::TriggerSplitCommand::SendText { text, .. }) => {
                                format!(" and run '{}'", text)
                            }
                            Some(par_term_emu_core_rust::terminal::TriggerSplitCommand::InitialCommand { command, .. }) => {
                                format!(" and launch '{}'", command)
                            }
                            None => String::new(),
                        };
                        let description = format!(
                            "Open a {} pane (in active pane){}",
                            dir_str, cmd_str
                        );
                        self.trigger_state
                            .pending_trigger_actions
                            .push(PendingTriggerAction {
                                trigger_id,
                                trigger_name,
                                action: ActionResult::SplitPane {
                                    trigger_id,
                                    direction,
                                    command,
                                    focus_new_pane,
                                    target,
                                    source_pane_id: None,
                                },
                                description,
                            });
                        continue;
                    }

                    // Execute split
                    self.execute_trigger_split_pane(trigger_id, direction, command, focus_new_pane);
                }
```

- [ ] **Add `execute_trigger_split_pane()` method** to `WindowState` (in `src/app/tab_ops/pane_ops.rs` or a new `src/app/triggers/split_pane.rs`):

```rust
    /// Execute a trigger-initiated SplitPane action.
    pub(crate) fn execute_trigger_split_pane(
        &mut self,
        trigger_id: u64,
        direction: par_term_emu_core_rust::terminal::TriggerSplitDirection,
        command: Option<par_term_emu_core_rust::terminal::TriggerSplitCommand>,
        focus_new_pane: bool,
    ) {
        use par_term_emu_core_rust::terminal::TriggerSplitDirection;
        use par_term_emu_core_rust::terminal::TriggerSplitCommand;
        use crate::pane::SplitDirection;

        // In tmux mode, delegate to tmux (command is ignored)
        let is_vertical = matches!(direction, TriggerSplitDirection::Vertical);
        if self.is_tmux_connected() && self.split_pane_via_tmux(is_vertical) {
            if command.is_some() {
                crate::debug_log!(
                    "TRIGGER",
                    "SplitPane trigger {} in tmux mode: command ignored",
                    trigger_id
                );
            }
            return;
        }

        // Determine native direction
        let native_direction = match direction {
            TriggerSplitDirection::Horizontal => SplitDirection::Horizontal,
            TriggerSplitDirection::Vertical => SplitDirection::Vertical,
        };

        // For InitialCommand: use split + new_with_command
        // For SendText / None: use standard split
        let new_pane_id = match &command {
            Some(TriggerSplitCommand::InitialCommand { command: cmd, args }) => {
                self.split_pane_with_command(
                    native_direction,
                    focus_new_pane,
                    cmd.clone(),
                    args.clone(),
                )
            }
            _ => {
                // Standard split (no custom command)
                self.split_pane_direction(native_direction, focus_new_pane)
            }
        };

        // For SendText: send text to the new pane after delay
        if let (Some(pane_id), Some(TriggerSplitCommand::SendText { text, delay_ms })) =
            (new_pane_id, command)
        {
            crate::debug_log!(
                "TRIGGER",
                "SplitPane trigger {}: sending text to new pane {} after {}ms",
                trigger_id,
                pane_id,
                delay_ms
            );
            self.send_text_to_pane_after_delay(pane_id, format!("{}\n", text), delay_ms);
        }
    }
```

> **Note:** You will need to add helper methods `split_pane_direction()`, `split_pane_with_command()`, and `send_text_to_pane_after_delay()` to `WindowState`. These are thin wrappers around existing functions. `split_pane_direction()` factors out the shared bounds-calculation code from `split_pane_horizontal()` / `split_pane_vertical()` by accepting a `SplitDirection` parameter.

- [ ] **Verify compilation:**
```bash
cargo check --workspace 2>&1 | grep "error"
```

- [ ] **Commit:**
```bash
git add src/app/triggers/mod.rs src/app/tab_ops/pane_ops.rs
git commit -m "feat(triggers): add SplitPane action handler"
```

---

### Task 11: Add confirmation dialog + config reload cleanup

**Files:**
- Modify: `src/app/triggers/mod.rs` (dialog rendering in `check_trigger_actions()`)
- Modify: `src/app/window_manager/config_propagation.rs` (clear `always_allow_trigger_ids` on reload)

- [ ] **Add dialog rendering** at the end of `check_trigger_actions()`, after the action loop:

```rust
        // Show confirmation dialog for the first pending dangerous action
        if !self.trigger_state.pending_trigger_actions.is_empty()
            && !self.trigger_state.trigger_prompt_dialog_open
        {
            self.trigger_state.trigger_prompt_dialog_open = true;
            // activated_frame is set when the dialog is rendered (in show_trigger_prompt_dialog)
        }
```

- [ ] **Add `show_trigger_prompt_dialog()` method** to `WindowState`, called from `render_egui_ui()` or the egui render pass. The dialog renders using `egui::Window`:

```rust
    pub(crate) fn show_trigger_prompt_dialog(&mut self, ctx: &egui::Context) {
        if !self.trigger_state.trigger_prompt_dialog_open {
            return;
        }
        let first = match self.trigger_state.pending_trigger_actions.first() {
            Some(a) => a,
            None => {
                self.trigger_state.trigger_prompt_dialog_open = false;
                return;
            }
        };

        let current_frame = ctx.cumulative_frame_nr();
        // Set activated_frame on first render of this dialog (flicker guard).
        // Use Option<u64> so frame 0 is a valid guard value.
        let activated_frame = *self
            .trigger_state
            .trigger_prompt_activated_frame
            .get_or_insert(current_frame);

        let title = first.trigger_name.clone();
        let description = first.description.clone();
        let trigger_id = first.trigger_id;

        let mut allow = false;
        let mut always_allow = false;
        let mut deny = false;

        egui::Window::new(format!("Trigger: {}", title))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Wants to: {}", description));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Allow").clicked() { allow = true; }
                    if ui.button("Always Allow").clicked() { always_allow = true; }
                    if ui.button("Deny").clicked() { deny = true; }
                });
            });

        if allow || always_allow {
            if always_allow {
                self.trigger_state.always_allow_trigger_ids.insert(trigger_id);
            }
            // Execute the action (with dialog approval bypassing rate limiter)
            if !self.trigger_state.pending_trigger_actions.is_empty() {
                let pending = self.trigger_state.pending_trigger_actions.remove(0);
                self.execute_approved_trigger_action(pending.action, trigger_id);
            }
            self.trigger_state.trigger_prompt_dialog_open = false;
            self.trigger_state.trigger_prompt_activated_frame = None;
        } else if deny {
            if !self.trigger_state.pending_trigger_actions.is_empty() {
                self.trigger_state.pending_trigger_actions.remove(0);
            }
            self.trigger_state.trigger_prompt_dialog_open = false;
            self.trigger_state.trigger_prompt_activated_frame = None;
        }
    }
```

> **Note:** `execute_approved_trigger_action()` dispatches the action from the approved `ActionResult`, bypassing the rate limiter but still applying the denylist. Find the egui render call site (`render_egui_ui()` or equivalent) and call `show_trigger_prompt_dialog(ctx)` there.

- [ ] **Clear `always_allow_trigger_ids` on config reload** in `src/app/window_manager/config_propagation.rs` — after the trigger resync loop:

```rust
            // Clear session-level "always allow" approvals since trigger IDs may have changed
            window_state.trigger_state.always_allow_trigger_ids.clear();
```

- [ ] **Build and smoke-test:**
```bash
make build
# Run with debug and verify dialog appears for a trigger with prompt_before_run: true
make run-debug
```

- [ ] **Commit:**
```bash
git add src/app/triggers/mod.rs src/app/window_manager/config_propagation.rs
git commit -m "feat(triggers): add prompt_before_run confirmation dialog with always-allow session support"
```

---

## Chunk 5: Settings UI + Documentation

### Task 12: Rename require_user_action → prompt_before_run in Settings UI

**Files:**
- Modify: `par-term-settings-ui/src/` (find the trigger action editor file)
- Modify: `par-term-settings-ui/src/sidebar.rs`

- [ ] **Find the trigger settings UI file:**
```bash
grep -rln "require_user_action\|Require user action" /Users/probello/Repos/par-term/par-term-settings-ui/src/
```

- [ ] **Rename the checkbox label and field references** — replace any display string containing "Require user action" with "Prompt before running dangerous actions" and update the tooltip to:
  > "When enabled, a confirmation dialog is shown before RunCommand, SendText, or SplitPane actions execute. Disable to allow the trigger to run automatically."

- [ ] **Update field access** — change any `trigger.require_user_action` to `trigger.prompt_before_run`.

- [ ] **Add keywords to `tab_search_keywords()`** in `par-term-settings-ui/src/sidebar.rs` — add to the Automation tab keywords:
  ```rust
  "prompt", "confirm", "dialog", "split pane", "split", "pane",
  ```

- [ ] **Verify compilation:**
```bash
cargo check -p par-term-settings-ui 2>&1 | grep "error"
```

- [ ] **Commit:**
```bash
git add par-term-settings-ui/src/
git commit -m "ui(settings): rename require_user_action to prompt_before_run in triggers UI"
```

---

### Task 13: Add SplitPane action editor in Settings UI

**Files:**
- Modify: `par-term-settings-ui/src/` (the trigger action editor component)

- [ ] **Read the existing action editor** to understand the current action type dropdown and sub-panels:
```bash
grep -n "RunCommand\|SendText\|action_type\|TriggerActionConfig" /Users/probello/Repos/par-term/par-term-settings-ui/src/*.rs | head -30
```

- [ ] **Add `SplitPane` to the action type dropdown** — wherever the dropdown/combobox of action types is defined, add `"Split Pane"` as a selectable option.

- [ ] **Add the SplitPane sub-panel** — when the action type is `SplitPane`, show:
  - **Direction**: two radio buttons or a `ComboBox` — "Horizontal (new pane below)" / "Vertical (new pane to the right)"
  - **Target**: two radio buttons — "Active Pane" / "Source Pane (when available)"
  - **Focus new pane**: `ui.checkbox(&mut action.focus_new_pane, "Focus new pane")`
  - **Command type**: `ComboBox` — "None", "Send Text", "Initial Command"
  - If "Send Text" selected:
    - `ui.text_edit_singleline(&mut text)` with hint "Text to send (supports $1 $2 captures)"
    - `ui.add(egui::DragValue::new(&mut delay_ms).suffix(" ms").clamp_range(0..=5000))` labeled "Delay before sending"
  - If "Initial Command" selected:
    - `ui.text_edit_singleline(&mut command)` labeled "Command"
    - `ui.text_edit_singleline(&mut args_str)` labeled "Arguments (space-separated)"

- [ ] **Set `settings.has_changes = true` and `*changes_this_frame = true`** on any change, following the existing pattern in other action editors.

- [ ] **Verify compilation:**
```bash
cargo check -p par-term-settings-ui 2>&1 | grep "error"
```

- [ ] **Commit:**
```bash
git add par-term-settings-ui/src/
git commit -m "ui(settings): add SplitPane action editor in trigger settings"
```

---

### Task 14: Update docs/AUTOMATION.md

**Files:**
- Modify: `docs/AUTOMATION.md`

- [ ] **Read the current AUTOMATION.md** to understand its structure.

- [ ] **Add `SplitPane` action type documentation** in the trigger actions section. Include:
  - Description of the action
  - All fields with types and defaults
  - Full example YAML (copy from spec)
  - Note about `SplitTarget::Source` degrading to `Active` until per-pane polling is implemented

- [ ] **Update the `require_user_action` section** to `prompt_before_run`:
  - Explain the new behavior (dialog instead of silent block)
  - Note backward compatibility with `require_user_action` alias
  - Document the dialog buttons (Allow / Always Allow / Deny)
  - Note that "Always Allow" is session-only

- [ ] **Commit:**
```bash
git add docs/AUTOMATION.md
git commit -m "docs: update AUTOMATION.md with SplitPane action and prompt_before_run"
```

---

## Final Verification

- [ ] **Run full CI suite:**
```bash
make ci
```
Expected: format, lint, tests all pass.

- [ ] **Smoke test the full feature:**
  1. `make run-debug` to open par-term with debug logging
  2. Add a test trigger to `~/.config/par-term/config.yaml`:
     ```yaml
     triggers:
       - name: "Test Split"
         pattern: "TRIGGER_TEST"
         prompt_before_run: true
         actions:
           - type: split_pane
             direction: horizontal
             command:
               type: send_text
               text: "echo hello from trigger"
             focus_new_pane: true
             target: active
     ```
  3. In the terminal, type `echo TRIGGER_TEST` and press Enter
  4. Verify the confirmation dialog appears
  5. Click **Allow** — verify a new horizontal pane opens and `echo hello from trigger` runs
  6. Repeat — click **Always Allow** — verify subsequent matches execute without dialog
  7. Set `prompt_before_run: false` — verify trigger executes automatically

- [ ] **Test backward compat:** Use `require_user_action: false` in config — verify it still works (no parse error, treated as `prompt_before_run: false`).

- [ ] **Final commit:**
```bash
git status  # should be clean
```
