//! Session picker UI (UX.md A16): par-mux and tmux sessions in one list.
//!
//! An egui dialog that lists running par-mux sessions (attach here, switch,
//! create, rename, end, detach — see [`crate::session_picker_mux`]) and tmux
//! sessions (attach or create through the tmux gateway, when tmux
//! integration is on). Both lists load off the frame.

use egui::{Context, RichText};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

/// Deadline for `tmux list-sessions`. Instant against a healthy server; an
/// unresponsive one must not hold the egui frame.
const TMUX_LIST_TIMEOUT: Duration = Duration::from_secs(2);

/// Rows drawn before the tmux list scrolls.
const VISIBLE_ROWS: usize = 8;

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
    /// Filter over the tmux list.
    query: String,
    /// Selection and drawn window, on the shared picker (UX.md OV5).
    nav: crate::app::overlay::picker::ListNav,
    /// Whether the filter field should take focus (set on open).
    request_focus: bool,
    /// The live toggle chord for the footer; `None` when unbound.
    toggle_chord: Option<String>,
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
            query: String::new(),
            nav: Default::default(),
            request_focus: false,
            toggle_chord: None,
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
        self.query.clear();
        self.nav.reset();
        self.request_focus = true;
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

    /// Read the live chord from the registry (every frame, so every opening
    /// path shows the current binding).
    pub(crate) fn sync_toggle_chord(
        &mut self,
        registry: &par_term_keybindings::KeybindingRegistry,
    ) {
        self.toggle_chord = registry
            .chord_for_action("toggle_session_picker")
            .map(|c| crate::command_palette::catalog::chord_display(&c));
    }

    /// The chord the footer names.
    #[cfg(test)]
    pub(crate) fn toggle_chord(&self) -> Option<&str> {
        self.toggle_chord.as_deref()
    }

    /// Indices of the tmux sessions matching the filter, in list order.
    fn filtered(&self) -> Vec<usize> {
        let query = self.query.to_lowercase();
        self.sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| query.is_empty() || s.name.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect()
    }

    /// Show the session picker UI and return any requested action.
    ///
    /// The tmux list, filter, keys, and footer come from the shared picker
    /// (UX.md OV5): arrows, PageUp/PageDown, Home/End, Enter attaches the
    /// selected tmux session, Escape closes. The par-mux section, the
    /// loading state, Refresh, and Create sit in the picker's hooks, and
    /// the picker reads keys only while the filter (or nothing) has focus,
    /// so Enter still submits a rename or a new session name.
    pub fn show(
        &mut self,
        ctx: &Context,
        picker: &SessionPickerContext<'_>,
    ) -> SessionPickerAction {
        use crate::app::overlay::picker::{self, ListConfig, ListHooks, ListOutcome};

        if !self.visible {
            return SessionPickerAction::None;
        }
        let tmux_path = picker.tmux_path;

        // Load sessions on first show, off the frame (B69)
        if picker.tmux_enabled && !self.sessions_loaded {
            self.refresh_sessions(tmux_path);
        }
        self.poll_pending_load();

        let filtered = if picker.tmux_enabled {
            self.filtered()
        } else {
            Vec::new()
        };
        let loading = self.pending_load.is_some();
        let empty_text = if !picker.tmux_enabled {
            "Turn on tmux integration in Settings to list tmux sessions"
        } else if loading {
            "Loading sessions..."
        } else {
            "No tmux sessions found"
        };
        let config = ListConfig {
            id: "Sessions",
            hint: "Filter tmux sessions",
            visible_rows: VISIBLE_ROWS,
            width: crate::app::overlay::theme::WIDTH_LARGE,
            empty_text,
            enter_verb: "attach tmux session",
            toggle_chord: self.toggle_chord.as_deref(),
            alternates: false,
            alternate_labels: None,
            extra_keys: &[],
            multi_select: false,
        };

        let mut action = SessionPickerAction::None;
        let mut refresh_requested = false;
        let mut create_requested = false;
        let error = self.error_message.clone();
        let mux_section = &mut self.mux_section;
        let mux_action = &mut action;
        let mut above = |ui: &mut egui::Ui| {
            if let Some(mux) = picker.mux.as_ref()
                && let Some(chosen) = mux_section.show(ui, mux)
            {
                *mux_action = SessionPickerAction::Mux(chosen);
            }
            if !picker.tmux_enabled {
                return;
            }
            if picker.mux.is_some() {
                ui.add_space(8.0);
            }
            if let Some(err) = &error {
                ui.colored_label(crate::app::overlay::theme::DANGER, err);
            }
            ui.horizontal(|ui| {
                ui.heading("tmux Sessions");
                if loading {
                    ui.spinner();
                }
            });
        };
        let new_session_name = &mut self.new_session_name;
        let tmux_enabled = picker.tmux_enabled;
        let mut below = |ui: &mut egui::Ui| {
            if !tmux_enabled {
                return;
            }
            if ui.button("Refresh").clicked() {
                refresh_requested = true;
            }
            ui.add_space(8.0);
            ui.heading("Create New tmux Session");
            ui.horizontal(|ui| {
                ui.label("Session name:");
                ui.text_edit_singleline(new_session_name);
            });
            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    create_requested = true;
                }
                ui.label(
                    RichText::new("(leave empty for auto-generated name)")
                        .small()
                        .weak(),
                );
            });
        };
        let sessions = &self.sessions;
        let (outcome, query_changed) = picker::show_list_with(
            ctx,
            &config,
            ListHooks {
                keys_follow_filter: true,
                above: Some(&mut above),
                below: Some(&mut below),
            },
            &mut self.query,
            &mut self.request_focus,
            &mut self.nav,
            filtered.len(),
            |ui, index, selected| {
                let session = &sessions[filtered[index]];
                let name = if session.attached {
                    RichText::new(&session.name).strong()
                } else {
                    RichText::new(&session.name)
                };
                let mut clicked = false;
                ui.horizontal(|ui| {
                    clicked = ui.selectable_label(selected, name).clicked();
                    ui.label(
                        RichText::new(format!(
                            "({} window{})",
                            session.window_count,
                            if session.window_count == 1 { "" } else { "s" }
                        ))
                        .weak(),
                    );
                    if session.attached {
                        ui.label(
                            RichText::new("(attached)").color(crate::app::overlay::theme::SUCCESS),
                        );
                    }
                });
                clicked
            },
        );
        if query_changed {
            self.nav.reset();
        }
        if refresh_requested {
            self.refresh_sessions(tmux_path);
        }
        if create_requested {
            let name = (!self.new_session_name.is_empty()).then(|| self.new_session_name.clone());
            action = SessionPickerAction::CreateNew(name);
        }
        match outcome {
            ListOutcome::Chosen { index, .. } => {
                if let Some(&i) = filtered.get(index) {
                    action = SessionPickerAction::Attach(self.sessions[i].name.clone());
                }
            }
            ListOutcome::Closed => {
                self.visible = false;
                return SessionPickerAction::None;
            }
            ListOutcome::Open | ListOutcome::ToggleMark(_) => {}
        }
        if !matches!(action, SessionPickerAction::None) {
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
    use egui::Key;

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

    fn frame(
        ctx: &egui::Context,
        picker: &mut TmuxSessionPickerUI,
        key: Key,
    ) -> SessionPickerAction {
        ctx.begin_pass(egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            ..Default::default()
        });
        let action = picker.show(
            ctx,
            &SessionPickerContext {
                tmux_path: "/nonexistent/par-term-test-tmux",
                tmux_enabled: true,
                mux: None,
            },
        );
        ctx.end_pass().textures_delta.clear();
        action
    }

    fn loaded(names: &[&str]) -> TmuxSessionPickerUI {
        let mut picker = TmuxSessionPickerUI::new();
        picker.show_picker();
        picker.sessions = names
            .iter()
            .map(|n| TmuxSessionInfo {
                id: format!("${n}"),
                name: n.to_string(),
                window_count: 1,
                attached: false,
            })
            .collect();
        picker.sessions_loaded = true;
        picker
    }

    #[test]
    fn escape_closes_the_picker_and_returns_no_action() {
        // B69: Escape closes on the egui side, first and unconditionally.
        let ctx = egui::Context::default();
        let mut picker = loaded(&["a"]);
        let action = frame(&ctx, &mut picker, Key::Escape);
        assert!(!picker.visible);
        assert!(matches!(action, SessionPickerAction::None));
    }

    #[test]
    fn arrows_and_enter_attach_the_selected_tmux_session() {
        let ctx = egui::Context::default();
        let mut picker = loaded(&["alpha", "beta"]);
        let _ = frame(&ctx, &mut picker, Key::ArrowDown);
        match frame(&ctx, &mut picker, Key::Enter) {
            SessionPickerAction::Attach(name) => assert_eq!(name, "beta"),
            other => panic!("expected Attach, got {other:?}"),
        }
        assert!(!picker.visible);
    }

    #[test]
    fn enter_in_an_embedded_name_field_does_not_attach() {
        // keys_follow_filter: while another field (a rename or the new
        // session name) holds focus, Enter submits that field, not the list.
        let ctx = egui::Context::default();
        let mut picker = loaded(&["alpha"]);
        let _ = frame(&ctx, &mut picker, Key::ArrowDown);
        ctx.memory_mut(|m| m.request_focus(egui::Id::new("new-session-name")));
        let action = frame(&ctx, &mut picker, Key::Enter);
        assert!(matches!(action, SessionPickerAction::None));
        assert!(picker.visible, "the list did not take the Enter");
    }

    #[test]
    fn the_filter_narrows_the_tmux_list() {
        let mut picker = loaded(&["alpha", "beta"]);
        picker.query = "BET".into();
        assert_eq!(picker.filtered(), vec![1]);
    }
}
