//! HTTP client helper with native-tls support.
//!
//! This module provides a configured HTTP agent that uses native-tls
//! for TLS connections, which works better in VM environments where
//! ring/rustls may have issues.

use ureq::tls::{TlsConfig, TlsProvider};
use ureq::Agent;

/// Create a new HTTP agent configured with native-tls.
///
/// This explicitly configures native-tls as the TLS provider, which uses
/// the system's TLS library (Schannel on Windows, OpenSSL on Linux,
/// Security.framework on macOS).
pub fn agent() -> Agent {
    let tls_config = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .build();

    Agent::config_builder()
        .tls_config(tls_config)
        .build()
        .into()
}
