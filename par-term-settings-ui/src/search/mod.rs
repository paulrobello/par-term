//! Settings search: one registry of sections and controls (UX.md SQ1–SQ8).
//!
//! # How the registry is built
//!
//! A section declares its title, id, and (rarely) keywords once, at the
//! `collapsing_section` call that draws it. Controls need no declaration:
//! [`harvest`] renders every tab once, headlessly, with AccessKit on and
//! everything forced open, and reads each control's caption, tooltip, and
//! combo options from the AccessKit tree. `.search_tag(&["yaml_key"])` adds
//! a YAML key or synonym to one control.
//!
//! A tab matches when any of its sections or controls does, so the sidebar,
//! the in-place section filter, and the result list all read one registry.

mod harvest;
#[cfg(test)]
mod harvest_tests;
mod live;
mod matcher;
mod registry;
#[cfg(test)]
mod registry_tests;

pub(crate) use harvest::harvest_tab;
pub use harvest::{SearchTag, tag};
pub(crate) use live::{
    DrawnSection, Jump, LiveView, forces_open, live_view, note_section, set_live_view, start_jump,
};
pub use matcher::{Haystack, Query};
pub use registry::{ControlEntry, Hit, Registry, SectionEntry, tab_name_matches};

/// The registry for a default config, built once per test thread.
#[cfg(test)]
pub(crate) fn test_registry() -> std::sync::Arc<Registry> {
    thread_local! {
        static REGISTRY: std::sync::Arc<Registry> = crate::settings_ui::SettingsUI::new_for_tests(
            par_term_config::Config::default(),
        )
        .search_registry();
    }
    REGISTRY.with(std::sync::Arc::clone)
}

/// Whether `query` has a result on `tab` (tests).
#[cfg(test)]
pub(crate) fn tab_has_result(tab: crate::sidebar::SettingsTab, query: &str) -> bool {
    let registry = test_registry();
    let query = Query::parse(query);
    let hits = registry.search(&query);
    registry.tab_matches(tab, &query, &hits)
}
#[cfg(test)]
mod ui_tests;
