//! Terminal search functionality.
//!
//! This module provides search functionality for the terminal scrollback buffer,
//! including an egui-based search bar overlay, search engine with regex support,
//! and match highlighting.

mod engine;
pub mod types;

pub use engine::SearchEngine;
pub use types::{SearchAction, SearchConfig, SearchMatch};

use egui::{Color32, Context, Frame, Key, RichText, Window, epaint::Shadow};
use std::time::Instant;

/// Search debounce delay in milliseconds.
const SEARCH_DEBOUNCE_MS: u64 = 150;

/// Search UI overlay for terminal.
pub struct SearchUI {
    /// Whether the search UI is currently visible.
    pub visible: bool,
    /// Current search query.
    query: String,
    /// Whether search is case-sensitive.
    case_sensitive: bool,
    /// Whether to use regex matching.
    use_regex: bool,
    /// Whether to match whole words only.
    whole_word: bool,
    /// All matches found.
    matches: Vec<SearchMatch>,
    /// Index of the currently highlighted match.
    current_match_index: usize,
    /// Search engine instance.
    engine: SearchEngine,
    /// Last time the query changed (for debouncing).
    last_query_change: Option<Instant>,
    /// Whether search needs to be re-run.
    needs_search: bool,
    /// Last query that was actually searched.
    last_searched_query: String,
    /// Last case sensitivity setting that was searched.
    last_searched_case_sensitive: bool,
    /// Last regex setting that was searched.
    last_searched_use_regex: bool,
    /// Last whole word setting that was searched.
    last_searched_whole_word: bool,
    /// Whether the text input should request focus.
    request_focus: bool,
    /// Regex error message (if any).
    regex_error: Option<String>,
}

impl Default for SearchUI {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchUI {
    /// Create a new search UI.
    pub fn new() -> Self {
        Self {
            visible: false,
            query: String::new(),
            case_sensitive: false,
            use_regex: false,
            whole_word: false,
            matches: Vec::new(),
            current_match_index: 0,
            engine: SearchEngine::new(),
            last_query_change: None,
            needs_search: false,
            last_searched_query: String::new(),
            last_searched_case_sensitive: false,
            last_searched_use_regex: false,
            last_searched_whole_word: false,
            request_focus: false,
            regex_error: None,
        }
    }

