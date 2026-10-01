//! Window-level tests for profile identity and automatic switching (UX.md
//! PR5/PR6), on stub tabs — no PTY. The rule evaluation itself is the pure
//! `profile_auto_switch::decide`, tested beside it.

use crate::app::overlay::toast::ToastAction;
use crate::app::window_state::WindowState;
use crate::config::Config;
use crate::profile::Profile;
use crate::tab::{AutoRule, Tab};
use std::sync::Arc;

fn window() -> WindowState {
    let runtime = Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime"),
    );
    let mut ws = WindowState::new(Config::default(), runtime);
    // `new` loads the user's profiles.yaml; start from none.
    ws.overlay_ui.profile_manager = crate::profile::ProfileManager::new();
    ws
}

fn with_profile(ws: &mut WindowState, name: &str) -> crate::profile::ProfileId {
    let mut profile = Profile::new(name);
    profile.badge_text = Some(format!("{name}-badge"));
    let id = profile.id;
    ws.overlay_ui.profile_manager.add(profile);
    id
}

fn tab_tooltip(ws: &WindowState, tab: crate::tab::TabId) -> Option<String> {
    let tab = ws.tab_manager.get_tab(tab)?;
    crate::tab::profile_tooltip(&tab.profile, |id| {
        ws.overlay_ui
            .profile_manager
            .get(&id)
            .map(|p| p.name.clone())
    })
}

#[test]
fn an_auto_switch_names_the_profile_and_offers_undo() {
    // UX.md MP3 acceptance: a tab's tooltip names its profile and an
    // auto-switch shows a toast with Undo.
    let mut ws = window();
    let prod = with_profile(&mut ws, "Prod");
    ws.tab_manager.push_tab_for_test(Tab::new_stub(1, 1));
    ws.tab_manager.switch_to(1);

    ws.apply_auto_profile(1, AutoRule::Host, prod, "host prod.example.com".to_string());

    assert_eq!(
        tab_tooltip(&ws, 1).as_deref(),
        Some("Profile: Prod (auto: host prod.example.com)")
    );
    let toast = ws.overlay_state.toasts.newest().expect("switch toast");
    assert_eq!(
        toast.message,
        "Profile Prod applied (host prod.example.com)"
    );
    assert_eq!(
        toast.action,
        Some(ToastAction {
            label: "Undo".to_string(),
            action_id: "undo_profile_switch:1".to_string(),
        })
    );
    // The per-tab badge (B60) carries the profile's badge text.
    assert_eq!(
        ws.tab_manager
            .get_tab(1)
            .unwrap()
            .profile
            .badge_override
            .as_deref(),
        Some("Prod-badge")
    );

    // The Undo button runs its action id through the registry dispatch.
    assert!(ws.execute_keybinding_action("undo_profile_switch:1"));
    let tab = ws.tab_manager.get_tab(1).unwrap();
    assert_eq!(tab.profile.effective_profile_id(), None);
    assert_eq!(tab.title, "Tab 1", "title restored");
    assert_eq!(tab.profile.badge_override, None, "badge restored");
    assert_eq!(
        tab.profile.declined_profile_id,
        Some(prod),
        "the undone profile is declined so the next frame does not re-apply it"
    );
}

#[test]
fn a_background_tab_switches_without_touching_the_active_tab() {
    // PR6: background tabs are evaluated too.
    let mut ws = window();
    let prod = with_profile(&mut ws, "Prod");
    ws.tab_manager.push_tab_for_test(Tab::new_stub(1, 1));
    ws.tab_manager.push_tab_for_test(Tab::new_stub(2, 2));
    ws.tab_manager.switch_to(1);

    ws.apply_auto_profile(2, AutoRule::Directory, prod, "directory /srv".to_string());

    assert_eq!(
        ws.tab_manager
            .get_tab(2)
            .unwrap()
            .profile
            .effective_profile_id(),
        Some(prod)
    );
    assert_eq!(
        ws.tab_manager
            .get_tab(1)
            .unwrap()
            .profile
            .effective_profile_id(),
        None
    );
}

#[test]
fn an_undone_switch_withdraws_its_pending_profile_command() {
    // Undo must not leave the confirmation dialog offering to run the
    // command of a profile the user just undid.
    let mut ws = window();
    let mut profile = Profile::new("Cmd");
    profile.command = Some("echo".to_string());
    let id = profile.id;
    ws.overlay_ui.profile_manager.add(profile);
    ws.tab_manager.push_tab_for_test(Tab::new_stub(1, 1));
    ws.tab_manager.push_tab_for_test(Tab::new_stub(2, 2));
    ws.tab_manager.switch_to(1);

    ws.apply_auto_profile(2, AutoRule::Host, id, "host h".to_string());

    let pending = ws
        .tab_manager
        .get_tab(2)
        .unwrap()
        .profile
        .pending_command_id;
    assert!(pending.is_some(), "the command waits for confirmation");
    assert!(
        ws.trigger_state
            .pending_trigger_actions
            .iter()
            .any(|p| Some(p.trigger_id) == pending)
    );
    assert!(ws.undo_profile_switch(2));
    assert!(
        ws.trigger_state
            .pending_trigger_actions
            .iter()
            .all(|p| Some(p.trigger_id) != pending),
        "Undo withdraws the switch's pending command"
    );
}

