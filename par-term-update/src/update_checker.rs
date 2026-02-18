//! Automatic update checking for par-term.
//!
//! This module handles checking GitHub releases for new versions of par-term.
//! It respects the configured check frequency (daily, weekly, monthly, or never)
//! and can notify users when updates are available.

use par_term_config::{Config, UpdateCheckFrequency};
use chrono::{DateTime, Utc};
use semver::Version;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Repository for update checks
const REPO: &str = "paulrobello/par-term";

/// GitHub API URL for latest release
const RELEASE_API_URL: &str = "https://api.github.com/repos/paulrobello/par-term/releases/latest";

/// Information about an available update
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// The new version available
    pub version: String,
    /// Release notes/body from GitHub
    pub release_notes: Option<String>,
    /// URL to the release page
    pub release_url: String,
    /// When the release was published
    pub published_at: Option<String>,
}

/// Result of an update check
#[derive(Debug, Clone)]
pub enum UpdateCheckResult {
    /// No update available - current version is latest
    UpToDate,
    /// A new version is available
    UpdateAvailable(UpdateInfo),
    /// Update check is disabled
    Disabled,
    /// Check was skipped (not enough time since last check)
    Skipped,
    /// Error occurred during check
    Error(String),
}

/// Manages update checking with periodic checks while running
pub struct UpdateChecker {
    /// Last check result (shared for UI access)
    last_result: Arc<Mutex<Option<UpdateCheckResult>>>,
    /// Whether a check is currently in progress
    check_in_progress: Arc<AtomicBool>,
    /// Time of last check attempt (for rate limiting)
    last_check_time: Arc<Mutex<Option<Instant>>>,
    /// Minimum time between checks (prevents hammering the API)
    min_check_interval: Duration,
}

impl Default for UpdateChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateChecker {
    /// Create a new update checker
    pub fn new() -> Self {
        Self {
            last_result: Arc::new(Mutex::new(None)),
            check_in_progress: Arc::new(AtomicBool::new(false)),
            last_check_time: Arc::new(Mutex::new(None)),
            // Don't check more than once per hour even if forced
            min_check_interval: Duration::from_secs(3600),
        }
    }

    /// Get the last check result
    pub fn last_result(&self) -> Option<UpdateCheckResult> {
        self.last_result.lock().ok()?.clone()
    }

    /// Check if it's time to perform an update check based on config
    pub fn should_check(&self, config: &Config) -> bool {
        // Never check if disabled
        if config.update_check_frequency == UpdateCheckFrequency::Never {
            return false;
        }

        // Get duration since last check based on config
        let Some(check_interval_secs) = config.update_check_frequency.as_seconds() else {
            return false;
        };

        // Check if we have a last check timestamp
        let Some(ref last_check_str) = config.last_update_check else {
            // Never checked before, should check
            return true;
        };

        // Parse the timestamp
        let Ok(last_check) = DateTime::parse_from_rfc3339(last_check_str) else {
            // Invalid timestamp, should check
            return true;
        };

        // Check if enough time has passed
        let now = Utc::now();
        let elapsed = now.signed_duration_since(last_check.with_timezone(&Utc));
        let elapsed_secs = elapsed.num_seconds();

        elapsed_secs >= check_interval_secs as i64
    }

    /// Check if we're rate-limited (prevent hammering the API)
    fn is_rate_limited(&self) -> bool {
        if let Ok(last_time) = self.last_check_time.lock()
            && let Some(last) = *last_time
        {
            return last.elapsed() < self.min_check_interval;
        }
        false
    }

    /// Perform an update check (blocking)
    ///
    /// Returns the check result and whether the config should be updated
    /// (to save the new last_update_check timestamp).
    pub fn check_now(&self, config: &Config, force: bool) -> (UpdateCheckResult, bool) {
        // Check if disabled
        if config.update_check_frequency == UpdateCheckFrequency::Never && !force {
            return (UpdateCheckResult::Disabled, false);
        }

        // Check if we should skip based on timing
        if !force && !self.should_check(config) {
            return (UpdateCheckResult::Skipped, false);
        }

        // Check rate limiting (even for forced checks)
        if !force && self.is_rate_limited() {
            return (UpdateCheckResult::Skipped, false);
        }

        // Prevent concurrent checks
        if self
            .check_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return (UpdateCheckResult::Skipped, false);
        }

        // Update last check time
        if let Ok(mut last_time) = self.last_check_time.lock() {
            *last_time = Some(Instant::now());
        }

        // Perform the actual check
        let result = self.perform_check(config);

        // Store result
        if let Ok(mut last_result) = self.last_result.lock() {
            *last_result = Some(result.clone());
        }

        // Release the check lock
        self.check_in_progress.store(false, Ordering::SeqCst);

        // Should save config if check was successful (to update timestamp)
        let should_save = !matches!(result, UpdateCheckResult::Error(_));

