# par-term-terminal

Terminal manager for the par-term terminal emulator.

This crate wraps the core PTY session from `par-term-emu-core-rust` and provides a
high-level API for all terminal operations. It is the primary interface between the
application layer and the VT sequence processing / PTY I/O layer.

## What This Crate Provides

- `TerminalManager` — high-level terminal operations over a core PTY session
- `ScrollbackMetadata` / `LineMetadata` — per-line metadata for prompt marks, command
  tracking, and shell integration
- `CommandSnapshot` — snapshot of a completed shell command (command text, exit code, CWD)
- `StyledSegment` / `extract_styled_segments` — extract ANSI-styled text segments from
  terminal content for search and export
- `SearchMatch` — represents a single match position in the scrollback buffer
- `ShellLifecycleEvent` — events emitted by shell integration (prompt shown, command
  started, command finished, CWD changed)
- Re-exports of `ClipboardEntry`, `ClipboardSlot`, `HyperlinkInfo` from the core library

## Key Capabilities

| Area | Functionality |
|------|--------------|
| PTY I/O | `read()`, `write()`, `paste()` |
| Lifecycle | `spawn()`, `resize()`, `kill()` |
| Shell integration | CWD tracking, exit codes, command history via OSC sequences |
| Inline graphics | Sixel, iTerm2, Kitty protocol rendering data |
| Search | Full-text search in scrollback with match positions |
| Scrollback | Line metadata, prompt marks, semantic navigation |
| Recording | Session recording and screenshot capture |
| Coprocesses | Subprocess I/O wired into the PTY read loop |
| tmux | Control mode integration via gateway state |

## Workspace Position

Layer 2 in the dependency graph. Depends on `par-term-config` and
`par-term-emu-core-rust` (external). Used directly by the root `par-term` crate and
re-exported as `par_term::terminal`.

## Installation

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
par-term-terminal = { version = "0.2.6" }
```

## Usage

```rust
use par_term_terminal::TerminalManager;
use par_term_config::Config;

let config = Config::load()?;
let mut terminal = TerminalManager::spawn(&config, None)?;

// Write to the PTY
terminal.write(b"echo hello\n")?;

// Read terminal output
let data = terminal.read()?;
```

## Related Documentation

- [Scrollback Buffer](../docs/SCROLLBACK.md) — scrollback and semantic history
- [Semantic History](../docs/SEMANTIC_HISTORY.md) — command and prompt tracking
- [Session Logging](../docs/SESSION_LOGGING.md) — session recording
- [Automation](../docs/AUTOMATION.md) — coprocess and trigger integration
- [Crate Structure](../docs/CRATE_STRUCTURE.md) — dependency layers
