//! HTTP client helper with native-tls support.
//!
//! This module provides a configured HTTP agent that uses native-tls
//! for TLS connections, which works better in VM environments where
//! ring/rustls may have issues.
//!
//! # Security
//!
//! [`validate_download_url`] enforces HTTPS-only and a host allowlist for
//! shader-download URLs, matching the validation used by the self-update
//! subsystem in `par-term-update`. [`get_validated`] additionally re-applies
//! that allowlist to *every* redirect hop rather than only to the URL the caller
//! passed in.
//!
//! Per-hop revalidation is hardening, not a fix for an observed exploit: no
//! redirect off an allowlisted GitHub host has been demonstrated. It closes the
//! gap that the allowlist was only ever checked once, before the first request.

use std::time::Duration;
use ureq::Agent;
use ureq::http::{Response, StatusCode};
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
use ureq::{Body, Error as UreqError};

/// Global timeout for all HTTP operations (30 seconds).
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum number of redirects followed manually by [`get_validated`].
///
/// A `shaders.zip` download takes two hops, measured against the live release:
/// `github.com/…/releases/latest/download/shaders.zip` →
/// `github.com/…/releases/download/<tag>/shaders.zip` →
/// `release-assets.githubusercontent.com/…`. This leaves headroom for GitHub to
/// add another hop without letting a redirect loop run indefinitely.
const MAX_REDIRECTS: u32 = 5;

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
///
/// `release-assets.githubusercontent.com` is where GitHub currently terminates
/// a release-asset download, and it is **load-bearing**: `shaders.zip`'s
/// `browser_download_url` redirects there, so removing it breaks every download
/// once redirects are revalidated per hop. The two older `*.githubusercontent.com`
/// CDN names are kept because GitHub has rotated this host before.
const ALLOWED_DOWNLOAD_HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "objects.githubusercontent.com",
    "github-releases.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

/// Validate that a URL is safe to use for shader download operations.
///
/// Enforces:
/// - HTTPS scheme only (no HTTP, ftp, file://, etc.)
/// - Host must be in the GitHub allowlist
///
/// Returns `Ok(())` if the URL is acceptable, or an error string describing
/// why it was rejected. URLs are redacted in those messages — see
/// [`redact_url`].
pub fn validate_download_url(url: &str) -> Result<(), String> {
    let parsed =
        url::Url::parse(url).map_err(|e| format!("Invalid URL '{}': {}", redact_url(url), e))?;

    // Enforce HTTPS only — plain HTTP can be intercepted and downgraded.
    match parsed.scheme() {
        "https" => {}
        scheme => {
            return Err(format!(
                "Insecure URL scheme '{}' rejected; only HTTPS is allowed. \
                 URL: {}",
                scheme,
                redact_url(url)
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
            redact_url(url)
        ));
    }

    Ok(())
}

/// Render a URL for logs and error messages with its query string removed.
///
/// GitHub's final release-asset hop carries short-lived credentials in the query
/// (`?sig=…&jwt=…`). Those are bearer-equivalent, so the full URL must never
/// reach the debug log — only scheme, host and path do. Userinfo is dropped for
/// the same reason.
fn redact_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(parsed) => format!(
            "{}://{}{}",
            parsed.scheme(),
            parsed.host_str().unwrap_or(""),
            parsed.path()
        ),
        // Unparseable URLs never reach the network, and printing the raw string
        // would defeat the redaction, so describe it instead of echoing it.
        Err(_) => "<unparseable URL>".to_string(),
    }
}

