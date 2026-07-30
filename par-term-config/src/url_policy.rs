//! Scheme policy for caller-supplied URLs (ARC-006).
//!
//! par-term fetches over the network from four places, and the URL validation
//! in front of each one had drifted into near-identical hand-maintained copies.
//! They fall into two clusters:
//!
//! * **Caller-supplied hosts** — the dynamic-profile fetch and the preferences
//!   import. The user names the host, so there is no allowlist to check; the
//!   control is the *scheme*. That policy lives here, and both call sites now
//!   share it.
//! * **Fixed-vendor hosts** — the shader downloader (`par_term::http`) and the
//!   self-updater (`par_term_update::http`). Those two additionally pin the
//!   host to a GitHub allowlist and remain near-duplicates of each other.
//!
//! # Why a scheme allowlist and not a `file://` denylist
//!
//! The previous per-call-site checks tested `url.starts_with("https://")` and
//! special-cased `file://`. That is a denylist, and it leaked in two ways:
//! `HTTPS://` was misclassified as insecure (schemes are case-insensitive under
//! RFC 3986), and once a user set the HTTP opt-in, *every* remaining scheme —
//! `ftp:`, `data:`, and on some builds `file:` spelled with a single slash —
//! was accepted. [`validate_scheme`] allowlists `https`, plus `http` only under
//! an explicit opt-in, and rejects everything else unconditionally.
//!
//! # Why this lives in `par-term-config`
//!
//! It started in the root crate, which put it out of reach of
//! `par-term-settings-ui` — one of the two call sites. `par-term-config` is the
//! lowest layer both the root crate and the settings UI already depend on, so
//! it is the only place a single copy can serve both.
//!
//! Keep this module dependency-free: no `url` crate, no config types. Folding
//! in the *fixed-vendor* cluster would additionally need host extraction, and
//! therefore `url` as a dependency of this crate — which is why that cluster
//! has not been merged in.

/// The transport a URL resolved to after policy validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// `https://` — the only scheme allowed without an explicit opt-in.
    Https,
    /// `http://`, reachable only when the caller passed `allow_http = true`.
    Http,
}

/// Extract the URI scheme: everything before the first `:`.
///
/// Returns `None` when the URL has no scheme, or when the candidate violates
/// RFC 3986's `ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )` production — a
/// bare `example.com/x` or a Windows-style `C:\path` both fail here.
fn scheme_of(url: &str) -> Option<&str> {
    let scheme = url.split(':').next()?;
    if scheme.is_empty() || scheme.len() == url.len() {
        // Empty scheme, or no `:` in the string at all.
        return None;
    }

    let mut chars = scheme.chars();
    if !chars.next()?.is_ascii_alphabetic() {
        return None;
    }
    if chars.any(|c| !(c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))) {
        return None;
    }

    Some(scheme)
}

/// Validate the scheme of a caller-supplied URL.
///
/// `allow_http` reflects an explicit user opt-in to plaintext transport. Every
/// scheme other than `https` — and `http` under the opt-in — is rejected, so
/// `file:`, `ftp:` and `data:` cannot reach the network or filesystem layer no
/// matter how the opt-in is set.
///
/// # Errors
///
/// Returns a complete, user-facing sentence explaining the rejection.
pub fn validate_scheme(url: &str, allow_http: bool) -> Result<Transport, String> {
    let Some(scheme) = scheme_of(url) else {
        return Err(format!(
            "URL '{url}' has no usable scheme; only https:// URLs are supported."
        ));
    };

    // RFC 3986 schemes are case-insensitive: `HTTPS://` is a valid https URL.
    match scheme.to_ascii_lowercase().as_str() {
        "https" => Ok(Transport::Https),
        "http" if allow_http => Ok(Transport::Http),
        "http" => Err(format!(
            "URL '{url}' uses insecure HTTP, which a network-level attacker can \
             intercept and replace. Use https://, or enable the explicit HTTP \
             opt-in if you accept that risk."
        )),
        other => Err(format!(
            "URL '{url}' uses the '{other}' scheme, which is not permitted; only \
             https:// (or http:// with an explicit opt-in) is supported."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_is_always_accepted() {
        assert_eq!(
            validate_scheme("https://example.com/profiles.yaml", false),
            Ok(Transport::Https)
        );
        assert_eq!(
            validate_scheme("https://example.com/profiles.yaml", true),
            Ok(Transport::Https)
        );
    }

    #[test]
    fn scheme_matching_is_case_insensitive() {
        // The old `starts_with("https://")` check misclassified this as insecure.
        assert_eq!(
            validate_scheme("HTTPS://example.com/x", false),
            Ok(Transport::Https)
        );
        assert_eq!(
            validate_scheme("HtTp://example.com/x", true),
            Ok(Transport::Http)
        );
    }

    #[test]
    fn http_requires_the_opt_in() {
        let err = validate_scheme("http://example.com/x", false).expect_err("must reject");
        assert!(err.contains("insecure HTTP"), "unexpected error: {err}");

        assert_eq!(
            validate_scheme("http://example.com/x", true),
            Ok(Transport::Http)
        );
    }

    #[test]
    fn every_other_scheme_is_rejected_even_with_the_http_opt_in() {
        // The opt-in must widen the policy to `http` only — not to "anything
        // that is not https", which is what the previous per-site checks did.
        for url in [
            "file:///etc/passwd",
            "FILE://localhost/etc/passwd",
            "file:/etc/passwd",
            "ftp://example.com/x",
            "data:text/yaml;base64,LQo=",
            "javascript:alert(1)",
        ] {
            let Err(err) = validate_scheme(url, true) else {
                panic!("{url} must be rejected even with the HTTP opt-in");
            };
            assert!(
                err.contains("not permitted"),
                "unexpected error for {url}: {err}"
            );
        }
    }

    #[test]
    fn schemeless_and_malformed_urls_are_rejected() {
        for url in [
            "example.com/profiles.yaml",
            "",
            "://example.com",
            "1http://example.com",
            "/tmp/local:file.yaml",
        ] {
            let Err(err) = validate_scheme(url, true) else {
                panic!("{url:?} must be rejected");
            };
            assert!(
                err.contains("no usable scheme"),
                "unexpected error for {url:?}: {err}"
            );
        }
    }

    #[test]
    fn scheme_extraction_stops_at_the_first_colon() {
        assert_eq!(scheme_of("https://user:pass@example.com"), Some("https"));
        assert_eq!(scheme_of("http://example.com:8443/x"), Some("http"));
        assert_eq!(scheme_of("no-colon-here"), None);
    }
}
