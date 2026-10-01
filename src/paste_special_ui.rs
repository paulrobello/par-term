//! Paste Special UI - Command palette for text transformations.
//!
//! Provides a fuzzy-searchable command palette for applying text transformations
//! to clipboard content before pasting.

use crate::paste_transform::{PasteTransform, transform};
use egui::{Color32, Context, RichText};

/// Rows drawn before the transformation list scrolls.
const VISIBLE_ROWS: usize = 10;

/// Action to take after showing the UI
#[derive(Debug, Clone)]
pub enum PasteSpecialAction {
    /// No action needed
    None,
    /// Paste the transformed content
    Paste(String),
}

/// Paste Special UI manager using egui
pub struct PasteSpecialUI {
    /// Whether the paste special window is currently visible
    pub visible: bool,

    /// Current search query for filtering transformations
    search_query: String,

    /// Selection and drawn window, on the shared picker (UX.md OV5).
    nav: crate::app::overlay::picker::ListNav,

    /// Whether the search field should take focus (set on open).
    request_focus: bool,

    /// The clipboard content to transform
    content: String,

    /// Cached filtered transformations (updated when search changes)
    filtered_transforms: Vec<PasteTransform>,

    /// Preview of the transformed content (or error message)
    preview_result: Result<String, String>,
}

impl Default for PasteSpecialUI {
    fn default() -> Self {
        Self::new()
    }
}

impl PasteSpecialUI {
    /// Create a new paste special UI
    pub fn new() -> Self {
        let filtered = PasteTransform::all().to_vec();
        Self {
            visible: false,
            search_query: String::new(),
            nav: Default::default(),
            request_focus: false,
            content: String::new(),
            filtered_transforms: filtered,
            preview_result: Ok(String::new()),
        }
    }

    /// Open the paste special UI with the given clipboard content
    pub fn open(&mut self, content: String) {
        self.visible = true;
        self.content = content;
        self.search_query.clear();
        self.nav.reset();
        self.request_focus = true;
        self.update_filtered_transforms();
        self.update_preview();
    }

    /// Close the paste special UI
    pub fn close(&mut self) {
        self.visible = false;
        self.content.clear();
        self.search_query.clear();
    }

    /// Get the currently selected transformation
    pub fn selected_transform(&self) -> Option<PasteTransform> {
        self.filtered_transforms.get(self.nav.selected).copied()
    }

    /// Apply the selected transformation and return the result
    pub fn apply_selected(&self) -> Option<String> {
        self.selected_transform()
            .and_then(|t| transform(&self.content, t).ok())
    }

    /// Update the filtered transformations based on search query
    fn update_filtered_transforms(&mut self) {
        self.filtered_transforms = PasteTransform::all()
            .iter()
            .filter(|t| t.matches_query(&self.search_query))
            .copied()
            .collect();
        self.nav
            .keep_visible(self.filtered_transforms.len(), VISIBLE_ROWS);
    }

    /// Update the preview result for the current selection
    fn update_preview(&mut self) {
        if let Some(t) = self.selected_transform() {
            self.preview_result = transform(&self.content, t);
        } else {
            self.preview_result = Ok(self.content.clone());
        }
    }

