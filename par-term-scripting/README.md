# par-term-scripting

Scripting and observer system for the par-term terminal emulator.

This crate provides an observer-pattern event forwarding mechanism from the terminal core
to script subprocesses, along with per-tab script lifecycle management. Scripts receive
terminal events (output, commands, shell lifecycle) over a simple line-delimited protocol.

## What This Crate Provides

- `manager` — per-tab script lifecycle management: start, stop, and track running scripts
- `observer` — observer implementation that forwards terminal events to script processes
- `process` — script subprocess management: spawn, stdin/stdout I/O, graceful shutdown
- `protocol` — line-delimited event protocol sent to script stdin

## How It Works

When a script is started for a tab, a `ScriptProcess` is spawned. The `ScriptObserver`
registers with the terminal core and forwards events (terminal output, prompt detection,
command start/end) to the script's stdin as newline-delimited JSON messages. Scripts can
read from their stdin to react to terminal events and write responses to stdout.

## Workspace Position

Layer 2 in the dependency graph. Depends on `par-term-config` for script configuration
types. Used directly by the root `par-term` crate.

## Related Documentation

- [Automation](../docs/AUTOMATION.md) — triggers, coprocesses, and scripts
- [Config Reference](../docs/CONFIG_REFERENCE.md) — script configuration options
- [Architecture Overview](../docs/ARCHITECTURE.md) — workspace structure
- [Crate Structure](../docs/CRATE_STRUCTURE.md) — dependency layers
