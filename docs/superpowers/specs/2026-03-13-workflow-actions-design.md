# Workflow Actions Design Spec

**Date:** 2026-03-13
**Status:** Approved
**Feature:** Custom Actions — Workflow Power (Sequence, Condition, Repeat)

---

## Context

par-term's custom actions currently support 5 types: `ShellCommand`, `NewTab`, `InsertText`, `KeySequence`, `SplitPane`. Actions are triggered via keybindings or a two-stroke prefix-char mode. This design extends the system with three new action types — `Sequence`, `Condition`, and `Repeat` — to enable multi-step workflows, conditional branching, and retry loops without leaving the terminal.

**Problem:** Users with complex automation needs (e.g., build → test → deploy pipelines, retry logic, context-aware shortcuts) must cobble together shell scripts outside par-term. The custom actions system has no way to compose or branch actions.

**Intended outcome:** Users can define multi-step workflows entirely inside par-term's config, triggered by a single keybinding, with branching on exit code, output, environment, directory, or git state.

---

## Architecture

### New Action Types

Three new variants added to `CustomActionConfig` enum in `par-term-config/src/snippets.rs`.

**Important:** Every existing `CustomActionConfig` variant carries `id`, `title`, `keybinding`, `prefix_char`, `keybinding_enabled`, and `description` fields inline (there is no shared base struct). All three new variants must carry the same six fields to compile. All existing `impl CustomActionConfig` methods (`id()`, `title()`, `keybinding()`, `prefix_char()`, `normalized_prefix_char()`, `keybinding_enabled()`, `set_keybinding()`, `set_prefix_char()`, `set_keybinding_enabled()`) use exhaustive `match self { Variant { id, .. } | ... }` patterns and must be extended with new arms for each new variant.

#### `Sequence`

Runs an ordered list of steps. Each step references an existing action ID by name.

```rust
Sequence {
    // shared fields (same as every other variant)
    id: String,
    title: String,
    keybinding: Option<String>,
    prefix_char: Option<char>,
    keybinding_enabled: bool,
    description: Option<String>,
    // type-specific fields
    steps: Vec<SequenceStep>,
}

pub struct SequenceStep {
    pub action_id: String,
    pub delay_ms: u64,                      // delay BEFORE this step runs (default: 0)
    pub on_failure: SequenceStepBehavior,   // what to do if step "fails" (default: Abort)
}

pub enum SequenceStepBehavior {
    Abort,    // halt sequence and show error toast (default)
    Stop,     // halt sequence silently
    Continue, // ignore failure and proceed to next step
}
```

A step "fails" when:
- It is a `ShellCommand` with `capture_output: true` and exits with non-zero code
- It is a `Condition` whose check evaluates to false

Steps of all other types (InsertText, KeySequence, NewTab, SplitPane) always "succeed."

Sequences can reference other Sequence actions (composition is allowed; circular refs abort with error toast at execution time; see Circular Reference Detection below).

#### `Condition`

Evaluates a check and branches. Behavior differs by context:

