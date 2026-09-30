//! Command palette: a summonable, fuzzy-searchable launcher over every
//! dispatchable action.
//!
//! Ranking lives in [`fuzzy`] as pure functions so it can be tested without
//! standing up an egui context or a `WindowState`. The catalog join lives in
//! [`catalog`]. This module owns only the overlay's state machine and its
//! egui presentation, mirroring `crate::search::SearchUI`.

pub(crate) mod catalog;
pub(crate) mod fuzzy;

use catalog::{PaletteEntry, build_catalog, chord_display};
use egui::{Context, Frame, Key, RichText, Window, epaint::Shadow};
use par_term_keybindings::KeybindingRegistry;

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
    /// First drawn row of the `VISIBLE_ROWS` window, in the same index
    /// space as `selected` (B62).
    scroll_offset: usize,
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
            scroll_offset: 0,
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
    ///
    /// Every row's chord hint is then replaced with the LIVE registry chord
    /// (B22): the registry holds user rows plus unclaimed defaults after the
    /// §3.3 merge, so a rebound action advertises its new chord and an action
    /// whose default chord was claimed away (e.g. by `pass_to_terminal`)
    /// advertises nothing instead of a stale default. Runtime rows (plugins,
    /// agents, crashes) have no registry entry and never carry a chord.
    pub(crate) fn open(&mut self, plugin_rows: Vec<PaletteEntry>, registry: &KeybindingRegistry) {
        self.plugin_entries = plugin_rows;
        let mut merged = build_catalog();
        merged.extend(self.plugin_entries.iter().cloned());
        // Priority first, then label: runtime rows carrying a boost (the
        // agent-roster picker's blocked agents) lead the empty-query view,
        // while the all-zero default set keeps today's label order exactly.
        merged.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.label.cmp(&b.label))
        });
        for entry in &mut merged {
            entry.chord = registry
                .chord_for_action(&entry.action_id)
                .map(|combo| chord_display(&combo));
        }
        self.entries = merged;
        self.visible = true;
        self.query.clear();
        self.selected = 0;
        self.scroll_offset = 0;
        self.request_focus = true;
    }

    /// Pre-fill the filter (A18 opens the palette on its Move Tab rows).
    pub(crate) fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Hide the palette.
    pub(crate) fn close(&mut self) {
        self.visible = false;
    }

    /// Flip visibility, resetting state when opening.
    pub(crate) fn toggle(&mut self, plugin_rows: Vec<PaletteEntry>, registry: &KeybindingRegistry) {
        if self.visible {
            self.close();
        } else {
            self.open(plugin_rows, registry);
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

    /// Scroll the `VISIBLE_ROWS` window so `selected` stays drawn (B62).
    ///
    /// Runs every frame after the arrows move `selected`: moving past the
    /// window's last row advances it, above its first row pulls it back, and
    /// a list shorter than the window parks at offset 0.
    fn ensure_selection_visible(&mut self, len: usize) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + VISIBLE_ROWS {
            self.scroll_offset = self.selected + 1 - VISIBLE_ROWS;
        }
        self.scroll_offset = self.scroll_offset.min(len.saturating_sub(VISIBLE_ROWS));
    }

    /// Action id of the top-ranked row for the current query.
    ///
    /// Harness read: `--ui-test` scripts assert the ranking without standing
    /// up an egui context (ranking is pure `fuzzy::rank` over the query).
    pub(crate) fn top_action(&self) -> Option<&str> {
        self.filtered_ids(&self.query).first().copied()
    }

    /// Harness read (`--ui-test`): the selected row's index into the
    /// filtered list.
    pub(crate) fn selected_index(&self) -> usize {
        self.selected
    }

    /// Harness read (`--ui-test`): whether the selected row falls inside the
    /// drawn `VISIBLE_ROWS` window — the B62 invariant.
    pub(crate) fn selected_row_is_visible(&self) -> bool {
        self.selected >= self.scroll_offset && self.selected < self.scroll_offset + VISIBLE_ROWS
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

        // num_presses, not key_pressed: input events can coalesce into one
        // frame (fast typing, key repeat, the ui-test harness pressing
        // faster than the frame cadence), and each press must move the
        // selection — a boolean would swallow every press but the first.
        let downs = ctx.input(|i| i.num_presses(Key::ArrowDown));
        if downs > 0 && !matches.is_empty() {
            self.selected = (self.selected + downs).min(matches.len() - 1);
        }
        let ups = ctx.input(|i| i.num_presses(Key::ArrowUp));
        if ups > 0 {
            self.selected = self.selected.saturating_sub(ups);
        }
        self.ensure_selection_visible(matches.len());
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

                for (row, action_id) in matches
                    .iter()
                    .skip(self.scroll_offset)
                    .take(VISIBLE_ROWS)
                    .enumerate()
                {
                    let entry = self
                        .entries
                        .iter()
                        .find(|e| e.action_id == action_id.as_str())
                        .expect("filtered ids come from self.entries");

                    // Windowed row index plus the offset lands in the same
                    // space as `selected` (B62): comparing the raw windowed
                    // index let the highlight run off the drawn rows.
                    let selected = self.scroll_offset + row == self.selected;
                    ui.horizontal(|ui| {
                        let label = if selected {
                            RichText::new(&entry.label).strong()
                        } else {
                            RichText::new(&entry.label)
                        };
                        if ui.selectable_label(selected, label).clicked() {
                            chosen = Some(entry.action_id.clone());
                        }
                        if let Some(chord) = &entry.chord {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| ui.weak(chord.as_str()),
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
        let registry = KeybindingRegistry::new();
        palette.toggle(Vec::new(), &registry);
        assert!(palette.visible);
        palette.toggle(Vec::new(), &registry);
        assert!(!palette.visible);
    }

    #[test]
    fn opening_resets_query_and_selection() {
        let mut palette = CommandPalette::new();
        let registry = KeybindingRegistry::new();
        palette.open(Vec::new(), &registry);
        palette.query = "stale".to_string();
        palette.selected = 7;
        palette.close();
        palette.open(Vec::new(), &registry);
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
        let registry = KeybindingRegistry::new();
        palette.open(plugin_palette_entries(&[plugin_row]), &registry);
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
    fn open_places_priority_rows_ahead_of_the_label_sort() {
        // The agent-roster picker (A2b task 3) leads its blocked rows with
        // priority 2; everything else is priority 0 and must keep the
        // label-ordered view among themselves.
        let runtime_rows = vec![
            PaletteEntry {
                action_id: "agent-roster-focus:7".to_string(),
                label: "kimi: working".to_string(),
                chord: None,
                priority: 1,
            },
            PaletteEntry {
                action_id: "agent-roster-focus:3".to_string(),
                label: "claude: blocked".to_string(),
                chord: None,
                priority: 2,
            },
        ];
        let mut palette = CommandPalette::new();
        let registry = KeybindingRegistry::new();
        palette.open(runtime_rows, &registry);
        assert_eq!(
            palette.entries.first().map(|e| e.action_id.as_str()),
            Some("agent-roster-focus:3"),
            "the blocked agent row leads the empty-query view"
        );
        let zero_tier: Vec<&str> = palette
            .entries
            .iter()
            .filter(|e| e.priority == 0)
            .map(|e| e.label.as_str())
            .collect();
        let mut sorted = zero_tier.clone();
        sorted.sort_unstable();
        assert_eq!(
            zero_tier, sorted,
            "priority-0 rows stay label-ordered among themselves"
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
        let registry = KeybindingRegistry::new();
        palette.open(snapshot_a, &registry);
        palette.close();
        palette.open(snapshot_b, &registry);
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

    #[test]
    fn arrowing_to_the_20th_match_scrolls_it_into_view() {
        // B62: 12 rows are drawn, so selecting index 19 must advance the
        // window instead of highlighting an invisible row. The loop mirrors
        // show()'s ArrowDown math, which needs a live egui context.
        let mut palette = CommandPalette::new();
        let len = 50;
        for _ in 0..19 {
            palette.selected = (palette.selected + 1).min(len - 1);
        }
        palette.ensure_selection_visible(len);
        assert_eq!(palette.selected, 19);
        assert_eq!(
            palette.scroll_offset, 8,
            "row 19 must be the window's last drawn row"
        );
        assert!(palette.selected_row_is_visible());
    }

    #[test]
    fn arrowing_back_up_pulls_the_window_home() {
        let mut palette = CommandPalette::new();
        palette.selected = 19;
        palette.scroll_offset = 8;
        palette.selected = 0;
        palette.ensure_selection_visible(50);
        assert_eq!(palette.scroll_offset, 0);
        assert!(palette.selected_row_is_visible());
    }

    #[test]
    fn a_query_that_shortens_the_list_parks_the_window_at_zero() {
        // A stale offset from a longer result list must not blank a short one.
        let mut palette = CommandPalette::new();
        palette.selected = 3;
        palette.scroll_offset = 40;
        palette.ensure_selection_visible(5);
        assert_eq!(palette.scroll_offset, 0);
        assert!(palette.selected_row_is_visible());
    }

    #[test]
    fn opening_resets_the_scroll_window() {
        let mut palette = CommandPalette::new();
        let registry = KeybindingRegistry::new();
        palette.open(Vec::new(), &registry);
        palette.scroll_offset = 9;
        palette.close();
        palette.open(Vec::new(), &registry);
        assert_eq!(
            palette.scroll_offset, 0,
            "a reopened palette starts the window at the top"
        );
    }

    #[test]
    fn the_drawn_window_compares_indices_in_one_space() {
        // B62 pin: the draw loop must skip past hidden rows and compare
        // windowed row + offset against the global selection. `.take()`
        // alone compared a windowed row index against the global selection
        // and the highlight vanished below row 12. Assembled needles so the
        // scan cannot match this test's own source.
        let source = include_str!("mod.rs");
        let skip = [".skip(self.", "scroll_offset)"].join("");
        let same_space = ["self.scroll_offset + row == ", "self.selected"].join("");
        assert!(
            source.contains(&skip),
            "the draw loop must window the list with the scroll offset"
        );
        assert!(
            source.contains(&same_space),
            "the selected flag must add the scroll offset to the windowed row"
        );
    }

    #[test]
    fn palette_advertises_the_live_chord_after_a_rebind() {
        // Criterion 5 / B22: the palette's chord hints must come from the
        // live registry, not the static defaults table — rebinding
        // toggle_fullscreen to F9 must advertise F9, never the F11 default.
        let registry = KeybindingRegistry::from_config(&[par_term_config::KeyBinding {
            key: "F9".to_string(),
            action: "toggle_fullscreen".to_string(),
        }]);
        let mut palette = CommandPalette::new();
        palette.open(vec![], &registry);

        let entry = palette
            .entries
            .iter()
            .find(|e| e.action_id == "toggle_fullscreen")
            .expect("toggle_fullscreen is in the catalog");
        assert_eq!(entry.chord.as_deref(), Some("F9"));
    }

    #[test]
    fn palette_hides_a_default_chord_claimed_by_pass_to_terminal() {
        // The live registry is the authority: with F11 claimed for the
        // terminal, toggle_fullscreen has no live chord and must show none —
        // advertising the stale F11 default would send the user to a key the
        // app no longer answers.
        let registry = KeybindingRegistry::from_config(&[par_term_config::KeyBinding {
            key: "F11".to_string(),
            action: par_term_keybindings::PASS_TO_TERMINAL.to_string(),
        }]);
        let mut palette = CommandPalette::new();
        palette.open(vec![], &registry);

        let entry = palette
            .entries
            .iter()
            .find(|e| e.action_id == "toggle_fullscreen")
            .expect("toggle_fullscreen is in the catalog");
        assert_eq!(entry.chord, None);
    }

    #[test]
    fn palette_advertises_defaults_through_a_merged_registry() {
        // A registry built from the real merged defaults (what the app holds
        // after config load) still advertises chords for bound actions, so
        // the empty-config case renders exactly like the old static table.
        let registry = KeybindingRegistry::from_config(&par_term_config::defaults::keybindings());
        let mut palette = CommandPalette::new();
        palette.open(vec![], &registry);

        let entry = palette
            .entries
            .iter()
            .find(|e| e.action_id == "toggle_fullscreen")
            .expect("toggle_fullscreen is in the catalog");
        assert_eq!(entry.chord.as_deref(), Some("F11"));
    }
}
