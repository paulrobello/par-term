//! Tab profile and title tracking methods.
//!
//! Provides methods for auto-updating tab titles from OSC sequences, tracking
//! hostname/CWD changes for automatic profile switching, and managing the
//! auto-profile lifecycle.

use crate::tab::Tab;
use crate::ui_constants::VISUAL_BELL_FLASH_DURATION_MS;

impl Tab {
    /// Check if the visual bell is currently active (within flash duration)
    pub fn is_bell_active(&self) -> bool {
        // Use active_bell() to route through the focused pane in split-pane mode.
        if let Some(flash_start) = self.active_bell().visual_flash {
            flash_start.elapsed().as_millis() < VISUAL_BELL_FLASH_DURATION_MS
        } else {
            false
        }
    }

    /// Update tab title from terminal OSC sequences
    pub fn update_title(&mut self, title_mode: par_term_config::TabTitleMode) {
        // User-named tabs are static — never auto-update
        if self.user_named {
            return;
        }
        if let Ok(term) = self.terminal.try_write() {
            let osc_title = term.get_title();
            if !osc_title.is_empty() {
                self.title = osc_title;
                self.has_default_title = false;
            } else if title_mode == par_term_config::TabTitleMode::Auto
                && let Some(cwd) = term.shell_integration_cwd()
            {
                // Abbreviate home directory to ~
                let abbreviated = if let Some(home) = dirs::home_dir() {
                    cwd.replace(&home.to_string_lossy().to_string(), "~")
                } else {
                    cwd
                };
                // Use just the last component for brevity
                if let Some(last) = abbreviated.rsplit('/').next() {
                    if !last.is_empty() {
                        self.title = last.to_string();
                    } else {
                        self.title = abbreviated;
                    }
                } else {
                    self.title = abbreviated;
                }
                self.has_default_title = false;
            }
            // Otherwise keep the existing title (e.g., "Tab N")
        }
    }

    /// Set the tab's default title based on its position
    pub fn set_default_title(&mut self, tab_number: usize) {
        if self.has_default_title {
            self.title = format!("Tab {}", tab_number);
        }
    }

    /// Explicitly set the tab title (for tmux window names, etc.)
    ///
    /// This overrides any default title and marks the tab as having a custom title.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
        self.has_default_title = false;
    }

    /// Check if the terminal in this tab is still running
    pub fn is_running(&self) -> bool {
        if let Ok(term) = self.terminal.try_write() {
            term.is_running()
        } else {
            true // Assume running if locked
        }
    }

    /// Get the current working directory of this tab's shell
    pub fn get_cwd(&self) -> Option<String> {
        if let Ok(term) = self.terminal.try_write() {
            term.shell_integration_cwd()
        } else {
            self.working_directory.clone()
        }
    }

    /// Set a custom color for this tab
    pub fn set_custom_color(&mut self, color: [u8; 3]) {
        self.custom_color = Some(color);
    }

    /// Clear the custom color for this tab (reverts to default config colors)
    pub fn clear_custom_color(&mut self) {
        self.custom_color = None;
    }

    /// Check if this tab has a custom color set
    pub fn has_custom_color(&self) -> bool {
        self.custom_color.is_some()
    }

    /// Parse hostname from an OSC 7 file:// URL
    ///
    /// OSC 7 format: `file://hostname/path` or `file:///path` (localhost)
    /// Returns the hostname if present and not localhost, None otherwise.
    pub fn parse_hostname_from_osc7_url(url: &str) -> Option<String> {
        let path = url.strip_prefix("file://")?;

        if path.starts_with('/') {
            // file:///path - localhost implicit
            None
        } else {
            // file://hostname/path - extract hostname
            let hostname = path.split('/').next()?;
            if hostname.is_empty() || hostname == "localhost" {
                None
            } else {
                Some(hostname.to_string())
            }
        }
    }

    /// Check if hostname has changed and update tracking
    ///
    /// Returns Some(hostname) if a new remote hostname was detected,
    /// None if hostname hasn't changed or is local.
    ///
    /// This uses the hostname extracted from OSC 7 sequences by the terminal emulator.
    pub fn check_hostname_change(&mut self) -> Option<String> {
        let current_hostname = if let Ok(term) = self.terminal.try_write() {
            term.shell_integration_hostname()
        } else {
            return None;
        };

        // Check if hostname has changed
        if current_hostname != self.detected_hostname {
            let old_hostname = self.detected_hostname.take();
            self.detected_hostname = current_hostname.clone();

            crate::debug_info!(
                "PROFILE",
                "Hostname changed: {:?} -> {:?}",
                old_hostname,
                current_hostname
            );

            // Return the new hostname if it's a remote host (not None/localhost)
            current_hostname
        } else {
            None
        }
    }

    /// Check if CWD has changed and update tracking
    ///
    /// Returns Some(cwd) if the CWD has changed, None otherwise.
    /// Uses the CWD reported via OSC 7 by the terminal emulator.
    pub fn check_cwd_change(&mut self) -> Option<String> {
        let current_cwd = self.get_cwd();

        if current_cwd != self.detected_cwd {
            let old_cwd = self.detected_cwd.take();
            self.detected_cwd = current_cwd.clone();

            crate::debug_info!("PROFILE", "CWD changed: {:?} -> {:?}", old_cwd, current_cwd);

            current_cwd
        } else {
            None
        }
    }

    /// Clear auto-applied profile tracking
    ///
    /// Call this when manually switching profiles or when the hostname
    /// returns to local, or when disconnecting from tmux.
    pub fn clear_auto_profile(&mut self) {
        self.auto_applied_profile_id = None;
        self.auto_applied_dir_profile_id = None;
        self.profile_icon = None;
        if let Some(original) = self.pre_profile_title.take() {
            self.title = original;
        }
        self.badge_override = None;
    }
}

