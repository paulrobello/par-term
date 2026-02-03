//! tmux status bar UI using egui
//!
//! Displays the tmux status bar at the bottom of the terminal when connected
//! to a tmux session.

use crate::config::Config;
use crate::tmux::TmuxSession;

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

    /// Build the left side of the status bar from session state
    fn build_left_content(session: &TmuxSession, session_name: Option<&str>) -> String {
        let mut parts = Vec::new();

        // Session name
        if let Some(name) = session_name.or_else(|| session.session_name()) {
            parts.push(format!("[{}]", name));
        }

        // Windows list
        let windows = session.windows();
        if !windows.is_empty() {
            let mut window_parts = Vec::new();
            let mut window_list: Vec<_> = windows.values().collect();
            window_list.sort_by_key(|w| w.index);

            for window in window_list {
                let marker = if window.active { "*" } else { "" };
                window_parts.push(format!("{}:{}{}", window.index, window.name, marker));
            }
            parts.push(window_parts.join(" "));
        }

        parts.join(" ")
    }

    /// Build the right side of the status bar (time, pane info)
    fn build_right_content(session: &TmuxSession) -> String {
        let mut parts = Vec::new();

        // Focused pane info
        if let Some(pane_id) = session.focused_pane() {
            parts.push(format!("%{}", pane_id));
        }

        // Current time
        let now = chrono::Local::now();
        parts.push(now.format("%H:%M").to_string());

        parts.join(" | ")
    }

    /// Update cached content from session state
    pub fn update_from_session(&mut self, session: &TmuxSession, session_name: Option<&str>) {
        self.cached_left = Self::build_left_content(session, session_name);
        self.cached_right = Self::build_right_content(session);
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
            self.update_from_session(session, session_name);
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
