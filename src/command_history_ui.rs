//! Fuzzy command history search overlay UI.
//!
//! Provides a searchable popup for browsing and selecting from command history,
//! with fuzzy matching and ranked results with match highlighting.

use crate::command_history::CommandHistoryEntry;
use egui::Context;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use std::collections::VecDeque;

/// Rows drawn before the list scrolls.
const VISIBLE_ROWS: usize = 14;

/// Command history UI manager using egui
pub struct CommandHistoryUI {
    /// Whether the command history window is currently visible
    pub visible: bool,

    /// Current search query
    search_query: String,

    /// Selection and drawn window, on the shared picker (UX.md OV5).
    nav: crate::app::overlay::picker::ListNav,

    /// Cached command history entries (refreshed when shown)
    cached_entries: Vec<CommandHistoryEntry>,

    /// Fuzzy matcher instance
    matcher: SkimMatcherV2,

    /// Whether the search input should request focus
    request_focus: bool,

    /// The live toggle chord for the footer; `None` when unbound.
    toggle_chord: Option<String>,
}

/// Action to take after showing the UI
#[derive(Debug, Clone)]
pub enum CommandHistoryAction {
    /// No action needed
    None,
    /// Insert the selected command into the terminal
    Insert(String),
}

impl Default for CommandHistoryUI {
    fn default() -> Self {
        Self::new()
    }
}

/// A matched entry with score and match indices for highlighting
struct MatchedEntry {
    index: usize,
    score: i64,
    indices: Vec<usize>,
}

impl CommandHistoryUI {
    /// Create a new command history UI
    pub fn new() -> Self {
        Self {
            visible: false,
            search_query: String::new(),
            nav: Default::default(),
            cached_entries: Vec::new(),
            matcher: SkimMatcherV2::default(),
            request_focus: false,
            toggle_chord: None,
        }
    }

    /// Open the command history UI
    pub fn open(&mut self) {
        self.visible = true;
        self.search_query.clear();
        self.request_focus = true;
        self.nav.reset();
    }

    /// Close the command history UI
    pub fn close(&mut self) {
        self.visible = false;
        self.search_query.clear();
        self.nav.reset();
    }

    /// Toggle visibility
    pub fn toggle(&mut self) {
        if self.visible {
            self.close();
        } else {
            self.open();
        }
    }

    /// Update cached entries from persistent command history
    pub fn update_entries(&mut self, entries: &VecDeque<CommandHistoryEntry>) {
        self.cached_entries = entries.iter().cloned().collect();
    }

    /// Get the command text of the currently selected entry (if any).
    /// Re-runs fuzzy matching to resolve the filtered index.
    pub fn selected_command(&self) -> Option<String> {
        let matches = self.get_matched_entries();
        matches
            .get(self.nav.selected)
            .map(|m| self.cached_entries[m.index].command.clone())
    }

