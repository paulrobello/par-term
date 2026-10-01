//! Command palette: a summonable, fuzzy-searchable launcher over every
//! dispatchable action.
//!
//! Ranking lives in [`fuzzy`] as pure functions so it can be tested without
//! standing up an egui context or a `WindowState`. The catalog join lives in
//! [`catalog`], and each row's category and description line in [`meta`].
//! This module owns only the overlay's state machine and its egui
//! presentation, mirroring `crate::search::SearchUI`.
//!
//! OV6 ordering on an empty query: rows carrying a priority boost (blocked
//! agents, crash offers) lead, then the actions this window last ran from
//! the palette (most recent first), then everything else grouped by
//! category in [`meta::CATEGORIES`] order and label-sorted within a group.
//! A typed query ranks by fuzzy label match; ties keep that order. Recents
//! are kept per window in memory — they are a convenience for the next
//! summon, not state worth persisting.

pub(crate) mod catalog;
pub(crate) mod fuzzy;
pub(crate) mod meta;

use crate::app::overlay::picker::{self, ListConfig, ListNav, ListOutcome};
use catalog::{PaletteEntry, build_catalog, chord_display};
use egui::{Context, RichText};
use par_term_keybindings::KeybindingRegistry;

/// Rows drawn before the list scrolls.
const VISIBLE_ROWS: usize = 12;
/// How many recently run actions lead the empty-query view.
const MAX_RECENTS: usize = 5;

/// Summonable fuzzy launcher over the action catalog, drawn by the shared
/// list/picker component (UX.md OV5).
pub(crate) struct CommandPalette {
    /// Whether the palette is currently on screen.
    pub(crate) visible: bool,
    /// Current filter text.
    query: String,
    /// Selection (an index into the *filtered* list) and the drawn window,
    /// in one index space (B62).
    nav: ListNav,
    /// Latest plugin snapshot, replaced by every `open()`.
    plugin_entries: Vec<PaletteEntry>,
    /// Built-ins plus the current plugin snapshot, ordered priority first,
    /// then category, then label. Rebuilt on every `open()` so the merged
    /// view tracks the live plugin set; the built-in action set itself is
    /// static for the process.
    entries: Vec<PaletteEntry>,
    /// Action ids this window last ran from the palette, most recent first.
    recent: Vec<String>,
    /// Whether the text field should grab focus on the next frame.
    request_focus: bool,
    /// The live `toggle_command_palette` chord, read at open for the
    /// footer (UX.md OV5); `None` when unbound.
    toggle_chord: Option<String>,
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
            nav: ListNav::default(),
            plugin_entries: Vec::new(),
            entries: browse_sorted(build_catalog()),
            recent: Vec::new(),
            request_focus: false,
            toggle_chord: None,
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
        let mut merged = browse_sorted(merged);
        for entry in &mut merged {
            entry.chord = registry
                .chord_for_action(&entry.action_id)
                .map(|combo| chord_display(&combo));
        }
        self.entries = merged;
        self.toggle_chord = registry
            .chord_for_action("toggle_command_palette")
            .map(|combo| chord_display(&combo));
        self.visible = true;
        self.query.clear();
        self.nav.reset();
        self.request_focus = true;
    }

    /// Pre-fill the filter (A18 opens the palette on its Move Tab rows).
    pub(crate) fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.nav.reset();
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

    /// Record a palette run: the action moves to the front of the recents.
    fn record_use(&mut self, action_id: &str) {
        self.recent.retain(|id| id != action_id);
        self.recent.insert(0, action_id.to_string());
        self.recent.truncate(MAX_RECENTS);
    }

    /// Action ids matching `query`, best-first. An empty query is the
    /// browse view: boosted rows, then recents, then the category groups.
    fn filtered_ids(&self, query: &str) -> Vec<&str> {
        if query.is_empty() {
            return self.browse_order();
        }
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

    /// The empty-query order (see the module docs). Recents that are no
    /// longer rows (a consumed crash offer, a closed session) are skipped,
    /// and a recent that is also boosted keeps its boosted place.
    fn browse_order(&self) -> Vec<&str> {
        let boosted = self.entries.iter().filter(|e| e.priority > 0);
        let recents = self.recent.iter().filter_map(|id| {
            self.entries
                .iter()
                .find(|e| e.priority == 0 && e.action_id == *id)
        });
        let rest = self
            .entries
            .iter()
            .filter(|e| e.priority == 0 && !self.recent.contains(&e.action_id));
        boosted
            .chain(recents)
            .chain(rest)
            .map(|e| e.action_id.as_str())
            .collect()
    }

    /// Action id of the top-ranked row for the current query.
    ///
    /// Harness read: `--ui-test` scripts assert the ranking without standing
    /// up an egui context (ranking is pure `fuzzy::rank` over the query).
    pub(crate) fn top_action(&self) -> Option<&str> {
        self.filtered_ids(&self.query).first().copied()
    }

    /// Test read: the open palette's row for `action_id` (label, live
    /// chord).
    #[cfg(test)]
    pub(crate) fn row(&self, action_id: &str) -> Option<&PaletteEntry> {
        self.entries.iter().find(|e| e.action_id == action_id)
    }

    /// Harness read (`--ui-test`): the selected row's index into the
    /// filtered list.
    pub(crate) fn selected_index(&self) -> usize {
        self.nav.selected
    }

    /// Harness read (`--ui-test`): whether the selected row falls inside the
    /// drawn `VISIBLE_ROWS` window — the B62 invariant.
    pub(crate) fn selected_row_is_visible(&self) -> bool {
        self.nav.selected_is_visible(VISIBLE_ROWS)
    }

    /// Draw the palette. Returns the chosen action id when a row is activated.
    ///
    /// Keys, scrolling, and the footer come from the shared picker (UX.md
    /// OV5): arrows, PageUp/PageDown, Home/End, Enter runs, Escape closes —
    /// on the egui side, where the overlay stack feeds every key it does
    /// not close on.
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
        let entries = &self.entries;
        let config = ListConfig {
            id: "Command Palette",
            hint: "Type a command",
            visible_rows: VISIBLE_ROWS,
            width: crate::app::overlay::theme::WIDTH_LARGE,
            empty_text: "No matching actions",
            enter_verb: "run",
            toggle_chord: self.toggle_chord.as_deref(),
            alternates: false,
            alternate_labels: None,
            extra_keys: &[],
            multi_select: false,
        };
        let (outcome, query_changed) = picker::show_list(
            ctx,
            &config,
            &mut self.query,
            &mut self.request_focus,
            &mut self.nav,
            matches.len(),
            |ui, index, selected| {
                entries
                    .iter()
                    .find(|e| e.action_id == matches[index])
                    .is_some_and(|entry| draw_row(ui, entry, selected))
            },
        );
        if query_changed {
            self.nav.reset();
        }
        match outcome {
            ListOutcome::Chosen { index, .. } => {
                self.close();
                let chosen = matches.get(index).cloned();
                if let Some(id) = &chosen {
                    self.record_use(id);
                }
                chosen
            }
            ListOutcome::Closed => {
                self.close();
                None
            }
            ListOutcome::Open | ListOutcome::ToggleMark(_) => None,
        }
    }
}

