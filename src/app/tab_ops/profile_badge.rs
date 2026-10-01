//! Per-tab badge and profile variables (UX.md PR5, B60).
//!
//! The window keeps one `BadgeState` (color, font, margins, variables), but
//! the badge *text* follows the active tab: a tab's `badge_override` — set by
//! a switched profile's `badge_text` or a script's SetBadge — is interpolated
//! in place of the window format while that tab is active. Switching tabs
//! therefore switches badges, and a reverted switch restores the tab's own.
//!
//! `tab.profile_name` is the active tab's profile name, alongside the
//! window-wide `session.profile_name`, for badges and status-bar formats.

use crate::app::window_state::WindowState;

impl WindowState {
    /// The badge to draw this frame: the window badge, with its text
    /// replaced by the active tab's override when one is set.
    ///
    /// On a tab switch the window badge style (color, font, margins) is
    /// rebuilt from the config and then the new tab's profile, so a
    /// profile's styling never stays on a tab whose profile set none.
    pub(crate) fn frame_badge(&mut self) -> crate::badge::BadgeState {
        let active = self.tab_manager.active_tab_id();
        if active != self.badge_tab {
            self.badge_tab = active;
            let config = self.config.load_full();
            self.badge_state.update_config(&config);
            self.badge_state
                .set_format(config.badge.badge_format.clone());
            if let Some(profile) = self
                .tab_manager
                .active_tab()
                .and_then(|t| t.profile.effective_profile_id())
                .and_then(|id| self.overlay_ui.profile_manager.get(&id).cloned())
            {
                self.badge_state.apply_profile_settings(&profile);
            }
            self.badge_state.mark_dirty();
        }
        if self.badge_state.is_dirty() {
            self.badge_state.interpolate();
        }
        let mut badge = self.badge_state.clone();
        if let Some(format) = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.profile.badge_override.clone())
        {
            let vars = self.badge_state.variables.read();
            badge.rendered_text = crate::badge::interpolate_badge_format(&format, &vars);
        }
        badge
    }

    /// The active tab's profile name: its auto-applied or source profile,
    /// "Default" for a plain tab.
    pub(crate) fn active_tab_profile_name(&self) -> String {
        self.tab_manager
            .active_tab()
            .and_then(|t| t.profile.effective_profile_id())
            .and_then(|id| self.overlay_ui.profile_manager.get(&id))
            .map_or_else(|| "Default".to_string(), |p| p.name.clone())
    }

    /// Keep `tab.profile_name` and `session.profile_name` on the active
    /// tab's profile. Runs every frame whether or not the badge is drawn:
    /// the Profile status-bar widget and status-bar formats read them too,
    /// and badges are off by default.
    pub(crate) fn sync_tab_profile_variable(&mut self) {
        let name = self.active_tab_profile_name();
        let mut vars = self.badge_state.variables_mut();
        if vars.tab_profile_name != name || vars.profile_name != name {
            vars.tab_profile_name = name.clone();
            vars.profile_name = name;
            drop(vars);
            self.badge_state.mark_dirty();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::window_state::WindowState;
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
    fn the_badge_follows_the_active_tab() {
        // B60: badge_override was written in eight places and read in none.
        let mut config = Config::default();
        config.badge.badge_enabled = true;
        config.badge.badge_format = "window".to_string();
        let runtime = Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime"),
        );
        let mut ws = WindowState::new(config, runtime);
        ws.overlay_ui.profile_manager = crate::profile::ProfileManager::new();
        let mut a = Tab::new_stub(1, 1);
        a.profile.badge_override = Some("tab-a".to_string());
        ws.tab_manager.push_tab_for_test(a);
        ws.tab_manager.push_tab_for_test(Tab::new_stub(2, 2));
        ws.tab_manager.switch_to(1);
        assert_eq!(ws.frame_badge().rendered_text, "tab-a");
        ws.tab_manager.switch_to(2);
        assert_eq!(ws.frame_badge().rendered_text, "window");
    }

    #[test]
    fn a_profiles_badge_style_does_not_stay_on_a_tab_whose_profile_has_none() {
        // apply_profile_settings only overrides fields a profile sets, so
        // the window badge must be rebuilt on a tab switch or tab A's red
        // badge stays on tab B.
        let mut ws = window();
        let mut red = Profile::new("Red");
        red.badge_color = Some([255, 0, 0]);
        red.badge_text = Some("RED".to_string());
        let red_id = red.id;
        let plain = Profile::new("Plain");
        let plain_id = plain.id;
        ws.overlay_ui.profile_manager.add(red);
        ws.overlay_ui.profile_manager.add(plain);
        let configured = ws.config.load().badge.badge_color;
        let mut a = Tab::new_stub(1, 1);
        a.profile.source_profile_id = Some(red_id);
        a.profile.badge_override = Some("RED".to_string());
        let mut b = Tab::new_stub(2, 2);
        b.profile.source_profile_id = Some(plain_id);
        ws.tab_manager.push_tab_for_test(a);
        ws.tab_manager.push_tab_for_test(b);

        ws.tab_manager.switch_to(1);
        let badge = ws.frame_badge();
        assert_eq!(badge.color, [255, 0, 0]);
        assert_eq!(badge.rendered_text, "RED");

        ws.tab_manager.switch_to(2);
        let badge = ws.frame_badge();
        assert_eq!(badge.color, configured, "the configured color is back");
        assert_ne!(badge.rendered_text, "RED");
    }

    #[test]
    fn profile_variables_follow_the_tab_with_badges_off() {
        // Badges are off by default; the Profile status-bar widget and
        // status-bar formats still read the variables, so the frame syncs
        // them unconditionally (egui_submit calls this before frame_badge).
        let mut ws = window();
        assert!(!ws.badge_state.enabled, "the default config has badges off");
        let work = Profile::new("Work");
        let id = work.id;
        ws.overlay_ui.profile_manager.add(work);
        let mut tab = Tab::new_stub(1, 1);
        tab.profile.source_profile_id = Some(id);
        ws.tab_manager.push_tab_for_test(tab);
        ws.tab_manager.push_tab_for_test(Tab::new_stub(2, 2));

        ws.tab_manager.switch_to(1);
        ws.sync_tab_profile_variable();
        let vars = ws.badge_state.variables.read().clone();
        assert_eq!(vars.get("tab.profile_name").as_deref(), Some("Work"));
        assert_eq!(vars.get("session.profile_name").as_deref(), Some("Work"));
        let ctx_vars = vars;
        assert_eq!(
            crate::status_bar::widgets::widget_text(
                &crate::config::WidgetId::Profile,
                &crate::status_bar::widgets::WidgetContext::for_test(ctx_vars),
                None
            ),
            "Work"
        );

        ws.tab_manager.switch_to(2);
        ws.sync_tab_profile_variable();
        assert_eq!(
            ws.badge_state
                .variables
                .read()
                .get("session.profile_name")
                .as_deref(),
            Some("Default")
        );
    }

    #[test]
    fn tab_profile_name_names_the_active_tabs_profile() {
        let mut ws = window();
        let profile = Profile::new("Work");
        let id = profile.id;
        ws.overlay_ui.profile_manager.add(profile);
        let mut tab = Tab::new_stub(1, 1);
        tab.profile.source_profile_id = Some(id);
        ws.tab_manager.push_tab_for_test(tab);
        ws.tab_manager.push_tab_for_test(Tab::new_stub(2, 2));
        ws.tab_manager.switch_to(1);
        ws.sync_tab_profile_variable();
        assert_eq!(
            ws.badge_state.variables.read().get("tab.profile_name"),
            Some("Work".to_string())
        );
        ws.tab_manager.switch_to(2);
        ws.sync_tab_profile_variable();
        assert_eq!(
            ws.badge_state.variables.read().get("tab.profile_name"),
            Some("Default".to_string())
        );
    }
}
