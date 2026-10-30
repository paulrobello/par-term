//! Validated HTTP for shader downloads.
//!
//! The implementation lives in [`par_term_update::http`], par-term's single
//! validated-HTTP surface (AUDIT.md ARC-004): HTTPS-only, a host allowlist
//! re-applied at *every* redirect hop, a 50 MB body cap, and URL redaction in
//! every error. This module owns only the shader downloader's policy,
//! [`DOWNLOAD_POLICY`], and binds the shared functions to it.
//!
//! Per-hop revalidation is hardening, not a fix for an observed exploit: no
//! redirect off an allowlisted GitHub host has been demonstrated. It closes the
//! gap that the allowlist was only ever checked once, before the first request.

use par_term_update::http::{self as validated, HostPolicy};
use ureq::Body;
use ureq::http::Response;

pub use par_term_update::http::MAX_DOWNLOAD_SIZE;

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

/// The shader downloader's host policy.
pub const DOWNLOAD_POLICY: HostPolicy = HostPolicy {
    allowed_hosts: ALLOWED_DOWNLOAD_HOSTS,
    noun: "download",
    noun_title: "Download",
    request_error_hint: "",
};

/// Validate that a URL is safe to use for shader download operations.
///
/// Enforces HTTPS and [`DOWNLOAD_POLICY`]'s host allowlist. Rejected URLs are
/// redacted in the error message.
pub fn validate_download_url(url: &str) -> Result<(), String> {
    validated::validate_url_for(url, &DOWNLOAD_POLICY)
}

/// Perform a GET, following redirects manually with [`DOWNLOAD_POLICY`]
/// re-applied at every hop. See [`par_term_update::http::get_validated_for`].
///
/// # Errors
///
/// Returns an error if the URL or any redirect target fails allowlist
/// validation, if the request fails, if a redirect carries no usable
/// `Location`, or if the redirect chain is too long.
pub fn get_validated(url: &str, accept: Option<&str>) -> Result<Response<Body>, String> {
    validated::get_validated_for(url, accept, &DOWNLOAD_POLICY)
}

/// Download a file under [`DOWNLOAD_POLICY`] and return its bytes, capped at
/// [`MAX_DOWNLOAD_SIZE`]. See [`par_term_update::http::download_file_for`].
///
/// # Errors
///
/// Returns an error if the URL, or any redirect target, fails allowlist
/// validation; if the HTTP request fails; or if reading the body fails or
/// exceeds the size limit.
pub fn download_file(url: &str) -> Result<Vec<u8>, String> {
    validated::download_file_for(url, &DOWNLOAD_POLICY)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The shared enforcement (redirect resolution, redaction, scheme checks) is
    // tested in par-term-update; these pin the shader downloader's policy.

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
            err.contains("allowed list for download operations"),
            "error should mention the download allowlist: {err}"
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