/// Order rows for browsing: priority first — runtime rows carrying a boost
/// (the agent-roster picker's blocked agents) lead the empty-query view —
/// then by category (OV6), label-sorted within a group.
fn browse_sorted(mut entries: Vec<PaletteEntry>) -> Vec<PaletteEntry> {
    let category_rank = |entry: &PaletteEntry| {
        let category = meta::category(&entry.action_id);
        meta::CATEGORIES
            .iter()
            .position(|c| *c == category)
            .unwrap_or(meta::CATEGORIES.len())
    };
    entries.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| category_rank(a).cmp(&category_rank(b)))
            .then_with(|| a.label.cmp(&b.label))
    });
    entries
}

/// One palette row (OV6): the label with the live chord right-aligned,
/// and beneath it a dimmed line naming the category and what the action
/// does.
fn draw_row(ui: &mut egui::Ui, entry: &PaletteEntry, selected: bool) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        let label = if selected {
            RichText::new(&entry.label).strong()
        } else {
            RichText::new(&entry.label)
        };
        clicked = ui.selectable_label(selected, label).clicked();
        if let Some(chord) = &entry.chord {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(chord.as_str())
            });
        }
    });
    let category = meta::category(&entry.action_id);
    let line = match meta::description(&entry.action_id) {
        Some(description) => format!("{category} · {description}"),
        None => category.to_string(),
    };
    ui.label(RichText::new(line).weak().small());
    clicked
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
        palette.nav.selected = 7;
        palette.close();
        palette.open(Vec::new(), &registry);
        assert_eq!(
            palette.query, "",
            "a reopened palette must not show the last query"
        );
        assert_eq!(palette.nav.selected, 0);
    }

    /// The OV6 browse order among unboosted rows: categories in
    /// `meta::CATEGORIES` order, label-sorted within each category.
    fn assert_grouped_by_category(entries: &[&PaletteEntry]) {
        let keys: Vec<(usize, &str)> = entries
            .iter()
            .map(|e| {
                let c = meta::category(&e.action_id);
                (
                    meta::CATEGORIES.iter().position(|x| *x == c).unwrap(),
                    e.label.as_str(),
                )
            })
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(
            keys, sorted,
            "unboosted rows group by category, label-sorted within a group, \
             so the empty-query view stays stable"
        );
    }

    #[test]
    fn open_merges_plugin_rows_into_the_category_grouped_view() {
        let plugin_row = PluginActionRow {
            wire_id: "plugin-action:com.example.demo:aaaa".to_string(),
            label: "Aaaa First Plugin Action · Demo".to_string(),
        };
        let mut palette = CommandPalette::new();
        let registry = KeybindingRegistry::new();
        palette.open(plugin_palette_entries(&[plugin_row]), &registry);
        assert_grouped_by_category(&palette.entries.iter().collect::<Vec<_>>());
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
        // priority 2; everything else is priority 0 and keeps the
        // category-grouped view among themselves.
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
        let zero_tier: Vec<&PaletteEntry> =
            palette.entries.iter().filter(|e| e.priority == 0).collect();
        assert_grouped_by_category(&zero_tier);
    }

    #[test]
    fn recent_actions_lead_an_empty_query_below_boosted_rows() {
        // OV6: the last palette runs come first on an empty query, most
        // recent first — but a blocked agent still leads, because the
        // palette exists to answer "who is waiting".
        let blocked = PaletteEntry {
            action_id: "agent-roster-focus:3".to_string(),
            label: "claude: blocked".to_string(),
            chord: None,
            priority: 2,
        };
        let mut palette = CommandPalette::new();
        let registry = KeybindingRegistry::new();
        palette.record_use("toggle_fullscreen");
        palette.record_use("split_right");
        palette.record_use("toggle_fullscreen");
        palette.open(vec![blocked], &registry);
        let ids = palette.filtered_ids("");
        assert_eq!(
            &ids[..3],
            ["agent-roster-focus:3", "toggle_fullscreen", "split_right"],
            "boosted, then recents most-recent-first (deduplicated)"
        );
        assert_eq!(
            ids.iter().filter(|id| **id == "split_right").count(),
            1,
            "a recent is not listed twice"
        );
        // A typed query ranks by match, not recency.
        assert_eq!(
            palette.filtered_ids("fullscr").first(),
            Some(&"toggle_fullscreen")
        );
    }

    #[test]
    fn a_recent_whose_row_is_gone_is_skipped() {
        let mut palette = CommandPalette::new();
        palette.record_use("triage-crash:99");
        palette.open(vec![], &KeybindingRegistry::new());
        assert!(!palette.filtered_ids("").contains(&"triage-crash:99"));
    }

    #[test]
    fn running_a_row_records_it_as_recent() {
        let ctx = Context::default();
        let mut palette = CommandPalette::new();
        palette.open(Vec::new(), &KeybindingRegistry::new());
        frame(&ctx, &mut palette, &[]);
        frame(&ctx, &mut palette, &[Key::ArrowDown; 5]);
        let chosen = frame(&ctx, &mut palette, &[Key::Enter]).expect("Enter runs a row");
        palette.open(Vec::new(), &KeybindingRegistry::new());
        assert_eq!(palette.filtered_ids("").first(), Some(&chosen.as_str()));
        assert_eq!(palette.recent.len(), 1);
        for _ in 0..(MAX_RECENTS + 3) {
            palette.record_use(&format!("x{}", palette.recent.len()));
        }
        assert_eq!(palette.recent.len(), MAX_RECENTS, "recents are capped");
    }

    #[test]
    fn every_row_draws_a_description_line() {
        // OV6: the description line renders under each label. 12 two-line
        // rows must still fit the palette at the default 800-px height.
        let ctx = Context::default();
        let mut palette = CommandPalette::new();
        palette.open(Vec::new(), &KeybindingRegistry::new());
        frame(&ctx, &mut palette, &[]);
        // egui keys a Window's area by `Id::new(title.text())`, an
        // Option<Cow<str>>.
        let id = egui::Id::new(Some(std::borrow::Cow::Borrowed("Command Palette")));
        let rect = ctx.memory(|m| m.area_rect(id)).expect("palette drew");
        assert!(
            rect.max.y <= 800.0,
            "12 rows with description lines fit on screen: {rect:?}"
        );
        // Measured 2026-09-30: 472 px tall with description lines; one-line
        // rows would draw 12 x ~20 px plus the filter and footer.
        assert!(
            rect.height() > 12.0 * 30.0,
            "each row is two lines tall (label + description): {rect:?}"
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

    /// Run one headless egui frame of `show()` with `keys` pressed.
    fn frame(ctx: &Context, palette: &mut CommandPalette, keys: &[Key]) -> Option<String> {
        let events = keys
            .iter()
            .map(|&key| egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            })
            .collect();
        let mut chosen = None;
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| chosen = palette.show(ui.ctx()),
        );
        out.textures_delta.clear();
        chosen
    }

    use egui::Key;

    #[test]
    fn show_holds_the_focused_escape_close() {
        // With the palette's text field focused, egui holds the keyboard
        // and show() must close on Escape itself. Measured
        // pre-fix (2026-09-20): Escape with the field focused left the
        // palette open forever.
        let ctx = Context::default();
        let mut palette = CommandPalette::new();
        palette.open(Vec::new(), &KeybindingRegistry::new());
        frame(&ctx, &mut palette, &[]);
        assert_eq!(frame(&ctx, &mut palette, &[Key::Escape]), None);
        assert!(!palette.visible, "Escape must close the focused palette");
    }

    #[test]
    fn arrowing_to_the_20th_match_keeps_it_drawn_through_show() {
        // B62 through the real draw path: 19 ArrowDowns select row 19 and
        // scroll it into the 12-row window; Enter runs that row, not an
        // invisible one.
        let ctx = Context::default();
        let mut palette = CommandPalette::new();
        palette.open(Vec::new(), &KeybindingRegistry::new());
        frame(&ctx, &mut palette, &[]);
        frame(&ctx, &mut palette, &[Key::ArrowDown; 19]);
        assert_eq!(palette.selected_index(), 19);
        assert!(palette.selected_row_is_visible());
        let expected = palette.filtered_ids("")[19].to_string();
        assert_eq!(frame(&ctx, &mut palette, &[Key::Enter]), Some(expected));
    }

    #[test]
    fn page_and_home_end_keys_move_by_the_window() {
        let ctx = Context::default();
        let mut palette = CommandPalette::new();
        palette.open(Vec::new(), &KeybindingRegistry::new());
        let len = palette.filtered_ids("").len();
        frame(&ctx, &mut palette, &[]);
        frame(&ctx, &mut palette, &[Key::PageDown]);
        assert_eq!(palette.selected_index(), VISIBLE_ROWS);
        frame(&ctx, &mut palette, &[Key::End]);
        assert_eq!(palette.selected_index(), len - 1);
        assert!(palette.selected_row_is_visible());
        frame(&ctx, &mut palette, &[Key::Home]);
        assert_eq!(palette.selected_index(), 0);
        frame(&ctx, &mut palette, &[Key::PageUp]);
        assert_eq!(palette.selected_index(), 0, "PageUp at the top stays put");
    }

    #[test]
    fn palette_is_registered_as_modal() {
        // The palette must guard the terminal: that drives the modal guard
        // keeping keystrokes off the PTY while an overlay is open.
        // Unregistered, typed characters filtered the palette AND leaked to
        // the shell (measured 2026-09-20, --ui-test pre-fix run). The live
        // half — the palette opened on a real WindowState raises the guard —
        // is `overlay::stack_tests::b61_dialogs_block_the_terminal`.
        assert!(crate::app::overlay::OverlayId::CommandPalette.guards_terminal());
    }

    #[test]
    fn opening_resets_the_scroll_window() {
        let mut palette = CommandPalette::new();
        let registry = KeybindingRegistry::new();
        palette.open(Vec::new(), &registry);
        palette.nav.selected = 30;
        palette.nav.scroll_offset = 19;
        palette.close();
        palette.open(Vec::new(), &registry);
        assert_eq!(
            palette.nav,
            ListNav::default(),
            "a reopened palette starts the window at the top"
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
    fn the_footer_chord_is_read_from_the_live_registry_at_open() {
        // OV5: the footer names the palette's own toggle chord (it closes
        // the palette), live — a rebind shows the new chord, and an
        // unbound palette shows only Escape.
        let registry = KeybindingRegistry::from_config(&[par_term_config::KeyBinding {
            key: "F8".to_string(),
            action: "toggle_command_palette".to_string(),
        }]);
        let mut palette = CommandPalette::new();
        palette.open(vec![], &registry);
        assert_eq!(palette.toggle_chord.as_deref(), Some("F8"));
        palette.close();
        palette.open(vec![], &KeybindingRegistry::new());
        assert_eq!(palette.toggle_chord, None);
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
