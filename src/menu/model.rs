//! Declarative description of par-term's menu (UX.md 21.2, MD1).
//!
//! This is the single source of truth for the menu's contents. Two renderers
//! consume it:
//!
//! - [`super::MenuManager`] walks it to build the [`muda::Menu`] that macOS
//!   and Windows attach natively.
//! - [`super::egui_menu::AppMenuUi`] walks the same model to draw the in-app
//!   menu on platforms that cannot attach a native menu bar (Linux/BSD, where
//!   muda needs a `gtk::Window` that winit never creates — see `super::linux`).
//!
//! Neither renderer owns a list of commands, so the two cannot drift apart.
//! Which items are enabled, checked, or relabelled at a given moment is
//! [`super::state::MenuState`]'s job; the model only says which rule applies.

use super::actions::MenuAction;
use super::state::{Check, Requires};
use muda::accelerator::{Accelerator, Code, Modifiers};
use par_term_config::KeyBinding;
use par_term_keybindings::KeybindingRegistry;

/// Title of the Help section.
pub const HELP_SECTION_TITLE: &str = "Help";

/// Title of the Window section — registered as NSApp's Window menu on macOS
/// so AppKit appends the open-window list to it.
pub const WINDOW_SECTION_TITLE: &str = "Window";

/// A single activatable menu command.
#[derive(Debug, Clone)]
pub struct MenuItemSpec {
    /// Stable menu id. Also used as the egui widget id salt.
    pub id: &'static str,
    /// Human-readable label (the state may extend it, see
    /// [`super::state::MenuState::label`]).
    pub label: &'static str,
    /// Keyboard accelerator, if the command has one. Registry-backed items
    /// get theirs from the live registry; only menu-only commands carry a
    /// hardcoded one.
    pub accelerator: Option<Accelerator>,
    /// Action dispatched when the item is activated.
    pub action: MenuAction,
    /// When the item is applicable (disabled otherwise).
    pub requires: Requires,
    /// The state the item's checkmark mirrors, for toggles.
    pub check: Option<Check>,
    /// A second home for an action another item already carries: it never
    /// shows the chord, so one key equivalent maps to one native item.
    pub alias: bool,
}

impl MenuItemSpec {
    /// An item that runs `action`.
    pub(super) fn new(id: &'static str, label: &'static str, action: MenuAction) -> Self {
        Self {
            id,
            label,
            accelerator: None,
            action,
            requires: Requires::Always,
            check: None,
            alias: false,
        }
    }

    /// Mark the item as a second home for its action (no accelerator).
    pub(super) fn alias(mut self) -> Self {
        self.alias = true;
        self
    }

    /// An item that runs registry action `id` (its menu id is the action id).
    pub(super) fn action(id: &'static str, label: &'static str) -> Self {
        Self::new(id, label, MenuAction::Action(id))
    }

    /// Restrict the item to a context; it is disabled elsewhere.
    pub(super) fn when(mut self, requires: Requires) -> Self {
        self.requires = requires;
        self
    }

    /// Make the item a toggle whose checkmark mirrors `check`.
    pub(super) fn toggle(mut self, check: Check) -> Self {
        self.check = Some(check);
        self
    }

    /// Give a menu-only command a fixed accelerator.
    pub(super) fn accel(mut self, accelerator: Option<Accelerator>) -> Self {
        self.accelerator = accelerator;
        self
    }
}

/// A nested submenu.
#[derive(Debug, Clone)]
pub struct SubmenuSpec {
    /// Stable menu id.
    pub id: &'static str,
    /// Title shown in the parent menu.
    pub title: &'static str,
    /// When the whole submenu is applicable.
    pub requires: Requires,
    /// Entries in display order.
    pub entries: Vec<MenuEntry>,
}

/// One entry inside a menu section.
#[derive(Debug, Clone)]
pub enum MenuEntry {
    /// A command.
    Item(MenuItemSpec),
    /// A nested submenu.
    Submenu(SubmenuSpec),
    /// A horizontal rule.
    Separator,
    /// Insertion point for one entry per configured profile, followed by a
    /// separator when there is at least one.
    Profiles,
    /// Insertion point for one entry per saved window arrangement.
    Arrangements,
    /// macOS "Bring All to Front" (an AppKit predefined item; the other
    /// renderers skip it).
    BringAllToFront,
}

impl From<MenuItemSpec> for MenuEntry {
    fn from(spec: MenuItemSpec) -> Self {
        Self::Item(spec)
    }
}

