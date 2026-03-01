//! Shared test helpers for prettifier renderer tests.
//!
//! This module is gated with `#[cfg(test)]` and provides canonical factory
//! functions used across all renderer test files. Import with:
//!
//! ```ignore
//! use crate::prettifier::testing::test_renderer_config;
//! ```

use super::traits::RendererConfig;
use crate::config::Config;

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
