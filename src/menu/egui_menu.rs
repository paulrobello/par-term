//! The in-app menu, drawn with egui.
//!
//! par-term cannot attach a native menu bar on Linux/BSD: muda attaches one
//! through `Menu::init_for_gtk_window`, and winit's X11 and Wayland backends
//! create no `gtk::Window` to hand it (see [`super::linux`]). Without a menu,
//! `new_window`, `close_window`, `quit` and `select_all` have no route at all —
//! they are menu-only commands. This module gives those platforms a menu drawn
//! the same way par-term already draws its tab bar and settings window.
//!
//! It renders [`super::model`] — the same description the native menu is built
//! from — so the two cannot offer different commands. Activations go to
//! [`super::bridge`], which `WindowManager::process_menu_events` drains
//! alongside muda's own event channel.
//!
//! The trigger button lives inside the tab bar strip, whose height is already
//! reserved in the terminal grid layout, so the menu costs the terminal no
//! rows. The drop-down itself is an egui popup floating above everything.

use super::bridge;
use super::model::{self, MenuEntry, MenuSection};
use crate::profile::ProfileManager;
use egui::containers::menu::{MenuButton, SubMenuButton};

/// Glyph on the trigger button.
const TRIGGER_GLYPH: &str = "\u{2630}";

/// Font size of the trigger glyph, in logical pixels.
const TRIGGER_GLYPH_SIZE: f32 = 13.0;

/// Minimum width of the top-level drop-down, in logical pixels.
const MENU_MIN_WIDTH: f32 = 120.0;

/// Minimum width of a section's submenu, in logical pixels.
const SUBMENU_MIN_WIDTH: f32 = 210.0;

/// Environment variable that overrides whether the in-app menu is drawn.
///
/// `1`/`true`/`on` force it on, `0`/`false`/`off` force it off. Unset, it is
/// drawn exactly where no native menu bar can be attached. Forcing it on is how
/// the menu is inspected from macOS or Windows, where it is normally redundant.
pub const ENABLE_ENV_VAR: &str = "PAR_TERM_IN_APP_MENU";

/// True on the platforms whose native menu bar par-term actually attaches.
const HAS_NATIVE_MENU_BAR: bool = cfg!(any(target_os = "macos", target_os = "windows"));

/// The in-app menu's per-window state.
pub struct AppMenuUi {
    /// The menu to draw. Built once per window; the contents are static apart
    /// from the profile list, which is read from the live `ProfileManager`.
    sections: Vec<MenuSection>,
    /// Whether the drop-down was open during the last frame that drew it.
    open: bool,
}

impl Default for AppMenuUi {
    fn default() -> Self {
        Self::new()
    }
}

impl AppMenuUi {
    /// Width the trigger button occupies in a horizontal bar, in logical pixels.
    pub const BUTTON_WIDTH: f32 = 24.0;

    /// Build the menu for one window.
    pub fn new() -> Self {
        Self {
            // The in-app menu is the only menu wherever it is drawn, so it must
            // carry the commands a native application menu would otherwise own.
            sections: model::menu_model(false),
            open: false,
        }
    }

    /// Whether the in-app menu should be drawn in this process.
    pub fn enabled() -> bool {
        use std::sync::OnceLock;
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED
            .get_or_init(|| enabled_with_override(std::env::var(ENABLE_ENV_VAR).ok().as_deref()))
    }

    /// Whether the drop-down is currently open.
    ///
    /// While it is, keyboard input should not reach the terminal.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Tear the menu down for a frame that will not draw it.
    ///
    /// The bar the menu lives in can be hidden while the drop-down is open, and
    /// two pieces of state outlive that: egui's popup memory, which would
    /// re-open the drop-down the moment the bar returns, and a toggle request
    /// that no [`Self::show`] will ever consume while the bar is hidden, which
    /// would do the same. Both are discarded here.
    pub fn hide(&mut self, ctx: &egui::Context) {
        // A `toggle_menu` keybinding cannot reach a menu that is not drawn.
        // Dropping the request is what stops it from latching.
        let _ = bridge::take_toggle_request();

        // Guarded: `Popup::close_all` closes every popup in the application,
        // and this runs on every frame the bar is hidden.
        if self.open {
            egui::Popup::close_all(ctx);
            self.open = false;
        }
    }

