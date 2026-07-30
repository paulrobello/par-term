//! Menu support for par-term
//!
//! The menu's contents live in one place — [`model`] — and are rendered two
//! ways:
//!
//! - Natively with the `muda` crate, by [`MenuManager`]: a global application
//!   menu bar on macOS, a per-window Win32 menu bar on Windows.
//! - In-app with egui, by [`egui_menu::AppMenuUi`], on Linux/BSD, where muda
//!   cannot attach anything because it needs a `gtk::Window` that winit never
//!   creates (see `linux`).
//!
//! Both renderers walk the same model, so neither platform can end up with
//! commands the other lacks. Activations from either arrive as [`MenuAction`]s
//! at `WindowManager::process_menu_events`.

mod actions;
mod bridge;
pub mod egui_menu;
pub mod model;

/// macOS-specific menu building and NSApp initialization.
#[cfg(target_os = "macos")]
pub(super) mod macos;

/// Linux menu initialization — a no-op that explains itself.
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
pub(super) mod linux;

pub use actions::MenuAction;
pub use bridge::{dispatch, drain_pending_actions, request_toggle};
pub use egui_menu::AppMenuUi;

use crate::profile::Profile;
use anyhow::{Result, anyhow};
use model::MenuEntry;
use muda::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use std::collections::HashMap;
use std::sync::Arc;
use winit::window::Window;

/// Manages the native menu system
pub struct MenuManager {
    /// The root menu
    ///
    /// Only attached on macOS and Windows. Linux never reads it: muda needs a
    /// `gtk::Window` to attach a menubar and winit's X11/Wayland backends do
    /// not create one. Linux gets [`egui_menu::AppMenuUi`] instead; see
    /// `linux.rs`.
    #[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
    menu: Menu,
    /// Mapping from menu item IDs to actions
    action_map: HashMap<MenuId, MenuAction>,
    /// Profiles submenu for dynamic profile items
    profiles_submenu: Submenu,
    /// Track profile menu items for cleanup
    profile_menu_items: Vec<MenuItem>,
}

impl MenuManager {
    /// Create a new menu manager with the default menu structure
    ///
    /// The structure comes from [`model::platform_menu_model`]; this function
    /// only turns it into muda objects and records the id → action mapping.
    pub fn new() -> Result<Self> {
        let menu = Menu::new();
        let mut action_map = HashMap::new();
        let mut profiles_submenu = None;

        // macOS: Application menu (must be first submenu — becomes the macOS app menu).
        // It is built separately because it uses predefined items (Services,
        // Hide, Show All) that the cross-platform model cannot express.
        #[cfg(target_os = "macos")]
        macos::build_app_menu(&menu, &mut action_map)?;

        for section in model::platform_menu_model() {
            // macOS convention: the native Window menu sits just before Help.
            #[cfg(target_os = "macos")]
            if section.title == model::HELP_SECTION_TITLE {
                macos::build_window_menu(&menu, &mut action_map)?;
            }

            let submenu = Submenu::new(section.title, true);
            for entry in &section.entries {
                match entry {
                    MenuEntry::Separator => {
                        submenu.append(&PredefinedMenuItem::separator())?;
                    }
                    MenuEntry::Item(spec) => {
                        let item = MenuItem::with_id(spec.id, spec.label, true, spec.accelerator);
                        action_map.insert(item.id().clone(), spec.action);
                        submenu.append(&item)?;
                    }
                    MenuEntry::Profiles => {
                        // Filled in by `update_profiles`, which appends to the
                        // end of this submenu — so the model must not place
                        // entries after the insertion point.
                        profiles_submenu = Some(submenu.clone());
                    }
                }
            }
            menu.append(&submenu)?;
        }

        let profiles_submenu = profiles_submenu
            .ok_or_else(|| anyhow!("menu model has no profiles insertion point"))?;

        Ok(Self {
            menu,
            action_map,
            profiles_submenu,
            profile_menu_items: Vec::new(),
        })
    }

    /// Initialize the global menu system (macOS only).
    ///
    /// On macOS this attaches the menu to NSApp (the global application object),
    /// replacing winit's default menu. This should be called as early as possible
    /// — before any blocking GPU initialization — so that our custom accelerators
    /// (Cmd+, for Settings, Cmd+Q for graceful Quit) are active immediately.
    ///
    /// On other platforms this is a no-op; use [`Self::init_for_window`] to attach
    /// per-window menu bars.
    pub fn init_global(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            macos::init_for_nsapp(&self.menu)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Ok(())
        }
    }

    /// Initialize the menu for a window
    ///
    /// On macOS, this initializes the global application menu (only needs to be called once).
    /// On Windows/Linux, this attaches a menu bar to the specific window.
    #[allow(unused_variables)] // window is only used on Windows/Linux
    pub fn init_for_window(&self, window: &Arc<Window>) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            macos::init_for_nsapp(&self.menu)
        }

        #[cfg(target_os = "windows")]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = window.window_handle()
                && let RawWindowHandle::Win32(win32_handle) = handle.as_raw()
            {
                // SAFETY: We have a valid Win32 window handle from winit
                unsafe {
                    self.menu.init_for_hwnd(win32_handle.hwnd.get() as _)?;
                }
                log::info!("Initialized Windows menu bar for window");
            }
            return Ok(());
        }

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            linux::init_for_window(window)
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        )))]
        {
            log::warn!("Menu bar not supported on this platform");
            Ok(())
        }
    }

    /// Poll for menu events and return any triggered actions
    pub fn poll_events(&self) -> impl Iterator<Item = MenuAction> + '_ {
        std::iter::from_fn(|| {
            // Use try_recv to get events without blocking
            match MenuEvent::receiver().try_recv() {
                Ok(event) => self.action_map.get(&event.id).copied(),
                Err(_) => None,
            }
        })
    }

    /// Update the profiles submenu with the current list of profiles
    ///
    /// This should be called whenever profiles are loaded or modified.
    pub fn update_profiles(&mut self, profiles: &[&Profile]) {
        // Remove existing profile menu items
        for item in self.profile_menu_items.drain(..) {
            // Remove from action_map
            self.action_map.remove(item.id());
            // Remove from submenu
            let _ = self.profiles_submenu.remove(&item);
        }

        // Add new profile menu items in order. The entries come from the shared
        // model so the in-app menu lists the same profiles under the same labels.
        for entry in model::profile_entries(profiles.iter().copied()) {
            let item = MenuItem::with_id(entry.menu_id, &entry.label, true, None);

            self.action_map.insert(item.id().clone(), entry.action);

            // Add to submenu
            if let Err(e) = self.profiles_submenu.append(&item) {
                log::warn!("Failed to add profile menu item '{}': {}", entry.label, e);
                continue;
            }

            // Track for later removal
            self.profile_menu_items.push(item);
        }

        log::info!("Updated profiles menu with {} items", profiles.len());
    }

    /// Update profiles from a ProfileManager (convenience method)
    pub fn update_profiles_from_manager(&mut self, manager: &crate::profile::ProfileManager) {
        let profiles: Vec<&Profile> = manager.profiles_ordered();
        self.update_profiles(&profiles);
    }
}
