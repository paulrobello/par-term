//! Harvest spike: the AccessKit tree yields captions, tooltips, and combo
//! options under the right section.

use par_term_config::Config;

use super::harvest::harvest_tab;
use super::registry::SectionEntry;
use crate::settings_ui::SettingsUI;
use crate::sidebar::SettingsTab;

fn harvest(tab: SettingsTab) -> Vec<SectionEntry> {
    let mut settings = SettingsUI::new_for_tests(Config::default());
    harvest_tab(&mut settings, tab)
}

fn find<'a>(sections: &'a [SectionEntry], title: &str) -> &'a SectionEntry {
    sections
        .iter()
        .find(|s| s.title == title)
        .unwrap_or_else(|| {
            panic!(
                "no section {title:?}; have {:?}",
                sections.iter().map(|s| &s.title).collect::<Vec<_>>()
            )
        })
}

#[test]
fn cursor_blink_lands_in_appearance_cursor() {
    let sections = harvest(SettingsTab::Appearance);
    let cursor = find(&sections, "Cursor");
    assert!(
        cursor.controls.iter().any(|c| c.label == "Cursor blink"),
        "no Cursor blink; have {:#?}",
        cursor.controls
    );
}

#[test]
fn a_tooltip_is_attached_to_its_own_control() {
    let sections = harvest(SettingsTab::Window);
    let display = find(&sections, "Display");
    let allow = display
        .controls
        .iter()
        .find(|c| c.label == "Allow apps to change window title")
        .unwrap_or_else(|| panic!("no control; have {:#?}", display.controls));
    assert!(
        allow
            .extra
            .iter()
            .any(|t| t.contains("OSC escape sequences")),
        "tooltip not attached: {allow:?}"
    );
}

#[test]
fn a_slider_value_and_unit_are_never_indexed() {
    let sections = harvest(SettingsTab::Window);
    let display = find(&sections, "Display");
    for control in &display.controls {
        for text in std::iter::once(&control.label).chain(&control.extra) {
            assert!(
                !text.ends_with(" columns") && !text.ends_with(" px"),
                "a value leaked into the index: {control:?}"
            );
        }
    }
}

#[test]
fn option_key_combo_carries_meta_from_its_popup() {
    let sections = harvest(SettingsTab::Input);
    let keyboard = find(&sections, "Keyboard");
    let meta = keyboard
        .controls
        .iter()
        .any(|c| c.extra.iter().any(|t| t == "Meta") || c.label == "Meta");
    assert!(meta, "no Meta option: {:#?}", keyboard.controls);
}

#[test]
fn every_tab_harvests_in_reasonable_time() {
    let mut settings = SettingsUI::new_for_tests(Config::default());
    let started = std::time::Instant::now();
    let mut counts = Vec::new();
    for tab in SettingsTab::all() {
        let sections = harvest_tab(&mut settings, *tab);
        let controls: usize = sections.iter().map(|s| s.controls.len()).sum();
        counts.push((*tab, sections.len(), controls));
    }
    let elapsed = started.elapsed();
    eprintln!("harvest: {elapsed:?} {counts:?}");
    for (tab, sections, controls) in &counts {
        assert!(*sections > 0 && *controls > 0, "{tab:?} harvested nothing");
    }
}
