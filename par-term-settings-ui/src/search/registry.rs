//! The section/control registry and the queries over it (UX.md SQ1, SQ2).

use std::collections::HashSet;

use super::matcher::{Haystack, Query};
use crate::sidebar::SettingsTab;

/// One searchable control inside a section.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlEntry {
    /// Visible text: a checkbox or button caption, or a row label.
    pub label: String,
    /// Tooltip text, combo-box options, descriptions, and tagged terms (YAML
    /// keys, synonyms). Numeric values and their units are never included,
    /// so a unit cannot match in place of the label.
    pub extra: Vec<String>,
}

impl ControlEntry {
    fn haystack(&self) -> Haystack {
        Haystack::of(
            std::iter::once(self.label.as_str()).chain(self.extra.iter().map(String::as_str)),
        )
    }
}

/// One collapsible section of one tab, with the controls it rendered.
#[derive(Debug, Clone, Default)]
pub struct SectionEntry {
    /// Tab the section lives on.
    pub tab: SettingsTab,
    /// Index of the tab's sub-page the section is drawn on.
    pub page: usize,
    /// Stable id: the collapse-state key persisted in
    /// `collapsed_settings_sections`.
    pub id: String,
    /// Visible header text.
    pub title: String,
    /// Extra terms the section declares (synonyms found in no label).
    pub keywords: Vec<String>,
    /// Whether the section starts expanded.
    pub default_open: bool,
    /// Index of the enclosing section, for sections nested in another's body.
    pub parent: Option<usize>,
    /// Controls in render order.
    pub controls: Vec<ControlEntry>,
}

impl SectionEntry {
    fn haystack(&self) -> Haystack {
        Haystack::of(
            std::iter::once(self.title.as_str()).chain(self.keywords.iter().map(String::as_str)),
        )
    }
}

/// A search result: a section, or one control inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hit {
    /// Index into [`Registry::sections`].
    pub section: usize,
    /// Index into that section's controls; `None` for a section-level hit.
    pub control: Option<usize>,
}

/// Every searchable section and control of the Settings window.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    /// Sections in tab order, then render order.
    pub sections: Vec<SectionEntry>,
}

impl Registry {
    /// Run `query` over the registry, in tab and render order.
    ///
    /// A section hits when every token is found in its title, keywords, or
    /// tab name, and at least one in its title or keywords. A control hits
    /// when every token is found in its own text, its section's title and
    /// keywords, or its tab name, and at least one in its own text; so
    /// "cursor" lists the Cursor section once instead of every control
    /// inside it, and "badge color" finds the badge's "Text color".
    pub fn search(&self, query: &Query) -> Vec<Hit> {
        if query.is_empty() {
            return Vec::new();
        }
        let mut hits = Vec::new();
        for (si, section) in self.sections.iter().enumerate() {
            let meta = section.haystack();
            let tab = Haystack::new(section.tab.display_name());
            if query.matches_hays(&[&meta, &tab]) && query.any_token_in(&meta) {
                hits.push(Hit {
                    section: si,
                    control: None,
                });
            }
            for (ci, control) in section.controls.iter().enumerate() {
                let own = control.haystack();
                if query.matches_hays(&[&own, &meta, &tab]) && query.any_token_in(&own) {
                    hits.push(Hit {
                        section: si,
                        control: Some(ci),
                    });
                }
            }
        }
        hits
    }

    /// The section and every section enclosing it, innermost first.
    pub fn path(&self, section: usize) -> Vec<usize> {
        let mut path = vec![section];
        let mut at = section;
        while let Some(parent) = self.sections.get(at).and_then(|s| s.parent) {
            path.push(parent);
            at = parent;
        }
        path
    }

    /// Section ids the live view shows for `hits`: each section with a hit
    /// and the sections enclosing it, every section nested inside a
    /// section-level hit, and every section of a tab whose name matches.
    pub fn visible_sections(&self, query: &Query, hits: &[Hit]) -> HashSet<String> {
        let mut visible = HashSet::new();
        for hit in hits {
            for at in self.path(hit.section) {
                visible.insert(self.sections[at].id.clone());
            }
            if hit.control.is_none() {
                for (i, _) in self.sections.iter().enumerate() {
                    if self.path(i).contains(&hit.section) {
                        visible.insert(self.sections[i].id.clone());
                    }
                }
            }
        }
        for section in &self.sections {
            if tab_name_matches(section.tab, query) {
                visible.insert(section.id.clone());
            }
        }
        visible
    }

    /// Every section id in the registry.
    pub fn known_sections(&self) -> HashSet<String> {
        self.sections.iter().map(|s| s.id.clone()).collect()
    }

    /// The control a hit points at, if it is a control hit.
    pub fn control(&self, hit: Hit) -> Option<&ControlEntry> {
        hit.control
            .and_then(|c| self.sections.get(hit.section)?.controls.get(c))
    }

    /// Whether `tab` has any hit, or its own name matches the query.
    pub fn tab_matches(&self, tab: SettingsTab, query: &Query, hits: &[Hit]) -> bool {
        tab_name_matches(tab, query) || hits.iter().any(|h| self.sections[h.section].tab == tab)
    }
}

/// Whether the tab's own name satisfies the query (then the whole tab
/// counts as a match).
pub fn tab_name_matches(tab: SettingsTab, query: &Query) -> bool {
    !query.is_empty() && query.matches(tab.display_name())
}
