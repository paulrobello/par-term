//! Reusable helpers for the `par-term-acp-harness` binary.
//!
//! # Modules
//!
//! - [`transcript`] — transcript file writing (tee stdout + file)
//! - [`recovery`] — harness event flags and permission-option selection

pub mod recovery;
pub mod transcript;

// Convenience re-exports so callers can write `par_term_acp::harness::*`.
pub use recovery::{HarnessEventFlags, choose_permission_option};
pub use transcript::{init_transcript, println_tee, transcript_slot};
