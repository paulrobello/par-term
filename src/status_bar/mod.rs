//! Status bar system for displaying session and system information.
//!
//! The status bar is a configurable panel that can display widgets such as
//! the current time, username, git branch, CPU/memory usage, and more.

pub mod config;
pub mod system_monitor;
pub mod widgets;

use parking_lot::Mutex;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::badge::SessionVariables;
use crate::config::{Config, StatusBarPosition};
use config::StatusBarSection;
use system_monitor::SystemMonitor;
use widgets::{WidgetContext, sorted_widgets_for_section, widget_text};

/// Snapshot of git repository status.
#[derive(Debug, Clone, Default)]
pub struct GitStatus {
    /// Current branch name.
    pub branch: Option<String>,
    /// Commits ahead of upstream.
    pub ahead: u32,
    /// Commits behind upstream.
    pub behind: u32,
    /// Whether the working tree has uncommitted changes.
    pub dirty: bool,
}

/// Git branch poller that runs on a background thread.
struct GitBranchPoller {
    /// Shared git status (read from render thread, written by poll thread).
    status: Arc<Mutex<GitStatus>>,
    /// Current working directory to poll in.
    cwd: Arc<Mutex<Option<String>>>,
    /// Whether the poller is running.
    running: Arc<AtomicBool>,
    /// Handle to the polling thread.
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl GitBranchPoller {
    fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(GitStatus::default())),
            cwd: Arc::new(Mutex::new(None)),
            running: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
        }
    }

    /// Start the background polling thread.
    fn start(&self, poll_interval_secs: f32) {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let status = Arc::clone(&self.status);
        let cwd = Arc::clone(&self.cwd);
        let running = Arc::clone(&self.running);
        let interval = Duration::from_secs_f32(poll_interval_secs.max(1.0));

        let handle = std::thread::Builder::new()
            .name("status-bar-git".into())
            .spawn(move || {
                while running.load(Ordering::SeqCst) {
                    let dir = cwd.lock().clone();
                    let result = dir.map(|d| poll_git_status(&d)).unwrap_or_default();
                    *status.lock() = result;
                    std::thread::sleep(interval);
                }
            })
            .expect("Failed to spawn git branch poller thread");

        *self.thread.lock() = Some(handle);
    }

    /// Stop the background polling thread.
    fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread.lock().take() {
            let _ = handle.join();
        }
    }

    /// Update the working directory to poll in.
    fn set_cwd(&self, new_cwd: Option<&str>) {
        *self.cwd.lock() = new_cwd.map(String::from);
    }

    /// Get the current git status snapshot.
    fn status(&self) -> GitStatus {
        self.status.lock().clone()
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Poll git for branch name, ahead/behind counts, and dirty status.
fn poll_git_status(dir: &str) -> GitStatus {
    // Get branch name
    let branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                let b = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if b.is_empty() { None } else { Some(b) }
            } else {
                None
            }
        });

    if branch.is_none() {
        return GitStatus::default();
    }

    // Get ahead/behind counts via rev-list
    let (ahead, behind) = Command::new("git")
        .args(["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])
        .current_dir(dir)
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                let parts: Vec<&str> = text.trim().split('\t').collect();
                if parts.len() == 2 {
                    let a = parts[0].parse::<u32>().unwrap_or(0);
                    let b = parts[1].parse::<u32>().unwrap_or(0);
                    Some((a, b))
                } else {
                    None
                }
            } else {
                // No upstream configured
                None
            }
        })
        .unwrap_or((0, 0));

    // Check dirty status (fast: just check if there are any changes)
    let dirty = Command::new("git")
        .args(["status", "--porcelain", "-uno"])
        .current_dir(dir)
        .output()
        .ok()
        .is_some_and(|out| out.status.success() && !out.stdout.is_empty());

    GitStatus {
        branch,
        ahead,
        behind,
        dirty,
    }
}

