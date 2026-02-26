# Security Model: par-term Scripting Protocol

## Overview

Script subprocesses communicate with the terminal via a JSON-line protocol
defined in `src/protocol.rs`. Scripts read `ScriptEvent` objects from stdin
and write `ScriptCommand` objects to stdout. Most commands (`Log`, `SetPanel`,
`ClearPanel`, `Notify`, `SetBadge`, `SetVariable`) are low-risk display
operations. Two commands — **`WriteText`** and **`RunCommand`** — have the
potential to cause significant harm and require a security gate before they
are implemented.

---

## Commands Requiring Security Review

### WriteText

`WriteText` injects an arbitrary string into the PTY as if the user had typed
it. This is equivalent to the existing `SendText` trigger action and is
inherently dangerous because:

- It can execute arbitrary shell commands in the active terminal session.
- It can inject terminal escape sequences that reposition the cursor, corrupt
  output, or exfiltrate data.
- It can be used to modify shell history, override aliases, or change
  environment variables.

### RunCommand

`RunCommand` spawns an arbitrary external process as a direct child of the
terminal emulator. This is distinct from `WriteText` in that it does **not**
go through the PTY shell — it is a raw `std::process::Command::spawn()` call.
This is dangerous because:

- It can execute any binary visible on `$PATH` or reachable by absolute path.
- It bypasses the active shell's environment and job control.
- It can silently exfiltrate data, open network connections, or modify files
  without any visible output in the terminal.
- It can re-invoke the terminal emulator itself to spawn additional processes.

---

## Threat Model

Scripts are user-configured subprocesses launched from `ScriptConfig` entries
in `~/.config/par-term/config.yaml`. The script binary is therefore implicitly
trusted (it was placed there by the user). However, the trust must be bounded
because:

1. **Supply-chain attacks**: A malicious package could replace a trusted script
   with one that emits `RunCommand` payloads to exfiltrate credentials.

2. **Injection through event data**: A malicious terminal sequence (from `cat`
   of a hostile file) could produce a `TriggerMatched` or `CwdChanged` event
   whose payload is forwarded to the script via `ScriptEvent`. A poorly written
   script could then reflect that data back as a `RunCommand` or `WriteText`
   payload. This is analogous to the terminal-injection risk documented in the
   trigger security model (`src/app/triggers.rs`).

3. **Compromised scripts**: A script may be modified after initial deployment
   (e.g., via a writable file on a shared filesystem). The terminal cannot
   detect that the trusted binary has been replaced.

4. **Unbounded execution rate**: Without rate limiting, a script could emit
   thousands of `RunCommand` or `WriteText` commands per second as a
   denial-of-service attack against the host system.

### Attack Surface Summary

| Vector | Risk | Mitigation |
|--------|------|------------|
| Malicious terminal output reflected by script | Terminal injection / arbitrary command | `require_consent` flag + denylist |
| Compromised script binary | Arbitrary command execution | User consent prompt on first execution + file-hash check (future) |
| Rapid-fire command emission | DoS / resource exhaustion | Per-script rate limiting |
| Embedded escape sequences in `WriteText` | Terminal escape injection | Strip or validate ANSI/VT sequences |

---

## Authorization Requirements

### When WriteText May Be Used

`WriteText` execution requires **all** of the following conditions to be met:

1. **Explicit opt-in in `ScriptConfig`**: The script's configuration entry must
   include `allow_write_text: true`. This field must default to `false`. A
   script that does not declare this permission must never be allowed to write
   to the PTY, even if it emits a `WriteText` command.

2. **VT sequence stripping**: Before the text is written to the PTY, any ANSI
   escape sequences (CSI, OSC, DCS, APC, PM, SOS sequences and raw ESC-prefixed
   sequences) must be stripped or the entire `WriteText` command must be
   rejected with a log warning.

3. **Rate limiting**: No more than `write_text_rate_limit` writes per second
   (suggested default: 10/s). Commands that exceed the rate limit are dropped
   with a warning log.

### When RunCommand May Be Used

`RunCommand` execution requires **all** of the following conditions to be met:

1. **Explicit opt-in in `ScriptConfig`**: The script's configuration entry must
   include `allow_run_command: true`. This field must default to `false`.

