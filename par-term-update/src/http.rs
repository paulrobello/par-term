//! HTTP client helper with native-tls support for the self-update subsystem.
//!
//! # Security Design
//!
//! All network requests made by the self-update subsystem go through
//! [`validate_update_url`] before any network I/O occurs. Two invariants are
//! enforced:
//!
//! 1. **HTTPS only** — plain HTTP, `file://`, and any other non-HTTPS scheme are
//!    rejected unconditionally. This prevents a network-level attacker from
//!    downgrading the connection and serving a malicious binary.
//!
//! 2. **Host allowlist** — only the GitHub hostnames in [`ALLOWED_HOSTS`] are
//!    accepted. This prevents a compromised DNS server or a SSRF-style redirect from
//!    pointing the updater at an attacker-controlled server.
//!
//! 3. **Per-hop revalidation** (SEC-005) — the allowlist is re-applied to *every*
//!    redirect target, not just to the URL the caller passed in. See
//!    [`get_validated`].
//!
//! Additionally, response bodies are capped at [`MAX_API_RESPONSE_SIZE`] (API calls)
//! and [`MAX_DOWNLOAD_SIZE`] (binary downloads) to prevent memory exhaustion, and
//! downloaded binaries are checked for the correct platform magic bytes via
//! [`validate_binary_content`].

use std::time::Duration;
use ureq::Agent;
use ureq::http::{Response, StatusCode};
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
use ureq::{Body, Error as UreqError};

/// Global timeout for all HTTP operations (30 seconds).
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum number of redirects followed manually by [`get_validated`].
///
/// GitHub release downloads take exactly two hops
/// (`github.com/…/releases/download/…` → `github.com/…/releases/download/<tag>/…`
/// → `release-assets.githubusercontent.com/…`), so this leaves headroom without
/// allowing a redirect loop to run indefinitely.
const MAX_REDIRECTS: u32 = 5;

/// Maximum response body size for API responses (10 MB).
pub const MAX_API_RESPONSE_SIZE: u64 = 10 * 1024 * 1024;

/// Maximum response body size for file downloads (50 MB).
pub const MAX_DOWNLOAD_SIZE: u64 = 50 * 1024 * 1024;

/// Allowlisted hostnames for update-related network requests.
///
/// Only requests to GitHub's primary API and CDN hosts are permitted.
/// Any other host is rejected regardless of the URL path, preventing SSRF
/// or DNS-rebinding attacks that could redirect update traffic to an
/// attacker-controlled server.
///
/// `release-assets.githubusercontent.com` is where GitHub currently terminates
/// a release-asset download, and it is **load-bearing**: a `browser_download_url`
/// redirects there, so removing it breaks every download now that redirects are
/// revalidated per hop. The two older `*.githubusercontent.com` CDN names are
/// kept because GitHub has rotated this host before.
const ALLOWED_HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "objects.githubusercontent.com",
    "github-releases.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

/// Validate that a URL is safe to use for update operations.
///
/// Enforces:
/// - HTTPS scheme only (no HTTP, ftp, file://, etc.)
/// - Host must be in the GitHub allowlist
///
/// Returns `Ok(())` if the URL is acceptable, or an error string describing
/// why it was rejected.
pub fn validate_update_url(url: &str) -> Result<(), String> {
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
    if !ALLOWED_HOSTS.contains(&host) {
        return Err(format!(
            "URL host '{}' is not in the allowed list for update operations. \
             Allowed hosts: {}. \
             URL: {}",
            host,
            ALLOWED_HOSTS.join(", "),
            url
        ));
    }

    Ok(())
}

