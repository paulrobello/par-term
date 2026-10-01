//! The in-app menu, drawn with egui.
//!
//! par-term cannot attach a native menu bar on Linux/BSD: muda attaches one
//! through `Menu::init_for_gtk_window`, and winit's X11 and Wayland backends
//! create no `gtk::Window` to hand it (see [`super::linux`]). This module
//! gives those platforms a menu drawn the same way par-term already draws its
//! tab bar and settings window.
//!
//! It renders [`super::model`] — the same description the native menu is built
//! from — with the same [`MenuState`], so the two cannot offer different
//! commands, enabled items, or checkmarks. Activations go to
//! [`super::bridge`], which `WindowManager::process_menu_events` drains
//! alongside muda's own event channel.
//!
//! The trigger button lives inside the tab bar strip, whose height is already
//! reserved in the terminal grid layout, so the menu costs the terminal no
//! rows. The drop-down itself is an egui popup floating above everything.

use super::bridge;
use super::model::{self, MenuEntry, MenuItemSpec, MenuSection};
use super::state::MenuState;
use super::sync::MenuSync;
use crate::profile::ProfileManager;
use egui::containers::menu::{MenuButton, SubMenuButton};
use par_term_config::KeyBinding;

/// Glyph on the trigger button.
const TRIGGER_GLYPH: &str = "\u{2630}";

/// Font size of the trigger glyph, in logical pixels.
const TRIGGER_GLYPH_SIZE: f32 = 13.0;

/// Minimum width of the top-level drop-down, in logical pixels.
const MENU_MIN_WIDTH: f32 = 120.0;

/// Minimum width of a section's submenu, in logical pixels.
const SUBMENU_MIN_WIDTH: f32 = 230.0;

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
    /// The menu to draw, rebuilt by [`Self::sync`] when the bindings change.
    sections: Vec<MenuSection>,
    /// What `sections` was built from (UX.md MN3).
    built: MenuSync,
    /// The window's state, applied when drawing (UX.md MN2).
    state: MenuState,
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

    /// Build the menu for one window, sourcing accelerators from the live
    /// config's keybindings so a rebind shows here too.
    pub fn new_with(keybindings: &[KeyBinding]) -> Self {
        let mut built = MenuSync::new();
        built.record(keybindings, None);
        Self {
            // The in-app menu is the only menu wherever it is drawn, so it must
            // carry the commands a native application menu would otherwise own.
            sections: model::menu_model_with(false, keybindings),
            built,
            state: MenuState::default(),
            open: false,
        }
    }

    /// Build the menu for one window from the default bindings.
    pub fn new() -> Self {
        Self::new_with(&par_term_config::Config::default().keybindings)
    }

    /// Rebuild from `keybindings` when they differ from the ones the menu was
    /// built from (UX.md MN3), and take the window's current `state`.
    /// Returns whether a rebuild happened.
    pub fn sync(&mut self, keybindings: &[KeyBinding], state: MenuState) -> bool {
        // The in-app menu registers no key equivalents (its chords are
        // labels; the registry dispatches them), so it never releases any.
        let rebuilt = self.built.rebuild_reason(keybindings, &None).is_some();
        if rebuilt {
            self.sections = model::menu_model_with(false, keybindings);
            self.built.record(keybindings, None);
        }
        self.state = state;
        rebuilt
    }

    /// The sections this menu draws.
    pub fn sections(&self) -> &[MenuSection] {
        &self.sections
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
                    draw_entries(ui, &section.entries, &self.state, profiles);
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

/// Draw entries into an open (sub)menu, applying `state`.
fn draw_entries(
    ui: &mut egui::Ui,
    entries: &[MenuEntry],
    state: &MenuState,
    profiles: &ProfileManager,
) {
    for entry in entries {
        match entry {
            MenuEntry::Separator => {
                ui.separator();
            }
            MenuEntry::Item(spec) => draw_item(ui, spec, state),
            MenuEntry::Submenu(sub) => {
                ui.add_enabled_ui(state.satisfies(sub.requires), |ui| {
                    SubMenuButton::new(sub.title).ui(ui, |ui| {
                        ui.set_min_width(SUBMENU_MIN_WIDTH);
                        draw_entries(ui, &sub.entries, state, profiles);
                    });
                });
            }
            MenuEntry::Profiles => {
                let entries = model::profile_entries(profiles.profiles_ordered());
                draw_dynamic(ui, &entries, state);
                if !entries.is_empty() {
                    ui.separator();
                }
            }
            MenuEntry::Arrangements => {
                draw_dynamic(
                    ui,
                    &model::arrangement_entries_from(&state.arrangements),
                    state,
                );
            }
            // AppKit-only predefined item.
            MenuEntry::BringAllToFront => {}
        }
    }
}

/// One command: disabled when its rule fails, checkmarked when its toggle
/// is on, labelled with the live title for tab/window items.
fn draw_item(ui: &mut egui::Ui, spec: &MenuItemSpec, state: &MenuState) {
    let label = state.label(spec);
    let mut button = match spec.check {
        Some(check) => egui::Button::selectable(state.checked(check), label.as_ref()),
        None => egui::Button::new(label.as_ref()),
    };
    if let Some(accelerator) = &spec.accelerator {
        button = button.shortcut_text(model::accelerator_label(accelerator));
    }
    if ui.add_enabled(state.enabled(spec), button).clicked() {
        bridge::dispatch(spec.action);
    }
}

/// Generated entries (profiles, arrangements).
fn draw_dynamic(ui: &mut egui::Ui, entries: &[model::DynamicEntry], state: &MenuState) {
    for entry in entries {
        if ui
            .add_enabled(state.dynamic_enabled(), egui::Button::new(&entry.label))
            .clicked()
        {
            bridge::dispatch(entry.action);
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
#[path = "egui_menu_tests.rs"]
mod tests;
