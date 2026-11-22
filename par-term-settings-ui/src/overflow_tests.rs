//! Settings layout overflow regression test (cards 01a203eba5dc735da55d577491c40aa2
//! and 01a20483152c7726a1c1384503851268).
//!
//! egui's `horizontal_wrapped` cannot wrap between nested `ui.horizontal`
//! scopes: each scope is allocated at the row's full remaining width, so the
//! wrap condition (`remaining < desired`) is never true and the row overflows
//! the window instead of wrapping. The General tab's Common rows used that
//! pattern, and the standalone settings window showed rows cut off at the
//! right edge (which read as "~2x overflow" on a 2x display's pixels).
//!
//! This pins every tab's layout to the window width at the settings window's
//! default size (770pt) and at its 550pt minimum, plus the General tab at a
//! wider size to pin the reflow.

use par_term_config::Config;

use crate::settings_ui::SettingsUI;
use crate::sidebar::SettingsTab;

const SCREEN: egui::Vec2 = egui::vec2(770.0, 800.0);

/// The settings window's minimum inner size (src/settings_window/mod.rs pins
/// 550pt wide); content must reflow to fit it, not clip.
const MIN_WINDOW: egui::Vec2 = egui::vec2(550.0, 800.0);

/// Max x-extent over all shapes after `frames` runs of `show_as_panel`
/// for `tab` on a synthetic `SCREEN`-sized screen.
fn max_layout_extent(tab: SettingsTab, screen: egui::Vec2, frames: usize) -> f32 {
    let ctx = egui::Context::default();
    crate::nerd_font::configure_nerd_font(&ctx);
    let mut settings = SettingsUI::new_for_tests(Config::default());
    settings.selected_tab = tab;
    let mut max_x = 0.0f32;
    for _ in 0..frames {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)),
            time: Some(1.0),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            let _ = settings.show_as_panel(ui);
        });
        for cs in &out.shapes {
            max_x = max_x.max(cs.shape.visual_bounding_rect().max.x);
        }
        out.textures_delta.clear();
    }
    max_x
}

#[test]
fn every_tab_fits_the_settings_window_at_default_size() {
    for tab in [
        SettingsTab::General,
        SettingsTab::Appearance,
        SettingsTab::WindowsAndTabs,
        SettingsTab::Panes,
        SettingsTab::Sessions,
        SettingsTab::Profiles,
        SettingsTab::Keys,
        SettingsTab::Pointer,
        SettingsTab::Effects,
        SettingsTab::Automation,
        SettingsTab::Assistant,
        SettingsTab::Advanced,
    ] {
        let max_x = max_layout_extent(tab, SCREEN, 2);
        assert!(
            max_x <= SCREEN.x + 0.5,
            "{tab:?} lays out to {max_x:.1}pt in a {}pt window",
            SCREEN.x
        );
    }
}

#[test]
fn every_tab_fits_the_settings_window_at_minimum_size() {
    let mut overflows = Vec::new();
    for tab in [
        SettingsTab::General,
        SettingsTab::Appearance,
        SettingsTab::WindowsAndTabs,
        SettingsTab::Panes,
        SettingsTab::Sessions,
        SettingsTab::Profiles,
        SettingsTab::Keys,
        SettingsTab::Pointer,
        SettingsTab::Effects,
        SettingsTab::Automation,
        SettingsTab::Assistant,
        SettingsTab::Advanced,
    ] {
        let max_x = max_layout_extent(tab, MIN_WINDOW, 2);
        if max_x > MIN_WINDOW.x + 0.5 {
            overflows.push(format!(
                "{tab:?} lays out to {max_x:.1}pt in a {}pt window",
                MIN_WINDOW.x
            ));
        }
    }
    assert!(
        overflows.is_empty(),
        "tabs overflow the 550pt minimum window:\n{}",
        overflows.join("\n")
    );
}

#[test]
fn general_tab_reflows_at_a_wider_window() {
    let wide = egui::vec2(900.0, 800.0);
    let max_x = max_layout_extent(SettingsTab::General, wide, 2);
    assert!(
        max_x <= wide.x + 0.5,
        "General tab lays out to {max_x:.1}pt in a {}pt window",
        wide.x
    );
}