#[cfg(test)]
mod tests {
    use crate::tab::Tab;

    #[test]
    fn test_parse_hostname_from_osc7_url_localhost() {
        // file:///path - localhost implicit, should return None
        assert_eq!(Tab::parse_hostname_from_osc7_url("file:///home/user"), None);
        assert_eq!(Tab::parse_hostname_from_osc7_url("file:///"), None);
        assert_eq!(
            Tab::parse_hostname_from_osc7_url("file:///var/log/syslog"),
            None
        );
    }

    #[test]
    fn test_parse_hostname_from_osc7_url_remote() {
        // file://hostname/path - should extract hostname
        assert_eq!(
            Tab::parse_hostname_from_osc7_url("file://server.example.com/home/user"),
            Some("server.example.com".to_string())
        );
        assert_eq!(
            Tab::parse_hostname_from_osc7_url("file://myhost/tmp"),
            Some("myhost".to_string())
        );
        assert_eq!(
            Tab::parse_hostname_from_osc7_url("file://192.168.1.100/var/log"),
            Some("192.168.1.100".to_string())
        );
    }

    #[test]
    fn test_parse_hostname_from_osc7_url_localhost_explicit() {
        // file://localhost/path - localhost should return None
        assert_eq!(
            Tab::parse_hostname_from_osc7_url("file://localhost/home/user"),
            None
        );
    }

    #[test]
    fn test_parse_hostname_from_osc7_url_invalid() {
        // Invalid URLs should return None
        assert_eq!(Tab::parse_hostname_from_osc7_url(""), None);
        assert_eq!(
            Tab::parse_hostname_from_osc7_url("http://example.com"),
            None
        );
        assert_eq!(Tab::parse_hostname_from_osc7_url("/home/user"), None);
        assert_eq!(Tab::parse_hostname_from_osc7_url("file://"), None);
    }

    #[test]
    fn test_parse_hostname_from_osc7_url_edge_cases() {
        // Empty hostname after file://
        assert_eq!(Tab::parse_hostname_from_osc7_url("file:///"), None);

        // Hostname with no path (unusual but valid)
        assert_eq!(
            Tab::parse_hostname_from_osc7_url("file://host"),
            Some("host".to_string())
        );
    }
}
