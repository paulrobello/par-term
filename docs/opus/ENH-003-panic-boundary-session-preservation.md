# ENH-003 — Panic boundary that preserves session state

> **Impact**: high · **Effort**: medium · **Source**: AUDIT.md — cross-cutting over QA-004…QA-008, QA-014,
> QA-015; depends on QA-023

## Goal

Stop a single panic from destroying every tab, pane, scrollback buffer, and unsaved session across all windows.
Install a boundary around the event loop that, on panic, flushes session/arrangement/command-history state
through the atomic-save path, writes a diagnostic, and exits cleanly with a message the user can act on.

## Current State

`grep -rn "catch_unwind" --include='*.rs' .` returns **zero results in the entire workspace**. Combined with
the default `panic = "unwind"` profile, every reachable panic terminates the process and loses:

- all open tabs and split panes across **every** window,
- the scrollback buffer for each (default 10,000 lines),
- unsaved session state, window arrangements, and command history.

This is what escalates each index-confusion defect from an annoyance to data loss. The audit found six panics
reachable from ordinary interaction, of which the cheapest to trigger is **QA-004**: enter copy mode on a line
containing one accented character and press `/`. A user loses their whole workspace to a keystroke.

Two further considerations that shape the design:

- **`command_history` already has a shutdown-fragility bug** (QA-032, `src/command_history.rs:118-120`): it
  clears `dirty` *before* a spawn whose `Result` is discarded, and its sole caller is `WindowState::drop`. So
  the existing shutdown path is not a reliable model to copy — it is itself a finding.
- **Save paths are currently non-atomic** (QA-010): `src/session/storage.rs:37-46` truncates then writes, and
  profiles/arrangements use bare `fs::write`. Saving *during a panic* through a non-atomic path risks replacing
  good data with a truncated file — strictly worse than not saving. **QA-023's `save_yaml_atomic<T>` helper is
  therefore a hard prerequisite.**

There is a working precedent for the diagnostic half: `src/debug.rs` already owns a hardened log file
(symlink removal at `:102-108`, `O_NOFOLLOW` at `:122`, `temp_dir()` resolution at `:97`) and
`src/debug.rs:308` bridges the `log` crate into it.

## Implementation

### Step 1 — Land the prerequisite

Confirm QA-023's `save_yaml_atomic<T>` (temp-write + `rename`, mode `0600`, temp file in the same directory)
exists and is used by session, profile, and arrangement storage. **Do not start this item before that.** A
panic handler writing through a truncating path can corrupt the data it is trying to rescue.

### Step 2 — Install a panic hook for diagnostics (safe, do this first)

Independent of unwinding, set a hook early in `src/main.rs` (next to `init_log_bridge` at `:45`):

1. `std::panic::set_hook` — log the payload, location, and a backtrace via `crate::debug_error!` so it reaches
   the debug log rather than only stderr.
2. Keep the hook **allocation-light and infallible**. It runs in a broken process; anything that can panic again
   causes an abort during panic.
3. Chain to the previous hook so default behavior is preserved.

This alone is a real improvement: today a crash leaves nothing in the debug log that the log exists to capture.

### Step 3 — Add the recovery boundary

The winit event loop does not return control in a way that makes a single outer `catch_unwind` sufficient, so
scope the boundary to the per-iteration handlers:

1. Identify the entry points: `handle_window_event`
   (`src/app/handler/window_state_impl/handle_window_event.rs:14`) and `about_to_wait`
   (`src/app/handler/window_state_impl/about_to_wait.rs:13`).
2. Wrap each body in `std::panic::catch_unwind(AssertUnwindSafe(|| { … }))`.
3. On `Err`, do **not** attempt to continue rendering. Instead: mark the app as crashing, run the emergency
   save (Step 4), then exit with a non-zero status and a stderr message naming the debug-log path.

**Why not resume normally**: after a panic mid-render, GPU state, lock invariants, and terminal grid state are
all indeterminate. `AssertUnwindSafe` asserts a property that is *not* actually true here. The honest goal is
**save-then-die**, not save-and-continue. Attempting to continue would trade data loss for silent corruption,
which is worse.

