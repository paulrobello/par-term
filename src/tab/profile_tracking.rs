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

    /// Update tab title from terminal OSC sequences or shell integration data.
    ///
    /// Priority when on a **remote** host (hostname detected via OSC 7):
    ///   1. Explicit OSC title (`\033]0;...\007`) if `remote_osc_priority` is true
    ///   2. `remote_format` — formatted from hostname/username/cwd
    ///
    /// Priority when **local**:
    ///   1. Explicit OSC title
    ///   2. Last CWD component (only in `TabTitleMode::Auto`)
    ///
    /// User-named tabs are never auto-updated.
    pub fn update_title(
        &mut self,
        title_mode: par_term_config::TabTitleMode,
        remote_format: par_term_config::RemoteTabTitleFormat,
        remote_osc_priority: bool,
    ) {
        // User-named tabs are static — never auto-update
        if self.user_named {
            return;
        }

        // Step 2 — Snapshot focused pane ID before the mutable borrow.
        // This avoids a Rust borrow-checker conflict: all_panes_mut() takes &mut pane_manager,
        // and we must re-borrow it immutably in Step 4 after the loop ends.
        let focused_id = self
            .pane_manager
            .as_ref()
            .and_then(|pm| pm.focused_pane_id());

        // Cache per-frame values that are constant across all panes (avoid syscall per pane).
        let local_hostname = hostname::get().ok().and_then(|h| h.into_string().ok());
        let home_dir = dirs::home_dir();

        // Step 3 — Iterate all panes and update each one's title from its own terminal.
        // try_write: intentional — called every frame; blocking would stall rendering.
        // On contention: skip that pane this frame, no data loss.
        if let Some(pm) = self.pane_manager.as_mut() {
            for pane in pm.all_panes_mut() {
                if let Ok(term) = pane.terminal.try_write() {
                    let osc_title = term.get_title();
                    let hostname = term.shell_integration_hostname();
                    let username = term.shell_integration_username();
                    let cwd = term.shell_integration_cwd();
                    drop(term);

                    let is_remote = if let Some(reported_host) = &hostname {
                        local_hostname
                            .as_ref()
                            .map(|local| !reported_host.eq_ignore_ascii_case(local))
                            .unwrap_or(false)
                    } else {
                        false
                    };

                    if is_remote {
                        if remote_osc_priority && !osc_title.is_empty() {
                            pane.title = osc_title;
                            pane.has_default_title = false;
                        } else {
                            pane.title =
                                format_remote_title(hostname, username, cwd, remote_format);
                            pane.has_default_title = false;
                        }
                    } else if !osc_title.is_empty() {
                        pane.title = osc_title;
                        pane.has_default_title = false;
                    } else if title_mode == par_term_config::TabTitleMode::Auto
                        && let Some(cwd) = cwd
                    {
                        let abbreviated = if let Some(ref home) = home_dir {
                            cwd.replace(&home.to_string_lossy().to_string(), "~")
                        } else {
                            cwd
                        };
                        if let Some(last) = abbreviated.rsplit('/').next() {
                            if !last.is_empty() {
                                pane.title = last.to_string();
                            } else {
                                pane.title = abbreviated;
                            }
                        } else {
                            pane.title = abbreviated;
                        }
                        pane.has_default_title = false;
                    }
                    // else: keep existing pane.title unchanged this frame
                }
            }
        }
        // mutable borrow of pane_manager ends here

        // Step 4 — Derive tab.title from the focused pane (immutable re-borrow is now safe).
        if let Some((focused_id, pm)) = focused_id.zip(self.pane_manager.as_ref())
            && let Some(pane) = pm.get_pane(focused_id)
        {
            self.title = pane.title.clone();
            self.has_default_title = pane.has_default_title;
        }
    }

    /// Set the tab's default title based on its position
    pub fn set_default_title(&mut self, tab_number: usize) {
        if self.has_default_title {
            let title = format!("Tab {}", tab_number);
            self.title = title.clone();
            // Also write pane.title for every pane that still has a default title so
            // update_title()'s Step 4 derivation from pane.title returns "Tab N" correctly
            // (a brand-new pane has pane.title == "" which would otherwise overwrite).
            if let Some(pm) = self.pane_manager.as_mut() {
                for pane in pm.all_panes_mut() {
                    if pane.has_default_title {
                        pane.title = title.clone();
                    }
                }
            }
        }
    }

    /// Explicitly set the tab title (for tmux window names, etc.)
    ///
    /// This overrides any default title and marks the tab as having a custom title.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
        self.has_default_title = false;
        // Sync focused pane so update_title() doesn't overwrite on the next frame.
        if let Some(pane) = self
            .pane_manager
            .as_mut()
            .and_then(|pm| pm.focused_pane_mut())
        {
            pane.title = title.to_string();
            pane.has_default_title = false;
        }
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
        self.profile.auto_applied_profile_id = None;
        self.profile.auto_applied_dir_profile_id = None;
        self.profile.profile_icon = None;
        if let Some(original) = self.profile.pre_profile_title.take() {
            self.set_title(&original);
        }
        self.profile.badge_override = None;
    }
}

