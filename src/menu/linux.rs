//! Linux menu initialization — no *native* menu bar, by necessity.
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
//! Three ways to give Linux a menu were considered:
//!
//! 1. Draw it in-app with egui, as the tab bar and settings window already
//!    are, driving the existing action map. Works on X11 and Wayland alike.
//! 2. Export a global menu over DBus (`com.canonical.dbusmenu`). Works on KDE
//!    Plasma, not on GNOME without an extension.
//! 3. Leave it. Not viable: `new_window`, `close_window`, `quit` and
//!    `select_all` are absent from the bindable action list in
//!    `src/app/input_events/keybinding_actions.rs`, so with no menu they have
//!    no direct route at all. `maximize_vertically` is menu-only too.
//!
//! **Option 1 is what par-term does.** [`super::egui_menu::AppMenuUi`] draws
//! the menu from [`super::model`] — the same description the native menu bar is
//! built from — inside the tab bar strip, and its activations are dispatched
//! through the same [`super::MenuAction`] handler. So the commands above are
//! reachable here; what Linux lacks is only the *native* bar.
//!
//! This function therefore has nothing left to attach. It logs what it detected
//! and returns `Ok(())`. It must not claim to have initialized a native menu —
//! an old version logged "Linux menu bar initialized (GTK-based)", which was
//! false.

use anyhow::Result;
use std::sync::Arc;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

/// Report that no *native* menu bar is attached on Linux, and why.
///
/// Returns `Ok(())` because an absent native menu bar is not a startup failure,
/// and no longer costs the user any commands: the in-app egui menu carries
/// them. See the module docs.
pub fn init_for_window(window: &Arc<Window>) -> Result<()> {
    let display_server = match window.window_handle().map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Xlib(_)) => "X11",
        Ok(RawWindowHandle::Wayland(_)) => "Wayland",
        Ok(_) => "an unrecognised display server",
        Err(_) => "an unavailable window handle",
    };

    log::info!(
        "No native menu bar on Linux ({display_server}): muda needs a gtk::Window and winit \
         creates none. The in-app menu is drawn in the tab bar instead ({} here); it offers the \
         same commands, including new_window, close_window, quit and select_all, which have no \
         keybinding.",
        if super::AppMenuUi::enabled() {
            "enabled"
        } else {
            "disabled via PAR_TERM_IN_APP_MENU"
        }
    );
    Ok(())
}