2. **Command denylist check**: The command string must be checked against
   `par_term_config::check_command_denylist()` (the same denylist used for
   trigger `RunCommand` actions). Any match must block execution and emit a
   warning log. The denylist covers:
   - Destructive file operations (`rm -rf /`, `dd if=`, `mkfs.*`)
   - Shell evaluation (`eval`, `exec`)
   - Credential/key exfiltration (`.ssh/id_*`, `.gnupg/`, `ssh-add`)
   - Pipe-to-shell patterns (`| bash`, `| sh`)
   - System manipulation (`chmod 777`, `chown root`, `passwd`, `sudoers`)

3. **Optional user consent prompt** (recommended for `allow_run_command: true`
   scripts): The first time a script emits `RunCommand`, the terminal should
   display a one-time confirmation dialog: "Script '<name>' wants to run
   '<command>'. Allow?" with options: Allow Once / Allow Always / Deny. The
   "Allow Always" choice is persisted per (script-name, command-hash) pair in
   the session.

4. **Rate limiting**: No more than `run_command_rate_limit` executions per
   second (suggested default: 1/s). Commands that exceed the rate limit are
   dropped with a warning log.

5. **No shell expansion**: The command string must be split using shell-word
   tokenisation (not passed to `/bin/sh -c`) to prevent metacharacter
   injection. Use `shlex`-style splitting or a fixed-token split on whitespace
   for the initial implementation.

---

## Proposed ScriptConfig Permission Fields

When `WriteText` and `RunCommand` are implemented, the following fields must be
added to `ScriptConfig` in `par-term-config/src/scripting.rs`:

```rust
/// Allow this script to write text to the active PTY.
/// Defaults to false. Must be explicitly set to true.
#[serde(default)]
pub allow_write_text: bool,

/// Allow this script to spawn external commands via RunCommand.
/// Defaults to false. Must be explicitly set to true.
#[serde(default)]
pub allow_run_command: bool,

/// Maximum WriteText commands per second (0 = use default of 10).
#[serde(default)]
pub write_text_rate_limit: u32,

/// Maximum RunCommand executions per second (0 = use default of 1).
#[serde(default)]
pub run_command_rate_limit: u32,
```

---

## Dispatcher Security Gate (window_manager.rs)

The command dispatcher in `src/app/window_manager.rs` (the match on
`ScriptCommand` variants) must enforce the following checks before `WriteText`
and `RunCommand` are implemented:

```
ScriptCommand::WriteText { text } =>
    1. Reject if !script_config.allow_write_text
    2. Strip VT/ANSI escape sequences from text
    3. Check rate limit (write_text_rate_limit per second)
    4. Write to PTY via terminal_manager.write_to_pty()

ScriptCommand::RunCommand { command } =>
    1. Reject if !script_config.allow_run_command
    2. Tokenise command (no shell expansion)
    3. check_command_denylist(cmd, &args) -> block if Some(_)
    4. Check rate limit (run_command_rate_limit per second)
    5. Optional: prompt user for consent
    6. Spawn via std::process::Command (not /bin/sh -c)
```

---

## Relationship to Trigger Security Model

The trigger security model (`src/app/triggers.rs`) addresses the same
`RunCommand` / `SendText` concerns for trigger-fired actions:

- `require_user_action` flag (default `true`) blocks dangerous trigger actions.
- `check_command_denylist()` from `par-term-config` blocks known-bad patterns.
- Per-trigger rate limiting prevents flooding.

The scripting protocol security model is intentionally aligned with this
approach so that the same denylist and rate-limiting infrastructure can be
reused. The key difference is that scripts are long-lived subprocesses (not
one-shot trigger responses), so rate limits apply across the script's lifetime
rather than per trigger-fire.

---

## Audit Finding Reference

This document addresses **AUDIT FINDING M6** (Medium Security):
"Scripting protocol defines `WriteText`/`RunCommand` (unimplemented) — needs
security model when added."

Both commands remain unimplemented. They must not be activated without first:

1. Adding `allow_write_text` / `allow_run_command` permission flags to
   `ScriptConfig` with `default = false`.
2. Implementing all dispatcher-level checks described in this document.
3. Adding tests for the denylist, rate-limiting, and VT-stripping logic.
4. Conducting a security review of the implementation against this model.
