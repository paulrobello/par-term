# Crash Triage

When something par-term is supervising dies, the facts are already par-term's
data — the exit status, the recent output, the entry that kept failing. Crash
triage captures those facts locally and offers them to your default AI agent
on an explicit click. Nothing is sent anywhere until you activate the offer.

## What gets captured

| Trigger | Source |
|---------|--------|
| A pane's process exits non-zero | `src/app/handler/window_state_impl/shell_exit.rs` (per-frame capture pass) |
| A plugin entry crash-loops or fails to respawn past the restart cap (5 attempts in the 5 s grace window) | `PluginHost` supervision → `drain_crash_caps` → `egui_submit.rs` |
| The previous run ended in a panic | the crash-session snapshot consumed at session restore (`src/session/crash_guard.rs`), plus the rotated debug log tail where the panic report lives |

Clean exits (`exit 0`), plugin policy stops (a `never` restart policy doing
what it says), and user-disabled plugins are **not** crashes and produce no
offer.

The previous-run panic offer rides session restore (`restore_session` in
`src/app/window_manager/window_session.rs`): that setting gates both
publishing and consuming the crash snapshot, so the triage row appears in the
first restored window.

## Capture behavior

- **Local-only.** The capture is a markdown payload file written to the temp
  directory (`par_term_triage_<pid>_<n>.md`): what crashed, when, the exit
  status or give-up reason, and the last 40 lines of output (pane tail,
  plugin stderr, or the previous run's log tail).
- **Bounded.** At most 5 live offers; each expires from the palette after 15
  minutes. Expired payload files stay on disk for manual inspection.
- **Announced, never blocking.** A passive toast reports the capture. The
  offer itself never opens a dialog or steals focus.

## The consent surface

The sole surface is a command-palette row — `Triage crash: <what> (exit N)`
— produced by `palette_entries()` in `src/crash_triage.rs`. The palette only
opens on your command, so a crash mid-typing interrupts nothing.

## Agent handoff

Activating the row launches the **default agent** (the first `agents:` entry
with `default: true` — see [Agents (Launcher)](../CONFIG_REFERENCE.md#agents-launcher)), typing a prompt that
names what crashed and points at the payload file:

```
A terminal process crashed: <label> exited with status <code>.
Diagnose it using the facts in <payload path> (process, exit status, recent output).
```

Always a plain launch — triage never arms an agent's autonomous variant.

## Configuration

None yet. All thresholds (tail lines, offer cap, TTL) are constants in
`src/crash_triage.rs`.
