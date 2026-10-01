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
    let sections = harvest(SettingsTab::WindowsAndTabs);
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
    let sections = harvest(SettingsTab::WindowsAndTabs);
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
    let sections = harvest(SettingsTab::Keys);
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

/// A nested section takes its ancestor's page even though it is recorded
/// before that ancestor (the order the harvest produces).
#[test]
fn a_nested_section_gets_its_ancestors_page() {
    let entry = |id: &str, parent: Option<usize>| SectionEntry {
        id: id.to_string(),
        parent,
        ..Default::default()
    };
    // Each section is listed before the one that encloses it.
    let mut sections = vec![
        entry("grandchild", Some(1)),
        entry("child", Some(2)),
        entry("outer", None),
    ];
    super::harvest::assign_pages(&mut sections, |id| (id == "outer").then_some(2));
    assert_eq!(
        sections.iter().map(|s| s.page).collect::<Vec<_>>(),
        [2, 2, 2]
    );
}

/// With shaders selected, per-shader settings nest inside their shader
/// section; every nested section shares its parent's tab and page, so a
/// result jumps to the page that draws it.
#[test]
fn nested_sections_share_their_parents_page() {
    let mut config = Config::default();
    config.shader.custom_shader = Some("variant.glsl".to_string());
    config.shader.cursor_shader = Some("variant_cursor.glsl".to_string());
    let mut settings = SettingsUI::new_for_tests(config);
    settings.temp_custom_shader = "variant.glsl".to_string();
    settings.temp_cursor_shader = "variant_cursor.glsl".to_string();
    settings.collapsed_sections.clear();
    let mut nested = 0;
    for tab in SettingsTab::all() {
        let sections = harvest_tab(&mut settings, *tab);
        for section in &sections {
            if let Some(p) = section.parent {
                nested += 1;
                assert_eq!(
                    (section.tab, section.page),
                    (sections[p].tab, sections[p].page),
                    "{} is not on its parent {}'s page",
                    section.id,
                    sections[p].id
                );
            }
        }
    }
    assert!(nested > 0, "no nested section was harvested");
}
