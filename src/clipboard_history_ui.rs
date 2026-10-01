use egui::Context;
use par_term_terminal::{ClipboardEntry, ClipboardSlot};

/// Rows drawn before the list scrolls.
const VISIBLE_ROWS: usize = 12;

/// Shift+Enter opens paste special: the one alternate, taken as an extra
/// key so the footer does not advertise a Cmd+Enter the list does not use.
const PASTE_SPECIAL_KEY: [crate::app::overlay::picker::ExtraKey; 1] =
    [crate::app::overlay::picker::ExtraKey {
        modifiers: egui::Modifiers::SHIFT,
        key: egui::Key::Enter,
        label: "Shift+Enter paste special",
    }];

/// Clipboard history UI manager using egui
pub struct ClipboardHistoryUI {
    /// Whether the clipboard history window is currently visible
    pub visible: bool,

    /// Current search query
    search_query: String,

    /// Selection and drawn window, on the shared picker (UX.md OV5).
    nav: crate::app::overlay::picker::ListNav,

    /// Whether the search field should take focus (set on open).
    request_focus: bool,

    /// The live toggle chord for the footer; `None` when unbound.
    toggle_chord: Option<String>,

    /// Cached clipboard history entries (refreshed when shown)
    cached_entries: Vec<ClipboardEntry>,
}

/// Action to take after showing the UI
#[derive(Debug, Clone)]
pub enum ClipboardHistoryAction {
    /// No action needed
    None,
    /// Paste the selected entry content
    Paste(String),
    /// Open the paste-special dialog preloaded with the selected content
    /// (Shift+Enter from the history list)
    OpenPasteSpecial(String),
    /// Clear clipboard history for a slot
    ClearSlot(ClipboardSlot),
    /// Clear all clipboard history
    ClearAll,
}

impl Default for ClipboardHistoryUI {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardHistoryUI {
    /// Create a new clipboard history UI
    pub fn new() -> Self {
        Self {
            visible: false,
            search_query: String::new(),
            nav: Default::default(),
            request_focus: false,
            toggle_chord: None,
            cached_entries: Vec::new(),
        }
    }

    /// Toggle clipboard history window visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            self.nav.reset();
            self.request_focus = true;
        }
    }

    /// Update cached entries from terminal
    pub fn update_entries(&mut self, entries: Vec<ClipboardEntry>) {
        self.cached_entries = entries;
    }

    /// Entries matching the search query, in list order.
    fn filtered(&self) -> Vec<&ClipboardEntry> {
        let query = self.search_query.to_lowercase();
        self.cached_entries
            .iter()
            .filter(|e| query.is_empty() || e.content.to_lowercase().contains(&query))
            .collect()
    }

    /// Get the currently selected entry
    pub fn selected_entry(&self) -> Option<&ClipboardEntry> {
        self.filtered().get(self.nav.selected).copied()
    }

    /// Read the live chord from the registry (every frame, so every opening
    /// path shows the current binding).
    pub(crate) fn sync_toggle_chord(
        &mut self,
        registry: &par_term_keybindings::KeybindingRegistry,
    ) {
        self.toggle_chord = registry
            .chord_for_action("toggle_clipboard_history")
            .map(|c| crate::command_palette::catalog::chord_display(&c));
    }

    /// The chord the footer names.
    #[cfg(test)]
    pub(crate) fn toggle_chord(&self) -> Option<&str> {
        self.toggle_chord.as_deref()
    }

    /// Show the clipboard history window and return any action to take.
    ///
    /// Keys, scrolling, and the footer come from the shared picker (UX.md
    /// OV5): arrows, PageUp/PageDown, Home/End, Enter pastes, Shift+Enter
    /// opens paste special, Escape closes.
    pub fn show(&mut self, ctx: &Context) -> ClipboardHistoryAction {
        use crate::app::overlay::picker::{self, Activation, ListConfig, ListHooks, ListOutcome};

        if !self.visible {
            return ClipboardHistoryAction::None;
        }

        let matches: Vec<ClipboardEntry> = self.filtered().into_iter().cloned().collect();
        let config = ListConfig {
            id: "Clipboard History",
            hint: "Search clipboard history",
            visible_rows: VISIBLE_ROWS,
            width: crate::app::overlay::theme::WIDTH_MEDIUM,
            empty_text: "No clipboard history entries",
            enter_verb: "paste",
            toggle_chord: self.toggle_chord.as_deref(),
            alternates: false,
            alternate_labels: None,
            extra_keys: &PASTE_SPECIAL_KEY,
            multi_select: false,
        };
        let mut clear_requested = false;
        let mut clear_button = |ui: &mut egui::Ui| {
            if ui.button("Clear History").clicked() {
                clear_requested = true;
            }
        };
        let (outcome, query_changed) = picker::show_list_with(
            ctx,
            &config,
            ListHooks {
                keys_follow_filter: false,
                above: None,
                below: Some(&mut clear_button),
            },
            &mut self.search_query,
            &mut self.request_focus,
            &mut self.nav,
            matches.len(),
            |ui, index, selected| {
                let entry = &matches[index];
                let label = format!(
                    "[{}] {}",
                    format_timestamp(entry.timestamp),
                    truncate_preview(&entry.content, 80)
                );
                ui.selectable_label(selected, label)
                    .on_hover_text(&entry.content)
                    .clicked()
            },
        );
        if query_changed {
            self.nav.reset();
        }
        if clear_requested {
            return ClipboardHistoryAction::ClearAll;
        }
        match outcome {
            ListOutcome::Chosen { index, how } => {
                let content = matches.get(index).map(|e| e.content.clone());
                self.visible = false;
                match (content, how) {
                    (Some(c), Activation::Extra(_)) => ClipboardHistoryAction::OpenPasteSpecial(c),
                    (Some(c), _) => ClipboardHistoryAction::Paste(c),
                    (None, _) => ClipboardHistoryAction::None,
                }
            }
            ListOutcome::Closed => {
                self.visible = false;
                ClipboardHistoryAction::None
            }
            ListOutcome::Open | ListOutcome::ToggleMark(_) => ClipboardHistoryAction::None,
        }
    }
}

