//! Card 01a1a2e65b98: the session chip was unbudgeted in the horizontal
//! tab bar. With stretch-to-fill (the default) and a single attached mux
//! tab, the tab stretched over the full chip-less budget and the row
//! overflowed by the chip's width — the tab's right edge and the new-tab
//! button clipped off the bar. These tests pin the two fixes: the tabs
//! budget is taken from the row's actual consumption (chip included), and
//! the chip label is ellipsized past a width cap.

use super::TabBarAction;
use super::state::TabBarUI;
use crate::config::Config;
use crate::profile::ProfileManager;
use crate::session_chip::SessionChip;
use crate::tab::{Tab, TabManager};
use crate::ui_constants::{TAB_NEW_BTN_BASE_WIDTH, TAB_SPACING};

const BAR_WIDTH: f32 = 1200.0;

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
    chip: &SessionChip,
    events: Vec<egui::Event>,
) -> TabBarAction {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(BAR_WIDTH, 800.0),
        )),
        events,
        ..Default::default()
    };
    let profiles = ProfileManager::new();
    let mut action = TabBarAction::None;
    let mut output = ctx.run_ui(input, |ui| {
        action = bar.render(ui, tabs, config, &profiles, 0.0, chip);
    });
    output.textures_delta.clear();
    action
}

/// Two hover passes then press + release at (x, y): egui needs the pointer
/// position established and a press/release pair for `clicked()` to fire.
fn click_frames(x: f32, y: f32) -> Vec<Vec<egui::Event>> {
    vec![
        vec![egui::Event::PointerMoved(egui::pos2(x, y))],
        vec![egui::Event::PointerMoved(egui::pos2(x, y))],
        vec![egui::Event::PointerButton {
            pos: egui::pos2(x, y),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
        vec![egui::Event::PointerButton {
            pos: egui::pos2(x, y),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    ]
}

/// Run one frame per event batch, returning the last frame's action.
fn run_click(
    ctx: &egui::Context,
    bar: &mut TabBarUI,
    tabs: &TabManager,
    config: &Config,
    chip: &SessionChip,
    x: f32,
    y: f32,
) -> TabBarAction {
    let mut action = TabBarAction::None;
    for events in click_frames(x, y) {
        action = frame(ctx, bar, tabs, config, chip, events);
    }
    action
}

fn plus_click_x() -> f32 {
    BAR_WIDTH - TAB_NEW_BTN_BASE_WIDTH / 2.0
}

#[test]
fn chip_present_single_tab_keeps_new_tab_button_in_bar() {
    let ctx = egui::Context::default();
    let config = Config::default();
    let tabs = one_tab();
    let chip = SessionChip {
        session: Some("work".to_string()),
        ..Default::default()
    };
    let mut bar = TabBarUI::new();
    // Idle frames establish the pointer-hover baseline.
    let _ = frame(&ctx, &mut bar, &tabs, &config, &chip, vec![]);
    // Containment: the rendered tab must leave room for the + button.
    let (_, tab_rect) = bar.tab_rects.last().expect("one tab rendered");
    assert!(
        tab_rect.max.x <= BAR_WIDTH - TAB_NEW_BTN_BASE_WIDTH + 1.0,
        "tab right edge {} must leave the + button inside the {}px bar",
        tab_rect.max.x,
        BAR_WIDTH
    );
    // Interaction: clicking where the + button lives must fire NewTab.
    let action = run_click(&ctx, &mut bar, &tabs, &config, &chip, plus_click_x(), 12.0);
    assert_eq!(
        action,
        TabBarAction::NewTab,
        "click at the + button position"
    );
}

#[test]
fn chip_absent_budget_unchanged_regression_guard() {
    let ctx = egui::Context::default();
    let config = Config::default();
    let tabs = one_tab();
    let chip = SessionChip::default();
    let mut bar = TabBarUI::new();
    let _ = frame(&ctx, &mut bar, &tabs, &config, &chip, vec![]);
    let (_, tab_rect) = bar.tab_rects.last().expect("one tab rendered");
    // Without a chip the single tab stretches to fill everything left of
    // the + button — the budget fix must not over-subtract.
    assert!(
        tab_rect.max.x >= BAR_WIDTH - TAB_NEW_BTN_BASE_WIDTH - TAB_SPACING - 4.0,
        "chip-less budget unchanged: tab right edge {} near the + button",
        tab_rect.max.x
    );
    let action = run_click(&ctx, &mut bar, &tabs, &config, &chip, plus_click_x(), 12.0);
    assert_eq!(action, TabBarAction::NewTab);
}

#[test]
fn long_chip_name_ellipsized_within_cap() {
    let ctx = egui::Context::default();
    let config = Config::default();
    let tabs = one_tab();
    let long_name = "a".repeat(100);
    let chip = SessionChip {
        session: Some(long_name.clone()),
        ..Default::default()
    };
    let mut bar = TabBarUI::new();
    let _ = frame(&ctx, &mut bar, &tabs, &config, &chip, vec![]);
    // The clamped label: ellipsis present, measured galley within the cap
    // (0.35 * 1200 = 420).
    let cap = BAR_WIDTH * super::CHIP_MAX_WIDTH_FRACTION;
    let mut clamped = String::new();
    let mut width = 0.0_f32;
    let _ = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(BAR_WIDTH, 800.0),
            )),
            ..Default::default()
        },
        |ui| {
            clamped = crate::session_chip::clamp_label(
                ui,
                chip.label(),
                egui::Color32::from_rgb(76, 175, 80),
                cap,
            );
            let font_id = egui::TextStyle::Button.resolve(ui.style());
            width = ui
                .painter()
                .layout_no_wrap(
                    clamped.clone(),
                    font_id,
                    egui::Color32::from_rgb(76, 175, 80),
                )
                .size()
                .x;
        },
    );
    assert!(clamped.ends_with('…'), "label {clamped} ellipsized");
    assert!(
        width <= cap + 1.0,
        "clamped label width {width} within cap {cap}"
    );
    assert_ne!(clamped, long_name, "the raw name cannot have survived");
}