    /// Draw the trigger button, and the drop-down if it is open.
    ///
    /// `height` is the height of the bar the button sits in.
    pub fn show(&mut self, ui: &mut egui::Ui, profiles: &ProfileManager, height: f32) {
        let button = egui::Button::new(
            egui::RichText::new(TRIGGER_GLYPH)
                .size(TRIGGER_GLYPH_SIZE)
                .color(ui.visuals().weak_text_color()),
        )
        .min_size(egui::vec2(Self::BUTTON_WIDTH, height))
        .fill(egui::Color32::TRANSPARENT);

        let (response, inner) = MenuButton::from_button(button).ui(ui, |ui| {
            ui.set_min_width(MENU_MIN_WIDTH);
            for section in &self.sections {
                SubMenuButton::new(section.title).ui(ui, |ui| {
                    ui.set_min_width(SUBMENU_MIN_WIDTH);
                    section_entries(ui, section, profiles);
                });
            }
        });
        self.open = inner.is_some();

        // A `toggle_menu` keybinding cannot reach egui's popup memory directly,
        // so it leaves a request behind for this pass to apply. The popup state
        // was already read above, hence the repaint: the change lands next frame.
        if bridge::take_toggle_request() {
            egui::Popup::toggle_id(ui.ctx(), egui::Popup::default_response_id(&response));
            ui.ctx().request_repaint();
        }

        if response.hovered() {
            response.on_hover_text("Menu");
        }
    }
}

/// Draw one section's entries into an open submenu.
fn section_entries(ui: &mut egui::Ui, section: &MenuSection, profiles: &ProfileManager) {
    for entry in &section.entries {
        match entry {
            MenuEntry::Separator => {
                ui.separator();
            }
            MenuEntry::Item(spec) => {
                let mut button = egui::Button::new(spec.label);
                if let Some(accelerator) = &spec.accelerator {
                    button = button.right_text(model::accelerator_label(accelerator));
                }
                if ui.add(button).clicked() {
                    bridge::dispatch(spec.action);
                }
            }
            MenuEntry::Profiles => {
                for entry in model::profile_entries(profiles.profiles_ordered()) {
                    if ui.button(entry.label).clicked() {
                        bridge::dispatch(entry.action);
                    }
                }
            }
        }
    }
}

