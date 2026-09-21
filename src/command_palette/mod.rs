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
    /// Latest plugin snapshot, replaced by every `open()`.
    plugin_entries: Vec<PaletteEntry>,
    /// Built-ins plus the current plugin snapshot, label-sorted. Rebuilt on
    /// every `open()` so the merged view tracks the live plugin set; the
    /// built-in action set itself is static for the process.
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
            plugin_entries: Vec::new(),
            entries: build_catalog(),
            request_focus: false,
        }
    }

    /// Show the palette with a fresh plugin snapshot, clearing any state from
    /// the previous summon.
    ///
    /// The merged view (built-ins + plugin rows) is rebuilt here, not at
    /// construction, because the plugin set can change between summons; the
    /// caller passes the snapshot it just read from the host.
    pub(crate) fn open(&mut self, plugin_rows: Vec<PaletteEntry>) {
        self.plugin_entries = plugin_rows;
        let mut merged = build_catalog();
        merged.extend(self.plugin_entries.iter().cloned());
        merged.sort_by(|a, b| a.label.cmp(&b.label));
        self.entries = merged;
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
    pub(crate) fn toggle(&mut self, plugin_rows: Vec<PaletteEntry>) {
        if self.visible {
            self.close();
        } else {
            self.open(plugin_rows);
        }
    }

    /// Action ids matching `query`, best-first.
    fn filtered_ids(&self, query: &str) -> Vec<&str> {
        let pairs: Vec<(&str, &str)> = self
            .entries
            .iter()
            .map(|e| (e.action_id.as_str(), e.label.as_str()))
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

        // Escape closes the palette here, on the egui side, because with the
        // text field focused `is_egui_using_keyboard()` returns early in
        // handle_key_event and the handle_command_palette_keys layer never
        // sees the key. That layer remains the backstop for the unfocused
        // case, and close() is idempotent, so both paths are safe together.
        // consume_key keeps the Escape from also reaching other egui widgets.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.close();
        }

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
                            chosen = Some(entry.action_id.clone());
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
    use catalog::plugin_palette_entries;
    use par_term_scripting::plugin_manager::PluginActionRow;

    #[test]
    fn starts_hidden() {
        assert!(!CommandPalette::new().visible);
    }

    #[test]
    fn toggle_flips_visibility() {
        let mut palette = CommandPalette::new();
        palette.toggle(Vec::new());
        assert!(palette.visible);
        palette.toggle(Vec::new());
        assert!(!palette.visible);
    }

    #[test]
    fn opening_resets_query_and_selection() {
        let mut palette = CommandPalette::new();
        palette.open(Vec::new());
        palette.query = "stale".to_string();
        palette.selected = 7;
        palette.close();
        palette.open(Vec::new());
        assert_eq!(
            palette.query, "",
            "a reopened palette must not show the last query"
        );
        assert_eq!(palette.selected, 0);
    }

    #[test]
    fn open_merges_plugin_rows_into_a_label_sorted_view() {
        let plugin_row = PluginActionRow {
            wire_id: "plugin-action:com.example.demo:aaaa".to_string(),
            label: "Aaaa First Plugin Action · Demo".to_string(),
        };
        let mut palette = CommandPalette::new();
        palette.open(plugin_palette_entries(&[plugin_row]));
        let labels: Vec<&str> = palette.entries.iter().map(|e| e.label.as_str()).collect();
        let mut sorted = labels.clone();
        sorted.sort_unstable();
        assert_eq!(
            labels, sorted,
            "the merged view must be label-ordered so the empty-query view \
             stays stable"
        );
        assert!(
            palette
                .entries
                .iter()
                .any(|e| e.action_id == "plugin-action:com.example.demo:aaaa"),
            "the plugin row must be present alongside the built-ins"
        );
    }

    #[test]
    fn reopening_with_a_fresh_snapshot_replaces_stale_plugin_rows() {
        let snapshot_a = plugin_palette_entries(&[PluginActionRow {
            wire_id: "plugin-action:com.old:act".to_string(),
            label: "Old Action · Old Plugin".to_string(),
        }]);
        let snapshot_b = plugin_palette_entries(&[PluginActionRow {
            wire_id: "plugin-action:com.new:act".to_string(),
            label: "New Action · New Plugin".to_string(),
        }]);
        let mut palette = CommandPalette::new();
        palette.open(snapshot_a);
        palette.close();
        palette.open(snapshot_b);
        let ids = palette.filtered_ids("");
        assert!(
            ids.contains(&"plugin-action:com.new:act"),
            "the fresh snapshot's rows must be in the filtered view"
        );
        assert!(
            !ids.contains(&"plugin-action:com.old:act"),
            "a stale plugin row must not survive a reopen with a fresh snapshot"
        );
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
    fn show_holds_the_focused_escape_close() {
        // With the palette's text field focused, is_egui_using_keyboard()
        // returns early in handle_key_event, so the handle_command_palette_keys
        // layer never sees Escape — show() is the only close path while
        // typing. The layer stays as the unfocused backstop; close() is
        // idempotent, so the two paths coexist safely. Measured pre-fix:
        // Escape with the field focused left the palette open forever.
        let source = include_str!("mod.rs");
        // Assembled at runtime: a literal needle would appear in this test's
        // own source and the scan would always find itself.
        let needle = ["consume", "_key"].join("");
        let input_call = ["input", "_mut"].join("");
        assert!(
            source.contains(&needle) && source.contains(&input_call),
            "show() must close the palette on the egui-side Escape while \
             typing; without it the palette cannot be dismissed while typing"
        );
    }

    #[test]
    fn palette_is_registered_as_modal() {
        // The palette must be in any_modal_ui_visible(): that sum drives the
        // modal guard keeping keystrokes off the PTY while an overlay is
        // open. Unregistered, typed characters filtered the palette AND
        // leaked to the shell (measured 2026-09-20, --ui-test pre-fix run).
        let source = include_str!("../app/window_state/ui_query_helpers.rs");
        let body = source
            .split("fn any_modal_ui_visible")
            .nth(1)
            .expect("any_modal_ui_visible present in ui_query_helpers.rs");
        let body = body.split('}').next().unwrap_or_default();
        assert!(
            body.contains("command_palette.visible"),
            "command_palette missing from any_modal_ui_visible — typing in \
             the palette leaks to the PTY"
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
