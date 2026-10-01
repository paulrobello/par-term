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
    // These frames render the hidden bar, whose teardown runs
    // AppMenuUi::hide — a consumer of the menu bridge's process-global
    // toggle request. Hold the bridge's test lock so they cannot steal a
    // request another test just raised (see
    // the_in_app_menu_stays_reachable_with_the_bar_hidden).
    let _guard = crate::menu::bridge_test_lock();
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

/// UX.md MN4: with the tab bar hidden the in-app menu floats at the
/// window's top-left, so `toggle_menu` (and a click) still reach it.
#[test]
fn the_in_app_menu_stays_reachable_with_the_bar_hidden() {
    let _guard = crate::menu::bridge_test_lock();
    let _ = crate::menu::drain_pending_actions();
    let ctx = egui::Context::default();
    let mut bar = TabBarUI::new();
    let config = hidden_bar_config();
    assert!(!bar.should_show(1, config.tabs.tab_bar_mode), "bar hidden");

    let profiles = ProfileManager::new();
    let frame = |bar: &mut TabBarUI| {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            bar.show_hidden_bar_menu(ui, &profiles, config.tabs.tab_bar_height, true);
        });
        output.textures_delta.clear();
    };
    frame(&mut bar);
    assert!(!bar.is_app_menu_open());

    crate::menu::request_toggle();
    frame(&mut bar);
    frame(&mut bar);
    assert!(
        bar.is_app_menu_open(),
        "toggle_menu must open the floating menu while the bar is hidden"
    );

    // Where the menu is not drawn (a native menu bar exists), hiding still
    // tears it down so nothing re-opens later.
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        bar.show_hidden_bar_menu(ui, &profiles, config.tabs.tab_bar_height, false);
    });
    output.textures_delta.clear();
    assert!(!bar.is_app_menu_open());
}

#[test]
fn enter_submits_a_rename_opened_with_the_bar_hidden() {
    // Same constraint as the escape test above: the hidden-bar frames
    // consume the menu bridge's process-global toggle request, so they run
    // under the bridge's test lock.
    let _guard = crate::menu::bridge_test_lock();
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
