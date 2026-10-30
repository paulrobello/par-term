//! Copy mode implementation for WindowState.
//!
//! - `handler`: Key dispatch, enter/exit, and main loop.
//! - `search`: Search input, forward/backward scanning, and motion helpers.

pub mod handler;
#[cfg(test)]
mod handler_tests;
pub mod search;
