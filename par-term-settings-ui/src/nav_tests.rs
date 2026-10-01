//! SP3 acceptance criterion 4: deep links open the right page and section
//! (UX.md B51), and the last tab, page, and search are remembered (B54).

use par_term_config::Config;

use crate::layout::deep_link;
use crate::settings_ui::SettingsUI;
use crate::sidebar::SettingsTab;

fn frame(ctx: &egui::Context, settings: &mut SettingsUI, time: f64) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1000.0, 700.0),
        )),
        time: Some(time),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        settings.show_as_panel(ui);
    });
    output.textures_delta.clear();
}

/// Every deep link the host uses, with where it must land.
const DEEP_LINKS: &[(&str, SettingsTab, &str)] = &[
    (deep_link::PROFILES, SettingsTab::Profiles, "Profiles"),
    (
        deep_link::SAVE_ARRANGEMENT,
        SettingsTab::Sessions,
        "Arrangements",
    ),
];

#[test]
fn every_deep_link_selects_its_tab_and_page() {
    for (id, tab, page) in DEEP_LINKS {
        let mut settings = SettingsUI::new_for_tests(Config::default());
        settings.search_query = "something unrelated".to_string();
        settings.open_section(id);
        assert_eq!(settings.selected_tab, *tab, "{id}");
        assert_eq!(
            crate::layout::page_at(*tab, settings.selected_page()).title,
            *page,
            "{id}"
        );
        assert!(
            settings.search_query.is_empty(),
            "{id}: a search would hide the linked section"
        );
    }
}

/// B51: `save_arrangement` lands on Save Current Layout and flashes it,
/// from any starting tab.
#[test]
fn save_arrangement_link_expands_and_flashes_the_section() {
    let ctx = egui::Context::default();
    let mut settings = SettingsUI::new_for_tests(Config::default());
    settings.select_page(SettingsTab::Appearance, 2);
    frame(&ctx, &mut settings, 0.0);

    settings.open_section(deep_link::SAVE_ARRANGEMENT);
    let mut flashed = false;
    for i in 1..40 {
        frame(&ctx, &mut settings, f64::from(i) * 0.1);
        flashed |= crate::search::flashing(&ctx).as_deref() == Some("arrangements_save");
    }
    assert_eq!(settings.selected_tab, SettingsTab::Sessions);
    assert!(flashed, "the linked section never flashed");
}

#[test]
fn an_unknown_deep_link_changes_nothing() {
    let mut settings = SettingsUI::new_for_tests(Config::default());
    settings.select_page(SettingsTab::Keys, 1);
    settings.open_section("no_such_section");
    assert_eq!(settings.selected_tab, SettingsTab::Keys);
    assert_eq!(settings.selected_page(), 1);
}

/// B54: a new Settings window restored from the last one's state opens on
/// the same tab, the same page of every tab, and the same search.
#[test]
fn tab_page_and_search_survive_a_reopen() {
    let mut first = SettingsUI::new_for_tests(Config::default());
    first.select_page(SettingsTab::Appearance, 3);
    first.select_page(SettingsTab::Advanced, 2);
    first.search_query = "cursor".to_string();
    let nav = first.nav_state();

    let mut second = SettingsUI::new_for_tests(Config::default());
    assert_eq!(second.selected_tab, SettingsTab::General);
    second.restore_nav(nav);
    assert_eq!(second.selected_tab, SettingsTab::Advanced);
    assert_eq!(second.selected_page(), 2);
    second.set_selected_tab(SettingsTab::Appearance);
    assert_eq!(second.selected_page(), 3, "each tab keeps its own page");
    assert_eq!(second.search_query, "cursor");
}

/// B54 with the search live: the restored query shows its results on the
/// first frame, and remembering it is not an unsaved change.
#[test]
fn a_restored_search_is_live_and_not_an_edit() {
    let mut first = SettingsUI::new_for_tests(Config::default());
    first.search_query = "cursor blink".to_string();
    let nav = first.nav_state();

    let ctx = egui::Context::default();
    let mut second = SettingsUI::new_for_tests(Config::default());
    second.restore_nav(nav);
    frame(&ctx, &mut second, 0.0);
    assert!(
        !second.search_hits().is_empty(),
        "restored query has no results"
    );
    assert!(!second.has_unsaved_changes());
}

/// Switching tabs returns to the page last shown on that tab.
#[test]
fn a_tab_reopens_on_its_last_page() {
    let mut settings = SettingsUI::new_for_tests(Config::default());
    settings.select_page(SettingsTab::Automation, 4);
    settings.set_selected_tab(SettingsTab::General);
    assert_eq!(settings.selected_page(), 0);
    settings.set_selected_tab(SettingsTab::Automation);
    assert_eq!(settings.selected_page(), 4);
}

/// Every in-UI link (string-literal `open_section` calls, the Common page's "Go
/// to" links, the profile drawer pointer) names a section the layout has.
#[test]
fn every_open_section_literal_resolves() {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let mut ids = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).expect("read");
        for (at, _) in text.match_indices("open_section(\"") {
            let rest = &text[at + "open_section(\"".len()..];
            ids.push(rest[..rest.find('"').expect("closing quote")].to_string());
        }
        // `("Label", "section_id")` pairs in quick_settings' link table.
        if file.ends_with("quick_settings.rs") {
            for line in text.lines().filter(|l| l.trim_start().starts_with("(\"")) {
                if let Some(id) = line.split('"').nth(3) {
                    ids.push(id.to_string());
                }
            }
        }
    }
    assert!(ids.len() >= 7, "expected the in-UI links, found {ids:?}");
    let unknown: Vec<&String> = ids
        .iter()
        .filter(|id| *id != "no_such_section" && crate::layout::locate(id).is_none())
        .collect();
    assert!(unknown.is_empty(), "links to unknown sections: {unknown:?}");
}
