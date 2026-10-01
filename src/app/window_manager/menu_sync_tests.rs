//! UX.md MN3 seam: the menus follow the live bindings after a config apply
//! and after a keybinding change, and MN2 state reaches them.
//!
//! The native muda menu can only be built on the main thread, so these
//! tests drive the manager's real sync path for the in-app menu (which
//! rebuilds through the same model) and assert on the native menu's inputs
//! — the bindings and state `sync_menus` hands `MenuManager::rebuild` and
//! `apply_state` — plus the rebuild decision itself.

use super::WindowManager;
use crate::app::window_state::WindowState;
use crate::config::Config;
use crate::menu::MenuAction;
use crate::menu::model::{all_items, menu_model_with};
use crate::menu::sync::{MenuSync, RebuildReason};
use crate::tab::Tab;
use par_term_config::KeyBinding;
use std::sync::Arc;
use winit::window::WindowId;

fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime"),
    )
}

/// A manager holding one focused window with two titled tabs.
fn manager() -> WindowManager {
    let rt = runtime();
    let mut wm = WindowManager::new(
        Config::default(),
        Arc::clone(&rt),
        crate::cli::RuntimeOptions::default(),
    );
    let mut ws = WindowState::new(Config::default(), rt);
    let mut build = Tab::new_stub(1, 1);
    build.set_title("build");
    ws.tab_manager.push_tab_for_test(build);
    let mut logs = Tab::new_stub(2, 2);
    logs.set_title("logs");
    ws.tab_manager.push_tab_for_test(logs);
    ws.window_index = 1;
    wm.windows.insert(WindowId::from(1u64), ws);
    wm
}

fn focused(wm: &mut WindowManager) -> &mut WindowState {
    wm.windows
        .get_mut(&WindowId::from(1u64))
        .expect("the test window")
}

/// The chord label the in-app menu shows for item `id`, if any.
fn in_app_chord(wm: &mut WindowManager, id: &str) -> Option<String> {
    all_items(focused(wm).tab_bar_ui.app_menu().sections())
        .into_iter()
        .find(|s| s.id == id)
        .and_then(|s| s.accelerator)
        .map(|a| crate::menu::model::accelerator_label(&a))
}

fn label_for(chord: &str) -> Option<String> {
    crate::menu::registry_accel::accelerator_from_chord(chord)
        .map(|a| crate::menu::model::accelerator_label(&a))
}

/// A config whose bindings rebind `new_tab` to Ctrl+Alt+F2 and unbind
/// `clear_scrollback`.
fn rebound_config() -> Config {
    let mut config = Config::default();
    config.keybindings.retain(|kb| kb.action != "new_tab");
    config
        .keybindings
        .retain(|kb| kb.action != "clear_scrollback");
    config.keybindings.insert(
        0,
        KeyBinding {
            key: "Ctrl+Alt+F2".to_string(),
            action: "new_tab".to_string(),
        },
    );
    config
}

/// Settings apply/save path: `apply_config_to_windows` with new bindings
/// makes the native menu stale and rebuilds the in-app menu with the new
/// chord, the unbound one released.
#[test]
fn a_config_apply_rebuilds_the_menus() {
    let mut wm = manager();
    let mut native = MenuSync::new();
    let before = wm.native_menu_inputs().expect("focused window");
    native.record(&before.config.keybindings, before.capture.clone());
    wm.sync_menus();
    assert!(in_app_chord(&mut wm, "clear_scrollback").is_some());

    let config = rebound_config();
    wm.apply_config_to_windows(&config);
    let after = wm.native_menu_inputs().expect("focused window");
    assert_eq!(
        native.rebuild_reason(&after.config.keybindings, &after.capture),
        Some(RebuildReason::Bindings),
        "the native menu must rebuild after a binding change"
    );
    let rebuilt = menu_model_with(cfg!(target_os = "macos"), &after.config.keybindings);
    let new_tab = all_items(&rebuilt)
        .into_iter()
        .find(|s| s.id == "new_tab")
        .and_then(|s| s.accelerator);
    assert_eq!(
        new_tab.map(|a| crate::menu::model::accelerator_label(&a)),
        label_for("Ctrl+Alt+F2")
    );

    wm.sync_menus();
    assert_eq!(in_app_chord(&mut wm, "new_tab"), label_for("Ctrl+Alt+F2"));
    assert_eq!(
        in_app_chord(&mut wm, "clear_scrollback"),
        None,
        "an unbound chord must be released"
    );
}

