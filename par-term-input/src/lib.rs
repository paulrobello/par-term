//! Keyboard input handling and VT byte sequence generation for par-term.
//!
//! This crate converts `winit` keyboard events into the terminal input byte
//! sequences expected by shell applications. It handles character input,
//! named keys, function keys, modifier combinations, Option/Alt key modes,
//! clipboard operations, and the modifyOtherKeys protocol extension.
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
//! - `modifiers` — shift/ctrl/alt/super tracking + Option/Alt key modes
//! - `key_encoding` — VT byte sequence generation (character, named,
//!   function keys, modifyOtherKeys)
//! - `clipboard` — paste/copy and X11 primary selection
//!
//! The split is purely organizational; the public API is unchanged.

#![warn(missing_docs)]

use arboard::Clipboard;
use winit::event::{ElementState, KeyEvent, Modifiers};
use winit::keyboard::{Key, PhysicalKey};

use par_term_config::OptionKeyMode;

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

/// Input handler for converting winit events to terminal input
pub struct InputHandler {
    /// Latest modifier state reported by winit, consulted on every key encode.
    pub modifiers: Modifiers,
    clipboard: Option<Clipboard>,
    /// Option key mode for left Option/Alt key
    pub left_option_key_mode: OptionKeyMode,
    /// Option key mode for right Option/Alt key
    pub right_option_key_mode: OptionKeyMode,
    /// Track which Alt key is currently pressed (for determining mode on character input)
    /// True = left Alt is pressed, False = right Alt or no Alt
    left_alt_pressed: bool,
    /// True = right Alt is pressed
    right_alt_pressed: bool,
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
            left_option_key_mode: OptionKeyMode::default(),
            right_option_key_mode: OptionKeyMode::default(),
            left_alt_pressed: false,
            right_alt_pressed: false,
        }
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}
