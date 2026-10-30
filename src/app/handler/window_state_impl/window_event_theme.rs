use crate::app::window_state::WindowState;
use std::sync::Arc;

/// Apply a system theme change to the config (rcu probe-clone for both the
/// theme and the tab style) and propagate the new colors to every tab and
/// pane terminal.
pub(super) fn theme_changed(ws: &mut WindowState, system_theme: winit::window::Theme) {
    let is_dark = system_theme == winit::window::Theme::Dark;
    // Apply theme changes via rcu (apply_system_theme/tab_style require &mut self)
    let theme_changed = {
        let old = ws.config.load();
        // Check if change would occur using a clone (apply_system_theme needs &mut)
        let mut probe = (**old).clone();
        let changed = probe.apply_system_theme(is_dark);
        drop(old);
        if changed {
            ws.config.rcu(|old| {
                let mut new = (**old).clone();
                if new.apply_system_theme(is_dark) {
                    Arc::new(new)
                } else {
                    Arc::clone(old)
                }
            });
        }
        changed
    };
    let tab_style_changed = {
        let old = ws.config.load();
        let mut probe = (**old).clone();
        let changed = probe.apply_system_tab_style(is_dark);
        drop(old);
        if changed {
            ws.config.rcu(|old| {
                let mut new = (**old).clone();
                if new.apply_system_tab_style(is_dark) {
                    Arc::new(new)
                } else {
                    Arc::clone(old)
                }
            });
        }
        changed
    };

    if theme_changed {
        log::info!(
            "System theme changed to {}, switching to theme: {}",
            if is_dark { "dark" } else { "light" },
            ws.config.load().theme_colors.theme
        );
        let theme = ws.config.load().load_theme();
        for tab in ws.tab_manager.tabs_mut() {
            // try_lock: intentional — ThemeChanged fires in the sync event loop.
            // On miss: this tab keeps the old theme until the next theme event
            // or config reload. Cell cache is still invalidated to prevent stale
            // rendering with the old theme colors.
            if let Ok(mut term) = tab.terminal.try_write() {
                term.set_theme(theme.clone());
            }
            // Apply to split pane terminals (primary pane shares tab.terminal).
            // Theme changes recolor cells without bumping update_generation,
            // so every pane's cross-frame cell cache must be invalidated too.
            let tab_terminal = std::sync::Arc::clone(&tab.terminal);
            if let Some(pm) = tab.pane_manager_mut() {
                for pane in pm.all_panes_mut() {
                    if !std::sync::Arc::ptr_eq(&pane.terminal, &tab_terminal)
                        && let Ok(mut term) = pane.terminal.try_write()
                    {
                        term.set_theme(theme.clone());
                    }
                    pane.cache.invalidate_pane_cells();
                }
            }
            tab.active_cache_mut().cells = None;
        }
    }

    if tab_style_changed {
        log::info!(
            "Auto tab style: switching to {} tab style",
            if is_dark {
                ws.config.load().tabs.dark_tab_style.display_name()
            } else {
                ws.config.load().tabs.light_tab_style.display_name()
            }
        );
    }

    if theme_changed || tab_style_changed {
        ws.render_loop
            .external_config_changes
            .push(crate::app::window_state::ExternalConfigChange::SystemTheme { is_dark });
        ws.focus_state.needs_redraw = true;
        ws.request_redraw();
    }
}
