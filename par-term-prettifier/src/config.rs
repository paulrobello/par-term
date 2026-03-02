//! Configuration facade for par-term-prettifier.
//!
//! Re-exports the prettifier-relevant types from `par-term-config` using the
//! same module paths that the prettifier code expects (`crate::config::prettifier::*`,
//! `crate::config::Config`, etc.).  This allows all internal files to keep their
//! existing `use crate::config::…` import paths unchanged.

pub use par_term_config::Config;
pub use par_term_config::config;
pub use par_term_config::config::prettifier;
pub use par_term_config::{
    PrettifierConfigOverride, PrettifierYamlConfig, ResolvedPrettifierConfig,
    resolve_prettifier_config,
};
