//! macOS-specific menu initialization and setup.
//!
//! This module contains code that is only compiled on macOS and handles:
//! - Global application menu bar initialization via NSApp
//! - The macOS "app menu" (About, Settings, Services, Hide, Quit)
//! - Registering the Window and Help sections with NSApp

use anyhow::Result;
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};

use super::actions::MenuAction;
use std::collections::HashMap;

/// Attach the menu to NSApp and register its Window and Help sections.
///
/// The registration must follow `init_for_nsapp`: muda resolves the submenu
/// through NSApp's current main menu, so registering earlier is a silent
/// no-op (the P4 Window list never appeared for that reason). AppKit then
/// appends the open-window list to the Window menu and the search field to
/// Help.
pub fn init_for_nsapp(menu: &Menu, window_menu: Option<&Submenu>, help_menu: Option<&Submenu>) {
    menu.init_for_nsapp();
    if let Some(window_menu) = window_menu {
        window_menu.set_as_windows_menu_for_nsapp();
    }
    if let Some(help_menu) = help_menu {
        help_menu.set_as_help_menu_for_nsapp();
    }
    log::info!("Initialized macOS global menu bar");
}

/// Build and append the macOS application menu (the first submenu, which becomes
/// the "par-term" application menu in the macOS menu bar).
///
/// `quit_accelerator` is the `quit` action's registry chord, so unbinding
/// `quit` releases Cmd+Q. Settings keeps the platform's Cmd+, (the
/// registry's `open_settings` chord, F12, is shown by the Windows and in-app
/// Settings items).
pub fn build_app_menu(
    menu: &Menu,
    action_map: &mut HashMap<MenuId, MenuAction>,
    quit_accelerator: Option<Accelerator>,
) -> Result<()> {
    let app_menu = Submenu::new("par-term", true);

    let about_app = MenuItem::with_id("about_app", "About par-term", true, None);
    action_map.insert(about_app.id().clone(), MenuAction::About);
    app_menu.append(&about_app)?;

    app_menu.append(&PredefinedMenuItem::separator())?;

    let settings_app = MenuItem::with_id(
        "settings_app",
        "Settings...",
        true,
        Some(Accelerator::new(Modifiers::META, Code::Comma)),
    );
    action_map.insert(settings_app.id().clone(), MenuAction::OpenSettings);
    app_menu.append(&settings_app)?;

    app_menu.append(&PredefinedMenuItem::separator())?;
    app_menu.append(&PredefinedMenuItem::services(None))?;
    app_menu.append(&PredefinedMenuItem::separator())?;
    app_menu.append(&PredefinedMenuItem::hide(None))?;
    app_menu.append(&PredefinedMenuItem::hide_others(None))?;
    app_menu.append(&PredefinedMenuItem::show_all(None))?;
    app_menu.append(&PredefinedMenuItem::separator())?;

    // A custom MenuItem instead of PredefinedMenuItem::quit(None): the
    // predefined Quit calls [NSApp terminate:], which exits without running
    // Rust cleanup. This one fires through muda's event channel so
    // MenuAction::Quit can shut down gracefully.
    let quit_app = MenuItem::with_id("quit_app", "Quit par-term", true, quit_accelerator);
    action_map.insert(quit_app.id().clone(), MenuAction::Quit);
    app_menu.append(&quit_app)?;

    menu.append(&app_menu)?;
    Ok(())
}
