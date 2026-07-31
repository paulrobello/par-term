//! Scripting and observer system for par-term terminal emulator.
//!
//! Provides observer-pattern event forwarding from the terminal core to
//! script subprocesses, along with per-tab script lifecycle management.

pub mod confirm;
pub mod manager;
pub mod observer;
pub mod process;
pub mod protocol;
pub mod restart;
