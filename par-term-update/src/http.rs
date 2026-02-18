//! HTTP client helper with native-tls support.

use std::io::Read;
use ureq::Agent;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

/// Create a new HTTP agent configured with native-tls.
pub fn agent() -> Agent {
    let tls_config = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();

    Agent::config_builder()
        .tls_config(tls_config)
        .build()
        .into()
}

/// Download a file from a URL and return its bytes.
pub fn download_file(url: &str) -> Result<Vec<u8>, String> {
    let mut body = agent()
        .get(url)
        .header("User-Agent", "par-term")
        .call()
        .map_err(|e| format!("Failed to download file: {}", e))?
        .into_body();

    let mut bytes = Vec::new();
    body.as_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read download: {}", e))?;

    Ok(bytes)
}
