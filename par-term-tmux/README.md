# par-term-tmux

tmux control mode integration for the par-term terminal emulator.

This crate provides integration with tmux's control mode (`-CC` flag), allowing par-term
to attach to existing tmux sessions, display tmux panes natively using the split-pane
system, send input through the tmux control protocol, and receive output and notifications.

## What This Crate Provides

- `TmuxSession` — tmux session lifecycle and state management; drives control mode I/O
- `TmuxSync` / `SyncAction` — bidirectional state synchronization between tmux and par-term
- `TmuxCommand` — command builders for the tmux control protocol
- `ParserBridge` — bridges the core library's tmux parser to par-term's sync layer
- `TmuxPane` / `TmuxWindow` / `TmuxLayout` — core data types for tmux topology
- `PaneSync` / `WindowSync` — per-pane and per-window state synchronization helpers
- `PrefixKey` / `PrefixState` / `translate_command_key` — prefix key handling for
  forwarding key sequences through tmux's control mode
- `FormatContext` / `expand_format` — tmux format string expansion for status bars
- `sanitize_tmux_output` — strips tmux control sequences from output

## Control Mode Protocol

tmux control mode uses a line-based protocol over stdio:

- Commands are sent as plain text lines
- Notifications from tmux start with `%` (e.g., `%window-add`, `%output`)
- Output blocks are delimited by `%begin` / `%end`

The core library (`par-term-emu-core-rust`) provides the control mode parser. This crate
handles the higher-level state machine.

## Workspace Position

Layer 2 in the dependency graph. Depends on `par-term-config`. Used directly by the root
`par-term` crate and re-exported as `par_term::tmux`.

## Related Documentation

- [Architecture Overview](../docs/ARCHITECTURE.md) — tmux integration in the workspace
- [Config Reference](../docs/CONFIG_REFERENCE.md) — tmux configuration options
- [Tabs](../docs/TABS.md) — split pane system that tmux panes map to
- [Crate Structure](../docs/CRATE_STRUCTURE.md) — dependency layers
