//! UX.md A12 with the tab bar hidden: the keyboard rename opens a field
//! that must still draw and dismiss. The context menu owns the field's
//! Enter/Escape, and the modal guard holds the keyboard while it is open,
//! so a bar that skips drawing it locks input with nothing on screen.

use super::{TabBarAction, TabBarUI};
use crate::config::Config;
use crate::profile::ProfileManager;
use crate::tab::{Tab, TabManager};

fn one_tab() -> TabManager {
    let mut tabs = TabManager::new();
    tabs.push_tab_for_test(Tab::new_stub(1, 1));
    tabs
}

fn frame(
    ctx: &egui::Context,
    bar: &mut TabBarUI,
    tabs: &TabManager,
    config: &Config,
    events: Vec<egui::Event>,
) -> TabBarAction {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 800.0),
        )),
        events,
        ..Default::default()
    };
    let profiles = ProfileManager::new();
    let mut action = TabBarAction::None;
    let mut output = ctx.run_ui(input, |ui| {
        action = bar.render(
            ui,
            tabs,
            config,
            &profiles,
            0.0,
            &crate::session_chip::SessionChip::default(),
        );
    });
    output.textures_delta.clear();
    action
}

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}

/// UX.md V13: a tab switch scrolls the active tab into view — left when it
/// is left of the visible strip, right when it is past it, and not at all
/// when already visible.
#[test]
fn the_active_tab_scrolls_into_view() {
    // 100-wide tabs, 10 spacing, 250-wide area: tabs 0 and 1 fit at offset 0.
    let show = |offset, index| TabBarUI::scroll_to_show(offset, index, 100.0, 10.0, 250.0);
    assert_eq!(show(0.0, 1), 0.0, "already visible: unchanged");
    assert_eq!(show(0.0, 4), 440.0 + 100.0 - 250.0, "right edge brought in");
    assert_eq!(show(500.0, 2), 220.0, "left edge brought in");
}

fn hidden_bar_config() -> Config {
    let mut config = Config::default();
    config.tabs.tab_bar_mode = crate::config::TabBarMode::WhenMultiple;
    config
}

#[test]
fn escape_dismisses_a_rename_opened_with_the_bar_hidden() {
    let ctx = egui::Context::default();
    let mut bar = TabBarUI::new();
    let tabs = one_tab();
    let config = hidden_bar_config();
    assert!(!bar.should_show(1, config.tabs.tab_bar_mode), "bar hidden");

    frame(&ctx, &mut bar, &tabs, &config, vec![]);
    bar.begin_rename(1, "Tab 1", bar.rename_anchor(1), 0);
    assert!(bar.is_renaming());

    frame(&ctx, &mut bar, &tabs, &config, vec![]);
    // The context menu's own Escape ladder, unchanged by the hidden bar:
    // the first Escape leaves rename mode, the second closes the menu.
    frame(&ctx, &mut bar, &tabs, &config, vec![key(egui::Key::Escape)]);
    assert!(!bar.is_renaming(), "Escape leaves rename mode");
    frame(&ctx, &mut bar, &tabs, &config, vec![key(egui::Key::Escape)]);
    assert!(
        !bar.is_context_menu_open(),
        "a second Escape closes the menu even though the bar is hidden, \
         releasing the modal guard"
    );
}

/// The sabotage control: without the hidden-bar branch nothing draws the
/// menu, so the same Escapes leave it open — the lock this guards against.
#[test]
fn nothing_else_closes_the_menu_when_the_bar_is_hidden() {
    let source = include_str!("mod.rs");
    let guard = [
        "if let Some(context_tab_id) = self.context_menu_tab {\n",
        "                return self.render_context_menu",
    ]
    .join("");
    assert!(
        source.contains(&guard),
        "render() must draw the context menu before its hidden-bar return"
    );
}

#[test]
fn enter_submits_a_rename_opened_with_the_bar_hidden() {
    let ctx = egui::Context::default();
    let mut bar = TabBarUI::new();
    let tabs = one_tab();
    let config = hidden_bar_config();

    frame(&ctx, &mut bar, &tabs, &config, vec![]);
    bar.begin_rename(1, "Tab 1", bar.rename_anchor(1), 0);
    frame(&ctx, &mut bar, &tabs, &config, vec![]);
    let action = frame(&ctx, &mut bar, &tabs, &config, vec![key(egui::Key::Enter)]);
    assert_eq!(action, TabBarAction::RenameTab(1, "Tab 1".to_string()));
    assert!(!bar.is_context_menu_open());
}