impl Drop for GitBranchPoller {
    fn drop(&mut self) {
        self.stop();
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
    /// Last valid time format string (for fallback when user is mid-edit).
    last_valid_time_format: String,
}

impl StatusBarUI {
    /// Create a new status bar UI.
    pub fn new() -> Self {
        Self {
            system_monitor: SystemMonitor::new(),
            git_poller: GitBranchPoller::new(),
            last_mouse_activity: Instant::now(),
            visible: true,
            last_valid_time_format: "%H:%M:%S".to_string(),
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

    /// Start or stop the system monitor and git poller based on enabled widgets.
    pub fn sync_monitor_state(&self, config: &Config) {
        if !config.status_bar_enabled {
            if self.system_monitor.is_running() {
                self.system_monitor.stop();
            }
            if self.git_poller.is_running() {
                self.git_poller.stop();
            }
            return;
        }

        // System monitor
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

        // Git branch poller
        let needs_git = config
            .status_bar_widgets
            .iter()
            .any(|w| w.enabled && w.id == config::WidgetId::GitBranch);

        if needs_git && !self.git_poller.is_running() {
            self.git_poller.start(config.status_bar_git_poll_interval);
        } else if !needs_git && self.git_poller.is_running() {
            self.git_poller.stop();
        }
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

        // Update git poller cwd from active tab's path
        let cwd = if session_vars.path.is_empty() {
            None
        } else {
            Some(session_vars.path.as_str())
        };
        self.git_poller.set_cwd(cwd);

        // Validate time format — update last-known-good on success, fall back on failure
        {
            use chrono::format::strftime::StrftimeItems;
            let valid = !config.status_bar_time_format.is_empty()
                && StrftimeItems::new(&config.status_bar_time_format)
                    .all(|item| !matches!(item, chrono::format::Item::Error));
            if valid {
                self.last_valid_time_format = config.status_bar_time_format.clone();
            }
        }

        // Build widget context
        let git_status = self.git_poller.status();
        let widget_ctx = WidgetContext {
            session_vars: session_vars.clone(),
            system_data: self.system_monitor.data(),
            git_branch: git_status.branch,
            git_ahead: git_status.ahead,
            git_behind: git_status.behind,
            git_dirty: git_status.dirty,
            git_show_status: config.status_bar_git_show_status,
            time_format: self.last_valid_time_format.clone(),
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

        // Use an egui::Area with a fixed size so the status bar stops before
        // the scrollbar column.  TopBottomPanel always spans the full window
        // width and ignores every attempt to narrow it.
        let h_margin: f32 = 8.0; // left + right inner margin per side
        let v_margin: f32 = 2.0; // top + bottom inner margin per side
        let scrollbar_reserved = config.scrollbar_width + 2.0;
        let viewport = ctx.input(|i| i.viewport_rect());
        // Content width is the frame width minus both horizontal margins.
        let content_width = (viewport.width() - scrollbar_reserved - h_margin * 2.0).max(0.0);
        let content_height = (bar_height - v_margin * 2.0).max(0.0);

        let bar_pos = match config.status_bar_position {
            StatusBarPosition::Top => egui::pos2(0.0, 0.0),
            StatusBarPosition::Bottom => egui::pos2(0.0, viewport.height() - bar_height),
        };

        let frame = egui::Frame::NONE
            .fill(bg_color)
            .inner_margin(egui::Margin::symmetric(h_margin as i8, v_margin as i8));

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

        egui::Area::new(egui::Id::new("status_bar"))
            .fixed_pos(bar_pos)
            .order(egui::Order::Background)
            .interactable(false)
            .show(ctx, |ui| {
                // Constrain the outer UI so the frame cannot grow beyond the
                // intended total width (content + margins).
                ui.set_max_width(content_width + h_margin * 2.0);
                ui.set_max_height(bar_height);

                frame.show(ui, |ui| {
                    ui.set_min_size(egui::vec2(content_width, content_height));
                    ui.set_max_size(egui::vec2(content_width, content_height));

                    ui.horizontal_centered(|ui| {
                        // Clip widgets to the available content width so
                        // right-to-left layouts cannot expand past the bar edge.
                        ui.set_clip_rect(ui.max_rect());

                        // === Left section ===
                        let left_widgets = sorted_widgets_for_section(
                            &config.status_bar_widgets,
                            StatusBarSection::Left,
                        );
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
                                        let text =
                                            widget_text(&w.id, &widget_ctx, w.format.as_deref());
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
                        let right_widgets = sorted_widgets_for_section(
                            &config.status_bar_widgets,
                            StatusBarSection::Right,
                        );
                        if !right_widgets.is_empty() {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let mut first = true;
                                    for w in right_widgets.iter().rev() {
                                        let text =
                                            widget_text(&w.id, &widget_ctx, w.format.as_deref());
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
                    });
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
