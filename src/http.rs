//! HTTP client helper with native-tls support.
//!
//! This module provides a configured HTTP agent that uses native-tls
//! for TLS connections, which works better in VM environments where
//! ring/rustls may have issues.
//!
//! # Security
//!
//! The [`validate_download_url`] function enforces HTTPS-only and a host
//! allowlist for shader-download URLs, matching the validation pattern
//! used by the self-update subsystem in `par-term-update`.

use std::time::Duration;
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

/// Global timeout for all HTTP operations (30 seconds).
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum response body size for API responses (10 MB).
pub const MAX_API_RESPONSE_SIZE: u64 = 10 * 1024 * 1024;

/// Maximum response body size for file downloads (50 MB).
pub const MAX_DOWNLOAD_SIZE: u64 = 50 * 1024 * 1024;

/// Allowlisted hostnames for shader-download network requests.
///
/// Only requests to GitHub's primary API and CDN hosts are permitted.
/// Any other host is rejected regardless of the URL path, preventing
/// SSRF or DNS-rebinding attacks that could redirect download traffic
/// to an attacker-controlled server.
const ALLOWED_DOWNLOAD_HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "objects.githubusercontent.com",
    "github-releases.githubusercontent.com",
];

/// Validate that a URL is safe to use for shader download operations.
///
/// Enforces:
/// - HTTPS scheme only (no HTTP, ftp, file://, etc.)
/// - Host must be in the GitHub allowlist
///
/// Returns `Ok(())` if the URL is acceptable, or an error string describing
/// why it was rejected.
pub fn validate_download_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("Invalid URL '{}': {}", url, e))?;

    // Enforce HTTPS only — plain HTTP can be intercepted and downgraded.
    match parsed.scheme() {
        "https" => {}
        scheme => {
            return Err(format!(
                "Insecure URL scheme '{}' rejected; only HTTPS is allowed. \
                 URL: {}",
                scheme, url
            ));
        }
    }

    // Enforce domain allowlist — reject any host not operated by GitHub.
    let host = parsed.host_str().unwrap_or("");
    if !ALLOWED_DOWNLOAD_HOSTS.contains(&host) {
        return Err(format!(
            "URL host '{}' is not in the allowed list for download operations. \
             Allowed hosts: {}. \
             URL: {}",
            host,
            ALLOWED_DOWNLOAD_HOSTS.join(", "),
            url
        ));
    }

    Ok(())
}

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