    /// Toggle search UI visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            self.request_focus = true;
        }
    }

    /// Open the search UI (ensuring it's visible).
    pub fn open(&mut self) {
        self.visible = true;
        self.request_focus = true;
    }

    /// Close the search UI.
    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Get the current search query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Get all current matches.
    pub fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }

    /// Get the current match index.
    pub fn current_match_index(&self) -> usize {
        self.current_match_index
    }

    /// Get the current match (if any).
    pub fn current_match(&self) -> Option<&SearchMatch> {
        self.matches.get(self.current_match_index)
    }

    /// Move to the next match.
    ///
    /// Returns the new current match if navigation succeeded.
    pub fn next_match(&mut self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }

        self.current_match_index = (self.current_match_index + 1) % self.matches.len();
        self.matches.get(self.current_match_index)
    }

    /// Move to the previous match.
    ///
    /// Returns the new current match if navigation succeeded.
    pub fn prev_match(&mut self) -> Option<&SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }

        if self.current_match_index == 0 {
            self.current_match_index = self.matches.len() - 1;
        } else {
            self.current_match_index -= 1;
        }

        self.matches.get(self.current_match_index)
    }

    /// Update search results with new terminal content.
    ///
    /// # Arguments
    /// * `lines` - Iterator of (line_index, line_text) pairs from scrollback
    pub fn update_search<I>(&mut self, lines: I)
    where
        I: Iterator<Item = (usize, String)>,
    {
        // Check if we need to search based on debounce timing
        if let Some(last_change) = self.last_query_change
            && last_change.elapsed().as_millis() < SEARCH_DEBOUNCE_MS as u128
        {
            return;
        }

        // Check if settings changed
        let settings_changed = self.case_sensitive != self.last_searched_case_sensitive
            || self.use_regex != self.last_searched_use_regex
            || self.whole_word != self.last_searched_whole_word;

        // Only re-search if query or settings changed
        if !self.needs_search && self.query == self.last_searched_query && !settings_changed {
            return;
        }

        self.needs_search = false;
        self.last_searched_query = self.query.clone();
        self.last_searched_case_sensitive = self.case_sensitive;
        self.last_searched_use_regex = self.use_regex;
        self.last_searched_whole_word = self.whole_word;
        self.regex_error = None;

        let config = SearchConfig {
            case_sensitive: self.case_sensitive,
            use_regex: self.use_regex,
            whole_word: self.whole_word,
            wrap_around: true,
        };

        // Validate regex before searching
        if self.use_regex
            && !self.query.is_empty()
            && let Err(e) = regex::Regex::new(&self.query)
        {
            self.regex_error = Some(e.to_string());
            self.matches.clear();
            self.current_match_index = 0;
            return;
        }

        self.matches = self.engine.search(lines, &self.query, &config);

        // Reset current match index if it's out of bounds
        if self.current_match_index >= self.matches.len() {
            self.current_match_index = 0;
        }
    }

    /// Clear search results.
    pub fn clear(&mut self) {
        self.query.clear();
        self.matches.clear();
        self.current_match_index = 0;
        self.needs_search = false;
        self.last_searched_query.clear();
        self.regex_error = None;
    }

    /// Show the search UI and return any action to take.
    ///
    /// # Arguments
    /// * `ctx` - egui Context
    /// * `terminal_rows` - Number of visible terminal rows (for scroll calculation)
    /// * `scrollback_len` - Total scrollback length
    ///
    /// # Returns
    /// A SearchAction indicating what the caller should do.
    pub fn show(
        &mut self,
        ctx: &Context,
        terminal_rows: usize,
        scrollback_len: usize,
    ) -> SearchAction {
        if !self.visible {
            return SearchAction::None;
        }

        let mut action = SearchAction::None;
        let mut close_requested = false;

        // Ensure search bar is fully opaque regardless of terminal opacity
        let mut style = (*ctx.style()).clone();
        let solid_bg = Color32::from_rgba_unmultiplied(30, 30, 30, 255);
        style.visuals.window_fill = solid_bg;
        style.visuals.panel_fill = solid_bg;
        style.visuals.widgets.noninteractive.bg_fill = solid_bg;
        ctx.set_style(style);

        let viewport = ctx.input(|i| i.viewport_rect());

        // Position at top of window
        let window_width = 500.0_f32.min(viewport.width() - 20.0);

        Window::new("Search")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .fixed_size([window_width, 0.0])
            .fixed_pos([viewport.center().x - window_width / 2.0, 10.0])
            .frame(
                Frame::window(&ctx.style())
                    .fill(solid_bg)
                    .stroke(egui::Stroke::new(1.0, Color32::from_gray(60)))
                    .shadow(Shadow {
                        offset: [0, 2],
                        blur: 8,
                        spread: 0,
                        color: Color32::from_black_alpha(100),
                    })
                    .inner_margin(8.0),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Search icon/label
                    ui.label(RichText::new("Search:").strong());

                    // Search text input
                    let response = ui.add_sized(
                        [ui.available_width() - 180.0, 20.0],
                        egui::TextEdit::singleline(&mut self.query)
                            .hint_text("Enter search term...")
                            .desired_width(f32::INFINITY),
                    );

                    // Request focus when first opened
                    if self.request_focus {
                        response.request_focus();
                        self.request_focus = false;
                    }

                    // Track query changes for debouncing
                    if response.changed() {
                        self.last_query_change = Some(Instant::now());
                        self.needs_search = true;
                    }

                    // Handle Enter key for next match
                    if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        let shift = ui.input(|i| i.modifiers.shift);
                        if shift {
                            let match_line = self.prev_match().map(|m| m.line);
                            if let Some(line) = match_line {
                                action = self.calculate_scroll_action(
                                    line,
                                    terminal_rows,
                                    scrollback_len,
                                );
                            }
                        } else {
                            let match_line = self.next_match().map(|m| m.line);
                            if let Some(line) = match_line {
                                action = self.calculate_scroll_action(
                                    line,
                                    terminal_rows,
                                    scrollback_len,
                                );
                            }
                        }
                        response.request_focus();
                    }

                    // Handle Escape key
                    if ui.input(|i| i.key_pressed(Key::Escape)) {
                        close_requested = true;
                    }

                    // Match count display
                    let match_text = if self.matches.is_empty() {
                        if self.query.is_empty() {
                            String::new()
                        } else if self.regex_error.is_some() {
                            "Invalid".to_string()
                        } else {
                            "No matches".to_string()
                        }
                    } else {
                        format!("{} of {}", self.current_match_index + 1, self.matches.len())
                    };
                    ui.label(match_text);

                    // Navigation buttons
                    ui.add_enabled_ui(!self.matches.is_empty(), |ui| {
                        if ui
                            .button("\u{25B2}")
                            .on_hover_text("Previous (Shift+Enter)")
                            .clicked()
                        {
                            let match_line = self.prev_match().map(|m| m.line);
                            if let Some(line) = match_line {
                                action = self.calculate_scroll_action(
                                    line,
                                    terminal_rows,
                                    scrollback_len,
                                );
                            }
                        }
                        if ui
                            .button("\u{25BC}")
                            .on_hover_text("Next (Enter)")
                            .clicked()
                        {
                            let match_line = self.next_match().map(|m| m.line);
                            if let Some(line) = match_line {
                                action = self.calculate_scroll_action(
                                    line,
                                    terminal_rows,
                                    scrollback_len,
                                );
                            }
                        }
                    });

                    // Close button
                    if ui
                        .button("\u{2715}")
                        .on_hover_text("Close (Escape)")
                        .clicked()
                    {
                        close_requested = true;
                    }
                });

                // Second row: options
                ui.horizontal(|ui| {
                    // Case sensitivity toggle
                    let case_btn = ui.selectable_label(self.case_sensitive, "Aa");
                    if case_btn.on_hover_text("Case sensitive").clicked() {
                        self.case_sensitive = !self.case_sensitive;
                        self.needs_search = true;
                    }

                    // Regex toggle
                    let regex_btn = ui.selectable_label(self.use_regex, ".*");
                    if regex_btn.on_hover_text("Regular expression").clicked() {
                        self.use_regex = !self.use_regex;
                        self.needs_search = true;
                    }

                    // Whole word toggle
                    let word_btn = ui.selectable_label(self.whole_word, "\\b");
                    if word_btn.on_hover_text("Whole word").clicked() {
                        self.whole_word = !self.whole_word;
                        self.needs_search = true;
                    }

                    // Show regex error if present
                    if let Some(ref error) = self.regex_error {
                        ui.colored_label(
                            Color32::from_rgb(255, 100, 100),
                            format!("Regex error: {}", truncate_error(error, 40)),
                        );
                    }
                });

                // Keyboard hints
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Enter: Next | Shift+Enter: Prev | Escape: Close")
                            .weak()
                            .small(),
                    );
                });
            });

        // Handle keyboard shortcuts outside the UI
        let cmd_g_shift =
            ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(Key::G));
        let cmd_g =
            ctx.input(|i| i.modifiers.command && !i.modifiers.shift && i.key_pressed(Key::G));

        if cmd_g_shift {
            let match_line = self.prev_match().map(|m| m.line);
            if let Some(line) = match_line {
                action = self.calculate_scroll_action(line, terminal_rows, scrollback_len);
            }
        } else if cmd_g {
            let match_line = self.next_match().map(|m| m.line);
            if let Some(line) = match_line {
                action = self.calculate_scroll_action(line, terminal_rows, scrollback_len);
            }
        }

        if close_requested {
            self.visible = false;
            return SearchAction::Close;
        }

        action
    }

    /// Calculate the scroll offset needed to show a match at the given line.
    fn calculate_scroll_action(
        &self,
        match_line: usize,
        terminal_rows: usize,
        scrollback_len: usize,
    ) -> SearchAction {
        // Total lines = scrollback + visible screen
        let total_lines = scrollback_len + terminal_rows;

        // Calculate scroll offset to center the match on screen
        // scroll_offset = 0 means we're at the bottom (showing most recent content)
        // scroll_offset = scrollback_len means we're at the top

        // The match line is in terms of absolute line index (0 = oldest scrollback)
        // We need to convert this to a scroll_offset

        // If match is in the visible area at the bottom (most recent), scroll_offset = 0
        // If match is at the very top of scrollback, scroll_offset = scrollback_len

        // Calculate how far from the bottom the match line is
        let lines_from_bottom = total_lines.saturating_sub(match_line + 1);

        // We want to show the match near the center of the viewport
        let center_offset = terminal_rows / 2;

        // Scroll offset to put the match at the center
        let target_offset = lines_from_bottom.saturating_sub(center_offset);

        // Clamp to valid range
        let clamped_offset = target_offset.min(scrollback_len);

        SearchAction::ScrollToMatch(clamped_offset)
    }

    /// Initialize search settings from config.
    pub fn init_from_config(&mut self, case_sensitive: bool, use_regex: bool) {
        self.case_sensitive = case_sensitive;
        self.use_regex = use_regex;
    }
}

/// Truncate error message for display.
fn truncate_error(error: &str, max_len: usize) -> &str {
    if error.len() <= max_len {
        error
    } else {
        &error[..max_len]
    }
}
