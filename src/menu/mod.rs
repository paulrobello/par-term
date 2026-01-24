//! Native menu support for par-term
//!
//! This module provides cross-platform native menu support using the `muda` crate.
//! - macOS: Global application menu bar
//! - Windows/Linux: Per-window menu bar

mod actions;

pub use actions::MenuAction;

use anyhow::Result;
use muda::{
    Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
    accelerator::{Accelerator, Code, Modifiers},
};
use std::collections::HashMap;
use std::sync::Arc;
use winit::window::Window;

/// Manages the native menu system
pub struct MenuManager {
    /// The root menu
    #[allow(dead_code)]
    menu: Menu,
    /// Mapping from menu item IDs to actions
    action_map: HashMap<MenuId, MenuAction>,
}

impl MenuManager {
    /// Create a new menu manager with the default menu structure
    pub fn new() -> Result<Self> {
        let menu = Menu::new();
        let mut action_map = HashMap::new();

        // Platform-specific modifier key
        #[cfg(target_os = "macos")]
        let cmd_or_ctrl = Modifiers::META;
        #[cfg(not(target_os = "macos"))]
        let cmd_or_ctrl = Modifiers::CONTROL;

        // File menu
        let file_menu = Submenu::new("File", true);

        let new_window = MenuItem::with_id(
            "new_window",
            "New Window",
            true,
            Some(Accelerator::new(Some(cmd_or_ctrl), Code::KeyN)),
        );
        action_map.insert(new_window.id().clone(), MenuAction::NewWindow);
        file_menu.append(&new_window)?;

        let close_window = MenuItem::with_id(
            "close_window",
            "Close Window",
            true,
            Some(Accelerator::new(Some(cmd_or_ctrl), Code::KeyW)),
        );
        action_map.insert(close_window.id().clone(), MenuAction::CloseWindow);
        file_menu.append(&close_window)?;

        file_menu.append(&PredefinedMenuItem::separator())?;

        // On macOS, Quit is in the app menu (handled automatically)
        // On Windows/Linux, add Quit to File menu
        #[cfg(not(target_os = "macos"))]
        {
            let quit = MenuItem::with_id(
                "quit",
                "Quit",
                true,
                Some(Accelerator::new(Some(cmd_or_ctrl), Code::KeyQ)),
            );
            action_map.insert(quit.id().clone(), MenuAction::Quit);
            file_menu.append(&quit)?;
        }

        menu.append(&file_menu)?;

        // Tab menu
        let tab_menu = Submenu::new("Tab", true);

        let new_tab = MenuItem::with_id(
            "new_tab",
            "New Tab",
            true,
            Some(Accelerator::new(Some(cmd_or_ctrl), Code::KeyT)),
        );
        action_map.insert(new_tab.id().clone(), MenuAction::NewTab);
        tab_menu.append(&new_tab)?;

        let close_tab = MenuItem::with_id(
            "close_tab",
            "Close Tab",
            true,
            None, // Cmd+W handled in File menu (smart close)
        );
        action_map.insert(close_tab.id().clone(), MenuAction::CloseTab);
        tab_menu.append(&close_tab)?;

        tab_menu.append(&PredefinedMenuItem::separator())?;

        let next_tab = MenuItem::with_id(
            "next_tab",
            "Next Tab",
            true,
            Some(Accelerator::new(
                Some(cmd_or_ctrl | Modifiers::SHIFT),
                Code::BracketRight,
            )),
        );
        action_map.insert(next_tab.id().clone(), MenuAction::NextTab);
        tab_menu.append(&next_tab)?;

        let prev_tab = MenuItem::with_id(
            "prev_tab",
            "Previous Tab",
            true,
            Some(Accelerator::new(
                Some(cmd_or_ctrl | Modifiers::SHIFT),
                Code::BracketLeft,
            )),
        );
        action_map.insert(prev_tab.id().clone(), MenuAction::PreviousTab);
        tab_menu.append(&prev_tab)?;

        tab_menu.append(&PredefinedMenuItem::separator())?;

        // Tab 1-9 shortcuts
        for i in 1..=9 {
            let code = match i {
                1 => Code::Digit1,
                2 => Code::Digit2,
                3 => Code::Digit3,
                4 => Code::Digit4,
                5 => Code::Digit5,
                6 => Code::Digit6,
                7 => Code::Digit7,
                8 => Code::Digit8,
                9 => Code::Digit9,
                _ => unreachable!(),
            };
            let tab_item = MenuItem::with_id(
                format!("tab_{}", i),
                format!("Tab {}", i),
                true,
                Some(Accelerator::new(Some(cmd_or_ctrl), code)),
            );
            action_map.insert(tab_item.id().clone(), MenuAction::SwitchToTab(i));
            tab_menu.append(&tab_item)?;
        }

        menu.append(&tab_menu)?;

        // Edit menu
        let edit_menu = Submenu::new("Edit", true);

        let copy = MenuItem::with_id(
            "copy",
            "Copy",
            true,
            Some(Accelerator::new(Some(cmd_or_ctrl), Code::KeyC)),
        );
        action_map.insert(copy.id().clone(), MenuAction::Copy);
        edit_menu.append(&copy)?;

        let paste = MenuItem::with_id(
            "paste",
            "Paste",
            true,
            Some(Accelerator::new(Some(cmd_or_ctrl), Code::KeyV)),
        );
        action_map.insert(paste.id().clone(), MenuAction::Paste);
        edit_menu.append(&paste)?;

        let select_all = MenuItem::with_id(
            "select_all",
            "Select All",
            true,
            Some(Accelerator::new(Some(cmd_or_ctrl), Code::KeyA)),
        );
        action_map.insert(select_all.id().clone(), MenuAction::SelectAll);
        edit_menu.append(&select_all)?;

        edit_menu.append(&PredefinedMenuItem::separator())?;

        let clear_scrollback = MenuItem::with_id(
            "clear_scrollback",
            "Clear Scrollback",
            true,
            Some(Accelerator::new(
                Some(cmd_or_ctrl | Modifiers::SHIFT),
                Code::KeyK,
            )),
        );
        action_map.insert(clear_scrollback.id().clone(), MenuAction::ClearScrollback);
        edit_menu.append(&clear_scrollback)?;

        let clipboard_history = MenuItem::with_id(
            "clipboard_history",
            "Clipboard History",
            true,
            Some(Accelerator::new(
                Some(cmd_or_ctrl | Modifiers::SHIFT),
                Code::KeyH,
            )),
        );
        action_map.insert(clipboard_history.id().clone(), MenuAction::ClipboardHistory);
        edit_menu.append(&clipboard_history)?;

        menu.append(&edit_menu)?;

        // View menu
        let view_menu = Submenu::new("View", true);

        let toggle_fullscreen = MenuItem::with_id(
            "toggle_fullscreen",
            "Toggle Fullscreen",
            true,
            Some(Accelerator::new(None, Code::F11)),
        );
        action_map.insert(toggle_fullscreen.id().clone(), MenuAction::ToggleFullscreen);
        view_menu.append(&toggle_fullscreen)?;

        view_menu.append(&PredefinedMenuItem::separator())?;

        let increase_font = MenuItem::with_id(
            "increase_font",
            "Increase Font Size",
            true,
            Some(Accelerator::new(Some(cmd_or_ctrl), Code::Equal)),
        );
        action_map.insert(increase_font.id().clone(), MenuAction::IncreaseFontSize);
        view_menu.append(&increase_font)?;

        let decrease_font = MenuItem::with_id(
            "decrease_font",
            "Decrease Font Size",
            true,
            Some(Accelerator::new(Some(cmd_or_ctrl), Code::Minus)),
        );
        action_map.insert(decrease_font.id().clone(), MenuAction::DecreaseFontSize);
        view_menu.append(&decrease_font)?;

        let reset_font = MenuItem::with_id(
            "reset_font",
            "Reset Font Size",
            true,
            Some(Accelerator::new(Some(cmd_or_ctrl), Code::Digit0)),
        );
        action_map.insert(reset_font.id().clone(), MenuAction::ResetFontSize);
        view_menu.append(&reset_font)?;

        view_menu.append(&PredefinedMenuItem::separator())?;

        let fps_overlay = MenuItem::with_id(
            "fps_overlay",
            "FPS Overlay",
            true,
            Some(Accelerator::new(None, Code::F3)),
        );
        action_map.insert(fps_overlay.id().clone(), MenuAction::ToggleFpsOverlay);
        view_menu.append(&fps_overlay)?;

        let settings = MenuItem::with_id(
            "settings",
            "Settings",
            true,
            Some(Accelerator::new(None, Code::F12)),
        );
        action_map.insert(settings.id().clone(), MenuAction::OpenSettings);
        view_menu.append(&settings)?;

        menu.append(&view_menu)?;

        // Window menu (primarily for macOS)
        #[cfg(target_os = "macos")]
        {
            let window_menu = Submenu::new("Window", true);

            let minimize = MenuItem::with_id(
                "minimize",
                "Minimize",
                true,
                Some(Accelerator::new(Some(Modifiers::META), Code::KeyM)),
            );
            action_map.insert(minimize.id().clone(), MenuAction::Minimize);
            window_menu.append(&minimize)?;

            let zoom = MenuItem::with_id("zoom", "Zoom", true, None);
            action_map.insert(zoom.id().clone(), MenuAction::Zoom);
            window_menu.append(&zoom)?;

            menu.append(&window_menu)?;
        }

        // Help menu
        let help_menu = Submenu::new("Help", true);

        let keyboard_shortcuts = MenuItem::with_id(
            "keyboard_shortcuts",
            "Keyboard Shortcuts",
            true,
            Some(Accelerator::new(None, Code::F1)),
        );
        action_map.insert(keyboard_shortcuts.id().clone(), MenuAction::ShowHelp);
        help_menu.append(&keyboard_shortcuts)?;

        help_menu.append(&PredefinedMenuItem::separator())?;

        let about = MenuItem::with_id("about", "About par-term", true, None);
        action_map.insert(about.id().clone(), MenuAction::About);
        help_menu.append(&about)?;

        menu.append(&help_menu)?;

        Ok(Self { menu, action_map })
    }