/// Resolve a `Location` header against the URL that produced it, then validate it.
///
/// `Location` is allowed to be relative (RFC 9110 §10.2.2), so it is joined onto
/// the current URL before the allowlist is applied. Both halves matter: joining
/// without validating is the open-redirect hole, and validating without joining
/// rejects legitimate relative redirects.
fn resolve_redirect(current: &str, location: &str) -> Result<String, String> {
    let base = url::Url::parse(current)
        .map_err(|e| format!("Could not parse the current download URL: {}", e))?;

    let resolved = base.join(location).map_err(|e| {
        format!(
            "Download server redirected from {} to an unparseable Location: {}. \
             Download aborted.",
            redact_url(current),
            e
        )
    })?;

    let resolved = resolved.to_string();
    // Deliberately does *not* forward `validate_download_url`'s message: naming
    // the allowlist twice in one error reads badly, and the caller needs to see
    // both ends of the redirect.
    if validate_download_url(&resolved).is_err() {
        return Err(format!(
            "Download server redirected from {} to {}, which is not an allowed \
             download host. Download aborted — a redirect cannot move the \
             download off GitHub. Allowed hosts: {}.",
            redact_url(current),
            redact_url(&resolved),
            ALLOWED_DOWNLOAD_HOSTS.join(", ")
        ));
    }

    Ok(resolved)
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
///
/// `https_only(true)` means a redirect that downgrades the chain to plain HTTP
/// fails instead of being followed. This agent still follows redirects itself,
/// so it does **not** revalidate the host per hop; callers that need that must
/// use [`get_validated`].
pub fn agent() -> Agent {
    build_agent(true)
}

/// Agent used by [`get_validated`], with ureq's own redirect following disabled.
///
/// ureq's default is to follow up to ten redirects with no revalidation, which
/// would let an allowlisted host hand the download off to anywhere. Setting
/// `max_redirects(0)` surfaces each 3xx to [`get_validated`], which re-applies
/// the allowlist before making the next request.
fn no_redirect_agent() -> Agent {
    build_agent(false)
}

/// Shared TLS, timeout and HTTPS configuration for both agents.
fn build_agent(follow_redirects: bool) -> Agent {
    let tls_config = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();

    let builder = Agent::config_builder()
        .tls_config(tls_config)
        .timeout_global(Some(HTTP_TIMEOUT))
        .https_only(true);

    let builder = if follow_redirects {
        builder
    } else {
        builder.max_redirects(0)
    };

    builder.build().into()
}

/// Describe a failed request without leaking the query string.
fn describe_request_error(url: &str, error: &UreqError) -> String {
    format!(
        "Failed to fetch '{}': {}. \
         Check your internet connection and try again.",
        redact_url(url),
        error
    )
}

/// Perform a GET, following redirects manually with the allowlist re-applied at
/// every hop.
///
/// The URL the caller passes is validated before the first request, and each
/// `Location` is resolved and validated before the next one. A redirect to a
/// host outside `ALLOWED_DOWNLOAD_HOSTS`, a redirect with no `Location`, and a
/// chain longer than [`MAX_REDIRECTS`] are all hard errors — none of them fall
/// through to reading a body. A 3xx body is never read.
///
/// # Errors
///
/// Returns an error if the URL or any redirect target fails allowlist
/// validation, if the request fails (DNS, connection, TLS), if a redirect
/// carries no usable `Location`, or if the chain exceeds [`MAX_REDIRECTS`].
pub fn get_validated(url: &str, accept: Option<&str>) -> Result<Response<Body>, String> {
    let agent = no_redirect_agent();
    let mut current = url.to_string();

    for _ in 0..=MAX_REDIRECTS {
        // Re-validated on every iteration, not just the first.
        validate_download_url(&current)?;

        let mut request = agent.get(&current).header("User-Agent", "par-term");
        if let Some(accept) = accept {
            request = request.header("Accept", accept);
        }

        let response = request
            .call()
            .map_err(|e| describe_request_error(&current, &e))?;

        let status = response.status();
        if !status.is_redirection() {
            return Ok(response);
        }

        let location = redirect_location(&response, status, &current)?;
        // The 3xx body is intentionally never read.
        current = resolve_redirect(&current, &location)?;
    }

    Err(format!(
        "Download exceeded {} redirects starting from {}. \
         Download aborted — this may indicate a redirect loop.",
        MAX_REDIRECTS,
        redact_url(url)
    ))
}

/// Extract the `Location` header from a redirect response.
fn redirect_location(
    response: &Response<Body>,
    status: StatusCode,
    current: &str,
) -> Result<String, String> {
    response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "Download server returned redirect status {} from {} with no usable \
                 Location header. Download aborted.",
                status.as_u16(),
                redact_url(current)
            )
        })
}

