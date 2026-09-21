//! Scripting and observer system for par-term terminal emulator.
//!
//! Provides observer-pattern event forwarding from the terminal core to
//! script subprocesses, along with per-tab script lifecycle management.
//! The plugin host ([`plugin_manager::PluginHost`]) reuses the same process
//! runtime for window-scoped status-bar plugins.

pub mod confirm;
pub mod manager;
pub mod manifest;
pub mod observer;
pub mod plugin_manager;
pub mod process;
pub mod protocol;
pub mod restart;

pub use process::ScriptStatus;
pub use restart::{RestartAction, ScriptRestartState};
