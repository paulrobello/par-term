//! Settings window search, rendered headlessly (SP1 criteria 3 and 4, SQ7).
//!
//! A [`Window`] drives `show_as_panel` frame by frame on one egui context
//! with AccessKit on, so a test can find a widget by its caption and click
//! it the way an assistive-technology client would.

use std::collections::HashMap;

use egui::accesskit::{Action, ActionRequest, Node, NodeId, TreeId};
use par_term_config::Config;

use super::harvest::caption;
use crate::settings_ui::SettingsUI;
use crate::sidebar::SettingsTab;

const SCREEN: egui::Vec2 = egui::vec2(1000.0, 700.0);

struct Window {
    ctx: egui::Context,
    settings: SettingsUI,
    time: f64,
    nodes: HashMap<NodeId, Node>,
}

impl Window {
    fn new(config: Config) -> Self {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut window = Self {
            ctx,
            settings: SettingsUI::new_for_tests(config),
            time: 0.0,
            nodes: HashMap::new(),
        };
        window.frame(Vec::new());
        window
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 0.1;
        // Jumps switch AccessKit off when they finish; tests keep reading it.
        self.ctx.enable_accesskit();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let settings = &mut self.settings;
        let mut output = self.ctx.run_ui(input, |ui| {
            settings.show_as_panel(ui);
        });
        output.textures_delta.clear();
        if let Some(update) = output.platform_output.accesskit_update {
            self.nodes = update.nodes.into_iter().collect();
        }
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    fn type_query(&mut self, text: &str) {
        self.settings.search_query = text.to_string();
        self.frames(2);
    }

    /// Nodes captioned exactly `text`, top to bottom.
    fn find(&self, text: &str) -> Vec<(NodeId, egui::Rect)> {
        let mut found: Vec<(NodeId, egui::Rect)> = self
            .nodes
            .iter()
            .filter(|(_, n)| caption(n).as_deref() == Some(text))
            .filter_map(|(id, n)| {
                n.bounds().map(|b| {
                    (
                        *id,
                        egui::Rect::from_min_max(
                            egui::pos2(b.x0 as f32, b.y0 as f32),
                            egui::pos2(b.x1 as f32, b.y1 as f32),
                        ),
                    )
                })
            })
            .collect();
        found.sort_by(|a, b| a.1.min.y.total_cmp(&b.1.min.y));
        found
    }

    fn shows(&self, text: &str) -> bool {
        self.find(text).iter().any(|(_, r)| on_screen(*r))
    }

    fn click(&mut self, node: NodeId) {
        self.frame(vec![egui::Event::AccessKitActionRequest(ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: node,
            data: None,
        })]);
    }

    /// Click the first node captioned `text` whose left edge is left of `x`
    /// (the sidebar) or right of it (the content area).
    fn click_in_sidebar(&mut self, text: &str) {
        let node = self
            .find(text)
            .into_iter()
            .find(|(_, r)| r.min.x < SIDEBAR_RIGHT)
            .unwrap_or_else(|| panic!("no sidebar row {text:?}"))
            .0;
        self.click(node);
    }

    fn content(&self, text: &str) -> Option<egui::Rect> {
        self.find(text)
            .into_iter()
            .find(|(_, r)| r.min.x >= SIDEBAR_RIGHT)
            .map(|(_, r)| r)
    }
}

/// The sidebar is 150 points wide; anything left of this is a sidebar row.
const SIDEBAR_RIGHT: f32 = 160.0;

fn on_screen(rect: egui::Rect) -> bool {
    rect.max.y > 0.0 && rect.min.y < SCREEN.y
}

