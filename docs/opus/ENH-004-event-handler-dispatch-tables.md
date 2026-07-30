# ENH-004 — Dispatch tables for the Critical-complexity event handlers

> **Impact**: medium · **Effort**: large · **Source**: par-mem complexity + churn analytics (AUDIT.md
> Code Quality domain assessment)

## Goal

Convert the repo's six highest complexity × churn functions from enormous `match` ladders into dispatch tables
mapping action → small handler function, so adding a feature becomes a table entry plus an independently
testable function rather than another arm in a 100-branch match.

## Current State

par-mem ranks these as simultaneously the most complex and most-changed functions in the repository. Complexity
is cyclomatic; hotspot score is churn × complexity over a 120-day window:

| Function | File | Complexity | Churn | Hotspot |
|---|---|---:|---:|---:|
| `apply_renderer_config` | `src/app/window_manager/config_renderer_apply.rs:24` | 74 | 20 | **1480** |
| `handle_menu_action` | `src/app/window_manager/menu_actions.rs:17` | 104 | 10 | 1040 |
| `handle_key_event` | `src/app/input_events/key_handler/mod.rs:33` | 96 | 10 | 960 |
| `about_to_wait` | `src/app/handler/window_state_impl/about_to_wait.rs:13` | 86 | 11 | 946 |
| `execute_keybinding_action` | `src/app/input_events/keybinding_actions.rs:21` | 71 | 10 | 710 |
| `handle_window_event` | `src/app/handler/window_state_impl/handle_window_event.rs:14` | 83 | 6 | 498 |

All six are rated `risk: Critical` by complexity. Three consequences the audit observed directly:

1. **They are the repo's worst merge-conflict points.** Every feature adds an arm, and churn of 10–20 over four
   months on a single function means concurrent work collides constantly.
2. **They are effectively untestable.** Reaching arm N requires constructing full `WindowState`, and none of
   these functions has direct test coverage.
3. **They hide defects in plain sight.** DOC-008 found two keyboard shortcuts whose documented behavior is
   silently pre-empted by an earlier arm — `Cmd+,` is intercepted at
   `src/app/window_state/keyboard_handlers.rs:48-51` before reaching the cursor-style cycler at
   `key_handler/utility.rs:165-169`. In a 96-branch match, arm-ordering bugs are invisible. A dispatch table
   makes a duplicate key a **detectable collision** instead.

Related findings this interacts with: **QA-017** (six sites using an arbitrary `HashMap` window instead of the
focused one — several are in these files) and **ARC-005/ENH-006** (`WindowState`'s 39 fields / 94 `impl` blocks,
which is why these handlers can reach everything).

## Implementation

**Do one function per PR.** These are large, high-churn files; a six-function change would be unreviewable and
would conflict with everything. Recommended order — easiest and most mechanical first, to establish the pattern:

1. `execute_keybinding_action` (71) — cleanest action enum, most table-shaped
2. `handle_menu_action` (104) — same shape, larger
3. `apply_renderer_config` (74) — highest hotspot, but config-driven rather than action-driven; see Step 5
4. `handle_key_event` (96) — ordering-sensitive; see Step 4
5. `about_to_wait` (86) and `handle_window_event` (83) — winit-driven; least table-shaped, do last

### Step 1 — Characterize before changing

For the target function:

```bash
# enumerate the arms and confirm the discriminant
grep -n "^\s*\(Action\|MenuAction\|KeybindingAction\)::" src/app/window_manager/menu_actions.rs | wc -l
```

Establish whether arms are **independent** (pure dispatch on a discriminant) or **ordered** (later arms depend
on earlier ones not matching, or on fallthrough). This determines whether a table is safe at all — see Step 4.

### Step 2 — Extract each arm to a named function

Mechanical and compiler-verified. For each arm, move the body to
`fn handle_<action_name>(state: &mut WindowState, …) -> ActionResult`. Keep signatures uniform so they are
table-compatible. Do **not** change behavior in this step — extraction only, so the diff is reviewable and any
regression is bisectable.

Run `make checkall` after extraction and before tabulating. If it is green, extraction was faithful.

### Step 3 — Introduce the table

```rust
type ActionHandler = fn(&mut WindowState, &ActionContext) -> ActionResult;

static HANDLERS: &[(KeybindingAction, ActionHandler)] = &[
    (KeybindingAction::NewTab, handle_new_tab),
    (KeybindingAction::CloseTab, handle_close_tab),
    // …
];
```

