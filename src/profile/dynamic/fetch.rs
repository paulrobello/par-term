//! HTTP fetch logic for remote dynamic profile sources.
//!
//! Fetches YAML profile lists from remote URLs with:
//! - HTTPS-only policy (HTTP requires explicit opt-in)
//! - Authentication header protection over HTTP
//! - Configurable timeouts and response size limits
//! - Automatic cache write on successful fetch

use anyhow::Context;
use par_term_config::DynamicProfileSource;

use crate::profile::dynamic::cache::write_cache;

/// Result of fetching profiles from a remote source.
#[derive(Debug, Clone)]
pub struct FetchResult {
    /// The source URL that was fetched.
    pub url: String,
    /// Successfully parsed profiles (empty on error).
    pub profiles: Vec<par_term_config::Profile>,
    /// HTTP ETag header from the response.
    pub etag: Option<String>,
    /// Error message if the fetch failed.
    pub error: Option<String>,
}

/// Fetch profiles from a remote URL.
pub fn fetch_profiles(source: &DynamicProfileSource) -> FetchResult {
    let url = &source.url;
    crate::debug_info!("DYNAMIC_PROFILE", "Fetching profiles from {}", url);

    match fetch_profiles_inner(source) {
        Ok((profiles, etag)) => {
            crate::debug_info!(
                "DYNAMIC_PROFILE",
                "Fetched {} profiles from {}",
                profiles.len(),
                url
            );
            if let Err(e) = write_cache(url, &profiles, etag.clone()) {
                crate::debug_error!(
                    "DYNAMIC_PROFILE",
                    "Failed to cache profiles from {}: {}",
                    url,
                    e
                );
            }
            FetchResult {
                url: url.clone(),
                profiles,
                etag,
                error: None,
            }
        }
        Err(e) => {
            crate::debug_error!("DYNAMIC_PROFILE", "Failed to fetch from {}: {}", url, e);
            FetchResult {
                url: url.clone(),
                profiles: Vec::new(),
                etag: None,
                error: Some(e.to_string()),
            }
        }
    }
}

/// Internal fetch implementation.
fn fetch_profiles_inner(
    source: &DynamicProfileSource,
) -> anyhow::Result<(Vec<par_term_config::Profile>, Option<String>)> {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

    // Enforce HTTPS-only policy for dynamic profile URLs (unless the user has
    // explicitly opted in to HTTP via `allow_http_profiles: true` in the config).
    //
    // SECURITY: Profile data fetched over plain HTTP can be intercepted and
    // replaced by a network-level attacker (MITM). A malicious profile could
    // influence shell execution, environment, or other terminal behaviour.
    // HTTPS is the default requirement; HTTP is an explicit opt-in.
    if !source.url.starts_with("https://") && !source.url.starts_with("file://") {
        // Always refuse auth headers over HTTP regardless of the opt-in flag,
        // because credentials would be transmitted in the clear.
        if source.headers.keys().any(|k| {
            let lower = k.to_lowercase();
            lower == "authorization" || lower.contains("token") || lower.contains("secret")
        }) {
            anyhow::bail!(
                "Refusing to send authentication headers over insecure HTTP for {}. Use HTTPS.",
                source.url
            );
        }

        if !source.allow_http {
            // HTTP is not opted-in — refuse the fetch with a clear error.
            anyhow::bail!(
                "Dynamic profile URL '{}' uses insecure HTTP. \
                 Set `allow_http_profiles: true` in your config to allow HTTP (not recommended). \
                 Use HTTPS to prevent MITM injection of profiles.",
                source.url
            );
        }

        // User has explicitly opted in to HTTP — warn but proceed.
        crate::debug_error!(
            "DYNAMIC_PROFILE",
            "SECURITY WARNING: {} is using insecure HTTP (not HTTPS). \
             A MITM attacker could inject malicious profiles. Use HTTPS.",
            source.url
        );
        log::warn!(
            "par-term dynamic profile: fetching '{}' over insecure HTTP \
             (allow_http_profiles is enabled). MITM injection of profiles is possible. \
             Switch to HTTPS when possible.",
            source.url
        );
    }

    // Create an agent with the source-specific timeout
    let tls_config = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .tls_config(tls_config)
        .timeout_global(Some(std::time::Duration::from_secs(
            source.fetch_timeout_secs,
        )))
        .build()
        .into();

    let mut request = agent.get(&source.url);

    for (key, value) in &source.headers {
        request = request.header(key.as_str(), value.as_str());
    }

    let mut response = request
        .call()
        .with_context(|| format!("HTTP request failed for {}", source.url))?;

    let etag = response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let body = response
        .body_mut()
        .with_config()
        .limit(source.max_size_bytes as u64)
        .read_to_string()
        .with_context(|| format!("Failed to read response body from {}", source.url))?;

    let profiles: Vec<par_term_config::Profile> = serde_yaml_ng::from_str(&body)
        .with_context(|| format!("Failed to parse YAML from {}", source.url))?;

    Ok((profiles, etag))
}
