//! Terminal manager for PTY handling and terminal state management.
//!
//! This module re-exports types from the par-term-terminal crate for backward compatibility.
//! All terminal management types and functions are defined in par-term-terminal.

pub use par_term_terminal::terminal::clipboard;
pub use par_term_terminal::terminal::coprocess_env;
pub use par_term_terminal::terminal::graphics;
pub use par_term_terminal::terminal::hyperlinks;
pub use par_term_terminal::terminal::rendering;
pub use par_term_terminal::terminal::spawn;
pub use par_term_terminal::terminal::*;
pub use par_term_terminal::{ClipboardEntry, ClipboardSlot, HyperlinkInfo, SearchMatch};