#[test]
fn a_background_switch_targets_its_own_tab_with_the_profile_command() {
    // Security: a background tab's OSC 7 must never queue a command for
    // "whatever tab is active when the user clicks Allow".
    let window_id = winit::window::WindowId::dummy();
    assert_eq!(
        crate::app::tab_ops::profile_command::command_target(Some(window_id), Some(2)),
        Some(crate::app::window_state::AutomationTarget {
            window_id,
            tab_id: 2
        })
    );
    // The tmux gateway path (no tab) keeps the active-tab meaning.
    assert_eq!(
        crate::app::tab_ops::profile_command::command_target(Some(window_id), None),
        None
    );
}

/// Feed an OSC 7 `file://host/path` report into stub tab `tab`'s terminal,
/// the way a shell's integration announces where it is.
fn report_location(ws: &WindowState, tab: crate::tab::TabId, host: &str, path: &str) {
    let tab = ws.tab_manager.get_tab(tab).expect("tab");
    let term = tab.terminal.try_read().expect("terminal");
    term.process_data(format!("\x1b]7;file://{host}{path}\x07").as_bytes());
}

fn host_profile(ws: &mut WindowState, name: &str, pattern: &str) -> crate::profile::ProfileId {
    let mut profile = Profile::new(name);
    profile.hostname_patterns = vec![pattern.to_string()];
    let id = profile.id;
    ws.overlay_ui.profile_manager.add(profile);
    id
}

#[test]
fn the_evaluator_switches_a_background_tab_from_its_own_osc7() {
    // PR6 end to end: OSC 7 in a background tab -> check_auto_profile_switch
    // -> that tab switches, the toast names it with Undo, the tooltip says
    // why, and the active tab is untouched.
    let mut ws = window();
    let prod = host_profile(&mut ws, "Prod", "prod.example.com");
    ws.tab_manager.push_tab_for_test(Tab::new_stub(1, 1));
    ws.tab_manager.push_tab_for_test(Tab::new_stub(2, 2));
    ws.tab_manager.switch_to(1);

    report_location(&ws, 2, "prod.example.com", "/srv");
    assert!(
        ws.check_auto_profile_switch(),
        "the background tab switched"
    );

    assert_eq!(
        ws.tab_manager
            .get_tab(2)
            .unwrap()
            .profile
            .effective_profile_id(),
        Some(prod)
    );
    assert_eq!(
        ws.tab_manager
            .get_tab(1)
            .unwrap()
            .profile
            .effective_profile_id(),
        None
    );
    assert_eq!(
        tab_tooltip(&ws, 2).as_deref(),
        Some("Profile: Prod (auto: host prod.example.com)")
    );
    let toast = ws.overlay_state.toasts.newest().expect("toast");
    assert_eq!(
        toast.message,
        "Profile Prod applied (host prod.example.com)"
    );
    assert_eq!(
        toast.action.as_ref().map(|a| a.label.as_str()),
        Some("Undo")
    );

    // Undo, then the same rule on the next frame does not re-apply it.
    assert!(ws.execute_keybinding_action("undo_profile_switch:2"));
    assert!(!ws.check_auto_profile_switch());
    assert_eq!(
        ws.tab_manager
            .get_tab(2)
            .unwrap()
            .profile
            .effective_profile_id(),
        None
    );
}

#[test]
fn the_evaluator_leaves_a_pinned_tab_alone() {
    // UX.md MP3 acceptance: a pinned tab does not auto-switch.
    let mut ws = window();
    host_profile(&mut ws, "Prod", "prod.example.com");
    ws.tab_manager.push_tab_for_test(Tab::new_stub(1, 1));
    ws.tab_manager.switch_to(1);
    assert!(ws.execute_keybinding_action("toggle_tab_profile_pin"));
    ws.overlay_state.toasts.clear();

    report_location(&ws, 1, "prod.example.com", "/srv");
    assert!(!ws.check_auto_profile_switch());

    assert_eq!(
        ws.tab_manager
            .get_tab(1)
            .unwrap()
            .profile
            .effective_profile_id(),
        None
    );
    assert!(
        ws.overlay_state.toasts.newest().is_none(),
        "no switch, no toast"
    );
}