    /// Get fuzzy-matched and ranked entries based on current search query
    fn get_matched_entries(&self) -> Vec<MatchedEntry> {
        if self.search_query.is_empty() {
            // No query: return all entries in order (newest first)
            return self
                .cached_entries
                .iter()
                .enumerate()
                .map(|(i, _)| MatchedEntry {
                    index: i,
                    score: 0,
                    indices: Vec::new(),
                })
                .collect();
        }

        let mut matches: Vec<MatchedEntry> = self
            .cached_entries
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                self.matcher
                    .fuzzy_indices(&entry.command, &self.search_query)
                    .map(|(score, indices)| MatchedEntry {
                        index: i,
                        score,
                        indices,
                    })
            })
            .collect();

        // Sort by score descending (best matches first)
        matches.sort_by_key(|m| std::cmp::Reverse(m.score));
        matches
    }

    /// Record the live `toggle_command_history` chord the footer names
    /// (UX.md OV5); `None` when unbound. Set every frame by the renderer.
    pub fn set_toggle_chord(&mut self, chord: Option<String>) {
        self.toggle_chord = chord;
    }

    /// Read the live chord from the registry (every frame, so every opening
    /// path shows the current binding).
    pub(crate) fn sync_toggle_chord(
        &mut self,
        registry: &par_term_keybindings::KeybindingRegistry,
    ) {
        self.toggle_chord = registry
            .chord_for_action("toggle_command_history")
            .map(|c| crate::command_palette::catalog::chord_display(&c));
    }

    /// The chord the footer names.
    #[cfg(test)]
    pub(crate) fn toggle_chord(&self) -> Option<&str> {
        self.toggle_chord.as_deref()
    }

    /// Show the command history window and return any action to take.
    ///
    /// Keys, scrolling, and the footer come from the shared picker (UX.md
    /// OV5): arrows, PageUp/PageDown, Home/End, Enter inserts, Escape closes.
    pub fn show(&mut self, ctx: &Context) -> CommandHistoryAction {
        use crate::app::overlay::picker::{self, ListConfig, ListHooks, ListOutcome};

        if !self.visible {
            return CommandHistoryAction::None;
        }

        let matched_entries = self.get_matched_entries();
        let config = ListConfig {
            id: "Command History Search",
            hint: "Search commands",
            visible_rows: VISIBLE_ROWS,
            width: crate::app::overlay::theme::WIDTH_LARGE,
            empty_text: "No matching commands",
            enter_verb: "insert",
            toggle_chord: self.toggle_chord.as_deref(),
            alternates: false,
            alternate_labels: None,
            extra_keys: &[],
            multi_select: false,
        };
        let count = format!(
            "{} / {} commands",
            matched_entries.len(),
            self.cached_entries.len()
        );
        let mut show_count = |ui: &mut egui::Ui| {
            ui.label(count.as_str());
        };
        let entries = &self.cached_entries;
        let (outcome, query_changed) = picker::show_list_with(
            ctx,
            &config,
            ListHooks {
                keys_follow_filter: false,
                above: Some(&mut show_count),
                below: None,
            },
            &mut self.search_query,
            &mut self.request_focus,
            &mut self.nav,
            matched_entries.len(),
            |ui, index, selected| {
                let matched = &matched_entries[index];
                let entry = &entries[matched.index];
                let job = build_highlighted_label(
                    &entry.command,
                    &matched.indices,
                    selected,
                    entry.exit_code,
                    entry.timestamp_ms,
                );
                let response = ui
                    .selectable_label(selected, job)
                    .on_hover_text(format_tooltip(entry));
                response.clicked()
            },
        );
        if query_changed {
            self.nav.reset();
        }
        match outcome {
            ListOutcome::Chosen { index, .. } => {
                let command = matched_entries
                    .get(index)
                    .map(|m| self.cached_entries[m.index].command.clone());
                self.close();
                command.map_or(CommandHistoryAction::None, CommandHistoryAction::Insert)
            }
            ListOutcome::Closed => {
                self.close();
                CommandHistoryAction::None
            }
            ListOutcome::Open | ListOutcome::ToggleMark(_) => CommandHistoryAction::None,
        }
    }
}

impl crate::traits::OverlayComponent for CommandHistoryUI {
    type Action = CommandHistoryAction;