/// Render a URL for logs and error messages with its query string removed.
///
/// GitHub's final release-asset hop carries short-lived credentials in the query
/// (`?sig=…&jwt=…`). Those are bearer-equivalent, so the full URL must never
/// reach the debug log — only scheme, host and path do.
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
        .map_err(|e| format!("Could not parse the current update URL: {}", e))?;

    let resolved = base.join(location).map_err(|e| {
        format!(
            "Update server redirected from {} to an unparseable Location: {}. \
             Update aborted.",
            redact_url(current),
            e
        )
    })?;

    let resolved = resolved.to_string();
    // Deliberately does *not* forward `validate_update_url`'s message: that one
    // interpolates the whole URL, and a rejected redirect target still carries
    // GitHub's `sig`/`jwt` query credentials.
    if validate_update_url(&resolved).is_err() {
        return Err(format!(
            "Update server redirected from {} to {}, which is not an allowed \
             update host. Update aborted — a redirect cannot move the download \
             off GitHub. Allowed hosts: {}.",
            redact_url(current),
            redact_url(&resolved),
            ALLOWED_HOSTS.join(", ")
        ));
    }

    Ok(resolved)
}

/// Create a new HTTP agent configured with native-tls and a global timeout.
///
/// Redirects are disabled at the agent level (`max_redirects(0)`) so that
/// [`get_validated`] can follow them by hand and re-apply the allowlist at every
/// hop. ureq's default is to follow up to ten redirects with no revalidation,
/// which would let an allowlisted host hand the download off to anywhere.
///
/// `https_only(true)` is belt-and-braces: [`validate_update_url`] already rejects
/// non-HTTPS schemes, but ureq's default is `false` and a config-level guarantee
/// costs nothing.
pub fn agent() -> Agent {
    let tls_config = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();

    Agent::config_builder()
        .tls_config(tls_config)
        .timeout_global(Some(HTTP_TIMEOUT))
        .https_only(true)
        .max_redirects(0)
        .build()
        .into()
}

/// Describe a failed request without leaking the query string.
fn describe_request_error(url: &str, error: &UreqError) -> String {
    format!(
        "Failed to fetch '{}': {}. \
         Check your internet connection and try again. \
         If the problem persists, download manually from: \
         https://github.com/paulrobello/par-term/releases",
        redact_url(url),
        error
    )
}

/// Perform a GET, following redirects manually with the allowlist re-applied at
/// every hop.
///
/// The URL the caller passes is validated before the first request, and each
/// `Location` is resolved and validated before the next one. A redirect to a
/// host outside [`ALLOWED_HOSTS`], a redirect with no `Location`, and a chain
/// longer than [`MAX_REDIRECTS`] are all hard errors — none of them fall through
/// to reading a body.
///
/// This is hardening rather than a fix for an observed exploit: no redirect off
/// an allowlisted GitHub host has been demonstrated. It closes the gap that the
/// allowlist was only ever checked against the *first* URL.
pub fn get_validated(url: &str, accept: Option<&str>) -> Result<Response<Body>, String> {
    let agent = agent();
    let mut current = url.to_string();

    for _ in 0..=MAX_REDIRECTS {
        // Re-validated on every iteration, not just the first.
        validate_update_url(&current)?;

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
        "Update download exceeded {} redirects starting from {}. \
         Update aborted — this may indicate a redirect loop.",
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
                "Update server returned redirect status {} from {} with no usable \
                 Location header. Update aborted.",
                status.as_u16(),
                redact_url(current)
            )
        })
}

/// Download a file from a URL and return its bytes.
///
/// Validates the URL against the allowed-host allowlist before making any
/// network request, and again at every redirect hop. Response body is limited to
/// [`MAX_DOWNLOAD_SIZE`] (50 MB) to prevent memory exhaustion from malicious or
/// misbehaving servers.
///
/// # Errors
///
/// Returns an error if:
/// - The URL, or any redirect target, fails allowlist validation
/// - The HTTP request fails (DNS, connection, TLS, or non-2xx response)
/// - Reading the response body fails or exceeds the size limit
pub fn download_file(url: &str) -> Result<Vec<u8>, String> {
    let bytes = get_validated(url, None)?
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
        })?;

    Ok(bytes)
}