#[test]
fn the_palette_lists_each_profiles_rows_with_the_live_chord() {
    // UX.md MP3 acceptance (C2): Open / Open in New Window / Split with /
    // Change Tab Profile rows, plus Manage Profiles; a migrated shortcut
    // shows as the Open row's chord.
    let mut config = Config::default();
    let profile = Profile::new("Work");
    let id = profile.id;
    config.keybindings.push(crate::config::KeyBinding {
        key: "Ctrl+Alt+W".to_string(),
        action: format!("open_profile:{id}"),
    });
    let runtime = Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime"),
    );
    let mut ws = WindowState::new(config, runtime);
    ws.overlay_ui.profile_manager = crate::profile::ProfileManager::new();
    ws.overlay_ui.profile_manager.add(profile);
    assert!(ws.execute_keybinding_action("toggle_command_palette"));
    let palette = &ws.overlay_ui.command_palette;
    for (action, label) in [
        (format!("open_profile:{id}"), "Open Profile: Work"),
        (
            format!("open_profile_window:{id}"),
            "Open Profile in New Window: Work",
        ),
        (
            format!("split_profile:{id}:right"),
            "Split with Profile: Work",
        ),
        (format!("set_tab_profile:{id}"), "Change Tab Profile: Work"),
    ] {
        let row = palette
            .row(&action)
            .unwrap_or_else(|| panic!("{action} row"));
        assert_eq!(row.label, label);
    }
    assert_eq!(
        palette
            .row(&format!("open_profile:{id}"))
            .and_then(|r| r.chord.as_deref()),
        Some("Ctrl+Alt+W"),
        "the migrated chord shows live"
    );
    assert!(
        palette.row("manage_profiles").is_some(),
        "Manage Profiles row"
    );
}

#[test]
fn set_tab_profile_records_and_pins() {
    let mut ws = window();
    let work = with_profile(&mut ws, "Work");
    ws.tab_manager.push_tab_for_test(Tab::new_stub(1, 1));
    ws.tab_manager.switch_to(1);
    assert!(ws.execute_keybinding_action(&format!("set_tab_profile:{work}")));
    let tab = ws.tab_manager.get_tab(1).unwrap();
    assert_eq!(tab.profile.source_profile_id, Some(work));
    assert!(
        tab.profile.pinned,
        "an explicit choice outranks automatic switching"
    );
    assert_eq!(
        tab_tooltip(&ws, 1).as_deref(),
        Some("Profile: Work · pinned")
    );
}

#[test]
fn the_pin_action_toggles() {
    let mut ws = window();
    ws.tab_manager.push_tab_for_test(Tab::new_stub(1, 1));
    ws.tab_manager.switch_to(1);
    assert!(ws.execute_keybinding_action("toggle_tab_profile_pin"));
    assert!(ws.tab_manager.get_tab(1).unwrap().profile.pinned);
    assert!(ws.execute_keybinding_action("toggle_tab_profile_pin"));
    assert!(!ws.tab_manager.get_tab(1).unwrap().profile.pinned);
}

#[test]
fn manage_profiles_deep_links_to_the_profile_list() {
    // The Profiles menu's Manage Profiles... runs this action (UX.md MN1).
    let mut ws = window();
    assert!(ws.execute_keybinding_action("manage_profiles"));
    assert!(ws.overlay_state.open_settings_window_requested);
    assert_eq!(
        ws.overlay_state.open_settings_section,
        Some(par_term_settings_ui::layout::deep_link::PROFILES)
    );
}

#[test]
fn edit_tab_profile_opens_the_profile_list_and_names_the_profile() {
    let mut ws = window();
    let work = with_profile(&mut ws, "Work");
    ws.tab_manager.push_tab_for_test(Tab::new_stub(1, 1));
    ws.tab_manager.switch_to(1);
    ws.tab_manager
        .get_tab_mut(1)
        .unwrap()
        .profile
        .source_profile_id = Some(work);
    assert!(ws.execute_keybinding_action("edit_tab_profile"));
    assert_eq!(
        ws.overlay_state.open_settings_section,
        Some(par_term_settings_ui::layout::deep_link::PROFILES)
    );
    assert_eq!(ws.last_toast_text(), Some("Editing profile: Work"));
}

#[test]
fn a_profile_window_request_is_queued_for_the_manager() {
    let mut ws = window();
    let work = with_profile(&mut ws, "Work");
    assert!(ws.execute_keybinding_action(&format!("open_profile_window:{work}")));
    assert_eq!(ws.overlay_state.pending_profile_windows, vec![work]);
}

#[test]
fn a_deleted_profile_says_so_instead_of_failing_silently() {
    let mut ws = window();
    let gone = uuid::Uuid::new_v4();
    assert!(ws.execute_keybinding_action(&format!("open_profile:{gone}")));
    assert_eq!(ws.last_toast_text(), Some("That profile no longer exists"));
}