#[test]
fn an_empty_result_says_no_settings_match_and_every_tab_stays_clickable() {
    let mut window = Window::new(Config::default());
    window.type_query("xyzzy nothing matches this");
    assert!(
        window.shows(&crate::sidebar::no_match_message(
            "xyzzy nothing matches this"
        )),
        "no empty-state message"
    );
    for tab in [
        SettingsTab::Advanced,
        SettingsTab::Window,
        SettingsTab::Appearance,
    ] {
        let row = format!("{} {}", tab.icon(), tab.display_name());
        window.click_in_sidebar(&row);
        window.frame(Vec::new());
        assert_eq!(
            window.settings.selected_tab, tab,
            "dimmed tab {tab:?} did not take the click"
        );
    }
}

#[test]
fn selecting_a_result_switches_tab_expands_scrolls_and_flashes() {
    // Window › Scrollbar, the last Window section before the arrangements,
    // sits far below the fold; "Tab Bar Appearance" above it starts
    // collapsed, so the target's position depends on the jump.
    const TARGET: &str = "Autohide delay (0 = never):";
    let mut window = Window::new(Config::default());
    window.settings.selected_tab = SettingsTab::Window;
    window.frames(3);
    let before = window.content(TARGET).expect("target drawn on its tab");
    assert!(
        !on_screen(before),
        "test premise: target must start off screen, at {before:?}"
    );

    window.settings.selected_tab = SettingsTab::Appearance;
    window.type_query("autohide delay");
    window.click_in_sidebar(TARGET);
    assert_eq!(window.settings.selected_tab, SettingsTab::Window);
    // Without the query every section is back, so only the jump's scroll
    // can bring the target on screen.
    window.settings.search_query.clear();

    let mut flashed = false;
    for _ in 0..40 {
        window.frame(Vec::new());
        flashed |= super::live::flashing(&window.ctx).as_deref() == Some("window_scrollbar");
    }
    assert!(flashed, "the control never flashed");
    let rect = window
        .content(TARGET)
        .expect("control not drawn: section not expanded");
    assert!(on_screen(rect), "control not scrolled into view: {rect:?}");
}

#[test]
fn selecting_a_result_opens_a_collapsed_section() {
    // Status Bar › Auto-Hide starts collapsed. The query is cleared after the
    // click, so the search filter cannot be what opens it.
    let mut window = Window::new(Config::default());
    window.type_query("hide on mouse inactivity");
    window.click_in_sidebar("Hide on mouse inactivity");
    assert_eq!(window.settings.selected_tab, SettingsTab::StatusBar);
    window.settings.search_query.clear();
    window.frames(40);
    assert!(
        window.content("Hide on mouse inactivity").is_some(),
        "section not expanded"
    );
    assert!(
        window
            .settings
            .collapsed_sections
            .contains("status_bar_auto_hide"),
        "the jump did not leave the section open"
    );
}

#[test]
fn cmd_f_focuses_search_while_settings_has_focus() {
    let mut window = Window::new(Config::default());
    window.frames(2);
    // Move focus off the search field (it takes focus on open).
    window
        .ctx
        .memory_mut(|m| m.surrender_focus(crate::settings_ui::search_field_id()));
    window.frames(2);
    assert!(
        !window
            .ctx
            .memory(|m| m.has_focus(crate::settings_ui::search_field_id()))
    );

    let mods = egui::Modifiers::COMMAND;
    window.frame(vec![
        egui::Event::ModifiersChanged(mods),
        egui::Event::Key {
            key: egui::Key::F,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: mods,
        },
    ]);
    window.frames(2);
    assert!(
        window
            .ctx
            .memory(|m| m.has_focus(crate::settings_ui::search_field_id())),
        "Cmd/Ctrl+F did not focus the search field"
    );
}

#[test]
fn a_search_opens_a_collapsed_matching_section_in_place() {
    let mut window = Window::new(Config::default());
    window.settings.selected_tab = SettingsTab::StatusBar;
    window.frames(2);
    assert!(window.content("Hide on mouse inactivity").is_none());
    window.type_query("mouse inactivity");
    assert!(
        window.content("Hide on mouse inactivity").is_some(),
        "matching collapsed section did not open"
    );
    assert!(
        window.content("Enable status bar").is_none(),
        "non-matching section still shown"
    );
}
