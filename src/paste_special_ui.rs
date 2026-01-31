//! Paste Special UI - Command palette for text transformations.
//!
//! Provides a fuzzy-searchable command palette for applying text transformations
//! to clipboard content before pasting.

use crate::paste_transform::{transform, PasteTransform};
use egui::{Color32, Context, RichText, Window};

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

    /// Index of currently selected transformation (for keyboard navigation)
    selected_index: usize,

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
            selected_index: 0,
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
        self.selected_index = 0;
        self.update_filtered_transforms();
        self.update_preview();
    }

    /// Close the paste special UI
    pub fn close(&mut self) {
        self.visible = false;
        self.content.clear();
        self.search_query.clear();
    }

    /// Navigate selection up
    pub fn select_previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.update_preview();
        }
    }

    /// Navigate selection down
    pub fn select_next(&mut self) {
        if self.selected_index < self.filtered_transforms.len().saturating_sub(1) {
            self.selected_index += 1;
            self.update_preview();
        }
    }

    /// Get the currently selected transformation
    pub fn selected_transform(&self) -> Option<PasteTransform> {
        self.filtered_transforms.get(self.selected_index).copied()
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

        // Reset selection if out of bounds
        if self.selected_index >= self.filtered_transforms.len() {
            self.selected_index = 0;
        }
    }

    /// Update the preview result for the current selection
    fn update_preview(&mut self) {
        if let Some(t) = self.selected_transform() {
            self.preview_result = transform(&self.content, t);
        } else {
            self.preview_result = Ok(self.content.clone());
        }
    }

    /// Show the paste special window and return any action to take
    pub fn show(&mut self, ctx: &Context) -> PasteSpecialAction {
        if !self.visible {
            return PasteSpecialAction::None;
        }

        let mut action = PasteSpecialAction::None;
        let mut open = true;
        let mut search_changed = false;

        // Calculate center position
        let screen_rect = ctx.content_rect();
        let default_pos = egui::pos2(
            (screen_rect.width() - 500.0) / 2.0,
            (screen_rect.height() - 400.0) / 2.0,
        );

        Window::new("Paste Special")
            .resizable(true)
            .collapsible(false)
            .default_width(500.0)
            .default_height(400.0)
            .default_pos(default_pos)
            .open(&mut open)
            .show(ctx, |ui| {
                // Search bar with focus
                ui.horizontal(|ui| {
                    ui.label("Search:");
                    let response = ui.text_edit_singleline(&mut self.search_query);
                    if response.changed() {
                        search_changed = true;
                    }
                    // Request focus on the search box
                    response.request_focus();
                });

                ui.separator();

                // Track if we need to update preview after the UI loop
                let mut clicked_index: Option<usize> = None;
                let mut double_clicked = false;

                // Two-column layout: transformations list and preview
                ui.columns(2, |columns| {
                    // Left column: transformation list
                    columns[0].heading("Transformations");
                    egui::ScrollArea::vertical()
                        .id_salt("transforms_scroll")
                        .auto_shrink([false, false])
                        .max_height(250.0)
                        .show(&mut columns[0], |ui| {
                            if self.filtered_transforms.is_empty() {
                                ui.label("No matching transformations");
                            } else {
                                for (idx, transform_type) in
                                    self.filtered_transforms.iter().enumerate()
                                {
                                    let is_selected = idx == self.selected_index;
                                    let text = if is_selected {
                                        RichText::new(transform_type.display_name())
                                            .strong()
                                            .color(Color32::WHITE)
                                    } else {
                                        RichText::new(transform_type.display_name())
                                    };

                                    let response = ui.selectable_label(is_selected, text);

                                    if response.clicked() {
                                        clicked_index = Some(idx);
                                    }

                                    if response.double_clicked() {
                                        clicked_index = Some(idx);
                                        double_clicked = true;
                                    }

                                    // Show description on hover
                                    response.on_hover_text(transform_type.description());
                                }
                            }
                        });

                    // Right column: preview
                    columns[1].heading("Preview");
                    columns[1].separator();

                    // Show original content (truncated)
                    columns[1].label(
                        RichText::new("Original:")
                            .small()
                            .color(Color32::GRAY),
                    );
                    let original_preview = truncate_preview(&self.content, 100);
                    columns[1].label(
                        RichText::new(&original_preview)
                            .monospace()
                            .color(Color32::LIGHT_GRAY),
                    );

                    columns[1].add_space(8.0);

                    // Show transformed content (or error)
                    columns[1].label(
                        RichText::new("Result:")
                            .small()
                            .color(Color32::GRAY),
                    );
                    match &self.preview_result {
                        Ok(result) => {
                            let result_preview = truncate_preview(result, 100);
                            columns[1].label(
                                RichText::new(&result_preview)
                                    .monospace()
                                    .color(Color32::LIGHT_GREEN),
                            );
                        }
                        Err(error) => {
                            columns[1].label(
                                RichText::new(error)
                                    .monospace()
                                    .color(Color32::RED),
                            );
                        }
                    }
                });

                // Handle click events after the borrow ends
                if let Some(idx) = clicked_index {
                    self.selected_index = idx;
                    self.update_preview();
                    if double_clicked {
                        if let Some(result) = self.apply_selected() {
                            action = PasteSpecialAction::Paste(result);
                            self.visible = false;
                        }
                    }
                }

                ui.separator();

                // Action buttons
                ui.horizontal(|ui| {
                    let can_apply = self.preview_result.is_ok() && !self.filtered_transforms.is_empty();

                    if ui
                        .add_enabled(can_apply, egui::Button::new("Apply & Paste"))
                        .clicked()
                    {
                        if let Some(result) = self.apply_selected() {
                            action = PasteSpecialAction::Paste(result);
                            self.visible = false;
                        }
                    }

                    if ui.button("Cancel").clicked() {
                        self.visible = false;
                    }

                    // Show content length info
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{} chars", self.content.len()))
                                .small()
                                .color(Color32::GRAY),
                        );
                    });
                });

                // Keyboard hints
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("↑↓ Navigate  Enter Apply  Esc Cancel")
                            .small()
                            .color(Color32::GRAY),
                    );
                });
            });

        // Handle search changes
        if search_changed {
            self.update_filtered_transforms();
            self.update_preview();
        }

        // Handle window close
        if !open {
            self.visible = false;
        }

        action
    }
}

/// Truncate content for preview display
fn truncate_preview(content: &str, max_len: usize) -> String {
    // Replace newlines with visible markers
    let single_line = content.replace('\n', "↵").replace('\r', "").replace('\t', "→");

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

        assert_eq!(ui.selected_index, 0);

        ui.select_next();
        assert_eq!(ui.selected_index, 1);

        ui.select_previous();
        assert_eq!(ui.selected_index, 0);

        // Can't go below 0
        ui.select_previous();
        assert_eq!(ui.selected_index, 0);
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
