//! Fuzzy command history search overlay UI.
//!
//! Provides a searchable popup for browsing and selecting from command history,
//! with fuzzy matching and ranked results with match highlighting.

use crate::command_history::CommandHistoryEntry;
use egui::{Context, Window};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use std::collections::VecDeque;

/// Command history UI manager using egui
pub struct CommandHistoryUI {
    /// Whether the command history window is currently visible
    pub visible: bool,

    /// Current search query
    search_query: String,

    /// Index of currently selected entry in filtered results
    selected_index: Option<usize>,

    /// Cached command history entries (refreshed when shown)
    cached_entries: Vec<CommandHistoryEntry>,

    /// Fuzzy matcher instance
    matcher: SkimMatcherV2,

    /// Whether the search input should request focus
    request_focus: bool,
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
            selected_index: None,
            cached_entries: Vec::new(),
            matcher: SkimMatcherV2::default(),
            request_focus: false,
        }
    }

    /// Open the command history UI
    pub fn open(&mut self) {
        self.visible = true;
        self.search_query.clear();
        self.request_focus = true;
        self.selected_index = if self.cached_entries.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Close the command history UI
    pub fn close(&mut self) {
        self.visible = false;
        self.search_query.clear();
        self.selected_index = None;
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
        // Reset selection if out of bounds
        if let Some(idx) = self.selected_index
            && idx >= self.cached_entries.len()
        {
            self.selected_index = if self.cached_entries.is_empty() {
                None
            } else {
                Some(0)
            };
        }
    }

    /// Navigate selection up
    pub fn select_previous(&mut self) {
        if let Some(idx) = self.selected_index
            && idx > 0
        {
            self.selected_index = Some(idx - 1);
        }
    }

    /// Get the command text of the currently selected entry (if any).
    /// Re-runs fuzzy matching to resolve the filtered index.
    pub fn selected_command(&self) -> Option<String> {
        let idx = self.selected_index?;
        let matches = self.get_matched_entries();
        matches
            .get(idx)
            .map(|m| self.cached_entries[m.index].command.clone())
    }

    /// Navigate selection down
    pub fn select_next(&mut self, filtered_count: usize) {
        if let Some(idx) = self.selected_index {
            if idx < filtered_count.saturating_sub(1) {
                self.selected_index = Some(idx + 1);
            }
        } else if filtered_count > 0 {
            self.selected_index = Some(0);
        }
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
        matches.sort_by(|a, b| b.score.cmp(&a.score));
        matches
    }

    /// Show the command history window and return any action to take
    pub fn show(&mut self, ctx: &Context) -> CommandHistoryAction {
        if !self.visible {
            return CommandHistoryAction::None;
        }

        let mut action = CommandHistoryAction::None;
        let mut open = true;

        // Calculate center position for initial placement
        let screen_rect = ctx.content_rect();
        let default_pos = egui::pos2(
            (screen_rect.width() - 500.0) / 2.0,
            (screen_rect.height() - 350.0) / 2.0,
        );

        let matched_entries = self.get_matched_entries();

        Window::new("Command History Search")
            .resizable(true)
            .collapsible(false)
            .default_width(500.0)
            .default_height(350.0)
            .max_height(450.0)
            .default_pos(default_pos)
            .open(&mut open)
            .show(ctx, |ui| {
                // Search bar
                ui.horizontal(|ui| {
                    ui.label("Search:");
                    let response = ui.text_edit_singleline(&mut self.search_query);
                    if self.request_focus {
                        response.request_focus();
                        self.request_focus = false;
                    }
                });

                ui.separator();

                // Results count
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{} / {} commands",
                        matched_entries.len(),
                        self.cached_entries.len()
                    ));
                });

                ui.separator();

                // Entry list
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if matched_entries.is_empty() {
                            ui.label("No matching commands");
                        } else {
                            for (filtered_idx, matched) in matched_entries.iter().enumerate() {
                                let entry = &self.cached_entries[matched.index];
                                let is_selected = self.selected_index == Some(filtered_idx);

                                // Build highlighted text
                                let layout_job = build_highlighted_label(
                                    &entry.command,
                                    &matched.indices,
                                    is_selected,
                                    entry.exit_code,
                                    entry.timestamp_ms,
                                );

                                let response = ui.selectable_label(is_selected, layout_job);

                                if response.clicked() {
                                    self.selected_index = Some(filtered_idx);
                                }

                                if response.double_clicked() {
                                    action = CommandHistoryAction::Insert(entry.command.clone());
                                    self.visible = false;
                                }

                                // Show tooltip with full command and metadata on hover
                                // Auto-scroll to selected item
                                let response = response.on_hover_text(format_tooltip(entry));
                                if is_selected {
                                    response.scroll_to_me(Some(egui::Align::Center));
                                }
                            }
                        }
                    });

                ui.separator();

                // Action buttons
                ui.horizontal(|ui| {
                    if ui.button("Insert Selected").clicked()
                        && let Some(idx) = self.selected_index
                        && let Some(matched) = matched_entries.get(idx)
                    {
                        let entry = &self.cached_entries[matched.index];
                        action = CommandHistoryAction::Insert(entry.command.clone());
                        self.visible = false;
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
                    ui.label("Enter Insert");
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
    let status_char = match exit_code {
        Some(0) => "● ",
        Some(_) => "✗ ",
        None => "○ ",
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
