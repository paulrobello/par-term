//! HTTP client helper with native-tls support.

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

/// Download a file from a URL and return its bytes.
///
/// Response body is limited to [`MAX_DOWNLOAD_SIZE`] (50 MB) to prevent
/// memory exhaustion from malicious or misbehaving servers.
pub fn download_file(url: &str) -> Result<Vec<u8>, String> {
    let bytes = agent()
        .get(url)
        .header("User-Agent", "par-term")
        .call()
        .map_err(|e| format!("Failed to download file: {}", e))?
        .into_body()
        .with_config()
        .limit(MAX_DOWNLOAD_SIZE)
        .read_to_vec()
        .map_err(|e| format!("Failed to read download: {}", e))?;

    Ok(bytes)
}
