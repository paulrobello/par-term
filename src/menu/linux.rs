//! Linux menu initialization — currently a no-op, deliberately.
//!
//! par-term builds a full [`muda::Menu`] on every platform, but on Linux it is
//! never attached to anything, and this module is where that stops.
//!
//! muda attaches a menubar with `Menu::init_for_gtk_window<W, C>()`, whose
//! bounds are `W: IsA<gtk::Window> + IsA<gtk::Container>`. winit's X11 and
//! Wayland backends do not create GTK windows — they create a raw X11 window
//! or a Wayland surface directly — so there is no `gtk::Window` to hand it.
//! This is not winit failing to expose a handle: no GTK window exists to
//! expose. GTK would have to own the window from creation, which would mean
//! replacing the winit backend on Linux.
//!
//! The realistic ways to give Linux a menu are therefore:
//!
//! 1. Draw it in-app with egui, as the tab bar and settings window already
//!    are, driving the existing action map. Works on X11 and Wayland alike.
//! 2. Export a global menu over DBus (`com.canonical.dbusmenu`). Works on KDE
//!    Plasma, not on GNOME without an extension.
//! 3. Leave it. Most menu actions also have keybindings, so the menu is
//!    largely a discoverability aid — but four do not. `new_window`,
//!    `close_window`, `quit` and `select_all` exist only as menu items (they
//!    are absent from the bindable action list in
//!    `src/app/input_events/keybinding_actions.rs`), so on Linux they have no
//!    direct keyboard route at all. That makes this more than cosmetic.
//!
//! Until one of those lands, this function logs what it detected and returns
//! `Ok(())`. It must not claim to have initialized a menu — the previous
//! version logged "Linux menu bar initialized (GTK-based)", which was false.

use anyhow::Result;
use std::sync::Arc;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

/// Report that no menu bar is attached on Linux, and why.
///
/// Returns `Ok(())` because an absent menu bar is not a startup failure. It is
/// not harmless either: `new_window`, `close_window`, `quit` and `select_all`
/// are menu-only, so Linux has no direct keyboard route to them.
pub fn init_for_window(window: &Arc<Window>) -> Result<()> {
    let display_server = match window.window_handle().map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Xlib(_)) => "X11",
        Ok(RawWindowHandle::Wayland(_)) => "Wayland",
        Ok(_) => "an unrecognised display server",
        Err(_) => "an unavailable window handle",
    };

    log::info!(
        "No native menu bar on Linux ({display_server}): muda needs a gtk::Window and winit \
         creates none. Actions that have keybindings still work; new_window, close_window, \
         quit and select_all are menu-only and have no keyboard route here."
    );
    Ok(())
}
