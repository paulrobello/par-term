//! The native menu bar (macOS, Windows): a muda build of the model plus
//! the live state applied to it.
//!
//! [`NativeMenu`] is rebuilt whole whenever the bindings change (UX.md MN3).
//! muda cannot remove a key equivalent from an existing macOS item (its
//! `set_accelerator(None)` is a no-op there), so patching accelerators in
//! place could never release an unbound chord.

use super::actions::MenuAction;
use super::model::{self, MenuEntry, MenuItemSpec, MenuSection};
use super::state::MenuState;
use crate::profile::Profile;
use anyhow::Result;
use muda::{CheckMenuItem, IsMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use std::collections::HashMap;

/// One native item the state is applied to.
enum NativeItem {
    Plain(MenuItem, MenuItemSpec),
    Toggle(CheckMenuItem, MenuItemSpec),
}

/// A dynamic list (profiles, arrangements): the submenu it lives in, where
/// its entries start, and the entries currently inserted.
struct DynamicSlot {
    submenu: Submenu,
    position: usize,
    items: Vec<MenuItem>,
}

/// A built native menu.
pub(super) struct NativeMenu {
    /// The root menu, attached to NSApp or the window by `MenuManager`.
    pub(super) menu: Menu,
    /// Menu id → action, for every activatable item.
    pub(super) action_map: HashMap<MenuId, MenuAction>,
    /// Items whose state the per-tick sync applies.
    items: Vec<NativeItem>,
    /// Submenus gated on the state, with their rule.
    submenus: Vec<(Submenu, super::state::Requires)>,
    /// The Window section (registered as NSApp's Window menu on macOS).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(super) window_menu: Option<Submenu>,
    /// The Help section (registered as NSApp's Help menu on macOS).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(super) help_menu: Option<Submenu>,
    profiles: Option<DynamicSlot>,
    arrangements: Option<DynamicSlot>,
    /// The state last applied; `None` forces the next apply.
    applied: Option<MenuState>,
}

impl NativeMenu {
    /// Build the muda menu for `sections`. `quit_accelerator` is the `quit`
    /// action's registry chord (macOS shows Quit in the application menu,
    /// outside the model).
    pub(super) fn build(
        sections: &[MenuSection],
        #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] quit_accelerator: Option<
            muda::accelerator::Accelerator,
        >,
    ) -> Result<Self> {
        let mut built = Self {
            menu: Menu::new(),
            action_map: HashMap::new(),
            items: Vec::new(),
            submenus: Vec::new(),
            window_menu: None,
            help_menu: None,
            profiles: None,
            arrangements: None,
            applied: None,
        };

        // macOS: the application menu must be the first submenu. It uses
        // predefined items (Services, Hide, Show All) the model cannot express.
        #[cfg(target_os = "macos")]
        super::macos::build_app_menu(&built.menu, &mut built.action_map, quit_accelerator)?;

        for section in sections {
            let submenu = Submenu::new(section.title, true);
            built.append_entries(&submenu, &section.entries)?;
            built.menu.append(&submenu)?;
            if section.title == model::WINDOW_SECTION_TITLE {
                built.window_menu = Some(submenu);
            } else if section.title == model::HELP_SECTION_TITLE {
                built.help_menu = Some(submenu);
            }
        }
        Ok(built)
    }

    fn append_entries(&mut self, parent: &Submenu, entries: &[MenuEntry]) -> Result<()> {
        for entry in entries {
            match entry {
                MenuEntry::Separator => parent.append(&PredefinedMenuItem::separator())?,
                MenuEntry::Item(spec) => self.append_item(parent, spec)?,
                MenuEntry::Submenu(sub) => {
                    let child = Submenu::with_id(sub.id, sub.title, true);
                    self.append_entries(&child, &sub.entries)?;
                    parent.append(&child)?;
                    self.submenus.push((child, sub.requires));
                }
                MenuEntry::Profiles => {
                    self.profiles = Some(DynamicSlot {
                        submenu: parent.clone(),
                        position: parent.items().len(),
                        items: Vec::new(),
                    });
                }
                MenuEntry::Arrangements => {
                    self.arrangements = Some(DynamicSlot {
                        submenu: parent.clone(),
                        position: parent.items().len(),
                        items: Vec::new(),
                    });
                }
                MenuEntry::BringAllToFront => {
                    parent.append(&PredefinedMenuItem::bring_all_to_front(None))?;
                }
            }
        }
        Ok(())
    }

    fn append_item(&mut self, parent: &Submenu, spec: &MenuItemSpec) -> Result<()> {
        if spec.check.is_some() {
            let item = CheckMenuItem::with_id(spec.id, spec.label, true, false, spec.accelerator);
            self.action_map.insert(item.id().clone(), spec.action);
            parent.append(&item)?;
            self.items.push(NativeItem::Toggle(item, spec.clone()));
        } else {
            let item = MenuItem::with_id(spec.id, spec.label, true, spec.accelerator);
            self.action_map.insert(item.id().clone(), spec.action);
            parent.append(&item)?;
            self.items.push(NativeItem::Plain(item, spec.clone()));
        }
        Ok(())
    }

    /// Apply `state` to every item. Skipped when nothing changed since the
    /// last apply, unless [`Self::invalidate`] ran.
    pub(super) fn apply(&mut self, state: &MenuState) {
        if self.applied.as_ref() == Some(state) {
            return;
        }
        for item in &self.items {
            match item {
                NativeItem::Plain(item, spec) => {
                    item.set_enabled(state.enabled(spec));
                    if super::state::label_varies(spec.requires) {
                        item.set_text(state.label(spec));
                    }
                }
                NativeItem::Toggle(item, spec) => {
                    item.set_enabled(state.enabled(spec));
                    item.set_checked(spec.check.is_some_and(|c| state.checked(c)));
                }
            }
        }
        for (submenu, requires) in &self.submenus {
            submenu.set_enabled(state.satisfies(*requires));
        }
        let dynamic_enabled = state.dynamic_enabled();
        if let Some(slot) = &mut self.profiles {
            for item in &slot.items {
                item.set_enabled(dynamic_enabled);
            }
        }
        let wanted = model::arrangement_entries_from(&state.arrangements);
        if let Some(slot) = &mut self.arrangements {
            fill_slot(slot, &mut self.action_map, &wanted, false);
            for item in &slot.items {
                item.set_enabled(dynamic_enabled);
            }
        }
        self.applied = Some(state.clone());
    }

    /// Force the next [`Self::apply`] to write every item: a clicked
    /// `CheckMenuItem` flips its own checkmark, which the cached state
    /// cannot see.
    pub(super) fn invalidate(&mut self) {
        self.applied = None;
    }

    /// Replace the profile entries.
    pub(super) fn set_profiles(&mut self, profiles: &[&Profile]) {
        let wanted = model::profile_entries(profiles.iter().copied());
        if let Some(slot) = &mut self.profiles {
            fill_slot(slot, &mut self.action_map, &wanted, true);
        }
        self.applied = None;
    }
}

/// Replace a slot's entries with `wanted` (no-op when unchanged), followed
/// by a separator when `trailing_separator` and there is at least one.
fn fill_slot(
    slot: &mut DynamicSlot,
    action_map: &mut HashMap<MenuId, MenuAction>,
    wanted: &[model::DynamicEntry],
    trailing_separator: bool,
) {
    let current: Vec<String> = slot.items.iter().map(|i| i.id().0.clone()).collect();
    let target: Vec<String> = wanted.iter().map(|e| e.menu_id.clone()).collect();
    let labels_match = slot
        .items
        .iter()
        .zip(wanted)
        .all(|(item, entry)| item.text() == entry.label);
    if current == target && labels_match {
        return;
    }
    // Remove the old entries and their separator (always at the end).
    let old_len = slot.items.len() + usize::from(trailing_separator && !slot.items.is_empty());
    for item in slot.items.drain(..) {
        action_map.remove(item.id());
    }
    for _ in 0..old_len {
        let _ = slot.submenu.remove_at(slot.position);
    }
    let mut position = slot.position;
    for entry in wanted {
        let item = MenuItem::with_id(entry.menu_id.as_str(), &entry.label, true, None);
        if let Err(e) = slot.submenu.insert(&item, position) {
            log::warn!("menu: failed to add {:?}: {e}", entry.label);
            continue;
        }
        action_map.insert(item.id().clone(), entry.action);
        slot.items.push(item);
        position += 1;
    }
    if trailing_separator && !slot.items.is_empty() {
        let _ = slot.submenu.insert(
            &PredefinedMenuItem::separator() as &dyn IsMenuItem,
            position,
        );
    }
}