impl crate::traits::OverlayComponent for ClipboardHistoryUI {
    type Action = ClipboardHistoryAction;

    fn show(&mut self, ctx: &egui::Context) -> Self::Action {
        ClipboardHistoryUI::show(self, ctx)
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

/// Truncate content for preview display
fn truncate_preview(content: &str, max_len: usize) -> String {
    // Replace newlines with visible markers
    let single_line = content.replace('\n', "↵").replace('\r', "");

    if single_line.len() <= max_len {
        single_line
    } else {
        let boundary = single_line.floor_char_boundary(max_len);
        format!("{}...", &single_line[..boundary])
    }
}

/// Format timestamp for display
fn format_timestamp(timestamp_us: u64) -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let duration = Duration::from_micros(timestamp_us);
    let time = UNIX_EPOCH + duration;

    if let Ok(elapsed) = SystemTime::now().duration_since(time) {
        let secs = elapsed.as_secs();
        if secs < 60 {
            format!("{}s ago", secs)
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

    fn frame(
        ctx: &egui::Context,
        ui: &mut ClipboardHistoryUI,
        events: Vec<egui::Event>,
    ) -> ClipboardHistoryAction {
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

    fn entries() -> Vec<ClipboardEntry> {
        vec![
            ClipboardEntry {
                content: "b70-clip-BBB".into(),
                timestamp: 3_000_000_000,
                label: None,
            },
            ClipboardEntry {
                content: "b70-clip-AAA".into(),
                timestamp: 1_000_000_000,
                label: None,
            },
        ]
    }

    #[test]
    fn arrows_and_enter_paste_the_second_row() {
        let ctx = egui::Context::default();
        let mut ui = ClipboardHistoryUI::new();
        ui.update_entries(entries());
        ui.toggle();
        assert_eq!(ui.nav.selected, 0);

        let _ = frame(&ctx, &mut ui, vec![key_event(egui::Key::ArrowDown, false)]);
        assert_eq!(ui.nav.selected, 1);

        let action = frame(&ctx, &mut ui, vec![key_event(egui::Key::Enter, false)]);
        assert!(!ui.visible, "Enter closes the panel");
        match action {
            ClipboardHistoryAction::Paste(content) => assert_eq!(content, "b70-clip-AAA"),
            other => panic!("expected Paste, got {other:?}"),
        }
    }

    #[test]
    fn shift_enter_opens_paste_special_instead_of_pasting() {
        let ctx = egui::Context::default();
        let mut ui = ClipboardHistoryUI::new();
        ui.update_entries(entries());
        ui.toggle();

        let action = frame(&ctx, &mut ui, vec![key_event(egui::Key::Enter, true)]);
        assert!(!ui.visible, "Shift+Enter closes the history panel");
        match action {
            ClipboardHistoryAction::OpenPasteSpecial(content) => {
                assert_eq!(content, "b70-clip-BBB")
            }
            other => panic!("expected OpenPasteSpecial, got {other:?}"),
        }
    }

    #[test]
    fn escape_closes_without_an_action() {
        let ctx = egui::Context::default();
        let mut ui = ClipboardHistoryUI::new();
        ui.update_entries(entries());
        ui.toggle();
        let action = frame(&ctx, &mut ui, vec![key_event(egui::Key::Escape, false)]);
        assert!(!ui.visible);
        assert!(matches!(action, ClipboardHistoryAction::None));
    }

    #[test]
    fn the_footer_names_paste_special_and_the_live_chord() {
        let mut ui = ClipboardHistoryUI::new();
        ui.toggle_chord = Some("Cmd+Shift+V".into());
        let config = crate::app::overlay::picker::ListConfig {
            id: "t",
            hint: "",
            visible_rows: VISIBLE_ROWS,
            width: 0.0,
            empty_text: "",
            enter_verb: "paste",
            toggle_chord: ui.toggle_chord(),
            alternates: false,
            alternate_labels: None,
            extra_keys: &PASTE_SPECIAL_KEY,
            multi_select: false,
        };
        let footer = crate::app::overlay::picker::footer_text(&config);
        assert!(footer.contains("Shift+Enter paste special"));
        assert!(!footer.contains("Cmd+Enter"));
        assert!(footer.ends_with("Esc or Cmd+Shift+V close"));
    }

    #[test]
    fn cmd_enter_is_not_an_alternate() {
        // Only Shift+Enter is an alternate. Cmd+Enter is not advertised, and
        // egui's plain-Enter match rejects a held Command, so it does nothing.
        let ctx = egui::Context::default();
        let mut ui = ClipboardHistoryUI::new();
        ui.update_entries(entries());
        ui.toggle();
        let cmd_enter = egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        };
        let action = frame(&ctx, &mut ui, vec![cmd_enter]);
        assert!(matches!(action, ClipboardHistoryAction::None));
        assert!(ui.visible, "Cmd+Enter neither pastes nor closes");
    }

    #[test]
    fn a_filtered_list_selects_among_the_matches() {
        let mut ui = ClipboardHistoryUI::new();
        ui.update_entries(entries());
        ui.search_query = "AAA".into();
        assert_eq!(ui.selected_entry().unwrap().content, "b70-clip-AAA");
    }
}