Use a `match` on the discriminant returning the fn pointer if the action type is not hashable, or a
`phf`/`OnceLock<HashMap>` if it is. **Prefer the exhaustive `match`-returning-fn-pointer form**: it keeps the
compiler's exhaustiveness check, which is the main safety property protecting against a missed action. A
`HashMap` silently returns `None` for an unmapped action; a non-exhaustive `match` fails to build.

Add a test asserting every enum variant has a handler — trivial with a `strum`-style variant iterator or a
hand-maintained `all()` list (several enums in this repo already have one).

### Step 4 — Handle ordering-sensitive dispatch carefully

`handle_key_event` is **not** pure dispatch. It has precedence layers: platform modifiers, copy-mode capture,
utility shortcuts, keybinding actions, then raw PTY encoding. DOC-008's `Cmd+,` finding is precisely an
ordering interaction.

Model the layers explicitly rather than flattening them:

```rust
const LAYERS: &[fn(&mut WindowState, &KeyEvent) -> Handled] = &[
    try_copy_mode, try_platform_shortcut, try_utility_shortcut, try_keybinding, encode_to_pty,
];
```

Each layer returns `Handled::Yes | Handled::No`; the driver stops at the first `Yes`. This preserves precedence
*and* makes it readable and testable — which is what would have surfaced DOC-008 as a test failure. **Do not
reorder layers while refactoring**; capture current precedence exactly, land it, then fix DOC-008's two genuine
conflicts as a separate, deliberate change.

### Step 5 — `apply_renderer_config` is a different shape

Its 74 complexity is a long sequence of "if this config field changed, apply it", not action dispatch. The
right pattern is a table of `(field accessor, applier)` pairs — a change-detector list — or a `ConfigDelta`
struct computed once. This is also the natural place to fix **QA-019** (font-size change enumerating system
fonts twice via `block_on`), since both concern renderer rebuild.

### Step 6 — Add per-handler tests

The payoff. Each extracted handler now takes a narrow input, so test them directly. Prioritize the ones
DOC-008 showed are wrong and the ones QA-017 touches (`cli_timer.rs:54,89`,
`app_handler_impl.rs:369,406`, `settings_actions.rs:126,132`, `config_propagation.rs:263`) — use
`get_focused_window_id()` (`src/app/window_manager/mod.rs:159`) in the extracted handlers rather than
propagating the arbitrary-`HashMap`-entry bug into new functions.

## Files to Touch

Per PR, one of:

| File | Lines | Note |
|---|---:|---|
| `src/app/input_events/keybinding_actions.rs` | — | start here |
| `src/app/window_manager/menu_actions.rs` | — | also carries QA-013 (`:187`) and QA-017 |
| `src/app/window_manager/config_renderer_apply.rs` | — | highest hotspot; pair with QA-019 |
| `src/app/input_events/key_handler/mod.rs` | — | also carries QA-013 (`:315,539,596,623`); layer model |
| `src/app/handler/window_state_impl/about_to_wait.rs` | — | also QA-012's drain site, and ENH-003's boundary |
| `src/app/handler/window_state_impl/handle_window_event.rs` | — | also ENH-003's boundary |

## Verification

```bash
make checkall                    # after EVERY step, especially after extraction
make lint-all
cargo test --workspace
```

Behavioral equivalence is the acceptance criterion, and it needs more than a green suite because these paths are
barely covered today:

```bash
# complexity actually dropped
# (par-mem) find_most_complex_functions — the target should leave the Critical tier
make build && ./target/dev-release/par-term --screenshot /tmp/a.png --exit-after 5
```

Manually exercise every keybinding and menu item in the refactored surface. Add the exhaustiveness test from
Step 3 — it is the guard that a table refactor did not silently drop an action, which is this change's main
risk.

## Rollback

One function per PR is the rollback strategy: revert a single commit to restore that handler's `match`. Do not
batch.

Two real risks. **A dropped action** — a table entry omitted for an enum variant, producing a silently dead
feature; the Step 3 exhaustiveness test plus an exhaustive `match` (not a `HashMap`) is the mitigation, and this
is why the `match` form is preferred. **Changed precedence** in `handle_key_event`, which would break shortcuts
subtly rather than loudly; capture current precedence in tests *before* refactoring, and keep the DOC-008 fixes
out of this change.