    fn show(&mut self, ctx: &egui::Context) -> Self::Action {
        CommandHistoryUI::show(self, ctx)
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

/// Build an egui LayoutJob with fuzzy match highlighting
fn build_highlighted_label(
    command: &str,
    match_indices: &[usize],
    is_selected: bool,
    exit_code: Option<i32>,
    timestamp_ms: u64,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();

    // Exit code indicator
    let status_color = match exit_code {
        Some(0) => egui::Color32::from_rgb(100, 200, 100), // Green for success
        Some(_) => egui::Color32::from_rgb(200, 100, 100), // Red for failure
        None => egui::Color32::from_rgb(150, 150, 150),    // Gray for unknown
    };
    // Use Nerd Font PUA codepoints (confirmed in SymbolsNerdFontMono-Regular):
    // U+F05D = fa-check-circle, U+F057 = fa-times-circle
    // Unknown exit code falls back to ASCII '?' (always renderable)
    let status_char = match exit_code {
        Some(0) => "\u{f05d} ",
        Some(_) => "\u{f057} ",
        None => "? ",
    };
    job.append(
        status_char,
        0.0,
        egui::TextFormat {
            color: status_color,
            ..Default::default()
        },
    );

    // Command text with highlighting
    let normal_color = if is_selected {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_rgb(220, 220, 220)
    };
    let highlight_color = egui::Color32::from_rgb(255, 200, 0); // Yellow highlight

    let chars: Vec<char> = command.chars().collect();
    // Truncate display for very long commands
    let display_len = chars.len().min(120);

    let mut i = 0;
    while i < display_len {
        let is_match = match_indices.contains(&i);
        let color = if is_match {
            highlight_color
        } else {
            normal_color
        };

        // Batch consecutive chars with same highlight state
        let start = i;
        while i < display_len && match_indices.contains(&i) == is_match {
            i += 1;
        }

        let text: String = chars[start..i].iter().collect();
        let format = if is_match {
            egui::TextFormat {
                color,
                underline: egui::Stroke::new(1.0, highlight_color),
                ..Default::default()
            }
        } else {
            egui::TextFormat {
                color,
                ..Default::default()
            }
        };
        job.append(&text, 0.0, format);
    }

    if chars.len() > 120 {
        job.append(
            "...",
            0.0,
            egui::TextFormat {
                color: egui::Color32::GRAY,
                ..Default::default()
            },
        );
    }

    // Timestamp suffix
    let time_str = format_relative_time(timestamp_ms);
    job.append(
        &format!("  {time_str}"),
        0.0,
        egui::TextFormat {
            color: egui::Color32::from_rgb(120, 120, 120),
            ..Default::default()
        },
    );

    job
}

/// Format a tooltip with full command details
fn format_tooltip(entry: &CommandHistoryEntry) -> String {
    let mut parts = vec![entry.command.clone()];
    if let Some(code) = entry.exit_code {
        parts.push(format!("Exit: {code}"));
    }
    if let Some(ms) = entry.duration_ms {
        parts.push(format!("Duration: {}ms", ms));
    }
    parts.push(format_relative_time(entry.timestamp_ms));
    parts.join("\n")
}

/// Format a timestamp as relative time (e.g., "5m ago")
fn format_relative_time(timestamp_ms: u64) -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let time = UNIX_EPOCH + Duration::from_millis(timestamp_ms);
    if let Ok(elapsed) = SystemTime::now().duration_since(time) {
        let secs = elapsed.as_secs();
        if secs < 60 {
            format!("{secs}s ago")
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else {
            format!("{}d ago", secs / 86400)
        }
    } else {
        "just now".to_string()
    }
}

#[cfg(test)]
mod b70_tests {
    use super::*;

    fn key_event(key: egui::Key, shift: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                shift,
                ..Default::default()
            },
        }
    }

    /// One show() per press, mirroring the app's one-rendered-frame-per-key
    /// cadence the ui-test harness also produces.
    fn frame(
        ctx: &egui::Context,
        ui: &mut CommandHistoryUI,
        events: Vec<egui::Event>,
    ) -> CommandHistoryAction {
        let raw = egui::RawInput {
            events,
            ..Default::default()
        };
        ctx.begin_pass(raw);
        let action = ui.show(ctx);
        // Headless passes still allocate font-atlas textures; dropping an
        // unapplied TexturesDelta panics (epaint), so clear it.
        ctx.end_pass().textures_delta.clear();
        action
    }

    fn entries() -> VecDeque<CommandHistoryEntry> {
        let mk = |command: &str, ts: u64| CommandHistoryEntry {
            command: command.into(),
            timestamp_ms: ts,
            exit_code: Some(0),
            duration_ms: None,
        };
        VecDeque::from(vec![
            mk("echo b70-gamma", 3_000_000_000),
            mk("echo b70-beta", 2_000_000_000),
            mk("echo b70-alpha", 1_000_000_000),
        ])
    }

