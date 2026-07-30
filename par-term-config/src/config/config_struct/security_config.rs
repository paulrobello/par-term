//! Configuration security policy.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use serde::{Deserialize, Serialize};

/// Environment variable substitution allowlist and plain-HTTP profile fetching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Allow all environment variables in config `${VAR}` substitution.
    ///
    /// When `false` (default), only a safe allowlist of environment variables
    /// (HOME, USER, SHELL, XDG_*, PAR_TERM_*, LC_*, etc.) can be substituted.
    /// This prevents shared or downloaded config files from exfiltrating
    /// sensitive environment variables such as API keys or tokens.
    ///
    /// Set to `true` to restore the unrestricted pre-0.24 behaviour.
    #[serde(default = "crate::defaults::bool_false")]
    pub allow_all_env_vars: bool,

    /// Allow dynamic profile sources to be fetched over plain HTTP (not HTTPS).
    ///
    /// When `false` (the default), any `dynamic_profile_sources` entry whose URL
    /// uses the `http://` scheme will be refused with an error at fetch time.
    /// This prevents a network-level attacker from injecting malicious profiles
    /// via a man-in-the-middle attack on an untrusted network.
    ///
    /// Set to `true` only if you must fetch profiles from a server that does not
    /// support HTTPS (e.g., an internal dev server without TLS). A warning will
    /// still be logged in that case.
    #[serde(default = "crate::defaults::bool_false")]
    pub allow_http_profiles: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            allow_all_env_vars: crate::defaults::bool_false(),
            allow_http_profiles: crate::defaults::bool_false(),
        }
    }
}