    /// Show the paste special window and return any action to take.
    ///
    /// The transformation list, keys, and footer come from the shared
    /// picker (UX.md OV5): arrows, PageUp/PageDown, Home/End, Enter applies
    /// (a no-op while the transform fails), Escape closes. The preview is
    /// drawn below the list.
    pub fn show(&mut self, ctx: &Context) -> PasteSpecialAction {
        use crate::app::overlay::picker::{self, ListConfig, ListHooks, ListOutcome};

        if !self.visible {
            return PasteSpecialAction::None;
        }

        let before = (self.nav.selected, self.filtered_transforms.len());
        let config = ListConfig {
            id: "Paste Special",
            hint: "Search transformations",
            visible_rows: VISIBLE_ROWS,
            width: crate::app::overlay::theme::WIDTH_MEDIUM,
            empty_text: "No matching transformations",
            enter_verb: "apply",
            // No toggle action: the overlay stack has no chord that closes it.
            toggle_chord: None,
            alternates: false,
            alternate_labels: None,
            extra_keys: &[],
            multi_select: false,
        };
        let transforms = self.filtered_transforms.clone();
        let content = self.content.clone();
        let preview = self.preview_result.clone();
        let mut show_preview = |ui: &mut egui::Ui| {
            ui.label(RichText::new("Original:").small().color(Color32::GRAY));
            ui.label(
                RichText::new(truncate_preview(&content, 100))
                    .monospace()
                    .color(Color32::LIGHT_GRAY),
            );
            ui.add_space(4.0);
            ui.label(RichText::new("Result:").small().color(Color32::GRAY));
            match &preview {
                Ok(result) => ui.label(
                    RichText::new(truncate_preview(result, 100))
                        .monospace()
                        .color(Color32::LIGHT_GREEN),
                ),
                Err(error) => ui.label(RichText::new(error).monospace().color(Color32::RED)),
            };
            ui.label(
                RichText::new(format!("{} chars", content.len()))
                    .small()
                    .color(Color32::GRAY),
            );
        };
        let (outcome, query_changed) = picker::show_list_with(
            ctx,
            &config,
            ListHooks {
                keys_follow_filter: false,
                above: None,
                below: Some(&mut show_preview),
            },
            &mut self.search_query,
            &mut self.request_focus,
            &mut self.nav,
            transforms.len(),
            |ui, index, selected| {
                let t = transforms[index];
                ui.selectable_label(selected, t.display_name())
                    .on_hover_text(t.description())
                    .clicked()
            },
        );
        if query_changed {
            self.nav.reset();
            self.update_filtered_transforms();
        }
        match outcome {
            ListOutcome::Chosen { index, .. } => {
                self.nav.selected = index;
                match self.apply_selected() {
                    Some(result) => {
                        self.close();
                        PasteSpecialAction::Paste(result)
                    }
                    None => PasteSpecialAction::None,
                }
            }
            ListOutcome::Closed => {
                self.close();
                PasteSpecialAction::None
            }
            ListOutcome::Open | ListOutcome::ToggleMark(_) => {
                if query_changed || (self.nav.selected, self.filtered_transforms.len()) != before {
                    self.update_preview();
                }
                PasteSpecialAction::None
            }
        }
    }
}

impl crate::traits::OverlayComponent for PasteSpecialUI {
    type Action = PasteSpecialAction;

    fn show(&mut self, ctx: &egui::Context) -> Self::Action {
        PasteSpecialUI::show(self, ctx)
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
    let single_line = content
        .replace('\n', "↵")
        .replace('\r', "")
        .replace('\t', "→");

    if single_line.chars().count() <= max_len {
        single_line
    } else {
        let truncated: String = single_line.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_and_close() {
        let mut ui = PasteSpecialUI::new();
        assert!(!ui.visible);

        ui.open("test content".to_string());
        assert!(ui.visible);
        assert_eq!(ui.content, "test content");

        ui.close();
        assert!(!ui.visible);
        assert!(ui.content.is_empty());
    }

    #[test]
    fn test_navigation() {
        let mut ui = PasteSpecialUI::new();
        ui.open("test".to_string());

        assert_eq!(ui.nav.selected, 0);

        ui.nav.selected = 1;
        assert_eq!(ui.selected_transform(), Some(PasteTransform::all()[1]));
    }

    #[test]
    fn test_apply_selected() {
        let mut ui = PasteSpecialUI::new();
        ui.open("hello world".to_string());

        // Find UPPERCASE transform
        ui.search_query = "UPPER".to_string();
        ui.update_filtered_transforms();

        let result = ui.apply_selected();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "HELLO WORLD");
    }

    #[test]
    fn test_truncate_preview() {
        assert_eq!(truncate_preview("hello", 10), "hello");
        assert_eq!(truncate_preview("hello world", 5), "hello...");
        assert_eq!(truncate_preview("line1\nline2", 20), "line1↵line2");
        assert_eq!(truncate_preview("tab\there", 20), "tab→here");
    }

    #[test]
    fn test_search_filtering() {
        let mut ui = PasteSpecialUI::new();
        ui.open("test".to_string());

        // All transforms initially
        let all_count = PasteTransform::all().len();
        assert_eq!(ui.filtered_transforms.len(), all_count);

        // Filter to shell only
        ui.search_query = "shell".to_string();
        ui.update_filtered_transforms();
        assert_eq!(ui.filtered_transforms.len(), 3); // Single, Double, Backslash

        // Filter to base64
        ui.search_query = "base64".to_string();
        ui.update_filtered_transforms();
        assert_eq!(ui.filtered_transforms.len(), 2); // Encode and Decode
    }
}

#[cfg(test)]
mod b70_tests {
    use super::*;

