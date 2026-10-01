//! Keeping the menus in step with the live registry and window state
//! (UX.md MN2, MN3).
//!
//! [`WindowManager::sync_menus`] runs once per event-loop tick, after menu
//! events are processed. It rebuilds the native menu when the focused
//! window's bindings differ from the ones the menu was built from — which
//! covers every way the bindings change: Settings apply and save
//! (`apply_config_to_windows`), F5 (`reload_config`), and the config-file
//! watcher picking up an outside edit (`check_config_reload`) all end in the
//! window's config — and applies the focused window's state (enabled,
//! checked, tab titles). Each window's in-app menu is synced the same way.

use std::sync::Arc;

use winit::window::Window;

use crate::config::Config;
use crate::menu::state::{Capture, MenuState};

use super::WindowManager;

/// The platforms whose native menu bar par-term attaches. Elsewhere the
/// muda menu is built but never shown, so it is not rebuilt or patched.
const HAS_NATIVE_MENU_BAR: bool = cfg!(any(target_os = "macos", target_os = "windows"));

/// What the native menu is built from and shows: the focused window's
/// config (its bindings), which accelerators survive a captured keyboard
/// (macOS), and its state.
pub(crate) struct NativeMenuInputs {
    pub(crate) config: Arc<Config>,
    pub(crate) capture: Capture,
    pub(crate) state: MenuState,
}

impl WindowManager {
    /// Rebuild menus whose bindings went stale and apply the live state.
    pub(crate) fn sync_menus(&mut self) {
        let windows = self.window_list();
        let arrangements = self.arrangement_list();
        for window_state in self.windows.values_mut() {
            let mut state = window_state.menu_state();
            state.windows = windows.clone();
            state.arrangements = arrangements.clone();
            let config = window_state.config.load();
            window_state
                .tab_bar_ui
                .sync_app_menu(&config.keybindings, state);
        }

        if !HAS_NATIVE_MENU_BAR {
            return;
        }
        let Some(inputs) = self.native_menu_inputs() else {
            return;
        };
        let bindings = &inputs.config.keybindings;
        let Some(menu) = &self.menu else {
            return;
        };
        if let Some(reason) = menu.rebuild_reason(bindings, &inputs.capture) {
            log::debug!("menu: rebuilding ({reason:?})");
            let attached: Vec<Arc<Window>> = self
                .windows
                .values()
                .filter_map(|ws| ws.window.clone())
                .collect();
            if let Some(menu) = &mut self.menu
                && let Err(e) = menu.rebuild(bindings, inputs.capture.clone(), &attached)
            {
                log::warn!("menu: rebuild failed: {e}");
            }
        }
        if let Some(menu) = &mut self.menu {
            menu.apply_state(&inputs.state);
        }
    }

    /// The native menu's inputs, from the focused window.
    ///
    /// The macOS menu is global, so its key equivalents also fire while the
    /// Settings window has focus — whose text fields must receive the keys
    /// (Cmd+W, Cmd+F, …). The accelerators are released then too.
    pub(crate) fn native_menu_inputs(&self) -> Option<NativeMenuInputs> {
        let focused = self.focused_window()?;
        let mut state = focused.menu_state();
        state.windows = self.window_list();
        state.arrangements = self.arrangement_list();
        let settings_focused = self
            .settings_window
            .as_ref()
            .is_some_and(|sw| sw.is_focused());
        let capture = if !cfg!(target_os = "macos") {
            None
        } else if settings_focused {
            Some(Default::default())
        } else {
            focused.menu_capture()
        };
        Some(NativeMenuInputs {
            config: focused.config.load_full(),
            capture,
            state,
        })
    }

    /// `(window number, active tab title)` for every window, by number.
    fn window_list(&self) -> Vec<(usize, String)> {
        let mut list: Vec<(usize, String)> = self
            .windows
            .values()
            .map(|ws| {
                let title = ws
                    .tab_manager
                    .active_tab()
                    .map(|t| t.title.trim().to_string())
                    .unwrap_or_default();
                (ws.window_index, title)
            })
            .collect();
        list.sort_by_key(|(number, _)| *number);
        list
    }

    /// Saved arrangements, in display order.
    fn arrangement_list(&self) -> Vec<(crate::arrangements::ArrangementId, String)> {
        self.arrangement_manager
            .arrangements_ordered()
            .into_iter()
            .map(|a| (a.id, a.name.clone()))
            .collect()
    }
}
