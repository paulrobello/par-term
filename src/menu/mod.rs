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
//! # Accelerators are display-only off macOS
//!
//! Only macOS dispatches from menu accelerators (AppKit matches key
//! equivalents before winit sees the key). On Windows muda draws the
//! accelerator text, but accelerators only fire when the Win32 message loop
//! calls `TranslateAcceleratorW` with the menu's accelerator table, and
//! par-term never does (winit owns the loop). On Linux the egui menu only
//! draws labels. Everywhere except macOS the keybinding registry is the sole
//! key dispatcher, and the menu's chord text is a hint that mirrors it.
//! Verified at runtime on Windows 11 (card 01a0ef69fb). This applies equally
//! to the fixed Copy and Paste chords: they are inert menu text on Windows,
//! harmless because the key handler's own copy/paste branches run.
//!
//! Both renderers walk the same model and apply the same [`state::MenuState`],
//! so neither platform can end up with commands, enabled items, or checkmarks
//! the other lacks. Activations from either arrive as [`MenuAction`]s at
//! `WindowManager::process_menu_events`.
//!
//! Both are rebuilt from the live keybinding registry whenever the bindings
//! change (UX.md MN3); [`sync::MenuSync`] decides when.

mod actions;
mod bridge;
pub mod egui_menu;
pub mod model;
mod model_sections;
mod model_window;
mod native;
pub(crate) mod registry_accel;
pub mod state;
pub mod sync;

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

/// Serialise a test that touches the menu bridge's process-global queues.
#[cfg(test)]
pub(crate) fn bridge_test_lock() -> std::sync::MutexGuard<'static, ()> {
    bridge::TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

use crate::profile::Profile;
use anyhow::Result;
use muda::MenuEvent;
use native::NativeMenu;
use par_term_config::KeyBinding;
use par_term_keybindings::KeybindingRegistry;
use state::MenuState;
use std::sync::Arc;
use winit::window::Window;

/// Manages the native menu system
pub struct MenuManager {
    /// The current build. Replaced whole by [`Self::rebuild`].
    native: NativeMenu,
    /// What `native` was built from (UX.md MN3).
    sync: sync::MenuSync,
    /// The profile list last handed to [`Self::update_profiles`], re-applied
    /// after a rebuild.
    profiles: Vec<Profile>,
}

impl MenuManager {
    /// Build the menu with accelerators from `keybindings` (the live config's
    /// bindings, so the menu bar shows the user's chords).
    pub fn new_with(keybindings: &[KeyBinding]) -> Result<Self> {
        let mut sync = sync::MenuSync::new();
        sync.record(keybindings, None);
        Ok(Self {
            native: build_native(keybindings, &None)?,
            sync,
            profiles: Vec::new(),
        })
    }

    /// Why the menu must be rebuilt for `keybindings` and `capture`, or
    /// `None` when it is current.
    pub fn rebuild_reason(
        &self,
        keybindings: &[KeyBinding],
        capture: &state::Capture,
    ) -> Option<sync::RebuildReason> {
        self.sync.rebuild_reason(keybindings, capture)
    }

    /// Rebuild the whole menu from `keybindings` and attach it to `windows`
    /// (UX.md MN3): a rebound chord moves, an unbound one is released.
    /// `capture` drops the accelerators a focused dialog or text field must
    /// receive (see [`state::release_captured_accelerators`]).
    ///
    /// The old menu is dropped before the new one is attached: on Windows
    /// dropping a menu detaches it from every window it was set on, which
    /// would blank the replacement if it ran second.
    pub fn rebuild(
        &mut self,
        keybindings: &[KeyBinding],
        capture: state::Capture,
        windows: &[Arc<Window>],
    ) -> Result<()> {
        let fresh = build_native(keybindings, &capture)?;
        let old = std::mem::replace(&mut self.native, fresh);
        drop(old);
        let release = capture.is_some();
        self.sync.record(keybindings, capture);
        let profiles: Vec<Profile> = std::mem::take(&mut self.profiles);
        self.update_profiles(&profiles.iter().collect::<Vec<_>>());
        self.init_global()?;
        #[cfg(not(target_os = "macos"))]
        for window in windows {
            self.init_for_window(window)?;
        }
        #[cfg(target_os = "macos")]
        let _ = windows;
        log::info!(
            "Menu rebuilt from {} keybindings{}",
            keybindings.len(),
            if release {
                " (captured-key accelerators released)"
            } else {
                ""
            }
        );
        Ok(())
    }

