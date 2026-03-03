//! macOS-specific menu initialization and setup.
//!
//! This module contains code that is only compiled on macOS and handles:
//! - Global application menu bar initialization via NSApp
//! - The macOS "app menu" (About, Settings, Services, Hide, Quit)
//! - The macOS "Window" menu (Minimize, Zoom)

use anyhow::Result;
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};

use super::actions::MenuAction;
use std::collections::HashMap;

/// Initialize the macOS global application menu bar.
///
/// On macOS the menu is attached to NSApp (the global application object),
/// not to individual windows. This only needs to be called once.
pub fn init_for_nsapp(menu: &Menu) -> Result<()> {
    menu.init_for_nsapp();
    log::info!("Initialized macOS global menu bar");
    Ok(())
}

/// Build and append the macOS application menu (the first submenu, which becomes
/// the "par-term" application menu in the macOS menu bar).
///
/// This menu contains: About, separator, Settings, separator, Services,
/// separator, Hide/Hide Others/Show All, separator, Quit.
///
/// Returns `Ok(())` on success.
pub fn build_app_menu(menu: &Menu, action_map: &mut HashMap<MenuId, MenuAction>) -> Result<()> {
    let app_menu = Submenu::new("par-term", true);

    // About par-term
    let about_app = MenuItem::with_id("about_app", "About par-term", true, None);
    action_map.insert(about_app.id().clone(), MenuAction::About);
    app_menu.append(&about_app)?;

    app_menu.append(&PredefinedMenuItem::separator())?;

    // Settings... (Cmd+,) — standard macOS settings shortcut
    let settings_app = MenuItem::with_id(
        "settings_app",
        "Settings...",
        true,
        Some(Accelerator::new(Some(Modifiers::META), Code::Comma)),
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

    // Use a custom MenuItem instead of PredefinedMenuItem::quit(None) because
    // the predefined Quit directly calls [NSApp terminate:] which invokes
    // exit(0), bypassing all Rust cleanup (Drop impls, shutdown logic, etc.).
    // A custom MenuItem fires through muda's MenuEvent channel, allowing our
    // MenuAction::Quit handler to perform graceful shutdown.
    let quit_app = MenuItem::with_id(
        "quit_app",
        "Quit par-term",
        true,
        Some(Accelerator::new(Some(Modifiers::META), Code::KeyQ)),
    );
    action_map.insert(quit_app.id().clone(), MenuAction::Quit);
    app_menu.append(&quit_app)?;

    menu.append(&app_menu)?;
    Ok(())
}

/// Build and append the macOS Window menu (Minimize, Zoom).
///
/// The Window menu is a macOS convention that is not present on other platforms.
pub fn build_window_menu(menu: &Menu, action_map: &mut HashMap<MenuId, MenuAction>) -> Result<()> {
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
    Ok(())
}
