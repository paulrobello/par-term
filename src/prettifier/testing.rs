//! Shared test helpers for prettifier renderer tests.
//!
//! This module is gated with `#[cfg(test)]` and provides canonical factory
//! functions used across all renderer test files. Import with:
//!
//! ```ignore
//! use crate::prettifier::testing::{make_block, make_block_with_command, test_renderer_config};
//! ```

use super::traits::RendererConfig;
use super::types::ContentBlock;
use crate::config::Config;
use std::time::SystemTime;

/// Returns a `RendererConfig` suitable for renderer unit tests.
///
/// Uses an 80-column terminal width and all other fields at their defaults.
#[cfg(test)]
#[allow(dead_code)]
pub fn test_renderer_config() -> RendererConfig {
    RendererConfig {
        terminal_width: 80,
        ..Default::default()
    }
}

/// Returns a `Config` suitable for integration-style prettifier tests.
///
/// Uses all defaults; no temporary directories or I/O are set up.
#[cfg(test)]
#[allow(dead_code)]
pub fn test_global_config() -> Config {
    Config::default()
}

/// Creates a `ContentBlock` from a slice of string lines with no preceding command.
///
/// Use this in renderer tests where the command context is irrelevant.
/// For tests that need a specific command, use [`make_block_with_command`].
#[cfg(test)]
#[allow(dead_code)]
pub fn make_block(lines: &[&str]) -> ContentBlock {
    ContentBlock {
        lines: lines.iter().map(|s| s.to_string()).collect(),
        preceding_command: None,
        start_row: 0,
        end_row: lines.len(),
        timestamp: SystemTime::now(),
    }
}

/// Creates a `ContentBlock` from a slice of string lines with an optional preceding command.
///
/// Use this in detector tests where the preceding shell command affects detection logic.
/// Pass `None` for `command` when no command context is needed.
#[cfg(test)]
#[allow(dead_code)]
pub fn make_block_with_command(lines: &[&str], command: Option<&str>) -> ContentBlock {
    ContentBlock {
        lines: lines.iter().map(|s| s.to_string()).collect(),
        preceding_command: command.map(|s| s.to_string()),
        start_row: 0,
        end_row: lines.len(),
        timestamp: SystemTime::now(),
    }
}
