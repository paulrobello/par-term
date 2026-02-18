//! Self-update and update-check system for par-term terminal emulator.
//!
//! Provides:
//! - `manifest`: File tracking for bundled assets (shaders, shell integration)
//! - `update_checker`: GitHub release polling with configurable frequency
//! - `self_updater`: In-place binary replacement for standalone installs

pub mod http;
pub mod manifest;
pub mod self_updater;
pub mod update_checker;
