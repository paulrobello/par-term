//! Self-update and update-check system for par-term terminal emulator.
//!
//! Provides:
//! - `manifest`: File tracking for bundled assets (shaders, shell integration)
//! - `update_checker`: GitHub release polling with configurable frequency
//! - `self_updater`: In-place binary replacement for standalone installs
//! - `install_methods`: Installation type detection and platform-specific binary replacement
//! - `binary_ops`: Asset name resolution, SHA256 verification, download URLs
//! - `signature`: Detached minisign signature verification for downloaded releases

pub mod binary_ops;
pub mod http;
pub mod install_methods;
pub mod manifest;
pub mod self_updater;
pub mod signature;
pub mod update_checker;
