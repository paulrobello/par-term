//! HTTP client helper with native-tls support.
//!
//! This module provides a configured HTTP agent that uses native-tls
//! for TLS connections, which works better in VM environments where
//! ring/rustls may have issues.

use std::time::Duration;
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

/// Global timeout for all HTTP operations (30 seconds).
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum response body size for API responses (10 MB).
pub const MAX_API_RESPONSE_SIZE: u64 = 10 * 1024 * 1024;

/// Maximum response body size for file downloads (50 MB).
pub const MAX_DOWNLOAD_SIZE: u64 = 50 * 1024 * 1024;

/// Create a new HTTP agent configured with native-tls and a global timeout.
///
/// This explicitly configures native-tls as the TLS provider, which uses
/// the system's TLS library (Schannel on Windows, OpenSSL on Linux,
/// Security.framework on macOS).
///
/// We use PlatformVerifier to use the system's built-in root certificates.
///
/// A global timeout of 30 seconds is applied to prevent hanging on
/// unresponsive servers. Callers reading response bodies should use
/// `body.with_config().limit(N)` to enforce size limits.
pub fn agent() -> Agent {
    let tls_config = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();

    Agent::config_builder()
        .tls_config(tls_config)
        .timeout_global(Some(HTTP_TIMEOUT))
        .build()
        .into()
}