/// Download a file from a URL and return its bytes.
///
/// Validates the URL against the allowlist before making any network request,
/// and again at every redirect hop. The body is limited to [`MAX_DOWNLOAD_SIZE`]
/// (50 MB) to prevent memory exhaustion from a malicious or misbehaving server.
///
/// # Errors
///
/// Returns an error if the URL, or any redirect target, fails allowlist
/// validation; if the HTTP request fails; or if reading the body fails or
/// exceeds the size limit.
pub fn download_file(url: &str) -> Result<Vec<u8>, String> {
    get_validated(url, None)?
        .into_body()
        .with_config()
        .limit(MAX_DOWNLOAD_SIZE)
        .read_to_vec()
        .map_err(|e| {
            format!(
                "Failed to read downloaded content from '{}': {}. \
                 The response may have been truncated or the connection dropped.",
                redact_url(url),
                e
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_download_url ---

    #[test]
    fn allowlisted_github_hosts_are_accepted() {
        for url in [
            "https://api.github.com/repos/paulrobello/par-term/releases/latest",
            "https://github.com/paulrobello/par-term/releases/download/v1/shaders.zip",
            "https://objects.githubusercontent.com/asset/123/shaders.zip",
            "https://github-releases.githubusercontent.com/123/shaders.zip",
            "https://release-assets.githubusercontent.com/asset/123/shaders.zip",
        ] {
            assert!(
                validate_download_url(url).is_ok(),
                "expected {url} to be accepted"
            );
        }
    }

    #[test]
    fn allowlist_contains_the_host_release_downloads_actually_land_on() {
        // Load-bearing: `shaders.zip`'s `browser_download_url` 302s here, and
        // per-hop validation means dropping this host breaks every shader
        // download. Verified against the live chain by
        // `live_release_redirect_chain_stays_on_allowlisted_hosts`.
        assert!(ALLOWED_DOWNLOAD_HOSTS.contains(&"release-assets.githubusercontent.com"));
    }

    #[test]
    fn non_https_schemes_are_rejected() {
        let err = validate_download_url("http://api.github.com/repos/x/y/releases/latest")
            .expect_err("plain HTTP must be rejected");
        assert!(err.contains("http"), "error should name the scheme: {err}");
        assert!(err.contains("HTTPS"), "error should require HTTPS: {err}");

        assert!(validate_download_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn off_allowlist_hosts_are_rejected() {
        let err = validate_download_url("https://evil.example.com/shaders.zip")
            .expect_err("an off-allowlist host must be rejected");
        assert!(
            err.contains("evil.example.com"),
            "error should name the host: {err}"
        );
        assert!(
            err.contains("allowed list"),
            "error should mention the allowlist: {err}"
        );

        // A subdomain of an allowed host is not the allowed host.
        assert!(validate_download_url("https://fake.api.github.com/releases").is_err());
    }

    #[test]
    fn rejection_messages_do_not_echo_query_credentials() {
        let err = validate_download_url("https://evil.example.com/x?sig=SECRET&jwt=ALSOSECRET")
            .expect_err("an off-allowlist host must be rejected");
        assert!(!err.contains("SECRET"), "credentials leaked into: {err}");
    }

    #[test]
    fn unparseable_urls_are_rejected_without_being_echoed() {
        let err = validate_download_url("not a url?token=SECRET")
            .expect_err("an unparseable URL must be rejected");
        assert!(err.contains("Invalid URL"), "unexpected error: {err}");
        assert!(!err.contains("SECRET"), "credentials leaked into: {err}");
    }

    // --- redact_url ---

    #[test]
    fn redact_url_drops_the_query_string() {
        // GitHub's final asset hop carries `sig` and `jwt` credentials here.
        let redacted = redact_url(
            "https://release-assets.githubusercontent.com/asset/1140148702/abc?sig=SECRET&jwt=ALSOSECRET",
        );
        assert_eq!(
            redacted,
            "https://release-assets.githubusercontent.com/asset/1140148702/abc"
        );
        assert!(!redacted.contains("SECRET"));
    }

    #[test]
    fn redact_url_drops_userinfo() {
        assert_eq!(
            redact_url("https://user:SECRET@github.com/paulrobello/par-term"),
            "https://github.com/paulrobello/par-term"
        );
    }

    #[test]
    fn redact_url_does_not_echo_an_unparseable_url() {
        assert_eq!(redact_url("nonsense?token=SECRET"), "<unparseable URL>");
    }

    // --- resolve_redirect ---

    #[test]
    fn redirect_to_an_allowlisted_host_is_accepted() {
        let resolved = resolve_redirect(
            "https://github.com/paulrobello/par-term/releases/download/v1/shaders.zip",
            "https://release-assets.githubusercontent.com/asset/1?sig=abc",
        )
        .expect("an allowlisted redirect target must be accepted");
        assert!(resolved.starts_with("https://release-assets.githubusercontent.com/"));
    }

    #[test]
    fn relative_redirect_is_resolved_against_the_current_url() {
        let resolved = resolve_redirect(
            "https://github.com/paulrobello/par-term/releases/latest/download/shaders.zip",
            "/paulrobello/par-term/releases/download/v1/shaders.zip",
        )
        .expect("a relative redirect on the same host must be accepted");
        assert_eq!(
            resolved,
            "https://github.com/paulrobello/par-term/releases/download/v1/shaders.zip"
        );
    }

    #[test]
    fn redirect_off_the_allowlist_is_rejected() {
        let err = resolve_redirect(
            "https://github.com/paulrobello/par-term/releases/download/v1/shaders.zip",
            "https://evil.example.com/shaders.zip",
        )
        .expect_err("an off-allowlist redirect target must be rejected");
        assert!(
            err.contains("evil.example.com"),
            "error should name the rejected host: {err}"
        );
    }

    #[test]
    fn rejected_redirect_does_not_leak_query_credentials() {
        // The rejection message must name the host without echoing the query,
        // because a GitHub asset URL carries `sig` and `jwt` credentials there.
        let err = resolve_redirect(
            "https://github.com/paulrobello/par-term/releases/download/v1/shaders.zip",
            "https://evil.example.com/shaders.zip?sig=SECRET&jwt=ALSOSECRET",
        )
        .expect_err("an off-allowlist redirect target must be rejected");
        assert!(!err.contains("SECRET"), "credentials leaked into: {err}");
    }

    #[test]
    fn relative_redirect_cannot_escape_to_another_host() {
        // A protocol-relative Location changes host while looking relative.
        assert!(
            resolve_redirect(
                "https://github.com/paulrobello/par-term/releases/download/v1/shaders.zip",
                "//evil.example.com/shaders.zip",
            )
            .is_err()
        );
    }

    #[test]
    fn redirect_downgrading_to_http_is_rejected() {
        assert!(
            resolve_redirect(
                "https://github.com/paulrobello/par-term/releases/download/v1/shaders.zip",
                "http://github.com/paulrobello/par-term/releases/download/v1/shaders.zip",
            )
            .is_err()
        );
    }

    /// Live check that the real release-download chain still terminates on an
    /// allowlisted host, and that `max_redirects(0)` surfaces the 3xx to
    /// [`get_validated`] rather than erroring.
    ///
    /// Ignored by default because it needs the network. Run with
    /// `cargo test -p par-term --lib -- --ignored redirect_chain`.
    #[test]
    #[ignore = "requires network access to github.com"]
    fn live_release_redirect_chain_stays_on_allowlisted_hosts() {
        let response = get_validated(
            "https://github.com/paulrobello/par-term/releases/latest/download/shaders.zip",
            None,
        )
        .expect("the real shader download must survive per-hop validation");
        assert!(
            response.status().is_success(),
            "expected a 2xx after following redirects, got {}",
            response.status()
        );
    }
}
