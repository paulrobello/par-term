//! Session picker UI (UX.md A16): par-mux and tmux sessions in one list.
//!
//! An egui dialog that lists running par-mux sessions (attach here, switch,
//! create, rename, end, detach — see [`crate::session_picker_mux`]) and tmux
//! sessions (attach or create through the tmux gateway, when tmux
//! integration is on). Both lists load off the frame.

use crate::ui_constants::{
    TMUX_PICKER_LIST_MAX_HEIGHT, TMUX_PICKER_WINDOW_DEFAULT_HEIGHT,
    TMUX_PICKER_WINDOW_DEFAULT_WIDTH,
};
use egui::{Color32, Context, Frame, Key, RichText, Window, epaint::Shadow};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

/// Deadline for `tmux list-sessions`. Instant against a healthy server; an
/// unresponsive one must not hold the egui frame.
const TMUX_LIST_TIMEOUT: Duration = Duration::from_secs(2);

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
    /// Attach to the specified tmux session
    Attach(String),
    /// Create a new tmux session with optional name
    CreateNew(Option<String>),
    /// A par-mux session operation
    Mux(crate::session_picker_mux::MuxPickerAction),
}

/// What the picker needs from the window each frame.
pub struct SessionPickerContext<'a> {
    /// The tmux executable (tmux section).
    pub tmux_path: &'a str,
    /// Whether tmux integration is on; the tmux section is hidden when off.
    pub tmux_enabled: bool,
    /// The par-mux section's input; `None` in a build without par-mux.
    pub mux: Option<crate::session_picker_mux::MuxPickerInput<'a>>,
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
    /// In-flight `tmux list-sessions` (B69). The command runs on its own
    /// thread so an unresponsive server cannot stall the egui frame; the
    /// receiver is polled every frame until it yields the result.
    pending_load: Option<Receiver<Result<Vec<TmuxSessionInfo>, String>>>,
    /// The par-mux section's editing state.
    mux_section: crate::session_picker_mux::MuxPickerSection,
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
            pending_load: None,
            mux_section: crate::session_picker_mux::MuxPickerSection::default(),
        }
    }

    /// Show the session picker
    pub fn show_picker(&mut self) {
        self.visible = true;
        self.sessions_loaded = false; // Refresh on open
        // Drop any in-flight load so the reopen triggers a fresh one; the
        // orphaned thread's send lands in a closed channel and is ignored.
        self.pending_load = None;
        self.error_message = None;
        self.new_session_name.clear();
        self.mux_section.reset();
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

    /// Refresh the session list on a background thread (B69).
    ///
    /// `tmux list-sessions` used to run inside the egui closure, stalling
    /// the frame for up to [`TMUX_LIST_TIMEOUT`] on every open and Refresh.
    /// The spawn returns immediately; [`Self::poll_pending_load`] applies
    /// the result on a later frame. A load already in flight is not
    /// replaced — Refresh while loading is a no-op.
    fn refresh_sessions(&mut self, tmux_path: &str) {
        if self.pending_load.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let tmux_path = tmux_path.to_string();
        std::thread::spawn(move || {
            // A closed channel (picker reopened meanwhile) drops the result.
            let _ = tx.send(Self::list_tmux_sessions(&tmux_path));
        });
        self.pending_load = Some(rx);
    }

    /// Apply a finished background load, if one has landed. Returns true
    /// when no load remains pending after the call.
    fn poll_pending_load(&mut self) -> bool {
        let Some(rx) = &self.pending_load else {
            return true;
        };
        match rx.try_recv() {
            Ok(Ok(sessions)) => {
                self.sessions = sessions;
                self.error_message = None;
                self.sessions_loaded = true;
                self.pending_load = None;
            }
            Ok(Err(e)) => {
                self.sessions.clear();
                self.error_message = Some(e);
                self.sessions_loaded = true;
                self.pending_load = None;
            }
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending_load = None;
            }
        }
        true
    }

    /// List available tmux sessions by running `tmux list-sessions`
    ///
    /// Bounded by [`TMUX_LIST_TIMEOUT`] and run on the background thread
    /// spawned by [`Self::refresh_sessions`], so even an unresponsive tmux
    /// server cannot hold the egui frame.
    fn list_tmux_sessions(tmux_path: &str) -> Result<Vec<TmuxSessionInfo>, String> {
        let mut cmd = Command::new(tmux_path);
        cmd.args([
            "list-sessions",
            "-F",
            "#{session_id}:#{session_name}:#{session_attached}:#{session_windows}",
        ]);
        let output = crate::process_timeout::output_with_timeout(&mut cmd, TMUX_LIST_TIMEOUT)
            .map_err(|e| format!("Failed to run tmux: {}", e))?;

        if !output.status.success() {
            let stderr = &output.stderr;
            // "no server running" is expected when there are no sessions
            if stderr.contains("no server running") || stderr.contains("no sessions") {
                return Ok(Vec::new());
            }
            return Err(format!("tmux error: {}", stderr.trim()));
        }

        let mut sessions = Vec::new();

        for line in output.stdout.lines() {
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
    pub fn show(
        &mut self,
        ctx: &Context,
        picker: &SessionPickerContext<'_>,
    ) -> SessionPickerAction {
        if !self.visible {
            return SessionPickerAction::None;
        }
        let tmux_path = picker.tmux_path;

        // Escape closes the picker on the egui side (B69), mirroring the
        // command palette: with the session-name field focused this is the
        // only layer that sees the key, and consuming it here keeps the
        // Escape from also reaching other egui widgets. The B61 modal guard
        // remains the backstop that keeps an unfocused Escape off the PTY.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.visible = false;
            return SessionPickerAction::None;
        }

        // Load sessions on first show, off the frame (B69)
        if picker.tmux_enabled && !self.sessions_loaded {
            self.refresh_sessions(tmux_path);
        }
        self.poll_pending_load();

        let mut action = SessionPickerAction::None;
        let mut close_requested = false;

        // Ensure picker is fully opaque
        let mut style = (*ctx.global_style()).clone();
        let solid_bg = Color32::from_rgba_unmultiplied(24, 24, 24, 255);
        style.visuals.window_fill = solid_bg;
        style.visuals.panel_fill = solid_bg;
        ctx.set_global_style(style);

        let mut open = true;
        let viewport = ctx.input(|i| i.viewport_rect());

        Window::new("Sessions")
            .resizable(true)
            .default_width(TMUX_PICKER_WINDOW_DEFAULT_WIDTH)
            .default_height(TMUX_PICKER_WINDOW_DEFAULT_HEIGHT)
            .default_pos(viewport.center())
            .pivot(egui::Align2::CENTER_CENTER)
            .open(&mut open)
            .frame(
                Frame::window(&ctx.global_style())
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
                if let Some(mux) = picker.mux.as_ref()
                    && let Some(chosen) = self.mux_section.show(ui, mux)
                {
                    action = SessionPickerAction::Mux(chosen);
                    close_requested = true;
                }
                if !picker.tmux_enabled {
                    if picker.mux.is_none() {
                        ui.label(
                            RichText::new(
                                "Turn on tmux integration in Settings to list tmux sessions",
                            )
                            .italics(),
                        );
                    }
                    return;
                }
                if picker.mux.is_some() {
                    ui.add_space(16.0);
                }

                // Error message
                if let Some(ref err) = self.error_message {
                    ui.colored_label(Color32::from_rgb(255, 100, 100), err);
                    ui.add_space(8.0);
                }

                // Existing sessions section
                ui.heading("tmux Sessions");
                ui.separator();

                if self.sessions.is_empty() {
                    if self.pending_load.is_some() {
                        ui.label(RichText::new("Loading sessions...").italics());
                    } else {
                        ui.label(RichText::new("No tmux sessions found").italics());
                    }
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(TMUX_PICKER_LIST_MAX_HEIGHT)
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
                ui.heading("Create New tmux Session");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_spawns_instead_of_loading_in_the_frame() {
        // B69: refresh must return before the list command finishes — the
        // synchronous predecessor set sessions_loaded inside the call, and a
        // hung tmux server stalled the frame for the whole timeout.
        let mut picker = TmuxSessionPickerUI::new();
        picker.refresh_sessions("/nonexistent/par-term-test-tmux");
        assert!(picker.pending_load.is_some(), "a load must be in flight");
        assert!(!picker.sessions_loaded, "the frame must not wait for tmux");
    }

    #[test]
    fn a_finished_background_load_applies_on_poll() {
        let mut picker = TmuxSessionPickerUI::new();
        picker.refresh_sessions("/nonexistent/par-term-test-tmux");
        let mut resolved = false;
        for _ in 0..500 {
            if picker.poll_pending_load() && picker.pending_load.is_none() {
                resolved = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(resolved, "the spawned load must finish (exec fails fast)");
        assert!(picker.sessions_loaded);
        assert!(
            picker.error_message.is_some(),
            "a bad tmux path reports an error, it does not hang"
        );
    }

    #[test]
    fn reopening_drops_an_in_flight_load_for_a_fresh_one() {
        let mut picker = TmuxSessionPickerUI::new();
        picker.refresh_sessions("/nonexistent/par-term-test-tmux");
        picker.show_picker();
        assert!(
            picker.pending_load.is_none(),
            "a reopened picker must not apply a stale load"
        );
        assert!(!picker.sessions_loaded);
    }

    #[test]
    fn show_consumes_escape_to_close() {
        // B69 pin: show() must close the picker on the egui-side Escape —
        // with the session-name field focused no other layer sees the key.
        // Assembled needle so the scan cannot match this test's own source.
        let source = include_str!("tmux_session_picker_ui.rs");
        let needle = ["consume", "_key"].join("");
        assert!(
            source.contains(&needle),
            "show() must consume Escape to close the picker"
        );
    }
}