    #[test]
    fn arrows_and_enter_drive_selection_and_insert() {
        let ctx = egui::Context::default();
        let mut ui = CommandHistoryUI::new();
        ui.update_entries(&entries());
        ui.open();
        assert_eq!(ui.nav.selected, 0);

        let _ = frame(&ctx, &mut ui, vec![key_event(egui::Key::ArrowDown, false)]);
        let _ = frame(&ctx, &mut ui, vec![key_event(egui::Key::ArrowDown, false)]);
        assert_eq!(
            ui.nav.selected, 2,
            "two ArrowDown presses move to the third row"
        );

        let action = frame(&ctx, &mut ui, vec![key_event(egui::Key::Enter, false)]);
        assert!(!ui.visible, "Enter closes the panel");
        match action {
            CommandHistoryAction::Insert(cmd) => assert_eq!(cmd, "echo b70-alpha"),
            other => panic!("expected Insert, got {other:?}"),
        }
    }

    #[test]
    fn arrow_up_moves_back_toward_the_newest_row() {
        let ctx = egui::Context::default();
        let mut ui = CommandHistoryUI::new();
        ui.update_entries(&entries());
        ui.open();
        let _ = frame(&ctx, &mut ui, vec![key_event(egui::Key::ArrowDown, false)]);
        let _ = frame(&ctx, &mut ui, vec![key_event(egui::Key::ArrowUp, false)]);
        assert_eq!(ui.nav.selected, 0);
    }

    #[test]
    fn enter_without_arrows_inserts_the_newest_row() {
        let ctx = egui::Context::default();
        let mut ui = CommandHistoryUI::new();
        ui.update_entries(&entries());
        ui.open();
        let action = frame(&ctx, &mut ui, vec![key_event(egui::Key::Enter, false)]);
        match action {
            CommandHistoryAction::Insert(cmd) => assert_eq!(cmd, "echo b70-gamma"),
            other => panic!("expected Insert, got {other:?}"),
        }
    }

    #[test]
    fn escape_closes_without_an_action() {
        let ctx = egui::Context::default();
        let mut ui = CommandHistoryUI::new();
        ui.update_entries(&entries());
        ui.open();
        let action = frame(&ctx, &mut ui, vec![key_event(egui::Key::Escape, false)]);
        assert!(!ui.visible, "Escape closes the panel");
        assert!(matches!(action, CommandHistoryAction::None));
    }

    #[test]
    fn enter_with_no_entries_is_a_no_op() {
        let ctx = egui::Context::default();
        let mut ui = CommandHistoryUI::new();
        ui.update_entries(&VecDeque::new());
        ui.open();
        let action = frame(&ctx, &mut ui, vec![key_event(egui::Key::Enter, false)]);
        assert!(ui.visible, "nothing to insert — panel stays open");
        assert!(matches!(action, CommandHistoryAction::None));
    }

    #[test]
    fn the_footer_names_the_live_toggle_chord_and_insert() {
        let mut ui = CommandHistoryUI::new();
        ui.set_toggle_chord(Some("Cmd+Shift+H".into()));
        assert_eq!(ui.toggle_chord(), Some("Cmd+Shift+H"));
        let config = crate::app::overlay::picker::ListConfig {
            id: "t",
            hint: "",
            visible_rows: VISIBLE_ROWS,
            width: 0.0,
            empty_text: "",
            enter_verb: "insert",
            toggle_chord: ui.toggle_chord(),
            alternates: false,
            alternate_labels: None,
            extra_keys: &[],
            multi_select: false,
        };
        assert!(
            crate::app::overlay::picker::footer_text(&config)
                .ends_with("Enter insert · Esc or Cmd+Shift+H close")
        );
    }

    #[test]
    fn page_down_and_end_move_the_shared_selection() {
        let ctx = egui::Context::default();
        let mut ui = CommandHistoryUI::new();
        ui.update_entries(&entries());
        ui.open();
        let _ = frame(&ctx, &mut ui, vec![key_event(egui::Key::End, false)]);
        assert_eq!(ui.nav.selected, 2);
    }
}
