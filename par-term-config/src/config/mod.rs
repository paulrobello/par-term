//! Terminal configuration management.
//!
//! This module provides configuration loading, saving, and default values
//! for the terminal emulator.
//!
//! # Sub-modules
//!
//! - [`acp`] — ACP agent configuration types (`CustomAcpAgentConfig`, etc.)
//! - [`config_struct`] — Core `Config` struct and its `Default` impl
//! - [`config_methods`] — All `impl Config` methods (load/save/paths/helpers)
//! - [`env_vars`] — Environment-variable allowlist and `${VAR}` substitution
//! - [`prettifier`] — Content prettifier YAML config types

pub mod acp;
pub mod config_methods;
pub mod config_struct;
pub mod env_vars;
pub mod prettifier;

// Re-export the public API so downstream crates keep working with
// paths like `crate::config::Config`, `crate::config::ALLOWED_ENV_VARS`, etc.

pub use acp::{CustomAcpAgentActionConfig, CustomAcpAgentConfig};
pub use config_struct::Config;
pub use env_vars::{ALLOWED_ENV_VARS, is_env_var_allowed, substitute_variables, substitute_variables_with_allowlist};

// KeyBinding is referenced in generate_snippet_action_keybindings via `crate::config::KeyBinding`
pub use crate::types::KeyBinding;

// PaneBackgroundConfig is referenced in Config fields as `crate::config::PaneBackgroundConfig`
pub use crate::types::PaneBackgroundConfig;
