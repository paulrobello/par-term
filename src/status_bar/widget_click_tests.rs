//! Headless egui tests for clickable status-bar widgets in non-Right sections.
//!
//! The agent-usage and agent-roster widgets used to be clickable only when
//! placed in the Right section (documented as a v1 limitation in
//! docs/features/AGENT_USAGE.md). These tests pin the fix: every section
//! renders widgets through the same click path.

use std::fs;

use egui::{PointerButton, Pos2, RawInput, Rect, pos2};

use crate::agent_usage::store::UsageStore;
use crate::badge::SessionVariables;
use crate::config::{Config, StatusBarPosition, StatusBarSection, StatusBarWidgetConfig, WidgetId};
use crate::status_bar::{StatusBarAction, StatusBarUI};

const SCREEN: Rect = Rect {
    min: Pos2::ZERO,
    max: pos2(1200.0, 800.0),
};
/// The bar sits at y 0..height with an 8px horizontal inner margin, so the
/// first Left-section widget starts at x = 8.
const LEFT_CLICK: Pos2 = pos2(12.0, 11.0);
/// Center-section widgets are centered, so the screen midpoint is always on
/// the widget.
const CENTER_CLICK: Pos2 = pos2(600.0, 11.0);
/// The Right section ends just left of the scrollbar column.
const RIGHT_CLICK: Pos2 = pos2(1170.0, 11.0);

fn config_with(widget: WidgetId, section: StatusBarSection) -> Config {
    let mut config = Config::default();
    config.status_bar.status_bar_enabled = true;
    config.status_bar.status_bar_position = StatusBarPosition::Top;
    config.status_bar.status_bar_widgets = vec![StatusBarWidgetConfig {
        id: widget,
        enabled: true,
        section,
        order: 0,
        format: None,
    }];
    config
}

/// Point the usage store at a temp records dir holding one record so the
/// AgentUsage widget renders a non-empty summary line ("◆ 42%").
fn seed_usage(bar: &mut StatusBarUI) -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("tempdir");
    fs::write(
        dir.path().join("a.json"),
        r#"{"id":"claude","ready":true,"limits":[{"label":"Weekly","percent":42.0}]}"#,
    )
    .expect("write record");
    bar.usage = UsageStore::new(dir.path().to_path_buf());
    dir
}

/// Render frames under a headless egui context to click the widget at `pos`.
/// egui 0.36 hit-tests against the previous pass's widgets, so the widget
/// needs one pass to be created and another before the hit test binds it:
/// two hover frames, then press, then release. Returns the action the status
/// bar reported on the release frame.
fn click(bar: &mut StatusBarUI, config: &Config, pos: Pos2) -> Option<StatusBarAction> {
    let ctx = egui::Context::default();
    let session = SessionVariables::default();
    let mut action = None;

    let frame = |pos, pressed: Option<bool>| RawInput {
        screen_rect: Some(SCREEN),
        events: {
            let mut events = vec![egui::Event::PointerMoved(pos)];
            if let Some(pressed) = pressed {
                events.push(egui::Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                });
            }
            events
        },
        ..Default::default()
    };

    for pressed in [None, None, Some(true), Some(false)] {
        ctx.begin_pass(frame(pos, pressed));
        action = bar.render(&ctx, config, &session, false).1;
        ctx.end_pass().textures_delta.clear();
    }
    action
}

#[test]
fn agent_usage_click_in_left_opens_panel() {
    let config = config_with(WidgetId::AgentUsage, StatusBarSection::Left);
    let mut bar = StatusBarUI::new();
    let _records = seed_usage(&mut bar);

    assert_eq!(
        click(&mut bar, &config, LEFT_CLICK),
        Some(StatusBarAction::OpenAgentUsagePanel)
    );
}

#[test]
fn agent_usage_click_in_center_opens_panel() {
    let config = config_with(WidgetId::AgentUsage, StatusBarSection::Center);
    let mut bar = StatusBarUI::new();
    let _records = seed_usage(&mut bar);

    assert_eq!(
        click(&mut bar, &config, CENTER_CLICK),
        Some(StatusBarAction::OpenAgentUsagePanel)
    );
}

#[test]
fn agent_usage_click_in_right_still_opens_panel() {
    let config = config_with(WidgetId::AgentUsage, StatusBarSection::Right);
    let mut bar = StatusBarUI::new();
    let _records = seed_usage(&mut bar);

    assert_eq!(
        click(&mut bar, &config, RIGHT_CLICK),
        Some(StatusBarAction::OpenAgentUsagePanel)
    );
}

#[test]
fn agent_roster_click_in_center_opens_palette() {
    let config = config_with(WidgetId::AgentRoster, StatusBarSection::Center);
    let mut bar = StatusBarUI::new();
    bar.agent_roster_summary = Some("ag1 ag2".to_string());

    assert_eq!(
        click(&mut bar, &config, CENTER_CLICK),
        Some(StatusBarAction::OpenAgentPalette)
    );
}
