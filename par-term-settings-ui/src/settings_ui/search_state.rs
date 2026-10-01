//! Settings search state on [`SettingsUI`] (UX.md SQ1–SQ7).

use std::sync::Arc;

use crate::search::{Hit, Jump, LiveView, Query, Registry};
use crate::sidebar::SettingsTab;

use super::SettingsUI;

/// Search state kept across frames.
#[derive(Default)]
pub struct SearchState {
    /// Built on first use, then kept: labels do not change while Settings is
    /// open, except for the few that echo config values, which are not
    /// matched on.
    registry: Option<Arc<Registry>>,
    /// The query the view below was computed for.
    last_query: String,
    view: Arc<LiveView>,
    hits: Vec<Hit>,
}

impl SettingsUI {
    /// The section/control registry, built on first use by rendering every
    /// tab once on a throwaway copy of this window's state.
    pub fn search_registry(&mut self) -> Arc<Registry> {
        if let Some(registry) = &self.search.registry {
            return Arc::clone(registry);
        }
        let registry = Arc::new(self.build_search_registry());
        self.search.registry = Some(Arc::clone(&registry));
        registry
    }

    fn build_search_registry(&self) -> Registry {
        // The copy has no user lists (prompts) to draw: their rows are the
        // user's data, not settings, and are not search results.
        let mut copy =
            SettingsUI::new_with_assistant_prompts(self.config.clone(), Vec::new(), None);
        copy.available_agent_ids = self.available_agent_ids.clone();
        copy.supported_vsync_modes = self.supported_vsync_modes.clone();
        copy.installation_type = self.installation_type;
        copy.app_version = self.app_version;
        copy.profile_modal_ui
            .load_profiles(self.profile_modal_ui.get_working_profiles().to_vec());
        // Every section draws open, whatever the user collapsed.
        copy.collapsed_sections.clear();
        let mut registry = Registry::default();
        // Every page of every tab: a section on a page that is not selected
        // is still a result (harvest_tab renders each page in turn).
        for tab in SettingsTab::all() {
            let base = registry.sections.len();
            for mut section in crate::search::harvest_tab(&mut copy, *tab) {
                section.parent = section.parent.map(|p| p + base);
                registry.sections.push(section);
            }
        }
        registry
    }

    /// Hits for the current query, in tab and render order.
    pub fn search_hits(&self) -> &[Hit] {
        &self.search.hits
    }

    /// Recompute the view when the query changed and publish it for this
    /// frame's section helpers. Call once per frame before drawing tabs.
    pub(super) fn publish_search_view(&mut self, ctx: &egui::Context) {
        let query_text = self.search_query.trim().to_string();
        let changed = query_text != self.search.last_query;
        if changed {
            self.search.last_query = query_text.clone();
            let view = if query_text.is_empty() {
                self.search.hits.clear();
                LiveView::default()
            } else {
                let registry = self.search_registry();
                let query = Query::parse(&query_text);
                self.search.hits = registry.search(&query);
                LiveView {
                    active: true,
                    query_changed: true,
                    known: registry.known_sections(),
                    visible: registry.visible_sections(&query, &self.search.hits),
                }
            };
            self.search.view = Arc::new(view);
        } else if self.search.view.query_changed {
            let mut view = (*self.search.view).clone();
            view.query_changed = false;
            self.search.view = Arc::new(view);
        }
        crate::search::set_live_view(ctx, Arc::clone(&self.search.view));
    }

    /// Whether `tab` has a result for the query the view was last computed
    /// for (always true with no query).
    pub fn tab_has_results(&mut self, tab: SettingsTab) -> bool {
        let query = Query::parse(&self.search.last_query);
        if query.is_empty() {
            return true;
        }
        let registry = self.search_registry();
        registry.tab_matches(tab, &query, &self.search.hits)
    }

    /// Go to a result: switch tab, expand its sections, scroll to it, and
    /// flash it. The query is kept; the result's sections stay visible.
    pub fn go_to_result(&mut self, ctx: &egui::Context, hit: Hit) {
        let registry = self.search_registry();
        let Some(section) = registry.sections.get(hit.section) else {
            return;
        };
        self.select_page(section.tab, section.page);
        let path = registry
            .path(hit.section)
            .into_iter()
            .map(|i| registry.sections[i].id.clone())
            .collect();
        let label = registry.control(hit).map(|c| c.label.clone());
        crate::search::start_jump(ctx, Jump::new(path, label));
    }

    /// The search field. Cmd+F / Ctrl+F focuses it whenever Settings has
    /// focus (UX.md SQ7), not only when the window opens.
    pub(super) fn show_search_field(&mut self, ui: &mut egui::Ui) {
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
            self.focus_search = true;
        }
        ui.horizontal(|ui| {
            ui.label("Search:");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .id(search_field_id())
                    .hint_text("Setting name, description, or YAML key"),
            );
            if self.focus_search {
                self.focus_search = false;
                response.request_focus();
            }
            if !self.search_query.is_empty()
                && ui
                    .small_button("\u{2715}")
                    .on_hover_text("Clear search")
                    .clicked()
            {
                self.search_query.clear();
            }
        });
    }
}

/// Id of the search field (tests read its focus).
pub(crate) fn search_field_id() -> egui::Id {
    egui::Id::new("settings_search_field")
}