/// Validate that downloaded binary content is plausible for the current platform.
///
/// This is a lightweight sanity check — not a security guarantee — that catches
/// obviously wrong content (e.g., an HTML error page served instead of a binary).
///
/// On macOS, the content must begin with a ZIP local-file signature (`PK\x03\x04`)
/// because macOS releases are distributed as `.zip` archives.
/// On Linux, the content must begin with the ELF magic bytes (`\x7fELF`).
/// On Windows, the content must begin with the PE `MZ` header.
///
/// Returns `Ok(())` if the content looks valid, or an error string with
/// a human-readable description of what was expected vs. found.
pub fn validate_binary_content(data: &[u8]) -> Result<(), String> {
    let os = std::env::consts::OS;

    match os {
        "macos" => {
            // macOS releases ship as ZIP archives
            if data.len() < 4 || &data[..4] != b"PK\x03\x04" {
                let preview = format_bytes_preview(data);
                return Err(format!(
                    "Downloaded content does not look like a ZIP archive (expected PK\\x03\\x04 \
                     header for macOS release). Got: {}. \
                     This may indicate a corrupt download or an unexpected server response. \
                     Please try again or download manually from: \
                     https://github.com/paulrobello/par-term/releases",
                    preview
                ));
            }
        }
        "linux" => {
            // Linux releases are raw ELF binaries
            if data.len() < 4 || &data[..4] != b"\x7fELF" {
                let preview = format_bytes_preview(data);
                return Err(format!(
                    "Downloaded content does not look like an ELF binary (expected \\x7fELF \
                     header for Linux release). Got: {}. \
                     This may indicate a corrupt download or an unexpected server response. \
                     Please try again or download manually from: \
                     https://github.com/paulrobello/par-term/releases",
                    preview
                ));
            }
        }
        "windows" => {
            // Windows releases are PE executables
            if data.len() < 2 || &data[..2] != b"MZ" {
                let preview = format_bytes_preview(data);
                return Err(format!(
                    "Downloaded content does not look like a Windows executable (expected MZ \
                     header for Windows release). Got: {}. \
                     This may indicate a corrupt download or an unexpected server response. \
                     Please try again or download manually from: \
                     https://github.com/paulrobello/par-term/releases",
                    preview
                ));
            }
        }
        other => {
            // Unknown platform — log a warning but do not block the update.
            log::warn!(
                "Binary content validation skipped: unknown platform '{}'. \
                 Proceeding without magic-byte check.",
                other
            );
        }
    }

    Ok(())
}