/// A submenu entry with the given children.
pub(super) fn submenu(
    id: &'static str,
    title: &'static str,
    requires: Requires,
    entries: Vec<MenuEntry>,
) -> MenuEntry {
    MenuEntry::Submenu(SubmenuSpec {
        id,
        title,
        requires,
        entries,
    })
}

/// A top-level menu (Shell, Edit, …).
#[derive(Debug, Clone)]
pub struct MenuSection {
    /// Title shown in the menu bar.
    pub title: &'static str,
    /// Entries in display order.
    pub entries: Vec<MenuEntry>,
}

/// Every command in `entries`, depth first in display order.
pub fn items_in(entries: &[MenuEntry]) -> Vec<&MenuItemSpec> {
    let mut out = Vec::new();
    collect_items(entries, &mut out);
    out
}

fn collect_items<'a>(entries: &'a [MenuEntry], out: &mut Vec<&'a MenuItemSpec>) {
    for entry in entries {
        match entry {
            MenuEntry::Item(spec) => out.push(spec),
            MenuEntry::Submenu(sub) => collect_items(&sub.entries, out),
            _ => {}
        }
    }
}

/// Every command in the model, depth first in display order.
pub fn all_items(sections: &[MenuSection]) -> Vec<&MenuItemSpec> {
    sections
        .iter()
        .flat_map(|section| items_in(&section.entries))
        .collect()
}

/// The commands the macOS application menu (`super::macos`) carries. It is
/// built from muda predefined items the model cannot express, so it lives
/// outside [`menu_model`]; this list is what the coverage tests count as its
/// menu homes.
pub const APP_MENU_ACTIONS: &[MenuAction] = &[
    MenuAction::About,
    MenuAction::OpenSettings,
    MenuAction::Quit,
];

/// Build the menu model for the current platform's native menu.
///
/// macOS carries Quit, Settings and About in the separate application menu
/// built by [`super::macos::build_app_menu`], so they are omitted here.
pub fn platform_menu_model_with(keybindings: &[KeyBinding]) -> Vec<MenuSection> {
    menu_model_with(cfg!(target_os = "macos"), keybindings)
}

/// Build the menu model from the default bindings.
///
/// `has_native_app_menu` is true when the platform provides a separate
/// application menu that already carries Quit, Settings and About (macOS).
/// When it is false those commands are folded into Shell, Edit and Help,
/// which is what Windows and Linux expect — and what the in-app egui menu
/// always needs, since it is the only menu wherever it is drawn.
pub fn menu_model(has_native_app_menu: bool) -> Vec<MenuSection> {
    menu_model_with(
        has_native_app_menu,
        &par_term_config::Config::default().keybindings,
    )
}

/// Build the menu model, sourcing every accelerator from `keybindings`.
pub fn menu_model_with(has_native_app_menu: bool, keybindings: &[KeyBinding]) -> Vec<MenuSection> {
    menu_model_with_registry(
        has_native_app_menu,
        &KeybindingRegistry::from_config(keybindings),
    )
}

/// Build the menu model, sourcing every accelerator from the live registry
/// (UX.md MN3): a rebound action shows its new chord, an unbound one shows
/// none — see [`super::registry_accel`].
pub fn menu_model_with_registry(
    has_native_app_menu: bool,
    registry: &KeybindingRegistry,
) -> Vec<MenuSection> {
    let mut sections = structure(has_native_app_menu);
    super::registry_accel::apply_registry_accelerators(&mut sections, registry);
    sections
}

/// The menu's structure (UX.md 21.2), before the registry is applied.
fn structure(has_native_app_menu: bool) -> Vec<MenuSection> {
    use super::{model_sections as s, model_window as w};
    vec![
        MenuSection {
            title: "Shell",
            entries: s::shell(has_native_app_menu),
        },
        MenuSection {
            title: "Edit",
            entries: s::edit(has_native_app_menu),
        },
        MenuSection {
            title: "View",
            entries: s::view(),
        },
        MenuSection {
            title: "Session",
            entries: w::session(),
        },
        MenuSection {
            title: "Profiles",
            entries: w::profiles(),
        },
        MenuSection {
            title: WINDOW_SECTION_TITLE,
            entries: w::window(),
        },
        MenuSection {
            title: HELP_SECTION_TITLE,
            entries: w::help(has_native_app_menu),
        },
    ]
}

/// The platform's primary modifier for menu-only chords: Cmd on macOS,
/// Ctrl+Shift elsewhere (plain Ctrl+C/V belong to the shell there).
pub(super) fn primary_modifier() -> Modifiers {
    if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CONTROL | Modifiers::SHIFT
    }
}