/// Resolve [`AppMenuUi::enabled`] from the raw environment variable value.
///
/// Split out so the precedence is testable on every platform.
fn enabled_with_override(value: Option<&str>) -> bool {
    match value.map(str::trim) {
        Some("1" | "true" | "on" | "yes") => true,
        Some("0" | "false" | "off" | "no") => false,
        // An unrecognised value is not a reason to change the platform default.
        _ => !HAS_NATIVE_MENU_BAR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_forces_the_menu_on_or_off() {
        assert!(enabled_with_override(Some("1")));
        assert!(enabled_with_override(Some("true")));
        assert!(!enabled_with_override(Some("0")));
        assert!(!enabled_with_override(Some("off")));
    }

    /// Unset, or set to nonsense, the platform decides.
    #[test]
    fn default_follows_the_platform() {
        let expected = !HAS_NATIVE_MENU_BAR;
        assert_eq!(enabled_with_override(None), expected);
        assert_eq!(enabled_with_override(Some("maybe")), expected);
        assert_eq!(enabled_with_override(Some("")), expected);
    }

    /// The menu is drawn exactly where par-term cannot attach a native one.
    #[test]
    fn platform_default_matches_native_menu_availability() {
        if cfg!(any(target_os = "macos", target_os = "windows")) {
            assert!(!enabled_with_override(None));
        } else {
            assert!(enabled_with_override(None));
        }
    }

    /// Every section of the model the in-app menu draws must be reachable as a
    /// submenu, and the model must be the no-native-app-menu variant.
    #[test]
    fn menu_carries_the_full_command_set() {
        let menu = AppMenuUi::new();
        let titles: Vec<&str> = menu.sections.iter().map(|s| s.title).collect();
        assert!(titles.contains(&"File"));
        assert!(titles.contains(&"Edit"));
        assert!(titles.contains(&model::HELP_SECTION_TITLE));

        let actions: Vec<crate::menu::MenuAction> = menu
            .sections
            .iter()
            .flat_map(|section| &section.entries)
            .filter_map(|entry| match entry {
                MenuEntry::Item(spec) => Some(spec.action),
                _ => None,
            })
            .collect();
        for required in [
            crate::menu::MenuAction::NewWindow,
            crate::menu::MenuAction::CloseWindow,
            crate::menu::MenuAction::Quit,
            crate::menu::MenuAction::SelectAll,
            crate::menu::MenuAction::MaximizeVertically,
        ] {
            assert!(
                actions.contains(&required),
                "in-app menu is missing {required:?}, which has no keybinding on Linux"
            );
        }
    }

    /// A freshly built menu must not claim to be capturing input.
    #[test]
    fn menu_starts_closed() {
        assert!(!AppMenuUi::new().is_open());
    }

    /// Every section's entries must render — separators, items with and
    /// without an accelerator, and the profiles insertion point.
    ///
    /// Submenu bodies only open on hover, which
    /// `opens_and_closes_through_the_toggle_request` cannot drive, so they are
    /// exercised directly here.
    #[test]
    fn every_section_renders_its_entries() {
        let menu = AppMenuUi::new();
        let profiles = crate::profile::ProfileManager::new();
        egui::__run_test_ui(|ui| {
            for section in &menu.sections {
                section_entries(ui, section, &profiles);
            }
        });
    }

    /// Drive the real egui code path headlessly: closed, then opened through
    /// the same toggle request a `toggle_menu` keybinding would leave behind,
    /// then closed again. Covers the trigger button and the drop-down's
    /// top level.
    #[test]
    fn opens_and_closes_through_the_toggle_request() {
        let _guard = super::bridge::TEST_LOCK.lock();
        let _ = bridge::take_toggle_request();
        let _ = bridge::drain_pending_actions();

        let ctx = egui::Context::default();
        let profiles = crate::profile::ProfileManager::new();
        let mut menu = AppMenuUi::new();

        let frame = |menu: &mut AppMenuUi| {
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                egui::Panel::top("test_bar").show(ui, |ui| {
                    menu.show(ui, &profiles, 24.0);
                });
            });
        };

        frame(&mut menu);
        assert!(!menu.is_open(), "the menu must start closed");

        // The request is consumed after this frame reads the popup state, so
        // the drop-down appears on the frame after that.
        bridge::request_toggle();
        frame(&mut menu);
        frame(&mut menu);
        assert!(menu.is_open(), "toggle request did not open the menu");

        bridge::request_toggle();
        frame(&mut menu);
        frame(&mut menu);
        assert!(!menu.is_open(), "toggle request did not close the menu");

        // Drawing the menu must not dispatch anything on its own.
        assert!(bridge::drain_pending_actions().is_empty());
    }

    /// Hiding the bar must leave nothing behind that re-opens the menu later.
    ///
    /// Both halves of this regressed once: egui's popup memory kept the
    /// drop-down open across the hidden frames, and a toggle request raised
    /// while the bar was hidden latched until the bar came back.
    #[test]
    fn hiding_the_bar_discards_open_state_and_toggle_requests() {
        let _guard = super::bridge::TEST_LOCK.lock();
        let _ = bridge::take_toggle_request();

        let ctx = egui::Context::default();
        let profiles = crate::profile::ProfileManager::new();
        let mut menu = AppMenuUi::new();

        let frame = |menu: &mut AppMenuUi| {
            let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                egui::Panel::top("test_bar").show(ui, |ui| {
                    menu.show(ui, &profiles, 24.0);
                });
            });
        };

        bridge::request_toggle();
        frame(&mut menu);
        frame(&mut menu);
        assert!(menu.is_open());

        // The bar is hidden while the drop-down is open.
        menu.hide(&ctx);
        assert!(!menu.is_open());

        // And a keybinding fires while there is no menu to toggle.
        bridge::request_toggle();
        menu.hide(&ctx);
        assert!(!bridge::take_toggle_request(), "toggle request latched");

        // The bar comes back: the menu must still be closed.
        frame(&mut menu);
        frame(&mut menu);
        assert!(
            !menu.is_open(),
            "the menu re-opened itself after being hidden"
        );
    }
}
