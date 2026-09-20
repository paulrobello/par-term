//! Command palette: a summonable, fuzzy-searchable launcher over every
//! dispatchable action.
//!
//! Ranking lives in [`fuzzy`] as pure functions so it can be tested without
//! standing up an egui context or a `WindowState`. The catalog join lives in
//! [`catalog`]. This module owns only the overlay's state machine and its
//! egui presentation, mirroring `crate::search::SearchUI`.

pub(crate) mod catalog;
pub(crate) mod fuzzy;

use catalog::{PaletteEntry, build_catalog};
use egui::{Context, Frame, Key, RichText, Window, epaint::Shadow};

/// Rows drawn before the list scrolls.
const VISIBLE_ROWS: usize = 12;

/// Summonable fuzzy launcher over the action catalog.
pub(crate) struct CommandPalette {
    /// Whether the palette is currently on screen.
    pub(crate) visible: bool,
    /// Current filter text.
    query: String,
    /// Index into the *filtered* list, not the catalog.
    selected: usize,
    /// Built once at construction — the action set is static for the process.
    entries: Vec<PaletteEntry>,
    /// Whether the text field should grab focus on the next frame.
    request_focus: bool,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPalette {
    /// Build the palette and its catalog.
    pub(crate) fn new() -> Self {
        Self {
            visible: false,
            query: String::new(),
            selected: 0,
            entries: build_catalog(),
            request_focus: false,
        }
    }

    /// Show the palette, clearing any state from the previous summon.
    pub(crate) fn open(&mut self) {
        self.visible = true;
        self.query.clear();
        self.selected = 0;
        self.request_focus = true;
    }

    /// Hide the palette.
    pub(crate) fn close(&mut self) {
        self.visible = false;
    }

    /// Flip visibility, resetting state when opening.
    pub(crate) fn toggle(&mut self) {
        if self.visible {
            self.close();
        } else {
            self.open();
        }
    }

    /// Action ids matching `query`, best-first.
    fn filtered_ids(&self, query: &str) -> Vec<&str> {
        let pairs: Vec<(&str, &str)> = self
            .entries
            .iter()
            .map(|e| (e.action_id, e.label.as_str()))
            .collect();
        fuzzy::rank(query, &pairs)
            .into_iter()
            .map(|(id, _)| *id)
            .collect()
    }

    /// Keep `selected` inside a filtered list of `len` rows.
    fn clamp_selection(&mut self, len: usize) {
        self.selected = self.selected.min(len.saturating_sub(1));
    }

    /// Action id of the top-ranked row for the current query.
    ///
    /// Harness read: `--ui-test` scripts assert the ranking without standing
    /// up an egui context (ranking is pure `fuzzy::rank` over the query).
    pub(crate) fn top_action(&self) -> Option<&str> {
        self.filtered_ids(&self.query).first().copied()
    }

    /// Draw the palette. Returns the chosen action id when a row is activated.
    pub(crate) fn show(&mut self, ctx: &Context) -> Option<String> {
        if !self.visible {
            return None;
        }

        // Own the ids: a borrowing Vec<&str> from filtered_ids would pin the
        // immutable self-borrow across every mutation below.
        let matches: Vec<String> = self
            .filtered_ids(&self.query)
            .into_iter()
            .map(str::to_string)
            .collect();
        self.clamp_selection(matches.len());

        let mut chosen: Option<String> = None;

        // Escape is claimed by the handle_command_palette_keys key layer, not
        // here — handling it in both places would double-close the palette the
        // frame the layer already consumed the key.
        if ctx.input(|i| i.key_pressed(Key::ArrowDown)) && !matches.is_empty() {
            self.selected = (self.selected + 1).min(matches.len() - 1);
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
            self.selected = self.selected.saturating_sub(1);
        }
        if ctx.input(|i| i.key_pressed(Key::Enter))
            && let Some(id) = matches.get(self.selected)
        {
            chosen = Some(id.clone());
        }

        Window::new("Command Palette")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
            .frame(Frame::popup(&ctx.global_style()).shadow(Shadow::default()))
            .show(ctx, |ui| {
                let field = ui.text_edit_singleline(&mut self.query);
                if self.request_focus {
                    field.request_focus();
                    self.request_focus = false;
                }

                ui.separator();

                for (row, action_id) in matches.iter().take(VISIBLE_ROWS).enumerate() {
                    let entry = self
                        .entries
                        .iter()
                        .find(|e| e.action_id == action_id.as_str())
                        .expect("filtered ids come from self.entries");

                    let selected = row == self.selected;
                    ui.horizontal(|ui| {
                        let label = if selected {
                            RichText::new(&entry.label).strong()
                        } else {
                            RichText::new(&entry.label)
                        };
                        if ui.selectable_label(selected, label).clicked() {
                            chosen = Some(entry.action_id.to_string());
                        }
                        if let Some(chord) = entry.chord {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| ui.weak(chord),
                            );
                        }
                    });
                }

                if matches.is_empty() {
                    ui.weak("No matching actions");
                }
            });

        if chosen.is_some() {
            self.close();
        }
        chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_hidden() {
        assert!(!CommandPalette::new().visible);
    }

    #[test]
    fn toggle_flips_visibility() {
        let mut palette = CommandPalette::new();
        palette.toggle();
        assert!(palette.visible);
        palette.toggle();
        assert!(!palette.visible);
    }

    #[test]
    fn opening_resets_query_and_selection() {
        let mut palette = CommandPalette::new();
        palette.open();
        palette.query = "stale".to_string();
        palette.selected = 7;
        palette.close();
        palette.open();
        assert_eq!(
            palette.query, "",
            "a reopened palette must not show the last query"
        );
        assert_eq!(palette.selected, 0);
    }

    #[test]
    fn filtered_entries_narrow_with_the_query() {
        let palette = CommandPalette::new();
        let all = palette.filtered_ids("");
        let narrowed = palette.filtered_ids("fullscr");
        assert!(!all.is_empty());
        assert!(
            narrowed.len() < all.len(),
            "a specific query must narrow the list"
        );
        assert!(narrowed.contains(&"toggle_fullscreen"));
    }

    #[test]
    fn show_does_not_handle_escape_itself() {
        // Escape is owned by the key layer (handle_command_palette_keys), not
        // by show(). A palette that also closed itself on egui's Escape would
        // double-handle the key the frame the layer already consumed it.
        let source = include_str!("mod.rs");
        // Assembled at runtime: a literal needle would appear in this test's
        // own source and the scan would always find itself.
        let needle = ["Key", "Escape"].join("::");
        assert!(
            !source.contains(&needle),
            "Escape handling belongs in key_handler/command_palette.rs, not in show()"
        );
    }

    #[test]
    fn selection_clamps_to_the_filtered_length() {
        let mut palette = CommandPalette::new();
        palette.selected = 999;
        palette.clamp_selection(3);
        assert_eq!(palette.selected, 2, "selection clamps to last valid index");

        palette.clamp_selection(0);
        assert_eq!(palette.selected, 0, "an empty result list clamps to 0");
    }
}