/// Format the first few bytes of a buffer as a human-readable hex + ASCII preview.
///
/// Used in error messages to help diagnose what was actually downloaded.
fn format_bytes_preview(data: &[u8]) -> String {
    let take = data.len().min(16);
    let hex: Vec<String> = data[..take].iter().map(|b| format!("{:02x}", b)).collect();
    let ascii: String = data[..take]
        .iter()
        .map(|&b| if b.is_ascii_graphic() { b as char } else { '.' })
        .collect();
    format!("[{}] \"{}\"", hex.join(" "), ascii)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_update_url ---

    #[test]
    fn test_valid_api_github_com() {
        assert!(
            validate_update_url(
                "https://api.github.com/repos/paulrobello/par-term/releases/latest"
            )
            .is_ok()
        );
    }

    #[test]
    fn test_valid_objects_githubusercontent_com() {
        assert!(validate_update_url(
            "https://objects.githubusercontent.com/github-production-release-asset-123/par-term-linux-x86_64"
        )
        .is_ok());
    }

    #[test]
    fn test_valid_github_releases() {
        assert!(
            validate_update_url(
                "https://github-releases.githubusercontent.com/123/par-term-linux-x86_64"
            )
            .is_ok()
        );
    }

    #[test]
    fn test_valid_github_com() {
        assert!(validate_update_url("https://github.com/paulrobello/par-term/releases").is_ok());
    }

    #[test]
    fn test_rejected_http_scheme() {
        let result =
            validate_update_url("http://api.github.com/repos/paulrobello/par-term/releases/latest");
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("http"),
            "Error should mention the bad scheme: {msg}"
        );
        assert!(
            msg.contains("HTTPS"),
            "Error should mention HTTPS requirement: {msg}"
        );
    }

    #[test]
    fn test_rejected_file_scheme() {
        let result = validate_update_url("file:///etc/passwd");
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("file"),
            "Error should mention the bad scheme: {msg}"
        );
    }

    #[test]
    fn test_rejected_unknown_host() {
        let result = validate_update_url("https://evil.example.com/par-term-linux-x86_64");
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("evil.example.com"),
            "Error should name the rejected host: {msg}"
        );
        assert!(
            msg.contains("allowed list"),
            "Error should mention the allowlist: {msg}"
        );
    }

    #[test]
    fn test_rejected_lookalike_host() {
        // Subdomain-of-allowed is NOT the same as the allowed host itself.
        let result = validate_update_url("https://fake.api.github.com/releases");
        assert!(result.is_err());
    }

    #[test]
    fn test_rejected_invalid_url() {
        let result = validate_update_url("not a url at all");
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("Invalid URL"),
            "Error should mention parse failure: {msg}"
        );
    }

    // --- validate_binary_content ---

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos_valid_zip() {
        // ZIP local-file header magic
        let data = b"PK\x03\x04rest of zip content";
        assert!(validate_binary_content(data).is_ok());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos_invalid_not_zip() {
        let data = b"<html>404 Not Found</html>";
        let result = validate_binary_content(data);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("ZIP"), "Error should mention ZIP: {msg}");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_valid_elf() {
        let data = b"\x7fELFrest of elf binary";
        assert!(validate_binary_content(data).is_ok());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_invalid_not_elf() {
        let data = b"<html>404 Not Found</html>";
        let result = validate_binary_content(data);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("ELF"), "Error should mention ELF: {msg}");
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_valid_pe() {
        let data = b"MZrest of PE binary";
        assert!(validate_binary_content(data).is_ok());
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_invalid_not_pe() {
        let data = b"<html>404 Not Found</html>";
        let result = validate_binary_content(data);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("MZ"), "Error should mention MZ: {msg}");
    }

    #[test]
    fn test_validate_binary_content_empty() {
        // Empty data should fail on all recognized platforms since headers are
        // missing, and pass on unknown platforms (no-op path).
        let data: &[u8] = &[];
        let os = std::env::consts::OS;
        let result = validate_binary_content(data);
        match os {
            "macos" | "linux" | "windows" => {
                assert!(result.is_err(), "Empty data should be rejected on {os}");
            }
            _ => {
                // Unknown platform: validation is skipped, so result is Ok.
                assert!(result.is_ok());
            }
        }
    }

    // --- redact_url ---

    #[test]
    fn redact_url_drops_the_query_string() {
        // GitHub's final asset hop carries `sig` and `jwt` credentials here.
        let redacted = redact_url(
            "https://release-assets.githubusercontent.com/github-production-release-asset/1140148702/abc?sig=SECRET&jwt=ALSOSECRET",
        );
        assert_eq!(
            redacted,
            "https://release-assets.githubusercontent.com/github-production-release-asset/1140148702/abc"
        );
        assert!(!redacted.contains("SECRET"));
    }

    #[test]
    fn redact_url_does_not_echo_an_unparseable_url() {
        assert_eq!(redact_url("nonsense?token=SECRET"), "<unparseable URL>");
    }

    // --- resolve_redirect ---

    #[test]
    fn redirect_to_an_allowlisted_host_is_accepted() {
        let resolved = resolve_redirect(
            "https://github.com/paulrobello/par-term/releases/download/v1/par-term",
            "https://release-assets.githubusercontent.com/asset/1?sig=abc",
        )
        .expect("an allowlisted redirect target must be accepted");
        assert!(resolved.starts_with("https://release-assets.githubusercontent.com/"));
    }

    #[test]
    fn relative_redirect_is_resolved_against_the_current_url() {
        let resolved = resolve_redirect(
            "https://github.com/paulrobello/par-term/releases/latest/download/par-term",
            "/paulrobello/par-term/releases/download/v1/par-term",
        )
        .expect("a relative redirect on the same host must be accepted");
        assert_eq!(
            resolved,
            "https://github.com/paulrobello/par-term/releases/download/v1/par-term"
        );
    }

    #[test]
    fn redirect_off_the_allowlist_is_rejected() {
        // The core of SEC-005's redirect leg: an allowlisted host must not be
        // able to hand the download to an arbitrary server.
        let err = resolve_redirect(
            "https://github.com/paulrobello/par-term/releases/download/v1/par-term",
            "https://evil.example.com/par-term",
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
            "https://github.com/paulrobello/par-term/releases/download/v1/par-term",
            "https://evil.example.com/par-term?sig=SECRET&jwt=ALSOSECRET",
        )
        .expect_err("an off-allowlist redirect target must be rejected");
        assert!(!err.contains("SECRET"), "credentials leaked into: {err}");
    }

    #[test]
    fn allowlist_contains_the_host_release_downloads_actually_land_on() {
        // Load-bearing: `browser_download_url` 302s here, and per-hop validation
        // means dropping this host breaks every self-update. Proven by
        // `live_release_redirect_chain_stays_on_allowlisted_hosts`.
        assert!(ALLOWED_HOSTS.contains(&"release-assets.githubusercontent.com"));
    }

    #[test]
    fn relative_redirect_cannot_escape_to_another_host() {
        // A protocol-relative Location changes host while looking relative.
        assert!(
            resolve_redirect(
                "https://github.com/paulrobello/par-term/releases/download/v1/par-term",
                "//evil.example.com/par-term",
            )
            .is_err()
        );
    }

    #[test]
    fn redirect_downgrading_to_http_is_rejected() {
        assert!(
            resolve_redirect(
                "https://github.com/paulrobello/par-term/releases/download/v1/par-term",
                "http://github.com/paulrobello/par-term/releases/download/v1/par-term",
            )
            .is_err()
        );
    }

    /// Live check that the real release-download chain still terminates on an
    /// allowlisted host, and that `max_redirects(0)` surfaces the 3xx to
    /// [`get_validated`] rather than erroring.
    ///
    /// Ignored by default because it needs the network. Run with
    /// `cargo test -p par-term-update -- --ignored redirect_chain`.
    #[test]
    #[ignore = "requires network access to github.com"]
    fn live_release_redirect_chain_stays_on_allowlisted_hosts() {
        let response = get_validated(
            "https://github.com/paulrobello/par-term/releases/latest/download/par-term-macos-aarch64.zip",
            None,
        )
        .expect("the real release download must survive per-hop validation");
        assert!(
            response.status().is_success(),
            "expected a 2xx after following redirects, got {}",
            response.status()
        );
    }

    // --- format_bytes_preview ---

    #[test]
    fn test_format_bytes_preview_short() {
        let preview = format_bytes_preview(b"PK");
        assert!(
            preview.contains("50 4b"),
            "Should contain hex for 'PK': {preview}"
        );
        assert!(
            preview.contains("PK"),
            "Should contain ASCII for 'PK': {preview}"
        );
    }

    #[test]
    fn test_format_bytes_preview_non_ascii() {
        let preview = format_bytes_preview(b"\x7f\x00\xff");
        // Non-printable bytes should appear as '.'
        assert!(
            preview.contains("..."),
            "Non-printable bytes should show as dots: {preview}"
        );
    }
}
