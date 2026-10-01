//! Dispatch for the profile registry actions (UX.md PR3): `open_profile:<id>`
//! and its siblings, parsed by [`crate::profile::actions::ProfileAction`],
//! plus the per-tab profile actions the launcher, palette, and tab context
//! menu share: undo an automatic switch, pin a tab's profile, manage and edit
//! profiles.

use crate::app::window_state::WindowState;
use crate::profile::actions::{ProfileAction, ProfileSplit};

/// The toast's Undo of an automatic switch: `undo_profile_switch:<tab id>`.
const UNDO_SWITCH: &str = crate::app::tab_ops::profile_auto_switch::UNDO_PROFILE_SWITCH;

impl WindowState {
    /// Run a profile action id. `None` when `action` is not a profile action
    /// (the caller keeps resolving it); `Some(handled)` otherwise. A
    /// well-formed id naming a profile that no longer exists is handled — it
    /// says so in a toast — rather than reported as an unknown action.
    pub(crate) fn dispatch_profile_action(&mut self, action: &str) -> Option<bool> {
        if let Some(tab) = action.strip_prefix(UNDO_SWITCH) {
            return Some(tab.parse().is_ok_and(|id| self.undo_profile_switch(id)));
        }
        if !crate::profile::actions::PROFILE_ACTION_PREFIXES
            .iter()
            .any(|p| action.starts_with(p))
        {
            return None;
        }
        let Some(parsed) = ProfileAction::parse(action) else {
            log::warn!("Malformed profile action '{action}'");
            return Some(false);
        };
        let Some(profile) = self
            .overlay_ui
            .profile_manager
            .get(&parsed.profile_id())
            .cloned()
        else {
            self.show_toast("That profile no longer exists");
            return Some(true);
        };
        match parsed {
            ProfileAction::OpenTab(id) => self.open_profile(id),
            ProfileAction::OpenWindow(id) => {
                self.overlay_state.pending_profile_windows.push(id);
                self.request_redraw();
            }
            ProfileAction::Split(_, dir) => self.split_with_profile(&profile, dir),
            ProfileAction::SetTabProfile(_) => self.set_tab_profile(&profile),
        }
        Some(true)
    }

    /// Split the focused pane running `profile` (UX.md PR1 Cmd+D /
    /// Cmd+Shift+D in the launcher).
    ///
    /// Local tabs only. A par-mux daemon's `split-window` (core 0.57) takes a
    /// start directory but no command, and a tmux gateway split runs the
    /// session's shell, so a profile cannot reach an attached pane yet: the
    /// split is refused with a toast that says why, instead of opening a
    /// pane that silently ignores the profile.
    pub(crate) fn split_with_profile(
        &mut self,
        profile: &crate::profile::Profile,
        dir: ProfileSplit,
    ) {
        #[cfg(feature = "mux")]
        let attached =
            self.tmux_state.transport.is_some() && self.focused_mux_pane_from_native().is_some();
        #[cfg(not(feature = "mux"))]
        let attached = false;
        if attached || self.is_gateway_active() {
            self.show_toast(format!(
                "Split with Profile works in local tabs only — a par-mux or tmux pane \
                 cannot start {}'s program yet",
                profile.name
            ));
            return;
        }
        let direction = match dir {
            ProfileSplit::Right => crate::pane::SplitDirection::Vertical,
            ProfileSplit::Down => crate::pane::SplitDirection::Horizontal,
        };
        if self
            .split_pane_direction_with_profile(direction, profile)
            .is_some()
        {
            log::info!("Split with profile '{}' ({dir:?})", profile.name);
        }
    }

    /// Change Tab Profile (UX.md PR1 Cmd+Enter, PR5): the active tab takes
    /// `profile`'s identity — tooltip, icon, title, badge, shader — and is
    /// pinned, because an explicit choice outranks automatic switching. The
    /// running process is not replaced.
    pub(crate) fn set_tab_profile(&mut self, profile: &crate::profile::Profile) {
        let Some(tab) = self.tab_manager.active_tab_mut() else {
            return;
        };
        let pending = tab.profile.pending_command_id.take();
        if let crate::tab::TitleChange::Set(title) = tab.profile.revert_auto() {
            tab.set_title(&title);
        }
        tab.profile.source_profile_id = Some(profile.id);
        tab.profile.pinned = true;
        tab.profile.declined_profile_id = None;
        tab.profile.profile_icon = profile.icon.clone();
        tab.profile.badge_override = profile.badge_text.clone();
        if !tab.user_named {
            tab.set_title(profile.tab_name.as_deref().unwrap_or(&profile.name));
        }
        if let Some(id) = pending {
            self.withdraw_profile_command(id);
        }
        self.apply_profile_badge(profile);
        self.apply_profile_shader_settings(profile);
        self.show_toast(format!("Tab profile: {}", profile.name));
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Pin or unpin the active tab's profile (UX.md PR6).
    pub(crate) fn toggle_tab_profile_pin(&mut self) {
        let Some(tab) = self.tab_manager.active_tab_mut() else {
            return;
        };
        tab.profile.pinned = !tab.profile.pinned;
        let pinned = tab.profile.pinned;
        self.show_toast(if pinned {
            "Profile pinned: automatic switching is off for this tab"
        } else {
            "Profile unpinned: automatic switching is on for this tab"
        });
        self.request_redraw();
    }

    /// Edit Tab's Profile… (UX.md PR5): Settings › Profiles, naming the
    /// active tab's profile in a toast. Settings opens on the profile list;
    /// selecting the profile there needs a Settings-side entry point.
    pub(crate) fn edit_tab_profile(&mut self) {
        let name = self.active_tab_profile_name();
        self.manage_profiles();
        self.show_toast(format!("Editing profile: {name}"));
    }

    /// Manage Profiles… : Settings › Profiles.
    pub(crate) fn manage_profiles(&mut self) {
        self.overlay_state.open_settings_window_requested = true;
        self.overlay_state.open_settings_section =
            Some(par_term_settings_ui::layout::deep_link::PROFILES);
        self.request_redraw();
    }
}
