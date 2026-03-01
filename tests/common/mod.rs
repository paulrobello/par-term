//! Shared integration test helpers for par-term.
//!
//! This module provides canonical factory functions and test isolation
//! utilities used across the `tests/` integration test suite.
//!
//! # Usage
//!
//! Include this module at the top of each test file that needs it:
//!
//! ```ignore
//! mod common;
//! use common::{default_config_with_tmp_dir, config_with_shader_dir, TestContext};
//! ```
//!
//! Note: Rust integration tests use `mod common;` (not `use`) to bring in
//! helpers from `tests/common/mod.rs`. The `#[allow(dead_code)]` attributes
//! suppress warnings when only a subset of helpers are used per file.

#![allow(dead_code)]

use par_term::config::Config;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Creates a temporary directory and returns a `Config` whose config_dir-
/// related paths (e.g. for save/load tests) point inside that temp dir.
///
/// The `TempDir` must be kept alive for the duration of the test — drop it
/// only after all config I/O has completed.
pub fn default_config_with_tmp_dir() -> (Config, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = Config::default();
    (config, temp_dir)
}

/// Creates a temporary directory tree suitable for shader loading tests,
/// and returns a `Config` with the custom_shader field pointing at a GLSL
/// stub inside that directory.
///
/// The directory layout created:
/// ```text
/// <tmp>/
///   shaders/
///     test_shader.glsl   # empty stub
/// ```
pub fn config_with_shader_dir() -> (Config, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let shaders_dir = temp_dir.path().join("shaders");
    fs::create_dir_all(&shaders_dir).expect("Failed to create shaders dir");

    // Write a minimal passthrough GLSL stub so path-resolution tests work.
    let shader_path = shaders_dir.join("test_shader.glsl");
    fs::write(
        &shader_path,
        "void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n  fragColor = vec4(0.0);\n}\n",
    )
    .expect("Failed to write stub shader");

    let mut config = Config::default();
    config.custom_shader = Some(
        shader_path
            .to_str()
            .expect("shader path is valid UTF-8")
            .to_string(),
    );

    (config, temp_dir)
}

/// Creates a temporary config directory structure and returns its path.
///
/// Directory layout:
/// ```text
/// <tmp>/
///   par-term/            # config_dir equivalent
/// ```
pub fn setup_config_dir() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_dir = temp_dir.path().join("par-term");
    fs::create_dir_all(&config_dir).expect("Failed to create config dir");
    (temp_dir, config_dir)
}

/// Provides test isolation with automatic resource cleanup.
///
/// Wraps a `TempDir` and a `Config` pointing at it, ensuring the temporary
/// directory is removed when `TestContext` is dropped.
///
/// # Example
///
/// ```ignore
/// let ctx = TestContext::new();
/// // use ctx.config and ctx.dir ...
/// // ctx is dropped at end of scope, cleaning up the temp dir
/// ```
pub struct TestContext {
    /// Temporary directory — kept alive for the lifetime of the context.
    pub dir: TempDir,
    /// Config instance for the test.
    pub config: Config,
}

impl TestContext {
    /// Create a new `TestContext` with a fresh temp dir and default config.
    pub fn new() -> Self {
        let (config, dir) = default_config_with_tmp_dir();
        Self { dir, config }
    }

    /// Return the path to the temporary directory root.
    pub fn path(&self) -> &std::path::Path {
        self.dir.path()
    }
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}