### Step 4 — Emergency save

Add `WindowManager::emergency_save()`:

1. Iterate all windows; for each, capture session state via the existing `src/session/capture.rs` path.
2. Write through `save_yaml_atomic` to a **distinct** filename (e.g. `session-crash-<pid>.yaml`), not the normal
   session file. Never overwrite good state from a broken process.
3. Flush command history — and fix QA-032's premature `dirty` clear first, or the flush is unreliable.
4. Bound the whole operation with a hard timeout (~2 s) and `try_lock`-style acquisition only. **Never block on
   a mutex during panic** — the panicking thread may already hold it, which deadlocks instead of saving.
5. Note the caveat honestly: `src/session/capture.rs` has **no tests** (AUDIT.md), so exercise this path
   directly in Step 6.

### Step 5 — Offer recovery on next launch

On startup, if a `session-crash-*.yaml` exists, offer to restore it and then delete it. Reuse the existing
restore path, which is already defensive: `src/session/restore.rs:6-18` validates every cwd with `is_dir()` and
falls back to `$HOME`.

**Gotcha from AUDIT.md**: single-pane tabs must **not** call `restore_pane_layout()` — `pane_layout` is `None`
for `Leaf` nodes and `Some(...)` only for `Split` roots. Getting this wrong kills the shell on restore.

### Step 6 — Test the boundary

- Add a debug-only `--panic-test` flag (or a hidden keybinding) that panics on demand, so the path is
  exercisable without a real defect. This fits the repo's existing agent-operability flag convention
  (`--screenshot`, `--exit-after`, `--dump-state`).
- Assert a crash file is produced, is valid YAML, and restores.
- Assert the emergency save completes within its timeout while a lock is deliberately held elsewhere.

## Files to Touch

| File | Change |
|---|---|
| `src/main.rs` | install the panic hook next to `init_log_bridge` (`:45`) |
| `src/app/handler/window_state_impl/handle_window_event.rs` | wrap body in `catch_unwind` |
| `src/app/handler/window_state_impl/about_to_wait.rs` | same |
| `src/app/window_manager/mod.rs` | add `emergency_save()` |
| `src/session/storage.rs` | crash-file naming; consume `save_yaml_atomic` |
| `src/command_history.rs` | fix QA-032's premature `dirty` clear (prerequisite) |
| `src/session/restore.rs` | crash-file detection and recovery prompt |
| `src/cli.rs` (or equivalent) | `--panic-test` debug flag |
| `docs/features/SESSION_MANAGEMENT.md` | document crash recovery |

## Verification

```bash
make checkall
make build && ./target/dev-release/par-term --panic-test        # crash file written
ls "$TMPDIR"/par_term_debug.log && grep -c panic "$TMPDIR"/par_term_debug.log
./target/dev-release/par-term                                    # offers recovery, restores, deletes file
```

Then the cases that matter most:

1. **Panic while a lock is held** — confirm `emergency_save` times out and exits rather than deadlocking. This
   is the failure mode that would turn a crash into a hang, which users hate more.
2. **Split-pane restore** — crash with a 3-pane split and confirm restore reproduces the layout, honoring the
   `pane_layout = None` rule for single-pane tabs.
3. **Normal shutdown is unaffected** — no crash file left behind on a clean exit.
4. **No new clippy warnings** from `AssertUnwindSafe` usage.

## Rollback

Stage it so each piece reverts independently:

- **Step 2 (panic hook)** is safe and valuable alone — keep it even if the rest is reverted.
- **Step 3 (`catch_unwind`)** is the risky piece. If it causes problems, remove the two wrappers; behavior
  returns to today's immediate crash.
- **Steps 4–5** are additive; a leftover crash file is harmless and can be deleted.

Risks to state plainly: `AssertUnwindSafe` suppresses a real compiler check, so a panic mid-mutation can leave a
struct logically inconsistent — which is exactly why the design is save-then-exit rather than continue. And a
buggy emergency save could itself panic during panic, causing an abort; keep Step 4 minimal, allocation-light,
and wrapped in its own inner `catch_unwind`.
