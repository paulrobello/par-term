//! SP3 criterion 5 (profile half): the profile editor is split into the
//! six UX.md 15.2 sub-tabs, each tab draws its own fields, and Done/Cancel
//! stay reachable on every tab.

use std::collections::{HashMap, HashSet};

use egui::accesskit::{Node, NodeId};
use par_term_config::Profile;

use super::{ProfileEditTab, ProfileModalUI};
use crate::search::caption;

/// Captions drawn by the editor on `tab`.
fn captions_on(modal: &mut ProfileModalUI, tab: ProfileEditTab) -> HashSet<String> {
    modal.edit_tab = tab;
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    ctx.memory_mut(|m| m.set_everything_is_visible(true));
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(900.0, 4000.0),
        )),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            modal.show_inline(ui, &mut HashSet::new());
        });
    });
    output.textures_delta.clear();
    let nodes: HashMap<NodeId, Node> = output
        .platform_output
        .accesskit_update
        .map(|u| u.nodes.into_iter().collect())
        .unwrap_or_default();
    nodes.values().filter_map(caption).collect()
}

fn editing() -> ProfileModalUI {
    let mut modal = ProfileModalUI::new();
    let profile = Profile::new("work");
    let id = profile.id;
    modal.add_profile_for_test(profile);
    modal.start_edit_for_test(id);
    modal
}

#[test]
fn the_editor_has_the_six_sub_tabs() {
    let labels: Vec<&str> = ProfileEditTab::ALL.iter().map(|t| t.label()).collect();
    assert_eq!(
        labels,
        [
            "General",
            "Session",
            "Text & Badge",
            "Shader",
            "SSH",
            "Auto-Switch"
        ]
    );
    let mut modal = editing();
    let drawn = captions_on(&mut modal, ProfileEditTab::General);
    for label in labels {
        assert!(drawn.contains(label), "sub-tab {label:?} not in the bar");
    }
}

#[test]
fn each_sub_tab_draws_its_own_fields_and_the_footer() {
    let cases: &[(ProfileEditTab, &str, &str)] = &[
        (ProfileEditTab::General, "Name:", "Hosts:"),
        (ProfileEditTab::Session, "Tmux Auto-Connect", "Name:"),
        (
            ProfileEditTab::TextAndBadge,
            "Badge Text:",
            "Shader Overrides",
        ),
        (ProfileEditTab::Shader, "Shader Overrides", "Badge Text:"),
        (ProfileEditTab::Ssh, "SSH Connection", "Tab Name:"),
        (ProfileEditTab::AutoSwitch, "Hosts:", "Shell:"),
    ];
    for (tab, own, other) in cases {
        let mut modal = editing();
        let drawn = captions_on(&mut modal, *tab);
        assert!(drawn.contains(*own), "{tab:?} does not draw {own:?}");
        assert!(!drawn.contains(*other), "{tab:?} also draws {other:?}");
        for footer in ["Done", "Cancel"] {
            assert!(drawn.contains(footer), "{tab:?} hides {footer}");
        }
    }
}

#[test]
fn opening_another_profile_starts_on_general() {
    let mut modal = editing();
    modal.edit_tab = ProfileEditTab::Ssh;
    let other = Profile::new("home");
    let id = other.id;
    modal.add_profile_for_test(other);
    modal.start_edit_for_test(id);
    assert_eq!(modal.edit_tab, ProfileEditTab::General);
}