        (result, should_save)
    }

    /// Perform the actual HTTP request and version comparison
    fn perform_check(&self, config: &Config) -> UpdateCheckResult {
        // Get current version
        let current_version_str = env!("CARGO_PKG_VERSION");
        let current_version = match Version::parse(current_version_str) {
            Ok(v) => v,
            Err(e) => {
                return UpdateCheckResult::Error(format!(
                    "Failed to parse current version '{}': {}",
                    current_version_str, e
                ));
            }
        };

        // Fetch latest release info from GitHub
        let release_info = match fetch_latest_release() {
            Ok(info) => info,
            Err(e) => return UpdateCheckResult::Error(e),
        };

        // Parse the release version (strip leading 'v' if present)
        let version_str = release_info
            .version
            .strip_prefix('v')
            .unwrap_or(&release_info.version);
        let latest_version = match Version::parse(version_str) {
            Ok(v) => v,
            Err(e) => {
                return UpdateCheckResult::Error(format!(
                    "Failed to parse latest version '{}': {}",
                    release_info.version, e
                ));
            }
        };

        // Compare versions
        if latest_version > current_version {
            // Check if user skipped this version
            if let Some(ref skipped) = config.skipped_version
                && (skipped == version_str || skipped == &release_info.version)
            {
                return UpdateCheckResult::UpToDate;
            }

            UpdateCheckResult::UpdateAvailable(release_info)
        } else {
            UpdateCheckResult::UpToDate
        }
    }
}

/// Fetch the latest release information from GitHub API
pub fn fetch_latest_release() -> Result<UpdateInfo, String> {
    let mut body = crate::http::agent()
        .get(RELEASE_API_URL)
        .header("User-Agent", "par-term")
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Failed to fetch release info: {}", e))?
        .into_body();

    let body_str = body
        .read_to_string()
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    // Parse JSON (simple extraction without full JSON parsing)
    let version = extract_json_string(&body_str, "tag_name")
        .ok_or_else(|| "Could not find tag_name in release response".to_string())?;

    let release_url = extract_json_string(&body_str, "html_url")
        .unwrap_or_else(|| format!("https://github.com/{}/releases/latest", REPO));

    let release_notes = extract_json_string(&body_str, "body");
    let published_at = extract_json_string(&body_str, "published_at");

    Ok(UpdateInfo {
        version,
        release_notes,
        release_url,
        published_at,
    })
}

/// Extract a string value from JSON (simple extraction without full parsing)
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let search_pattern = format!("\"{}\":\"", key);
    let start_idx = json.find(&search_pattern)? + search_pattern.len();
    let remaining = &json[start_idx..];

    // Find the closing quote, handling escaped quotes
    let mut chars = remaining.chars().peekable();
    let mut value = String::new();
    let mut escaped = false;

    for ch in chars.by_ref() {
        if escaped {
            match ch {
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                '\\' => value.push('\\'),
                '"' => value.push('"'),
                _ => {
                    value.push('\\');
                    value.push(ch);
                }
            }
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            break;
        } else {
            value.push(ch);
        }
    }

    if value.is_empty() { None } else { Some(value) }
}

/// Get the current timestamp in ISO 8601 format
pub fn current_timestamp() -> String {
    Utc::now().to_rfc3339()
}

/// Format a timestamp for display
pub fn format_timestamp(timestamp: &str) -> String {
    match DateTime::parse_from_rfc3339(timestamp) {
        Ok(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        Err(_) => timestamp.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        let v1 = Version::parse("0.5.0").unwrap();
        let v2 = Version::parse("0.6.0").unwrap();
        assert!(v2 > v1);

        let v3 = Version::parse("1.0.0").unwrap();
        assert!(v3 > v2);
    }

    #[test]
    fn test_extract_json_string() {
        let json = r#"{"tag_name":"v0.6.0","html_url":"https://example.com"}"#;
        assert_eq!(
            extract_json_string(json, "tag_name"),
            Some("v0.6.0".to_string())
        );
        assert_eq!(
            extract_json_string(json, "html_url"),
            Some("https://example.com".to_string())
        );
        assert_eq!(extract_json_string(json, "missing"), None);
    }

    #[test]
    fn test_extract_json_string_with_escapes() {
        let json = r#"{"body":"Line 1\nLine 2\tTabbed"}"#;
        assert_eq!(
            extract_json_string(json, "body"),
            Some("Line 1\nLine 2\tTabbed".to_string())
        );
    }

    #[test]
    fn test_update_check_frequency_seconds() {
        assert_eq!(UpdateCheckFrequency::Never.as_seconds(), None);
        assert_eq!(UpdateCheckFrequency::Daily.as_seconds(), Some(86400));
        assert_eq!(UpdateCheckFrequency::Weekly.as_seconds(), Some(604800));
        assert_eq!(UpdateCheckFrequency::Monthly.as_seconds(), Some(2592000));
    }

    #[test]
    fn test_should_check_never() {
        let checker = UpdateChecker::new();
        let config = Config {
            update_check_frequency: UpdateCheckFrequency::Never,
            ..Default::default()
        };
        assert!(!checker.should_check(&config));
    }

    #[test]
    fn test_should_check_no_previous() {
        let checker = UpdateChecker::new();
        let config = Config {
            update_check_frequency: UpdateCheckFrequency::Weekly,
            last_update_check: None,
            ..Default::default()
        };
        assert!(checker.should_check(&config));
    }

    #[test]
    fn test_should_check_time_elapsed() {
        let checker = UpdateChecker::new();
        let mut config = Config {
            update_check_frequency: UpdateCheckFrequency::Daily,
            ..Default::default()
        };

        // Set last check to 2 days ago
        let two_days_ago = Utc::now() - chrono::Duration::days(2);
        config.last_update_check = Some(two_days_ago.to_rfc3339());
        assert!(checker.should_check(&config));

        // Set last check to 1 hour ago
        let one_hour_ago = Utc::now() - chrono::Duration::hours(1);
        config.last_update_check = Some(one_hour_ago.to_rfc3339());
        assert!(!checker.should_check(&config));
    }

    #[test]
    fn test_current_timestamp_format() {
        let ts = current_timestamp();
        // Should be parseable as RFC 3339
        assert!(DateTime::parse_from_rfc3339(&ts).is_ok());
    }
}