/// Format a tab title for a remote host based on the configured format.
///
/// Uses the remote username to abbreviate the home directory in `HostAndCwd` mode
/// (e.g. `/home/alice/projects` → `~/projects`) rather than the local `$HOME`,
/// which never matches remote paths.
fn format_remote_title(
    hostname: Option<String>,
    username: Option<String>,
    cwd: Option<String>,
    format: par_term_config::RemoteTabTitleFormat,
) -> String {
    use par_term_config::RemoteTabTitleFormat;
    let host = hostname.unwrap_or_default();
    match format {
        RemoteTabTitleFormat::UserAtHost => {
            if let Some(user) = username {
                format!("{}@{}", user, host)
            } else {
                host
            }
        }
        RemoteTabTitleFormat::Host => host,
        RemoteTabTitleFormat::HostAndCwd => {
            if let Some(cwd) = cwd {
                let abbrev = if let Some(ref user) = username {
                    let linux_home = format!("/home/{}", user);
                    let macos_home = format!("/Users/{}", user);
                    let abbrev_with = |home: &str| -> Option<String> {
                        if cwd == home {
                            Some("~".to_string())
                        } else if cwd.starts_with(&format!("{}/", home)) {
                            Some(format!("~{}", &cwd[home.len()..]))
                        } else {
                            None
                        }
                    };
                    if let Some(a) = abbrev_with(&linux_home) {
                        a
                    } else if let Some(a) = abbrev_with(&macos_home) {
                        a
                    } else {
                        cwd
                    }
                } else {
                    cwd
                };
                format!("{}:{}", host, abbrev)
            } else {
                host
            }
        }
    }
}

#[cfg(test)]
mod format_remote_title_tests {
    use super::format_remote_title;
    use par_term_config::RemoteTabTitleFormat;

    #[test]
    fn user_at_host_with_both() {
        let result = format_remote_title(
            Some("server".into()),
            Some("alice".into()),
            None,
            RemoteTabTitleFormat::UserAtHost,
        );
        assert_eq!(result, "alice@server");
    }

    #[test]
    fn user_at_host_no_username_falls_back_to_host() {
        let result = format_remote_title(
            Some("server".into()),
            None,
            None,
            RemoteTabTitleFormat::UserAtHost,
        );
        assert_eq!(result, "server");
    }

    #[test]
    fn host_only() {
        let result = format_remote_title(
            Some("mybox".into()),
            Some("bob".into()),
            Some("/home/bob/projects".into()),
            RemoteTabTitleFormat::Host,
        );
        assert_eq!(result, "mybox");
    }

    #[test]
    fn host_and_cwd_abbreviates_linux_home() {
        let result = format_remote_title(
            Some("server".into()),
            Some("alice".into()),
            Some("/home/alice/projects/foo".into()),
            RemoteTabTitleFormat::HostAndCwd,
        );
        assert_eq!(result, "server:~/projects/foo");
    }

