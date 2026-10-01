//! Registry gates (UX.md SQ8, SP1 acceptance criteria 1 and 2).
//!
//! These search the registry harvested from the real tabs, so a renamed
//! label, a deleted control, or a section that stops drawing fails here.

use super::registry::{Hit, Registry};
use super::{Query, test_registry};
use crate::sidebar::SettingsTab;

/// Where an audit query must land: a tab, a section title, and (for control
/// targets) the control's visible caption.
struct Expect {
    query: &'static str,
    tab: SettingsTab,
    section: &'static str,
    control: Option<&'static str>,
}

const fn to(
    query: &'static str,
    tab: SettingsTab,
    section: &'static str,
    control: &'static str,
) -> Expect {
    Expect {
        query,
        tab,
        section,
        control: Some(control),
    }
}

const fn to_section(query: &'static str, tab: SettingsTab, section: &'static str) -> Expect {
    Expect {
        query,
        tab,
        section,
        control: None,
    }
}

/// The 15 audit queries (UX.md 14.1). The audit's list was not written
/// down; this is reconstructed from the queries 14.1 names: eight that
/// failed ("cursor blink" … "par-mux") and seven that hit a dimmed,
/// unclickable tab ("notify" … "prompt library").
///
/// "leader" lands on Keys › Leader Key (K7), "par-mux" on Sessions ›
/// par-mux (SX1), and quit/close confirmation on General › Closing &
/// Quitting (SX2), their SP3 homes.
fn audit_queries() -> Vec<Expect> {
    use SettingsTab::*;
    vec![
        to("cursor blink", Appearance, "Cursor", "Cursor blink"),
        to("option as meta", Keys, "Keyboard", OPTION_SENDS),
        to(
            "copy on select",
            General,
            "Selection & Clipboard",
            "Auto-copy selection",
        ),
        to(
            "close confirmation",
            General,
            "Closing & Quitting",
            "Close Confirmation",
        ),
        to(
            "prompt on quit",
            General,
            "Closing & Quitting",
            "Confirm before quitting with open tabs",
        ),
        to_section("leader", Keys, "Leader Key"),
        to("detach", Keys, "Keybindings", "Move Tab to New Window"),
        to_section("par-mux", Sessions, "par-mux"),
        to(
            "notify",
            Advanced,
            "Activity",
            "Notify on activity after inactivity",
        ),
        to(
            "command complete",
            Advanced,
            "Alert Sounds",
            "Command Complete",
        ),
        to("mouse", Advanced, "Auto-Hide", "Hide on mouse inactivity"),
        to_section("general", Appearance, "General"),
        to_section("custom actions", Automation, "Custom Actions"),
        to_section("observer scripts", Automation, "Observer Scripts"),
        to_section("prompt library", Assistant, "Prompt Library"),
    ]
}

#[cfg(target_os = "macos")]
const OPTION_SENDS: &str = "Left Option sends:";
#[cfg(not(target_os = "macos"))]
const OPTION_SENDS: &str = "Left Alt sends:";

fn describe(registry: &Registry, hits: &[Hit]) -> Vec<String> {
    hits.iter()
        .take(12)
        .map(|h| {
            let s = &registry.sections[h.section];
            match registry.control(*h) {
                Some(c) => format!("{:?} › {} › {}", s.tab, s.title, c.label),
                None => format!("{:?} › {}", s.tab, s.title),
            }
        })
        .collect()
}

fn lands(registry: &Registry, hits: &[Hit], expect: &Expect) -> bool {
    hits.iter().any(|h| {
        let s = &registry.sections[h.section];
        s.tab == expect.tab
            && s.title == expect.section
            && match expect.control {
                None => h.control.is_none(),
                Some(label) => registry.control(*h).is_some_and(|c| c.label == label),
            }
    })
}

