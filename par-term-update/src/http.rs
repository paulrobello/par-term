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

/// Allowlisted hostnames for update-related network requests.
///
/// Only requests to GitHub's primary API and CDN hosts are permitted.
/// Any other host is rejected regardless of the URL path.
const ALLOWED_HOSTS: &[&str] = &[
    "github.com",
    "api.github.com",
    "objects.githubusercontent.com",
    "github-releases.githubusercontent.com",
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
/// Validates the URL against the allowed-host allowlist before making
/// any network request. Response body is limited to [`MAX_DOWNLOAD_SIZE`]
/// (50 MB) to prevent memory exhaustion from malicious or misbehaving servers.
///
/// # Errors
///
/// Returns an error if:
/// - The URL fails allowlist validation (wrong host or non-HTTPS scheme)
/// - The HTTP request fails (DNS, connection, TLS, or non-2xx response)
/// - Reading the response body fails or exceeds the size limit
pub fn download_file(url: &str) -> Result<Vec<u8>, String> {
    // Validate URL before making any network request.
    validate_update_url(url)?;

    let bytes = agent()
        .get(url)
        .header("User-Agent", "par-term")
        .call()
        .map_err(|e| {
            format!(
                "Failed to download '{}': {}. \
                 Check your internet connection and try again. \
                 If the problem persists, download manually from: \
                 https://github.com/paulrobello/par-term/releases",
                url, e
            )
        })?
        .into_body()
        .with_config()
        .limit(MAX_DOWNLOAD_SIZE)
        .read_to_vec()
        .map_err(|e| {
            format!(
                "Failed to read downloaded content from '{}': {}. \
                 The response may have been truncated or the connection dropped.",
                url, e
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