    /// Initialize the global menu system (macOS only).
    ///
    /// On macOS this attaches the menu to NSApp (the global application object),
    /// replacing winit's default menu, and registers the Window and Help menus.
    /// This should be called as early as possible — before any blocking GPU
    /// initialization — so that our accelerators (Cmd+, for Settings, Cmd+Q
    /// for graceful Quit) are active immediately.
    ///
    /// On other platforms this is a no-op; use [`Self::init_for_window`] to attach
    /// per-window menu bars.
    pub fn init_global(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        macos::init_for_nsapp(
            &self.native.menu,
            self.native.window_menu.as_ref(),
            self.native.help_menu.as_ref(),
        );
        Ok(())
    }

    /// Initialize the menu for a window
    ///
    /// On macOS, this initializes the global application menu (only needs to be called once).
    /// On Windows/Linux, this attaches a menu bar to the specific window.
    #[allow(unused_variables)] // window is only used on Windows/Linux
    pub fn init_for_window(&self, window: &Arc<Window>) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            self.init_global()
        }

        #[cfg(target_os = "windows")]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = window.window_handle()
                && let RawWindowHandle::Win32(win32_handle) = handle.as_raw()
            {
                // SAFETY: We have a valid Win32 window handle from winit
                unsafe {
                    self.native
                        .menu
                        .init_for_hwnd(win32_handle.hwnd.get() as _)?;
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
    ///
    /// Any activation invalidates the applied state: a clicked toggle flips
    /// its own checkmark, and the next sync must write the real state back.
    pub fn poll_events(&mut self) -> Vec<MenuAction> {
        let mut actions = Vec::new();
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if let Some(action) = self.native.action_map.get(&event.id) {
                actions.push(*action);
            }
        }
        if !actions.is_empty() {
            self.native.invalidate();
        }
        actions
    }

    /// Apply the focused window's state (enabled, checked, labels, the
    /// arrangement list). Cheap when nothing changed.
    pub fn apply_state(&mut self, state: &MenuState) {
        self.native.apply(state);
    }

    /// Update the profiles submenu with the current list of profiles
    ///
    /// This should be called whenever profiles are loaded or modified.
    pub fn update_profiles(&mut self, profiles: &[&Profile]) {
        self.native.set_profiles(profiles);
        self.profiles = profiles.iter().map(|p| (*p).clone()).collect();
        log::info!("Updated profiles menu with {} items", profiles.len());
    }

    /// Update profiles from a ProfileManager (convenience method)
    pub fn update_profiles_from_manager(&mut self, manager: &crate::profile::ProfileManager) {
        let profiles: Vec<&Profile> = manager.profiles_ordered();
        self.update_profiles(&profiles);
    }
}

/// Build the native menu for this platform from `keybindings`.
fn build_native(keybindings: &[KeyBinding], capture: &state::Capture) -> Result<NativeMenu> {
    let registry = KeybindingRegistry::from_config(keybindings);
    let mut sections = model::menu_model_with_registry(cfg!(target_os = "macos"), &registry);
    if let Some(open) = capture {
        state::release_captured_accelerators(&mut sections, open);
    }
    let quit = registry
        .chord_for_action("quit")
        .and_then(|combo| registry_accel::accelerator_from_combo(&combo));
    NativeMenu::build(&sections, quit, registry, capture.is_some())
}
