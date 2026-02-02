//! tmux Session Picker UI
//!
//! An egui dialog that lists available tmux sessions and allows the user to:
//! - Attach to an existing session
//! - Create a new session

use egui::{Color32, Context, Frame, RichText, Window, epaint::Shadow};
use std::process::Command;

/// Information about a tmux session
#[derive(Debug, Clone)]
pub struct TmuxSessionInfo {
    /// Session ID (e.g., "$0")
    pub id: String,
    /// Session name
    pub name: String,
    /// Number of windows
    pub window_count: usize,
    /// Whether the session has attached clients
    pub attached: bool,
}

/// Action requested by the session picker
#[derive(Debug, Clone)]
pub enum SessionPickerAction {
    /// No action
    None,
    /// Attach to the specified session
    Attach(String),
    /// Create a new session with optional name
    CreateNew(Option<String>),
}

/// tmux Session Picker UI
pub struct TmuxSessionPickerUI {
    /// Whether the picker is visible
    pub visible: bool,
    /// List of available sessions
    sessions: Vec<TmuxSessionInfo>,
    /// New session name input
    new_session_name: String,
    /// Error message to display
    error_message: Option<String>,
    /// Whether we've loaded sessions
    sessions_loaded: bool,
}

impl TmuxSessionPickerUI {
    /// Create a new session picker UI
    pub fn new() -> Self {
        Self {
            visible: false,
            sessions: Vec::new(),
            new_session_name: String::new(),
            error_message: None,
            sessions_loaded: false,
        }
    }

    /// Show the session picker
    pub fn show_picker(&mut self) {
        self.visible = true;
        self.sessions_loaded = false; // Refresh on open
        self.error_message = None;
        self.new_session_name.clear();
    }

    /// Hide the session picker
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Toggle visibility
    pub fn toggle(&mut self) {
        if self.visible {
            self.hide();
        } else {
            self.show_picker();
        }
    }

    /// Refresh the session list
    pub fn refresh_sessions(&mut self, tmux_path: &str) {
        match Self::list_tmux_sessions(tmux_path) {
            Ok(sessions) => {
                self.sessions = sessions;
                self.error_message = None;
                self.sessions_loaded = true;
            }
            Err(e) => {
                self.sessions.clear();
                self.error_message = Some(e);
                self.sessions_loaded = true;
            }
        }
    }

    /// List available tmux sessions by running `tmux list-sessions`
    fn list_tmux_sessions(tmux_path: &str) -> Result<Vec<TmuxSessionInfo>, String> {
        let output = Command::new(tmux_path)
            .args([
                "list-sessions",
                "-F",
                "#{session_id}:#{session_name}:#{session_attached}:#{session_windows}",
            ])
            .output()
            .map_err(|e| format!("Failed to run tmux: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // "no server running" is expected when there are no sessions
            if stderr.contains("no server running") || stderr.contains("no sessions") {
                return Ok(Vec::new());
            }
            return Err(format!("tmux error: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut sessions = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 4 {
                sessions.push(TmuxSessionInfo {
                    id: parts[0].to_string(),
                    name: parts[1].to_string(),
                    attached: parts[2] == "1",
                    window_count: parts[3].parse().unwrap_or(0),
                });
            }
        }

        Ok(sessions)
    }

    /// Show the session picker UI and return any requested action
    pub fn show(&mut self, ctx: &Context, tmux_path: &str) -> SessionPickerAction {
        if !self.visible {
            return SessionPickerAction::None;
        }

        // Load sessions on first show
        if !self.sessions_loaded {
            self.refresh_sessions(tmux_path);
        }

        let mut action = SessionPickerAction::None;
        let mut close_requested = false;

        // Ensure picker is fully opaque
        let mut style = (*ctx.style()).clone();
        let solid_bg = Color32::from_rgba_unmultiplied(24, 24, 24, 255);
        style.visuals.window_fill = solid_bg;
        style.visuals.panel_fill = solid_bg;
        ctx.set_style(style);

        let mut open = true;
        let viewport = ctx.input(|i| i.viewport_rect());

        Window::new("tmux Sessions")
            .resizable(true)
            .default_width(400.0)
            .default_height(350.0)
            .default_pos(viewport.center())
            .pivot(egui::Align2::CENTER_CENTER)
            .open(&mut open)
            .frame(
                Frame::window(&ctx.style())
                    .fill(solid_bg)
                    .stroke(egui::Stroke::NONE)
                    .shadow(Shadow {
                        offset: [0, 0],
                        blur: 0,
                        spread: 0,
                        color: Color32::TRANSPARENT,
                    }),
            )
            .show(ctx, |ui| {
                // Error message
                if let Some(ref err) = self.error_message {
                    ui.colored_label(Color32::from_rgb(255, 100, 100), err);
                    ui.add_space(8.0);
                }

                // Existing sessions section
                ui.heading("Existing Sessions");
                ui.separator();

                if self.sessions.is_empty() {
                    ui.label(RichText::new("No tmux sessions found").italics());
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for session in &self.sessions {
                                ui.horizontal(|ui| {
                                    // Session name
                                    let name_text = if session.attached {
                                        RichText::new(&session.name).strong()
                                    } else {
                                        RichText::new(&session.name)
                                    };
                                    ui.label(name_text);

                                    // Window count
                                    ui.label(
                                        RichText::new(format!(
                                            "({} window{})",
                                            session.window_count,
                                            if session.window_count == 1 { "" } else { "s" }
                                        ))
                                        .weak(),
                                    );

                                    // Attached indicator
                                    if session.attached {
                                        ui.label(
                                            RichText::new("(attached)")
                                                .color(Color32::from_rgb(100, 200, 100)),
                                        );
                                    }

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("Attach").clicked() {
                                                action = SessionPickerAction::Attach(
                                                    session.name.clone(),
                                                );
                                                close_requested = true;
                                            }
                                        },
                                    );
                                });
                            }
                        });
                }

                ui.add_space(16.0);

                // Refresh button
                if ui.button("Refresh").clicked() {
                    self.refresh_sessions(tmux_path);
                }

                ui.add_space(16.0);

                // Create new session section
                ui.heading("Create New Session");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Session name:");
                    ui.text_edit_singleline(&mut self.new_session_name);
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() {
                        let name = if self.new_session_name.is_empty() {
                            None
                        } else {
                            Some(self.new_session_name.clone())
                        };
                        action = SessionPickerAction::CreateNew(name);
                        close_requested = true;
                    }

                    ui.label(
                        RichText::new("(leave empty for auto-generated name)")
                            .small()
                            .weak(),
                    );
                });
            });

        // Handle close
        if !open || close_requested {
            self.visible = false;
        }

        action
    }
}

impl Default for TmuxSessionPickerUI {
    fn default() -> Self {
        Self::new()
    }
}
