//! Keyboard input handling and VT byte sequence generation for par-term.
//!
//! This crate converts `winit` keyboard events into the terminal input byte
//! sequences expected by shell applications. The byte-level encoding is the
//! shared encoder in `par-term-emu-core-rust` (`keyboard::encode_key_with`);
//! this crate maps winit events onto it and owns the frontend state it needs:
//! modifier tracking, which Alt key is held, the per-side Option-key modes,
//! and clipboard operations.
//!
//! The primary entry point is [`InputHandler`], which tracks modifier state
//! and translates each [`winit::event::KeyEvent`] into a `Vec<u8>` suitable
//! for writing directly to the PTY.
//!
//! # Crate layout (AUDIT.md ARC-006)
//!
//! The implementation is split across three `impl InputHandler` modules that
//! share the same struct defined here:
//!
//! - `modifiers` — shift/ctrl/alt/super tracking + Alt-side tracking
//! - `key_encoding` — winit → core key-event mapping and delegation to the
//!   shared encoder
//! - `clipboard` — paste/copy and X11 primary selection

#![warn(missing_docs)]

use arboard::Clipboard;
use par_term_emu_core_rust::keyboard::option_modes;
use par_term_emu_core_rust::terminal::Terminal;
use winit::event::{ElementState, KeyEvent, Modifiers};
use winit::keyboard::{Key, PhysicalKey};

use par_term_config::OptionKeyMode;

pub use par_term_emu_core_rust::keyboard::KeyEncodeOptions;

mod clipboard;
mod key_encoding;
mod modifiers;

/// The subset of [`winit::event::KeyEvent`] that byte-sequence encoding reads.
///
/// `KeyEvent` has a private platform-specific field and no public constructor,
/// so anything wanting to build one — a test, most often — has to forge it from
/// uninitialized memory, which is undefined behaviour and has cost this project
/// a segfault before. Encoding only ever needs these three fields, so it takes
/// this type instead; real winit events convert in via [`From`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyInput {
    /// Layout-resolved key, including any modifier the OS already applied.
    pub logical_key: Key,
    /// Physical key position, independent of the active layout.
    pub physical_key: PhysicalKey,
    /// Whether the key was pressed or released.
    pub state: ElementState,
}

impl From<KeyEvent> for KeyInput {
    fn from(event: KeyEvent) -> Self {
        Self {
            logical_key: event.logical_key,
            physical_key: event.physical_key,
            state: event.state,
        }
    }
}

impl From<&KeyEvent> for KeyInput {
    fn from(event: &KeyEvent) -> Self {
        Self {
            logical_key: event.logical_key.clone(),
            physical_key: event.physical_key,
            state: event.state,
        }
    }
}

/// Build the encoder's per-side Option-key options from the config modes.
pub fn key_encode_options(left: OptionKeyMode, right: OptionKeyMode) -> KeyEncodeOptions {
    KeyEncodeOptions {
        left_option: option_mode_value(left),
        right_option: option_mode_value(right),
    }
}

fn option_mode_value(mode: OptionKeyMode) -> u8 {
    match mode {
        OptionKeyMode::Normal => option_modes::NORMAL,
        OptionKeyMode::Meta => option_modes::META,
        OptionKeyMode::Esc => option_modes::ESC,
    }
}

/// The terminal modes the shared encoder reads, held on a private scratch
/// [`Terminal`].
///
/// `encode_key_with` takes its negotiated state (modifyOtherKeys level,
/// DECCKM, kitty flags) from a `Terminal`. The caller already resolved those
/// modes from the focused pane, so they are mirrored onto this one instead of
/// locking the pane's terminal a second time. Its kitty keyboard flags stay 0:
/// kitty gating is the app layer's decision, not this crate's.
#[derive(Default)]
struct EncoderModes {
    terminal: Option<Box<Terminal>>,
    modify_other_keys: u8,
    application_cursor: bool,
}

impl EncoderModes {
    /// Bring the scratch terminal's modes in line with the caller's.
    fn sync(&mut self, modify_other_keys: u8, application_cursor: bool) {
        let terminal = self
            .terminal
            .get_or_insert_with(|| Box::new(Terminal::with_scrollback(1, 1, 0)));
        if modify_other_keys != self.modify_other_keys {
            terminal.set_modify_other_keys_mode(modify_other_keys);
            self.modify_other_keys = modify_other_keys;
        }
        if application_cursor != self.application_cursor {
            // DECCKM has no direct setter; drive it the way an application does.
            terminal.process(if application_cursor {
                b"\x1b[?1h"
            } else {
                b"\x1b[?1l"
            });
            self.application_cursor = application_cursor;
        }
    }

    fn terminal(&self) -> &Terminal {
        self.terminal
            .as_deref()
            .expect("EncoderModes::sync runs before every encode")
    }
}

/// Input handler for converting winit events to terminal input
pub struct InputHandler {
    /// Latest modifier state reported by winit, consulted on every key encode.
    pub modifiers: Modifiers,
    clipboard: Option<Clipboard>,
    /// Per-side Option/Alt key modes handed to the shared encoder. Build it
    /// from config with [`key_encode_options`].
    pub key_options: KeyEncodeOptions,
    /// True while the left Alt key is held (tracked from physical key events).
    left_alt_pressed: bool,
    /// True while the right Alt key is held.
    right_alt_pressed: bool,
    encoder_modes: EncoderModes,
}

impl InputHandler {
    /// Create a new input handler
    pub fn new() -> Self {
        let clipboard = Clipboard::new().ok();
        if clipboard.is_none() {
            log::warn!("Failed to initialize clipboard support");
        }

        Self {
            modifiers: Modifiers::default(),
            clipboard,
            key_options: key_encode_options(OptionKeyMode::default(), OptionKeyMode::default()),
            left_alt_pressed: false,
            right_alt_pressed: false,
            encoder_modes: EncoderModes::default(),
        }
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}
