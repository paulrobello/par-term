use crate::terminal::{ClipboardEntry, ClipboardSlot};
use crate::ui_constants::{
    CLIPBOARD_WINDOW_DEFAULT_HEIGHT, CLIPBOARD_WINDOW_DEFAULT_WIDTH, CLIPBOARD_WINDOW_MAX_HEIGHT,
};
use egui::{Context, Window};

/// Clipboard history UI manager using egui
pub struct ClipboardHistoryUI {
    /// Whether the clipboard history window is currently visible
    pub visible: bool,

    /// Current search query
    search_query: String,

    /// Index of currently selected entry (for keyboard navigation)
    selected_index: Option<usize>,

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
            selected_index: None,
            cached_entries: Vec::new(),
        }
    }

    /// Toggle clipboard history window visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            // Reset selection when opening
            self.selected_index = if self.cached_entries.is_empty() {
                None
            } else {
                Some(0)
            };
        }
    }

    /// Update cached entries from terminal
    pub fn update_entries(&mut self, entries: Vec<ClipboardEntry>) {
        self.cached_entries = entries;
        // Reset selection if out of bounds
        if let Some(idx) = self.selected_index
            && idx >= self.cached_entries.len()
        {
            self.selected_index = if self.cached_entries.is_empty() {
                None
            } else {
                Some(self.cached_entries.len() - 1)
            };
        }
    }

    /// Navigate selection up
    pub fn select_previous(&mut self) {
        if let Some(idx) = self.selected_index {
            if idx > 0 {
                self.selected_index = Some(idx - 1);
            }
        } else if !self.cached_entries.is_empty() {
            self.selected_index = Some(self.cached_entries.len() - 1);
        }
    }

    /// Navigate selection down
    pub fn select_next(&mut self) {
        if let Some(idx) = self.selected_index {
            if idx < self.cached_entries.len().saturating_sub(1) {
                self.selected_index = Some(idx + 1);
            }
        } else if !self.cached_entries.is_empty() {
            self.selected_index = Some(0);
        }
    }

    /// Get the currently selected entry
    pub fn selected_entry(&self) -> Option<&ClipboardEntry> {
        self.selected_index
            .and_then(|idx| self.cached_entries.get(idx))
    }

    /// Show the clipboard history window and return any action to take
    pub fn show(&mut self, ctx: &Context) -> ClipboardHistoryAction {
        if !self.visible {
            return ClipboardHistoryAction::None;
        }

        let mut action = ClipboardHistoryAction::None;
        let mut open = true;

        // Calculate center position for initial placement
        let screen_rect = ctx.content_rect();
        let default_pos = egui::pos2(
            (screen_rect.width() - CLIPBOARD_WINDOW_DEFAULT_WIDTH) / 2.0,
            (screen_rect.height() - CLIPBOARD_WINDOW_DEFAULT_HEIGHT) / 2.0,
        );

        Window::new("Clipboard History")
            .resizable(true)
            .collapsible(false)
            .default_width(CLIPBOARD_WINDOW_DEFAULT_WIDTH)
            .default_height(CLIPBOARD_WINDOW_DEFAULT_HEIGHT)
            .max_height(CLIPBOARD_WINDOW_MAX_HEIGHT)
            .default_pos(default_pos)
            .open(&mut open)
            .show(ctx, |ui| {
                // Search bar
                ui.horizontal(|ui| {
                    ui.label("Search:");
                    ui.text_edit_singleline(&mut self.search_query);
                });

                ui.separator();

                // Entry list
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let filtered_entries: Vec<(usize, &ClipboardEntry)> = self
                            .cached_entries
                            .iter()
                            .enumerate()
                            .filter(|(_, entry)| {
                                if self.search_query.is_empty() {
                                    true
                                } else {
                                    entry
                                        .content
                                        .to_lowercase()
                                        .contains(&self.search_query.to_lowercase())
                                }
                            })
                            .collect();

                        if filtered_entries.is_empty() {
                            ui.label("No clipboard history entries");
                        } else {
                            for (original_idx, entry) in filtered_entries {
                                let is_selected = self.selected_index == Some(original_idx);
                                let preview = truncate_preview(&entry.content, 80);
                                let timestamp = format_timestamp(entry.timestamp);

                                let response = ui.selectable_label(
                                    is_selected,
                                    format!("[{}] {}", timestamp, preview),
                                );

                                if response.clicked() {
                                    self.selected_index = Some(original_idx);
                                }

                                if response.double_clicked() {
                                    action = ClipboardHistoryAction::Paste(entry.content.clone());
                                    self.visible = false;
                                }

                                // Show tooltip with full content on hover
                                response.on_hover_text(&entry.content);
                            }
                        }
                    });

                ui.separator();

                // Action buttons
                ui.horizontal(|ui| {
                    if ui.button("Paste Selected").clicked()
                        && let Some(entry) = self.selected_entry()
                    {
                        action = ClipboardHistoryAction::Paste(entry.content.clone());
                        self.visible = false;
                    }

                    if ui.button("Clear History").clicked() {
                        action = ClipboardHistoryAction::ClearAll;
                    }

                    if ui.button("Close").clicked() {
                        self.visible = false;
                    }
                });

                // Keyboard hints
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Hints:");
                    ui.label("↑↓ Navigate");
                    ui.label("Enter Paste");
                    ui.label("Shift+Enter Transform");
                    ui.label("Esc Close");
                });
            });

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
