//! Typed error variants for the par-term-config crate.
//!
//! Provides structured error types for config I/O and validation operations.
//! These are used internally and exposed for library consumers who want to
//! match on specific failure modes instead of opaque `anyhow` strings.

use thiserror::Error;

/// Errors that can occur when loading or saving configuration.
///
/// These errors are produced internally by `Config::load` and
/// `Config::save`, as well as by any helper that reads or writes YAML
/// state files.
///
/// For backward compatibility with existing callers that use `anyhow`, both
/// functions still return `anyhow::Result`; `ConfigError` values are
/// automatically coerced via the `From` impl that `anyhow` provides for any
/// `std::error::Error`.
///
/// # Example
///
/// ```rust,no_run
/// use par_term_config::ConfigError;
///
/// fn check_load_err(e: &anyhow::Error) {
///     if let Some(cfg_err) = e.downcast_ref::<ConfigError>() {
///         match cfg_err {
///             ConfigError::Io(io) => eprintln!("I/O error: {io}"),
///             ConfigError::Parse(p) => eprintln!("YAML parse error: {p}"),
///             ConfigError::Validation(msg) => eprintln!("Validation: {msg}"),
///         }
///     }
/// }
/// ```
#[derive(Debug, Error)]
pub enum ConfigError {
    /// An I/O error occurred reading or writing the config file.
    #[error("I/O error reading config: {0}")]
    Io(#[from] std::io::Error),

    /// The config file contained invalid YAML that could not be parsed.
    #[error("YAML parse error in config: {0}")]
    Parse(#[from] serde_yml::Error),

    /// A field value failed semantic validation.
    ///
    /// The inner string describes which field is invalid and why.
    #[error("Config validation error: {0}")]
    Validation(String),
}