    fn key_event(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }
    }

    fn frame(
        ctx: &egui::Context,
        ui: &mut PasteSpecialUI,
        events: Vec<egui::Event>,
    ) -> PasteSpecialAction {
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

    #[test]
    fn arrows_move_selection_and_enter_applies_it() {
        let ctx = egui::Context::default();
        let mut ui = PasteSpecialUI::new();
        ui.open("hello world".into());
        let start = ui.nav.selected;

        let _ = frame(&ctx, &mut ui, vec![key_event(egui::Key::ArrowDown)]);
        assert_eq!(ui.nav.selected, start + 1, "ArrowDown moves selection");
        let _ = frame(&ctx, &mut ui, vec![key_event(egui::Key::ArrowUp)]);
        assert_eq!(ui.nav.selected, start, "ArrowUp moves it back");

        let action = frame(&ctx, &mut ui, vec![key_event(egui::Key::Enter)]);
        assert!(!ui.visible, "Enter applies and closes");
        match action {
            PasteSpecialAction::Paste(result) => assert!(!result.is_empty()),
            other => panic!("expected Paste, got {other:?}"),
        }
    }

    #[test]
    fn escape_closes_without_applying() {
        let ctx = egui::Context::default();
        let mut ui = PasteSpecialUI::new();
        ui.open("hello world".into());
        let action = frame(&ctx, &mut ui, vec![key_event(egui::Key::Escape)]);
        assert!(!ui.visible);
        assert!(matches!(action, PasteSpecialAction::None));
    }

    #[test]
    fn enter_on_a_failing_transform_is_a_no_op() {
        let ctx = egui::Context::default();
        let mut ui = PasteSpecialUI::new();
        // Not valid base64: the decode transform returns Err.
        ui.open("not base64 !!".into());
        ui.search_query = "base64".into();
        ui.update_filtered_transforms();
        let failing = ui
            .filtered_transforms
            .iter()
            .position(|t| transform("not base64 !!", *t).is_err())
            .expect("a base64 transform fails on this input");
        ui.nav.selected = failing;
        let action = frame(&ctx, &mut ui, vec![key_event(egui::Key::Enter)]);
        assert!(ui.visible, "a failed transform keeps the dialog open");
        assert!(matches!(action, PasteSpecialAction::None));
    }

    #[test]
    fn the_footer_has_no_invented_chord() {
        let config = crate::app::overlay::picker::ListConfig {
            id: "t",
            hint: "",
            visible_rows: VISIBLE_ROWS,
            width: 0.0,
            empty_text: "",
            enter_verb: "apply",
            toggle_chord: None,
            alternates: false,
            alternate_labels: None,
            extra_keys: &[],
            multi_select: false,
        };
        assert!(
            crate::app::overlay::picker::footer_text(&config).ends_with("Enter apply · Esc close")
        );
    }
}
