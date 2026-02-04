//! tmux status bar UI using egui
//!
//! Displays the tmux status bar at the bottom of the terminal when connected
//! to a tmux session. Supports configurable format strings for customizing
//! the status bar content.
//!
//! ## Format Variables
//!
//! The following variables can be used in `tmux_status_bar_left` and
//! `tmux_status_bar_right` configuration options:
//!
//! - `{session}` - Session name
//! - `{windows}` - Window list with active marker (*)
//! - `{pane}` - Focused pane ID (e.g., "%0")
//! - `{time:FORMAT}` - Current time with strftime format (e.g., `{time:%H:%M}`)
//! - `{hostname}` - Machine hostname
//! - `{user}` - Current username
//!
//! ## Example Configuration
//!
//! ```yaml
//! tmux_status_bar_left: "[{session}] {windows}"
//! tmux_status_bar_right: "{user}@{hostname} | {time:%H:%M}"
//! ```

use crate::config::Config;
use crate::tmux::{FormatContext, TmuxSession, expand_format};

/// tmux status bar UI state
pub struct TmuxStatusBarUI {
    /// Cached left side content
    cached_left: String,
    /// Cached right side content
    cached_right: String,
    /// Last time the status bar was refreshed
    last_refresh: std::time::Instant,
}

impl TmuxStatusBarUI {
    /// Create a new tmux status bar UI
    pub fn new() -> Self {
        Self {
            cached_left: String::new(),
            cached_right: String::new(),
            last_refresh: std::time::Instant::now(),
        }
    }

    /// Check if a refresh is needed based on the configured interval
    pub fn needs_refresh(&self, refresh_interval_ms: u64) -> bool {
        self.last_refresh.elapsed().as_millis() >= refresh_interval_ms as u128
    }

    /// Get the height of the status bar (0 if not visible)
    pub fn height(config: &Config, is_connected: bool) -> f32 {
        if config.tmux_show_status_bar && is_connected {
            24.0 // Fixed height for status bar
        } else {
            0.0
        }
    }

    /// Update cached content from session state using configurable format strings
    pub fn update_from_session(
        &mut self,
        config: &Config,
        session: &TmuxSession,
        session_name: Option<&str>,
    ) {
        // Create format context with session data
        let ctx = FormatContext::new(Some(session), session_name);

        // Expand format strings from config
        self.cached_left = expand_format(&config.tmux_status_bar_left, &ctx);
        self.cached_right = expand_format(&config.tmux_status_bar_right, &ctx);
        self.last_refresh = std::time::Instant::now();
    }

    /// Render the status bar
    ///
    /// Returns the height consumed by the status bar.
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        config: &Config,
        session: Option<&TmuxSession>,
        session_name: Option<&str>,
    ) -> f32 {
        // Don't show if not configured or not connected
        let is_connected = session.is_some_and(|s| {
            use crate::tmux::SessionState;
            s.state() == SessionState::Connected
        });

        if !config.tmux_show_status_bar || !is_connected {
            return 0.0;
        }

        // Update content if needed
        if let Some(session) = session
            && self.needs_refresh(config.tmux_status_bar_refresh_ms)
        {
            self.update_from_session(config, session, session_name);
        }

        let bar_height = 24.0;

        // Status bar at the bottom
        egui::TopBottomPanel::bottom("tmux_status_bar")
            .exact_height(bar_height)
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(30, 40, 30)) // Dark green-ish background (tmux style)
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Left side - session and windows
                    let left_text = if !self.cached_left.is_empty() {
                        &self.cached_left
                    } else {
                        "tmux"
                    };

                    ui.label(
                        egui::RichText::new(left_text)
                            .color(egui::Color32::from_rgb(100, 200, 100)) // Green text
                            .size(12.0)
                            .monospace(),
                    );

                    // Right side - pane info and time (right-aligned)
                    if !self.cached_right.is_empty() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(&self.cached_right)
                                    .color(egui::Color32::from_rgb(150, 200, 150)) // Light green text
                                    .size(12.0)
                                    .monospace(),
                            );
                        });
                    }
                });
            });

        bar_height
    }
}

impl Default for TmuxStatusBarUI {
    fn default() -> Self {
        Self::new()
    }
}
