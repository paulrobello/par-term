# par-term-input

Keyboard input handling and VT byte sequence generation for the par-term terminal emulator.

This crate converts winit keyboard events into the terminal input byte sequences expected by
shell applications. It handles character input, named keys, function keys, modifier
combinations, Option/Alt key modes, clipboard operations, and the modifyOtherKeys protocol
extension.

## What This Crate Provides

- `InputHandler` — converts `winit::event::KeyEvent` to terminal input byte sequences
- Modifier state tracking (`Ctrl`, `Alt`, `Shift`, `Super`)
- Option/Alt key modes: `Normal`, `Meta` (set high bit), `Esc` (ESC prefix)
- Left and right Alt key tracking for independent mode configuration
- modifyOtherKeys support (modes 0, 1, 2) for enhanced key reporting to applications
- Application cursor mode (DECCKM): arrow keys send SS3 vs CSI sequences
- Clipboard read/write via `arboard`
- Primary selection support on Linux X11

## Key Types

| Type | Purpose |
|------|---------|
| `InputHandler` | Main struct: tracks modifiers and handles key events |
| `OptionKeyMode` | Configures Alt/Option key behavior (from `par-term-config`) |

## Key Sequences Generated

| Key | Normal | Application Cursor |
|-----|--------|-------------------|
| Arrow Up | `ESC [ A` | `ESC O A` |
| Arrow Down | `ESC [ B` | `ESC O B` |
| F1–F4 | `ESC O P`–`ESC O S` | same |
| F5–F12 | `ESC [ 15 ~` etc. | same |
| Ctrl+letter | Byte 1–26 | same |
| Shift+Tab | `ESC [ Z` | same |
| Backspace | `DEL (0x7F)` | same |

## Workspace Position

Layer 2 in the dependency graph. Depends on `par-term-config` for `OptionKeyMode`.
Used directly by the root `par-term` crate for event handling.

## Installation

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
par-term-input = { version = "0.1.12" }
```

## Usage

```rust
use par_term_input::InputHandler;
use par_term_config::types::OptionKeyMode;

let mut handler = InputHandler::new(OptionKeyMode::Esc);

// Handle a key event from winit
if let Some(bytes) = handler.handle_key_event(&key_event) {
    pty.write_all(&bytes)?;
}
```

## Related Documentation

- [Keyboard Shortcuts](../docs/KEYBOARD_SHORTCUTS.md) — user-facing keyboard reference
- [Config Reference](../docs/CONFIG_REFERENCE.md) — input configuration options
- [Architecture Overview](../docs/ARCHITECTURE.md) — workspace structure
- [Crate Structure](../docs/CRATE_STRUCTURE.md) — dependency layers
