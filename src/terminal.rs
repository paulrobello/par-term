//! Terminal manager for PTY handling and terminal state management.
//!
//! Re-exports types from the `par-term-terminal` sub-crate for backward compatibility.
//! All terminal management types and functions are defined in `par-term-terminal`.
//!
//! # Re-exports from `par-term-terminal`
//!
//! This module is a thin facade so the rest of the main crate can use
//! `crate::terminal::TerminalManager` rather than depending directly on
//! `par_term_terminal`. The actual implementation lives in `par-term-terminal`.

// Submodules
pub use par_term_terminal::terminal::clipboard;
pub use par_term_terminal::terminal::coprocess_env;
pub use par_term_terminal::terminal::graphics;
pub use par_term_terminal::terminal::hyperlinks;
pub use par_term_terminal::terminal::rendering;
pub use par_term_terminal::terminal::spawn;

// Types from par_term_terminal::terminal
pub use par_term_terminal::terminal::{
    ClipboardEntry, ClipboardSlot, ShellLifecycleEvent, TerminalManager,
};

// Types from par_term_terminal root
pub use par_term_terminal::{HyperlinkInfo, SearchMatch};