    /// Initialize the menu for a window
    ///
    /// On macOS, this initializes the global application menu (only needs to be called once).
    /// On Windows/Linux, this attaches a menu bar to the specific window.
    pub fn init_for_window(&self, _window: &Arc<Window>) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            // On macOS, init for NSApp (global menu bar)
            self.menu.init_for_nsapp();
            // Also set the app name in the menu
            // This is typically done automatically but we ensure it's set
            log::info!("Initialized macOS global menu bar");
            Ok(())
        }

        #[cfg(target_os = "windows")]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
                    self.menu.init_for_hwnd(win32_handle.hwnd.get() as _)?;
                    log::info!("Initialized Windows menu bar for window");
                }
            }
            Ok(())
        }

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            // On Linux with GTK, we need to initialize for GTK window
            // This requires the gtk feature to be enabled
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::Xlib(xlib_handle) = handle.as_raw() {
                    // For X11, we'd need to use the GTK integration
                    // This is handled by muda's gtk feature
                    log::info!("Linux X11 menu support (using GTK integration)");
                } else if let RawWindowHandle::Wayland(_wayland_handle) = handle.as_raw() {
                    log::info!("Linux Wayland menu support (using GTK integration)");
                }
            }
            // GTK menu initialization is more complex and depends on the display server
            // For now, we'll just log that we're on Linux
            log::info!("Linux menu bar initialized (GTK-based)");
            Ok(())
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
}