- **Inside a Sequence:** if check is true → step succeeds (sequence continues); if false → step fails (triggering the step's `on_failure` behavior). `on_true_id`/`on_false_id` are silently ignored when inside a Sequence.
- **Standalone (direct keybinding):** if check is true → execute `on_true_id` action; if false → execute `on_false_id` action.

```rust
Condition {
    // shared fields
    id: String,
    title: String,
    keybinding: Option<String>,
    prefix_char: Option<char>,
    keybinding_enabled: bool,
    description: Option<String>,
    // type-specific fields
    check: ConditionCheck,
    on_true_id: Option<String>,    // action ID; standalone use only (ignored in Sequence)
    on_false_id: Option<String>,   // action ID; standalone use only (ignored in Sequence)
}

pub enum ConditionCheck {
    ExitCode { value: i32 },
    OutputContains { pattern: String, case_sensitive: bool },
    EnvVar { name: String, value: Option<String> },  // None = check existence only
    DirMatches { pattern: String },                  // glob on current terminal CWD
    GitBranch { pattern: String },                   // glob on current git branch
}
```

#### `Repeat`

Runs a single action ID (any action type, including Sequence) up to N times with an optional delay and early-stop conditions.

```rust
Repeat {
    // shared fields
    id: String,
    title: String,
    keybinding: Option<String>,
    prefix_char: Option<char>,
    keybinding_enabled: bool,
    description: Option<String>,
    // type-specific fields
    action_id: String,          // any action type valid (including Sequence, Condition, Repeat)
    count: u32,                 // max repetitions (1–100)
    delay_ms: u64,              // delay between repetitions (default: 0)
    stop_on_success: bool,      // stop early when action exits 0 (default: false)
    stop_on_failure: bool,      // stop early when action fails (default: false)
}
```

`Repeat::action_id` may reference any action type. Circular reference detection (Repeat referencing itself, or a chain leading back to itself) uses the same depth-first visited-set approach as Sequence.

### WorkflowContext

A transient struct (never serialized to config) that threads execution state through chained actions:

```rust
pub struct WorkflowContext {
    pub last_exit_code: Option<i32>,
    pub last_output: Option<String>,   // captured stdout + stderr (capped at 64KB)
}
```

#### Storing context in app state

A `last_workflow_context: Arc<Mutex<Option<WorkflowContext>>>` field is added to app state. This `Arc` clone is passed into every background thread that runs a `ShellCommand` with `capture_output: true`, allowing the thread to write back the result when the command completes without holding an exclusive lock on the full window state.

Standalone `Condition` actions read from `last_workflow_context` for exit_code and output_contains checks.

### ShellCommand: `capture_output` Flag

`ShellCommand` gains one new field:

```rust
capture_output: bool,   // default: false; enables context capture and exit-code-based sequencing
```

When true:
- stdout and stderr are collected into a `String` (capped at 64KB)
- On completion, the background thread writes `WorkflowContext { last_exit_code, last_output }` into the `Arc<Mutex<Option<WorkflowContext>>>` it received at spawn time

---

## Execution Flow

### Threading model

`execute_custom_action` is called from the keybinding handler on the winit event loop thread. Blocking this thread freezes rendering. The rules are:

- **Sequence with any `delay_ms > 0` OR containing a `ShellCommand` step** → dispatch to a new `std::thread::spawn` background thread. Steps that require writing to the PTY (InsertText, KeySequence) use the existing `terminal.try_write()` / `terminal.blocking_lock()` patterns from that background thread, matching how `NewTab` and `SplitPane` shell-mode already do deferred writes.
- **Sequence with all `delay_ms == 0` and no ShellCommand steps** → may execute synchronously (InsertText, KeySequence, SplitPane direct-mode are all non-blocking in practice), but spawning a thread is acceptable and simpler.
- **Repeat** → same rule as Sequence (spawn background thread when delay > 0 or the inner action is ShellCommand).
- **Condition (standalone)** → evaluates synchronously on the event loop thread (env_var, dir, git_branch reads are fast); only the resulting `on_true_id`/`on_false_id` dispatch follows the same rules.

### Step result type

To distinguish "condition evaluated false" from "action error" inside `execute_sequence`, a private helper returns a typed result:

```rust
enum StepOutcome {
    Success,
    Failure,   // condition false or ShellCommand non-zero; triggers on_failure behavior
    Abort,     // unrecoverable error (action not found, circular ref); always halts
}
```

`execute_custom_action(&mut self, action_id: &str) -> bool` retains its existing signature and `bool` return type for backward compatibility with the two existing callers (`snippet_actions.rs:71` and `keybinding_actions.rs:361`). The new logic for sequencing uses a private `execute_action_as_step()` helper that returns `StepOutcome`. `keybinding_actions.rs` does not change — it continues calling `execute_custom_action` with only an action ID.

### Execution outline

```
Keybinding → execute_custom_action(id, ctx: Option<Arc<Mutex<Option<WorkflowContext>>>>)
  │
  ├─ Sequence → spawn background thread:
  │    for each step:
  │      1. sleep delay_ms
  │      2. resolve action_id (Abort if not found)
  │      3. detect circular refs (visited set; Abort if cycle)
  │      4. execute step → StepOutcome
  │      5. match outcome + on_failure → continue / stop / abort+toast
  │
  ├─ Condition (standalone) →
  │    evaluate check (sync)
  │    dispatch on_true_id or on_false_id via execute_custom_action
  │
  ├─ Condition (in Sequence) →
  │    evaluate check → StepOutcome::Success or StepOutcome::Failure
  │
  ├─ Repeat → spawn background thread:
  │    for i in 0..count:
  │      1. execute action_id → StepOutcome
  │      2. check stop_on_success / stop_on_failure
  │      3. sleep delay_ms (skip after last iteration)
  │
  └─ ShellCommand (capture_output: true) →
       (existing background thread + new output capture + context write-back)
```

### Circular Reference Detection

At execution time (not config load), `execute_sequence` and `execute_repeat` maintain a `HashSet<String>` of visited action IDs in the current call stack. If the next `action_id` is already in the set, emit an error toast and return `StepOutcome::Abort`.

---

## Settings UI

All changes are additive to `par-term-settings-ui/src/actions_tab.rs`.

### Type integer encoding

The existing `temp_action_type: usize` uses integers 0–4. New mappings:

| Value | Type |
|-------|------|
| 0 | ShellCommand |
| 1 | NewTab |
| 2 | InsertText |
| 3 | KeySequence |
| 4 | SplitPane |
| 5 | Sequence *(new)* |
| 6 | Condition *(new)* |
| 7 | Repeat *(new)* |

### Required update sites in `actions_tab.rs`

Besides the new form branches:
- **Save button match block**: add arms for 5, 6, 7 before the `_ => unreachable!()` arm
- **Action list display match**: add arms for Sequence/Condition/Repeat to show display strings
- **`start_edit_index` population match**: add arms to populate `temp_` fields when editing an existing action
- **`temp_` field reset block (~lines 343–360)**: reset ALL 14 new `temp_` fields listed below whenever a new action is started or the form is cleared, to prevent stale state
- **`clone_action()` function (~line 774)**: has an exhaustive `match` over all 5 existing variants; must gain arms for `Sequence`, `Condition`, and `Repeat` or it will not compile
- **Type-specific form field renderer (`_ => {}` fallthrough, ~line 765)**: silently renders nothing for unknown type integers; add form branches here for types 5/6/7

### Type dropdown

Add `Sequence`, `Condition`, `Repeat` to the action type dropdown.

### Action list display

| Type | Display string |
|------|---------------|
| Sequence | `Sequence (N steps)` |
| Condition | `Condition (exit_code)` / `Condition (git_branch)` etc. |
| Repeat | `Repeat ×N` |

### Sequence form

```
Steps:
  [build_step ▼]  [delay: 0 ms]  [on_fail: abort ▼]  [↑] [↓] [✕]
  [check_clean ▼] [delay: 0 ms]  [on_fail: stop  ▼]  [↑] [↓] [✕]
  [+ Add Step]
```

Each row: action-ID dropdown (all existing actions), delay_ms integer input, on_failure dropdown (abort/stop/continue), reorder up/down arrows, delete button.

### Condition form

```
Check type:  [exit_code ▼]
Value:       [0          ]    ← changes by check type (see below)
On True:     [none ▼]         ← action picker; note: "standalone only"
On False:    [none ▼]
```

Value field rendering by check type (`temp_action_check_type: usize`, values 0–4):

| Value | Check | Input fields |
|-------|-------|-------------|
| 0 | exit_code | integer input |
| 1 | output_contains | text input + case-sensitive bool toggle |
| 2 | env_var | name text input + optional value text input + "existence only" checkbox |
| 3 | dir_matches | text input (glob hint label) |
| 4 | git_branch | text input (glob hint label) |

### Repeat form

```
Action:           [retry_deploy ▼]
Count:            [3            ]
Delay between:    [1000         ] ms
Stop on success:  [✓]
Stop on failure:  [✗]
```

### New `temp_` state fields in `SettingsUI`

```rust
// Sequence
temp_action_steps: Vec<(String, u64, SequenceStepBehavior)>,

// Condition
temp_action_check_type: usize,          // 0=exit_code, 1=output_contains, 2=env_var, 3=dir_matches, 4=git_branch
temp_action_check_value: String,        // exit_code value or pattern string
temp_action_case_sensitive: bool,       // output_contains only
temp_action_env_name: String,           // env_var: var name
temp_action_env_value: String,          // env_var: expected value (empty = existence only)
temp_action_env_check_existence: bool,  // env_var: existence-only mode
temp_action_on_true_id: String,
temp_action_on_false_id: String,

// Repeat
temp_action_repeat_action_id: String,
temp_action_repeat_count: u32,
temp_action_repeat_delay_ms: u64,
temp_action_stop_on_success: bool,
temp_action_stop_on_failure: bool,
```

All 14 fields must be reset to defaults in the `temp_` reset block (~lines 343–360 in `actions_tab.rs`) alongside the existing resets.

---

## Files to Modify

| File | Change |
|------|--------|
| `par-term-config/src/snippets.rs` | Add `Sequence`, `Condition`, `Repeat` variants (with all 6 shared fields) + supporting types; add `capture_output` to `ShellCommand`; extend all `impl CustomActionConfig` methods |
| `src/app/input_events/snippet_actions.rs` | Add `WorkflowContext`, `StepOutcome`, `execute_action_as_step()`; implement `execute_sequence`, `execute_condition`, `execute_repeat`; thread `Arc<Mutex<Option<WorkflowContext>>>` context; update ShellCommand handler for `capture_output` |
| `src/app/input_events/keybinding_actions.rs` | No signature change needed; `execute_custom_action` call at line ~361 passes only `action_id` (unchanged) |
| `src/app/window_state.rs` (or wherever app state is defined) | Add `last_workflow_context: Arc<Mutex<Option<WorkflowContext>>>` |
| `par-term-settings-ui/src/actions_tab.rs` | Add 14 temp state fields; extend type dropdown (types 5/6/7); add form branches at renderer and save; update display/start_edit/clone_action match arms; extend reset block at ~lines 343–360 |
| `docs/SNIPPETS.md` | Document new types with YAML examples |

---

## Verification

### Automated Tests

- Serialization/deserialization round-trip for each new config type (`par-term-config`)
- Unit tests for each `ConditionCheck` evaluation function (pure functions — no I/O)
- Unit test for circular reference detection in sequence/repeat execution
- Unit test for `WorkflowContext` capture and write-back via `Arc<Mutex<>>`

### Manual End-to-End Test Matrix

| Scenario | Expected result |
|----------|----------------|
| Sequence: all steps succeed | Runs to completion, no toast |
| Sequence: step 1 fails, `on_failure: abort` | Stops at step 1, error toast |
| Sequence: step 1 fails, `on_failure: continue` | All steps run regardless |
| Sequence with `delay_ms > 0` | UI remains responsive during delay |
| Condition standalone: last ShellCommand exit code 0, check `exit_code == 0` | `on_true_id` action fires |
| Condition standalone: check `env_var HOME` exists | `on_true_id` action fires |
| Condition standalone: check `git_branch "main"` on main branch | `on_true_id` action fires |
| Condition in Sequence: check fails, `on_failure: stop` | Sequence halts silently |
| Condition in Sequence: `on_true_id` set | `on_true_id` silently ignored |
| Repeat ×3, `stop_on_success: true`, 2nd run succeeds | Runs 2 times |
| Repeat ×3, `stop_on_success: false` | Always runs all 3 |
| Repeat with `delay_ms > 0` | UI remains responsive between reps |
| Sequence → Sequence composition | Inner sequence runs inline |
| Circular Sequence reference (A → B → A) | Error toast, no crash, no infinite loop |
| Repeat referencing itself | Error toast, no crash |
| Edit Sequence, then click Add New Action | New form shows clean state (no stale steps) |

### Build Gate

`make checkall` must pass (rustfmt, clippy `-D warnings`, all tests) before merge.
