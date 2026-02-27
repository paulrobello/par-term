//! Typed error variants for the par-term-config crate.
//!
//! Provides structured error types for config I/O and validation operations.
//! These are used internally and exposed for library consumers who want to
//! match on specific failure modes instead of opaque `anyhow` strings.

use std::fmt;

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
///             ConfigError::PathTraversal(msg) => eprintln!("Path traversal: {msg}"),
///         }
///     }
/// }
/// ```
#[derive(Debug)]
pub enum ConfigError {
    /// An I/O error occurred reading or writing the config file.
    Io(std::io::Error),

    /// The config file contained invalid YAML that could not be parsed.
    Parse(serde_yml::Error),

    /// A field value failed semantic validation.
    ///
    /// The inner string describes which field is invalid and why.
    Validation(String),

    /// A path resolved outside the expected configuration directory,
    /// indicating a potential directory traversal attempt.
    ///
    /// The inner string includes the offending path and the expected base.
    PathTraversal(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "I/O error reading config: {e}"),
            ConfigError::Parse(e) => write!(f, "YAML parse error in config: {e}"),
            ConfigError::Validation(msg) => write!(f, "Config validation error: {msg}"),
            ConfigError::PathTraversal(msg) => write!(f, "Path traversal detected: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::Io(e) => Some(e),
            ConfigError::Parse(e) => Some(e),
            ConfigError::Validation(_) | ConfigError::PathTraversal(_) => None,
        }
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::Io(e)
    }
}

impl From<serde_yml::Error> for ConfigError {
    fn from(e: serde_yml::Error) -> Self {
        ConfigError::Parse(e)
    }
}