#[test]
fn the_audit_queries_resolve_to_their_controls() {
    let registry = test_registry();
    let mut failures = Vec::new();
    for expect in audit_queries() {
        let hits = registry.search(&Query::parse(expect.query));
        if !lands(&registry, &hits, &expect) {
            failures.push(format!(
                "{:?} -> expected {:?} › {} › {:?}; got {:?}",
                expect.query,
                expect.tab,
                expect.section,
                expect.control,
                describe(&registry, &hits)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "audit queries that miss:\n{}",
        failures.join("\n")
    );
}

#[test]
fn the_empty_page_queries_now_show_something() {
    // UX.md 14.1: these lit a tab and showed no section.
    let registry = test_registry();
    for query in ["tmux", "gateway", "export", "run command"] {
        let hits = registry.search(&Query::parse(query));
        assert!(!hits.is_empty(), "{query:?} has no result");
    }
}

/// SP1 criterion 1 / SQ8: every section title and keyword, and every
/// control label, is a query that returns a result on its own tab, and that
/// result's section is shown by the live view (never hidden by the search
/// that found it).
#[test]
fn every_section_keyword_and_control_label_resolves_on_its_own_tab() {
    let registry = test_registry();
    let mut failures = Vec::new();
    let mut checked = 0;
    for (si, section) in registry.sections.iter().enumerate() {
        let terms = std::iter::once((section.title.as_str(), None))
            .chain(section.keywords.iter().map(|k| (k.as_str(), None)))
            .chain(
                section
                    .controls
                    .iter()
                    .enumerate()
                    .map(|(ci, c)| (c.label.as_str(), Some(ci))),
            );
        for (term, control) in terms {
            let query = Query::parse(term);
            if query.is_empty() {
                continue;
            }
            checked += 1;
            let hits = registry.search(&query);
            let visible = registry.visible_sections(&query, &hits);
            let own = hits.iter().any(|h| {
                registry.sections[h.section].tab == section.tab
                    && (control.is_none() || (h.section == si && h.control == control))
            });
            if !own || !visible.contains(&section.id) {
                failures.push(format!(
                    "{:?} › {} › {term:?} (own hit: {own}, section shown: {})",
                    section.tab,
                    section.title,
                    visible.contains(&section.id)
                ));
            }
        }
    }
    assert!(checked > 500, "only {checked} terms checked");
    assert!(
        failures.is_empty(),
        "{} of {checked} terms do not resolve on their own tab:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Search terms the user docs promise (docs/features/AUTOMATION.md,
/// FILE_TRANSFERS.md) find their tab.
#[test]
fn documented_search_terms_find_their_tab() {
    use SettingsTab::*;
    let cases: &[(SettingsTab, &[&str])] = &[
        (
            Automation,
            &[
                "trigger",
                "regex",
                "pattern",
                "match",
                "action",
                "highlight",
                "notify",
                "coprocess",
                "pipe",
                "subprocess",
                "auto start",
                "restart",
                "script",
                "observer",
                "event",
                "panel",
                "subscriptions",
            ],
        ),
        (
            General,
            &["download", "upload", "transfer", "save location"],
        ),
    ];
    let mut missing = Vec::new();
    for (tab, terms) in cases {
        for term in *terms {
            if !super::tab_has_result(*tab, term) {
                missing.push(format!("{tab:?}: {term:?}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "documented terms not found: {missing:?}"
    );
}

/// SQ2: YAML keys find their control, including numeric controls whose
/// only other text is a unit suffix.
#[test]
fn yaml_keys_find_their_controls() {
    use SettingsTab::*;
    let registry = test_registry();
    for (key, tab, label) in [
        ("window_padding", WindowsAndTabs, "Padding:"),
        ("cursor_blink", Appearance, "Cursor blink"),
        ("scrollback_lines", Advanced, "Scrollback lines:"),
        (
            "prompt_on_quit",
            General,
            "Confirm before quitting with open tabs",
        ),
        (
            "status_bar_auto_hide_mouse_inactive",
            Advanced,
            "Hide on mouse inactivity",
        ),
        ("mux_auto_attach", Sessions, "Attach on launch:"),
        ("leader_timeout_ms", Keys, "Timeout:"),
    ] {
        let hits = registry.search(&Query::parse(key));
        assert!(
            hits.iter().any(|h| registry.sections[h.section].tab == tab
                && registry.control(*h).is_some_and(|c| c.label == label)),
            "{key:?} -> expected {tab:?} › {label:?}; got {:?}",
            describe(&registry, &hits)
        );
    }
}

/// SQ6: macOS-only controls register only on macOS, so their terms do not
/// light the Window tab where the controls are compiled out.
#[test]
fn platform_gated_controls_register_only_on_their_platform() {
    for query in ["mission control", "window blur", "target space"] {
        assert_eq!(
            super::tab_has_result(SettingsTab::WindowsAndTabs, query),
            cfg!(target_os = "macos"),
            "{query:?} on the Windows & Tabs tab"
        );
    }
}

/// No section is registered twice under one id: the id is the jump target
/// and the persisted collapse key.
#[test]
fn section_ids_are_unique() {
    let registry = test_registry();
    let mut seen = std::collections::HashSet::new();
    for section in &registry.sections {
        assert!(
            seen.insert(&section.id),
            "section id {:?} registered twice",
            section.id
        );
    }
}
