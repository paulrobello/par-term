//! Window glue for Open Profiles… (UX.md PR1/PR2, MD2): open and toggle the
//! launcher, pin it as the drawer, and run what it chose through the
//! profile registry actions — a launcher pick and a bound shortcut take one
//! path (`dispatch_profile_action`).

use crate::app::window_state::WindowState;
use crate::profile::actions::{ProfileAction, ProfileSplit};
use crate::profile_launcher_ui::{LaunchTarget, LauncherChoice};

impl WindowState {
    /// The launcher's rows, each carrying its live `open_profile:<id>` chord.
    fn launcher_rows(&self) -> Vec<crate::profile_launcher_ui::LauncherRow> {
        let registry = &self.keybinding_registry;
        crate::profile_launcher_ui::launcher_rows(&self.overlay_ui.profile_manager, |action| {
            registry
                .chord_for_action(action)
                .map(|combo| crate::command_palette::catalog::chord_display(&combo))
        })
    }

    /// Open the launcher popup (the tab bar's chevron, Cmd+T with
    /// `new_tab_shortcut_shows_profiles`).
    pub(crate) fn open_profile_launcher(&mut self) {
        let rows = self.launcher_rows();
        let chord = self.live_chord_hint("toggle_profile_drawer");
        self.overlay_ui.profile_launcher_ui.open(rows, chord);
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Open Profiles… (Cmd+O, `toggle_profile_drawer`): toggle the popup.
    pub fn toggle_profile_drawer(&mut self) {
        if self.overlay_ui.profile_launcher_ui.visible {
            self.overlay_ui.profile_launcher_ui.close();
        } else {
            self.open_profile_launcher();
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Show or hide the Profiles drawer (UX.md PR2: the launcher pinned to
    /// the window's right edge). The terminal reflows beside it.
    pub(crate) fn toggle_profiles_drawer(&mut self) {
        let drawer = &mut self.overlay_ui.profile_drawer_ui;
        drawer.toggle();
        if drawer.expanded {
            let rows = self.launcher_rows();
            let launcher = &mut self.overlay_ui.profile_launcher_ui;
            launcher.set_rows(rows);
            launcher.prepare_pinned();
        }
        self.sync_ai_inspector_width();
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Whether a physical-pixel mouse position is over the open Profiles
    /// drawer. The terminal reflows beside the drawer (UX.md PR2), so mouse
    /// input only stays with egui over the drawer itself; the terminal to
    /// its left keeps working.
    pub(crate) fn mouse_over_profiles_drawer(&self, position: (f64, f64)) -> bool {
        let drawer = &self.overlay_ui.profile_drawer_ui;
        if !drawer.expanded {
            return false;
        }
        self.with_window(|window| {
            let scale = window.scale_factor();
            let right = window.inner_size().width as f64
                - f64::from(self.overlay_ui.ai_inspector.consumed_width()) * scale;
            position.0 >= right - f64::from(drawer.consumed_width()) * scale
        })
        .unwrap_or(true)
    }

    /// Refresh the drawer's rows while it is open (profiles and chords can
    /// change under it).
    pub(crate) fn refresh_profiles_drawer_rows(&mut self) {
        if self.overlay_ui.profile_drawer_ui.expanded {
            let rows = self.launcher_rows();
            self.overlay_ui.profile_launcher_ui.set_rows(rows);
        }
    }

    /// While the launcher is open, the split chords split with the
    /// selected profile (UX.md PR1 Cmd+D / Cmd+Shift+D).
    ///
    /// On macOS and Windows the native menu's Split Right / Split Down
    /// accelerators fire before the launcher's egui frame receives the key
    /// and run `split_right` / `split_down` through
    /// `execute_keybinding_action`; this turns that into the launcher's own
    /// action. Returns `true` when it handled the action.
    pub(crate) fn launcher_takes_split(&mut self, action: &str) -> bool {
        if !self.overlay_ui.profile_launcher_ui.visible {
            return false;
        }
        let target = match action {
            "split_right" => LaunchTarget::SplitRight,
            "split_down" => LaunchTarget::SplitDown,
            _ => return false,
        };
        if let Some(choice) = self.overlay_ui.profile_launcher_ui.choose_selected(target) {
            self.run_launcher_choice(choice);
        }
        true
    }

    /// Run a launcher choice.
    pub(crate) fn run_launcher_choice(&mut self, choice: LauncherChoice) {
        match choice {
            LauncherChoice::Open { profiles, target } => {
                for profile in profiles {
                    self.launch_profile_at(profile, target);
                }
            }
            LauncherChoice::Edit(_) => self.manage_profiles(),
            LauncherChoice::Manage => self.manage_profiles(),
            LauncherChoice::Duplicate(id) => self.duplicate_profile(id),
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Open one launcher row at `target`. The Default row (`None`) is a
    /// plain tab, window, or split; a profile goes through its registry
    /// action.
    fn launch_profile_at(
        &mut self,
        profile: Option<crate::profile::ProfileId>,
        target: LaunchTarget,
    ) {
        let Some(id) = profile else {
            match target {
                LaunchTarget::NewTab => self.new_tab(),
                LaunchTarget::NewWindow => {
                    crate::menu::dispatch(crate::menu::MenuAction::NewWindow);
                }
                LaunchTarget::SplitRight => {
                    self.execute_keybinding_action("split_right");
                }
                LaunchTarget::SplitDown => {
                    self.execute_keybinding_action("split_down");
                }
                LaunchTarget::ThisTab => self.clear_tab_profile(),
            }
            return;
        };
        let action = match target {
            LaunchTarget::NewTab => ProfileAction::OpenTab(id),
            LaunchTarget::NewWindow => ProfileAction::OpenWindow(id),
            LaunchTarget::SplitRight => ProfileAction::Split(id, ProfileSplit::Right),
            LaunchTarget::SplitDown => ProfileAction::Split(id, ProfileSplit::Down),
            LaunchTarget::ThisTab => ProfileAction::SetTabProfile(id),
        };
        self.execute_keybinding_action(&action.id());
    }

    /// Change Tab Profile to Default: the tab drops its profile identity
    /// and stays pinned (an explicit choice).
    fn clear_tab_profile(&mut self) {
        let Some(tab) = self.tab_manager.active_tab_mut() else {
            return;
        };
        if let crate::tab::TitleChange::Set(title) = tab.profile.revert_auto() {
            tab.set_title(&title);
        }
        tab.profile.source_profile_id = None;
        tab.profile.profile_icon = None;
        tab.profile.badge_override = None;
        tab.profile.pinned = true;
        self.badge_tab = None;
        self.show_toast("Tab profile: Default");
    }

    /// Duplicate a profile (UX.md PR1 row menu): a copy with a new id, the
    /// name suffixed " (copy)", placed after the original, local, saved.
    pub(crate) fn duplicate_profile(&mut self, id: crate::profile::ProfileId) {
        let Some(original) = self.overlay_ui.profile_manager.get(&id).cloned() else {
            return;
        };
        let copy = duplicate_of(&original);
        let name = copy.name.clone();
        let mut profiles = self.overlay_ui.profile_manager.to_vec();
        let at = profiles
            .iter()
            .position(|p| p.id == id)
            .map_or(profiles.len(), |i| i + 1);
        profiles.insert(at, copy);
        for (order, profile) in profiles.iter_mut().enumerate() {
            profile.order = order;
        }
        self.apply_profile_changes(profiles);
        self.show_toast(format!("Duplicated as {name}"));
    }
}

/// A copy of `profile` for Duplicate: new id, " (copy)" name, local source,
/// no legacy shortcut (a chord belongs to one profile).
pub(crate) fn duplicate_of(profile: &crate::profile::Profile) -> crate::profile::Profile {
    let mut copy = profile.clone();
    copy.id = uuid::Uuid::new_v4();
    copy.name = format!("{} (copy)", profile.name);
    copy.source = crate::profile::ProfileSource::Local;
    copy.keyboard_shortcut = None;
    copy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::profile::Profile;
    use crate::tab::Tab;
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

    #[test]
    fn the_open_profiles_chord_toggles_the_launcher() {
        let mut ws = window();
        ws.overlay_ui.profile_manager.add(Profile::new("Work"));
        assert!(ws.execute_keybinding_action("toggle_profile_drawer"));
        assert!(ws.overlay_ui.profile_launcher_ui.visible);
        assert!(ws.overlay_is_open(crate::app::overlay::OverlayId::ProfileLauncher));
        assert!(
            ws.any_modal_ui_visible(),
            "the launcher joins the key guard"
        );
        assert!(ws.execute_keybinding_action("toggle_profile_drawer"));
        assert!(!ws.overlay_ui.profile_launcher_ui.visible);
    }

    #[test]
    fn new_tab_with_the_profiles_setting_opens_the_launcher() {
        // MD2: new_tab_shortcut_shows_profiles opens the launcher, not the
        // removed chevron window.
        let mut config = Config::default();
        config.tabs.new_tab_shortcut_shows_profiles = true;
        let runtime = Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime"),
        );
        let mut ws = WindowState::new(config, runtime);
        ws.overlay_ui.profile_manager = crate::profile::ProfileManager::new();
        ws.overlay_ui.profile_manager.add(Profile::new("Work"));
        assert!(ws.execute_keybinding_action("new_tab"));
        assert!(ws.overlay_ui.profile_launcher_ui.visible);
    }

    #[test]
    fn a_native_menu_split_while_the_launcher_is_open_splits_with_the_profile() {
        // macOS: the menu's Cmd+D accelerator wins over the launcher's egui
        // frame and runs split_right; the launcher must still own it.
        let mut ws = window();
        let mut work = Profile::new("Work");
        work.command = Some("/bin/sh".to_string());
        ws.overlay_ui.profile_manager.add(work);
        ws.tab_manager.push_tab_for_test(Tab::new_stub(1, 1));
        ws.open_profile_launcher();
        ws.overlay_ui.profile_launcher_ui.set_query_for_test("work");
        assert!(ws.launcher_takes_split("split_right"));
        assert!(
            !ws.overlay_ui.profile_launcher_ui.visible,
            "the launcher closed: it took the split"
        );
        // A split with no launcher open is an ordinary split.
        assert!(!ws.launcher_takes_split("split_right"));
    }

    #[test]
    fn the_drawer_is_the_launcher_pinned_and_a_panel_in_the_stack() {
        // PR2: the drawer is the same list, joins the stack as a Panel
        // (keys only while focused), and the toggle refills its rows.
        let mut ws = window();
        ws.overlay_ui.profile_manager.add(Profile::new("Work"));
        assert!(ws.execute_keybinding_action("toggle_profiles_panel"));
        assert!(ws.overlay_ui.profile_drawer_ui.expanded);
        assert!(ws.overlay_ui.profile_drawer_ui.consumed_width() > 0.0);
        assert!(ws.overlay_is_open(crate::app::overlay::OverlayId::ProfileDrawer));
        assert!(
            !ws.any_modal_ui_visible(),
            "an unfocused drawer does not block the terminal"
        );
        let labels: Vec<String> = ws
            .overlay_ui
            .profile_launcher_ui
            .filtered()
            .iter()
            .map(|r| r.label.clone())
            .collect();
        assert_eq!(labels, ["Default", "Work"]);
        assert!(ws.execute_keybinding_action("toggle_profiles_panel"));
        assert_eq!(ws.overlay_ui.profile_drawer_ui.consumed_width(), 0.0);
    }

    #[test]
    fn duplicate_copies_with_a_new_id_and_no_shortcut() {
        let mut original = Profile::new("Work");
        original.keyboard_shortcut = Some("Ctrl+Alt+W".to_string());
        original.tags = vec!["dev".to_string()];
        let copy = duplicate_of(&original);
        assert_ne!(copy.id, original.id);
        assert_eq!(copy.name, "Work (copy)");
        assert_eq!(copy.tags, original.tags);
        assert_eq!(copy.keyboard_shortcut, None);
    }
}