/// Keybinding-change path that does not go through Settings (F5
/// `reload_config` and the config-file watcher both store the new bindings
/// in the window's config): the next sync picks it up.
#[test]
fn a_window_local_keybinding_change_rebuilds_the_menus() {
    let mut wm = manager();
    wm.sync_menus();
    let mut native = MenuSync::new();
    let before = wm.native_menu_inputs().expect("focused window");
    native.record(&before.config.keybindings, before.capture.clone());
    assert_eq!(
        native.rebuild_reason(&before.config.keybindings, &before.capture),
        None
    );

    let bindings = rebound_config().keybindings;
    focused(&mut wm).config.rcu(|old| {
        let mut new = (**old).clone();
        new.keybindings = bindings.clone();
        Arc::new(new)
    });
    let after = wm.native_menu_inputs().expect("focused window");
    assert_eq!(
        native.rebuild_reason(&after.config.keybindings, &after.capture),
        Some(RebuildReason::Bindings)
    );
    wm.sync_menus();
    assert_eq!(in_app_chord(&mut wm, "new_tab"), label_for("Ctrl+Alt+F2"));
}

/// MN2 state reaches the menu: Tab N titles, the tab count, and toggles.
#[test]
fn the_menu_state_reflects_the_window() {
    let mut wm = manager();
    let inputs = wm.native_menu_inputs().expect("focused window");
    assert_eq!(inputs.state.tab_count, 2);
    assert_eq!(inputs.state.tab_titles, ["build", "logs"]);
    assert_eq!(inputs.state.windows, [(1, "build".to_string())]);
    assert!(!inputs.state.multiple_panes);
    assert!(!inputs.state.session_attached);
    assert!(!inputs.state.checked(crate::menu::state::Check::FpsOverlay));

    focused(&mut wm).debug.show_fps_overlay = true;
    let inputs = wm.native_menu_inputs().expect("focused window");
    assert!(inputs.state.checked(crate::menu::state::Check::FpsOverlay));
}

/// UX.md MP3: Session › Pin Profile mirrors the active tab's pin, and
/// View › Profiles Panel mirrors the drawer. Open Profiles is a popup: its
/// chord stays live while it is open (so it closes it), with no checkmark.
#[test]
fn the_profile_items_reflect_the_window() {
    use crate::menu::state::Check;
    let mut wm = manager();
    let inputs = wm.native_menu_inputs().expect("focused window");
    assert!(!inputs.state.checked(Check::TabProfilePinned));
    assert!(!inputs.state.checked(Check::ProfileDrawer));

    assert!(focused(&mut wm).execute_keybinding_action("toggle_tab_profile_pin"));
    let inputs = wm.native_menu_inputs().expect("focused window");
    assert!(inputs.state.checked(Check::TabProfilePinned));

    assert!(focused(&mut wm).execute_keybinding_action("toggle_profiles_panel"));
    let inputs = wm.native_menu_inputs().expect("focused window");
    assert!(inputs.state.checked(Check::ProfileDrawer));
    assert!(inputs.state.open_toggles.contains("toggle_profiles_panel"));
    assert!(!inputs.state.open_toggles.contains("toggle_profile_drawer"));

    focused(&mut wm).open_profile_launcher();
    let inputs = wm.native_menu_inputs().expect("focused window");
    assert!(inputs.state.open_toggles.contains("toggle_profile_drawer"));
}

/// B61 parity: an open dialog marks the state modal (items disable) and
/// captures the keyboard (macOS releases the accelerators), keeping the
/// open panel's own toggle so its chord still closes it.
#[test]
fn an_open_dialog_blocks_the_menu() {
    let mut wm = manager();
    let idle = wm.native_menu_inputs().expect("window");
    assert!(!idle.state.modal_open);
    assert_eq!(idle.capture, None);

    focused(&mut wm).overlay_ui.command_palette.visible = true;
    let inputs = wm.native_menu_inputs().expect("window");
    assert!(inputs.state.modal_open);
    if cfg!(target_os = "macos") {
        let open = inputs.capture.expect("captured keyboard");
        assert!(open.contains("toggle_command_palette"));
    } else {
        assert_eq!(inputs.capture, None, "only macOS releases accelerators");
    }
    let palette = MenuAction::Action("toggle_command_palette");
    let new_tab = MenuAction::Action("new_tab");
    let registry = &focused(&mut wm).keybinding_registry;
    assert!(inputs.state.passes_with(&palette, registry));
    assert!(!inputs.state.passes_with(&new_tab, registry));
}
