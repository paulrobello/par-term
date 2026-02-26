//! Terminal configuration management.
//!
//! Re-exports all configuration types from the `par-term-config` crate.
//! All configuration types, defaults, and utilities are defined in `par-term-config`.

pub use par_term_config::*;

// Re-export the prettifier submodule so `crate::config::prettifier::*` works
pub use par_term_config::config::prettifier;
