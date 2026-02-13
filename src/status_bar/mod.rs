//! Status bar system for displaying session and system information.
//!
//! The status bar is a configurable panel that can display widgets such as
//! the current time, username, git branch, CPU/memory usage, and more.

pub mod config;
pub mod system_monitor;
pub mod widgets;

use std::process::Command;
use std::time::Instant;

use crate::badge::SessionVariables;
use crate::config::{Config, StatusBarPosition};
use config::StatusBarSection;
use system_monitor::SystemMonitor;
use widgets::{WidgetContext, sorted_widgets_for_section, widget_text};

/// Git branch poller state.
#[derive(Debug)]
struct GitBranchPoller {
    /// Most recently detected branch name.
    branch: Option<String>,
    /// When we last polled.
    last_poll: Instant,
    /// Last working directory we polled in (re-poll if it changes).
    cwd: Option<String>,
}

impl Default for GitBranchPoller {
    fn default() -> Self {
        Self {
            branch: None,
            last_poll: Instant::now(),
            cwd: None,
        }
    }
}

/// Status bar UI state and renderer.
pub struct StatusBarUI {
    /// Background system resource monitor.
    system_monitor: SystemMonitor,
    /// Git branch poller.
    git_poller: GitBranchPoller,
    /// Timestamp of the last mouse activity (for auto-hide).
    last_mouse_activity: Instant,
    /// Whether the status bar is currently visible.
    visible: bool,
}

impl StatusBarUI {
    /// Create a new status bar UI.
    pub fn new() -> Self {
        Self {
            system_monitor: SystemMonitor::new(),
            git_poller: GitBranchPoller::default(),
            last_mouse_activity: Instant::now(),
            visible: true,
        }
    }

    /// Compute the effective height consumed by the status bar.
    ///
    /// Returns 0 if the status bar is hidden or disabled.
    pub fn height(&self, config: &Config, is_fullscreen: bool) -> f32 {
        if !config.status_bar_enabled || self.should_hide(config, is_fullscreen) {
            0.0
        } else {
            config.status_bar_height
        }
    }

    /// Determine whether the status bar should be hidden right now.
    fn should_hide(&self, config: &Config, is_fullscreen: bool) -> bool {
        if !config.status_bar_enabled {
            return true;
        }
        if config.status_bar_auto_hide_fullscreen && is_fullscreen {
            return true;
        }
        if config.status_bar_auto_hide_mouse_inactive {
            let elapsed = self.last_mouse_activity.elapsed().as_secs_f32();
            if elapsed > config.status_bar_mouse_inactive_timeout {
                return true;
            }
        }
        false
    }

    /// Record mouse activity (resets auto-hide timer).
    pub fn on_mouse_activity(&mut self) {
        self.last_mouse_activity = Instant::now();
        self.visible = true;
    }

    /// Start or stop the system monitor based on enabled widgets.
    pub fn sync_monitor_state(&self, config: &Config) {
        if !config.status_bar_enabled {
            if self.system_monitor.is_running() {
                self.system_monitor.stop();
            }
            return;
        }

        let needs_monitor = config
            .status_bar_widgets
            .iter()
            .any(|w| w.enabled && w.id.needs_system_monitor());

        if needs_monitor && !self.system_monitor.is_running() {
            self.system_monitor
                .start(config.status_bar_system_poll_interval);
        } else if !needs_monitor && self.system_monitor.is_running() {
            self.system_monitor.stop();
        }
    }

    /// Poll git branch if enough time has elapsed or the cwd changed.
    fn poll_git_branch(&mut self, config: &Config, cwd: Option<&str>) {
        // Skip polling if git branch widget is not enabled
        let git_enabled = config
            .status_bar_widgets
            .iter()
            .any(|w| w.enabled && w.id == config::WidgetId::GitBranch);
        if !git_enabled {
            self.git_poller.branch = None;
            return;
        }

        let cwd_changed = match (&self.git_poller.cwd, cwd) {
            (Some(old), Some(new)) => old != new,
            (None, Some(_)) => true,
            _ => false,
        };

        let interval_elapsed = self.git_poller.last_poll.elapsed().as_secs_f32()
            >= config.status_bar_git_poll_interval;

        if !cwd_changed && !interval_elapsed {
            return;
        }

        self.git_poller.cwd = cwd.map(String::from);
        self.git_poller.last_poll = Instant::now();

        // Only poll if we have a directory to poll in
        let Some(dir) = cwd else {
            self.git_poller.branch = None;
            return;
        };

        // Run git rev-parse --abbrev-ref HEAD
        let result = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(dir)
            .output();

        self.git_poller.branch = result.ok().and_then(|out| {
            if out.status.success() {
                let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if branch.is_empty() {
                    None
                } else {
                    Some(branch)
                }
            } else {
                None
            }
        });
    }