    #[test]
    fn host_and_cwd_abbreviates_macos_home() {
        let result = format_remote_title(
            Some("mac".into()),
            Some("alice".into()),
            Some("/Users/alice/dev".into()),
            RemoteTabTitleFormat::HostAndCwd,
        );
        assert_eq!(result, "mac:~/dev");
    }

    #[test]
    fn host_and_cwd_no_cwd_falls_back_to_host() {
        let result = format_remote_title(
            Some("server".into()),
            Some("alice".into()),
            None,
            RemoteTabTitleFormat::HostAndCwd,
        );
        assert_eq!(result, "server");
    }

    #[test]
    fn host_and_cwd_unknown_path_no_abbreviation() {
        let result = format_remote_title(
            Some("server".into()),
            Some("alice".into()),
            Some("/var/log".into()),
            RemoteTabTitleFormat::HostAndCwd,
        );
        assert_eq!(result, "server:/var/log");
    }

    #[test]
    fn host_and_cwd_does_not_abbreviate_partial_username_match() {
        let result = format_remote_title(
            Some("server".into()),
            Some("alice".into()),
            Some("/home/alice2/projects".into()),
            RemoteTabTitleFormat::HostAndCwd,
        );
        assert_eq!(result, "server:/home/alice2/projects");
    }

    #[test]
    fn host_and_cwd_exact_home_dir_shows_tilde() {
        let result = format_remote_title(
            Some("server".into()),
            Some("alice".into()),
            Some("/home/alice".into()),
            RemoteTabTitleFormat::HostAndCwd,
        );
        assert_eq!(result, "server:~");
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

#[cfg(test)]
mod default_title_tests {
    use crate::tab::Tab;

    /// set_default_title() must write pane.title for default-titled panes
    /// so that update_title()'s Step 4 derivation doesn't produce an empty string.
    #[test]
    fn set_default_title_syncs_pane_title() {
        let mut tab = Tab::new_stub(1, 1);
        // Fresh pane: has default title, pane.title starts as ""
        {
            let pm = tab.pane_manager.as_ref().unwrap();
            let pane = pm.focused_pane().unwrap();
            assert!(pane.has_default_title);
            assert_eq!(pane.title, "");
        }
        tab.set_default_title(3);
        assert_eq!(tab.title, "Tab 3");
        // Pane must also be updated so derivation survives the next frame
        let pm = tab.pane_manager.as_ref().unwrap();
        let pane = pm.focused_pane().unwrap();
        assert_eq!(pane.title, "Tab 3");
        assert!(pane.has_default_title);
    }

    /// set_default_title() must NOT overwrite panes that already have a real title.
    #[test]
    fn set_default_title_skips_non_default_panes() {
        let mut tab = Tab::new_stub(1, 1);
        // Simulate pane having received a real title
        {
            let pm = tab.pane_manager.as_mut().unwrap();
            let pane = pm.focused_pane_mut().unwrap();
            pane.title = "vim".to_string();
            pane.has_default_title = false;
        }
        // tab.has_default_title stays true (simulates multi-pane where focused has real title
        // but tab-level tracking is slightly stale)
        tab.has_default_title = true;
        tab.set_default_title(2);
        // Pane with a real title must be untouched
        let pm = tab.pane_manager.as_ref().unwrap();
        let pane = pm.focused_pane().unwrap();
        assert_eq!(pane.title, "vim");
        assert!(!pane.has_default_title);
    }

    #[test]
    fn set_title_syncs_focused_pane() {
        let mut tab = Tab::new_stub(1, 1);
        tab.set_title("my-session");
        assert_eq!(tab.title, "my-session");
        assert!(!tab.has_default_title);
        let pm = tab.pane_manager.as_ref().unwrap();
        let pane = pm.focused_pane().unwrap();
        assert_eq!(pane.title, "my-session");
        assert!(!pane.has_default_title);
    }
}
