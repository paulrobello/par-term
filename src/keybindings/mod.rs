//! Keybinding system re-exports from the `par-term-keybindings` crate.

pub use par_term_keybindings::{
    KeyCombo, KeybindingMatcher, KeybindingRegistry, ParseError, key_combo_to_bytes,
    parse_key_sequence,
};

// Re-export submodule for backward compatibility
pub use par_term_keybindings::parser;