/// A fixed accelerator on the platform's primary modifier.
pub(super) fn primary(code: Code) -> Option<Accelerator> {
    Some(Accelerator::new(primary_modifier(), code))
}

/// One dynamically generated entry (a profile or a saved arrangement).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicEntry {
    /// Stable menu id.
    pub menu_id: String,
    /// Label as shown in the menu.
    pub label: String,
    /// Keyboard accelerator, resolved from the live registry the way
    /// `registry_accel` resolves a static item's: a profile entry carries its
    /// migrated chord (UX.md 21.2), an arrangement entry none.
    pub accelerator: Option<Accelerator>,
    /// Action dispatched when the entry is activated.
    pub action: MenuAction,
}

/// Expand [`MenuEntry::Profiles`] into one entry per configured profile.
///
/// Shared by the muda and egui renderers so both show the same profiles, with
/// the same labels, accelerators, and order. The accelerator comes from
/// `registry` through [`MenuAction::keybinding_action`] — the same chain the
/// static items use — so a rebound chord moves and an unbound one shows none.
pub fn profile_entries<'a>(
    profiles: impl IntoIterator<Item = &'a crate::profile::Profile>,
    registry: &KeybindingRegistry,
) -> Vec<DynamicEntry> {
    profiles
        .into_iter()
        .map(|profile| {
            let action = MenuAction::OpenProfile(profile.id);
            DynamicEntry {
                menu_id: format!("profile_{}", profile.id),
                label: profile.display_label(),
                accelerator: action
                    .keybinding_action()
                    .and_then(|id| registry.chord_for_action(&id))
                    .and_then(|combo| super::registry_accel::accelerator_from_combo(&combo)),
                action,
            }
        })
        .collect()
}

/// Expand [`MenuEntry::Arrangements`] into one entry per saved arrangement,
/// from the `(id, name)` pairs [`super::state::MenuState`] carries.
pub fn arrangement_entries_from(
    arrangements: &[(crate::arrangements::ArrangementId, String)],
) -> Vec<DynamicEntry> {
    arrangements
        .iter()
        .map(|(id, name)| DynamicEntry {
            menu_id: format!("arrangement_{id}"),
            label: name.clone(),
            accelerator: None,
            action: MenuAction::RestoreArrangement(*id),
        })
        .collect()
}

/// Render an accelerator the way a menu displays it, e.g. `⌘N` or `Ctrl+Shift+N`.
///
/// Derived from the same [`Accelerator`] the native menu registers, so the
/// in-app menu cannot advertise a shortcut the native menu does not have.
pub fn accelerator_label(accelerator: &Accelerator) -> String {
    let mut label = String::new();
    // muda 0.20 stores META verbatim (the old META→SUPER normalisation is gone;
    // SUPER is a separate legacy bit nothing produces any more), so the table
    // keys on META. macOS renders modifiers as adjacent symbols; everywhere
    // else they are spelled out and joined with '+'.
    let named: [(Modifiers, &str, &str); 4] = [
        (Modifiers::CONTROL, "⌃", "Ctrl"),
        (Modifiers::ALT, "⌥", "Alt"),
        (Modifiers::SHIFT, "⇧", "Shift"),
        (Modifiers::META, "⌘", "Super"),
    ];
    let mods = accelerator.modifiers();
    for (flag, symbol, word) in named {
        if mods.contains(flag) {
            if cfg!(target_os = "macos") {
                label.push_str(symbol);
            } else {
                label.push_str(word);
                label.push('+');
            }
        }
    }
    label.push_str(&code_label(accelerator.key()));
    label
}

/// Human-readable name for a key code (`KeyN` → `N`, `BracketLeft` → `[`).
fn code_label(code: Code) -> String {
    let raw = format!("{code:?}");
    match raw.as_str() {
        "Comma" => ",".to_string(),
        "Period" => ".".to_string(),
        "Equal" => "+".to_string(),
        "Minus" => "-".to_string(),
        "BracketLeft" => "[".to_string(),
        "BracketRight" => "]".to_string(),
        "Space" => "Space".to_string(),
        // Without these the in-app menu would advertise "ArrowLeft"; "Left" is
        // also what the settings window's keybinding table prints.
        "ArrowLeft" => "Left".to_string(),
        "ArrowRight" => "Right".to_string(),
        "ArrowUp" => "Up".to_string(),
        "ArrowDown" => "Down".to_string(),
        other => other
            .strip_prefix("Key")
            .or_else(|| other.strip_prefix("Digit"))
            .unwrap_or(other)
            .to_string(),
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