    /// Render the status bar.
    ///
    /// Returns the height consumed by the status bar (0 if hidden).
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        config: &Config,
        session_vars: &SessionVariables,
        is_fullscreen: bool,
    ) -> f32 {
        if !config.status_bar_enabled || self.should_hide(config, is_fullscreen) {
            return 0.0;
        }

        // Poll git branch (uses session_vars.path as cwd)
        let cwd = if session_vars.path.is_empty() {
            None
        } else {
            Some(session_vars.path.as_str())
        };
        self.poll_git_branch(config, cwd);

        // Build widget context
        let widget_ctx = WidgetContext {
            session_vars: session_vars.clone(),
            system_data: self.system_monitor.data(),
            git_branch: self.git_poller.branch.clone(),
        };

        let bar_height = config.status_bar_height;
        let [bg_r, bg_g, bg_b] = config.status_bar_bg_color;
        let bg_alpha = (config.status_bar_bg_alpha * 255.0) as u8;
        let bg_color = egui::Color32::from_rgba_unmultiplied(bg_r, bg_g, bg_b, bg_alpha);

        let [fg_r, fg_g, fg_b] = config.status_bar_fg_color;
        let fg_color = egui::Color32::from_rgb(fg_r, fg_g, fg_b);
        let font_size = config.status_bar_font_size;
        let separator = &config.status_bar_separator;
        let sep_color = fg_color.linear_multiply(0.4);

        // Choose top or bottom panel based on config
        let panel_id = "status_bar";
        let frame = egui::Frame::NONE
            .fill(bg_color)
            .inner_margin(egui::Margin::symmetric(8, 2));

        let show_panel = |ui_fn: &mut dyn FnMut(&mut egui::Ui)| match config.status_bar_position {
            StatusBarPosition::Top => {
                egui::TopBottomPanel::top(panel_id)
                    .exact_height(bar_height)
                    .frame(frame)
                    .show(ctx, |ui| ui_fn(ui));
            }
            StatusBarPosition::Bottom => {
                egui::TopBottomPanel::bottom(panel_id)
                    .exact_height(bar_height)
                    .frame(frame)
                    .show(ctx, |ui| ui_fn(ui));
            }
        };

        let make_rich_text = |text: &str| -> egui::RichText {
            egui::RichText::new(text)
                .color(fg_color)
                .size(font_size)
                .monospace()
        };

        let make_sep = |sep: &str| -> egui::RichText {
            egui::RichText::new(sep)
                .color(sep_color)
                .size(font_size)
                .monospace()
        };

        show_panel(&mut |ui: &mut egui::Ui| {
            ui.horizontal_centered(|ui| {
                // === Left section ===
                let left_widgets =
                    sorted_widgets_for_section(&config.status_bar_widgets, StatusBarSection::Left);
                let mut first = true;
                for w in &left_widgets {
                    let text = widget_text(&w.id, &widget_ctx, w.format.as_deref());
                    if text.is_empty() {
                        continue;
                    }
                    if !first {
                        ui.label(make_sep(separator));
                    }
                    first = false;
                    ui.label(make_rich_text(&text));
                }

                // === Center section ===
                let center_widgets = sorted_widgets_for_section(
                    &config.status_bar_widgets,
                    StatusBarSection::Center,
                );
                if !center_widgets.is_empty() {
                    ui.with_layout(
                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            let mut first = true;
                            for w in &center_widgets {
                                let text = widget_text(&w.id, &widget_ctx, w.format.as_deref());
                                if text.is_empty() {
                                    continue;
                                }
                                if !first {
                                    ui.label(make_sep(separator));
                                }
                                first = false;
                                ui.label(make_rich_text(&text));
                            }
                        },
                    );
                }

                // === Right section ===
                let right_widgets =
                    sorted_widgets_for_section(&config.status_bar_widgets, StatusBarSection::Right);
                if !right_widgets.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Render in reverse order so first widget ends up rightmost
                        let mut first = true;
                        for w in right_widgets.iter().rev() {
                            let text = widget_text(&w.id, &widget_ctx, w.format.as_deref());
                            if text.is_empty() {
                                continue;
                            }
                            if !first {
                                ui.label(make_sep(separator));
                            }
                            first = false;
                            ui.label(make_rich_text(&text));
                        }
                    });
                }
            });
        });

        bar_height
    }
}

impl Default for StatusBarUI {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for StatusBarUI {
    fn drop(&mut self) {
        self.system_monitor.stop();
    }
}
