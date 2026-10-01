//! The one list-editor row component (UX.md SC4).
//!
//! Every list in Settings whose items are edited one at a time (triggers,
//! snippets, custom actions, coprocesses, observer scripts, dynamic profile
//! sources, CLI agents, agent commands, prompts) draws each row's buttons
//! through [`row_actions`]: reorder ↑↓, Duplicate, Edit, and a Delete that
//! always asks first. A list passes only the buttons its data supports and
//! applies the returned [`RowAction`] after its row loop, so the row never
//! mutates the list it is iterating.
//!
//! `consistency_tests` fails if a list draws its own Edit or Delete button.

use crate::delete_confirm::{PendingDelete, confirm_delete_button};

/// Which buttons a list's rows offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowButtons {
    /// ↑ and ↓ (the list's order matters to the user).
    pub reorder: bool,
    /// Duplicate (the list can hold two copies of an item).
    pub duplicate: bool,
    /// Edit.
    pub edit: bool,
    /// Delete, armed by a first click and confirmed by a second.
    pub delete: bool,
}

impl RowButtons {
    /// Edit and Delete.
    pub const EDIT_DELETE: Self = Self {
        reorder: false,
        duplicate: false,
        edit: true,
        delete: true,
    };

    /// Every button.
    pub const ALL: Self = Self {
        reorder: true,
        duplicate: true,
        edit: true,
        delete: true,
    };
}

/// What a row's buttons asked for this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAction {
    MoveUp(usize),
    MoveDown(usize),
    Duplicate(usize),
    Edit(usize),
    /// Delete, already confirmed.
    Delete(usize),
}

/// One row's identity for the shared buttons.
pub struct Row<'a> {
    /// Index of the row in its list.
    pub index: usize,
    /// Number of rows in the list (disables ↓ on the last).
    pub len: usize,
    /// Name of the list, scoping the armed-delete state.
    pub list: &'static str,
    /// Stable key of the item (an id or name), so an armed delete never
    /// moves to another item when the list shifts.
    pub key: &'a str,
    /// Caption of the delete button ("Delete", "Remove").
    pub delete_label: &'static str,
}

/// Draw a row's buttons, right to left from the current cursor: Delete,
/// Edit, Duplicate, ↓, ↑. Call inside a right-to-left layout so they sit at
/// the row's end. Returns the action clicked, if any.
pub fn row_actions(
    ui: &mut egui::Ui,
    pending: &mut PendingDelete,
    row: Row<'_>,
    buttons: RowButtons,
) -> Option<RowAction> {
    let mut action = None;
    if buttons.delete && confirm_delete_button(ui, pending, row.list, row.key, row.delete_label) {
        action = Some(RowAction::Delete(row.index));
    }
    if buttons.edit && ui.small_button("Edit").clicked() {
        action = Some(RowAction::Edit(row.index));
    }
    if buttons.duplicate
        && ui
            .small_button("Duplicate")
            .on_hover_text("Add a copy of this item below it")
            .clicked()
    {
        action = Some(RowAction::Duplicate(row.index));
    }
    if buttons.reorder {
        let last = row.index + 1 >= row.len;
        if ui
            .add_enabled(!last, egui::Button::new("\u{2193}").small())
            .on_hover_text("Move down")
            .clicked()
        {
            action = Some(RowAction::MoveDown(row.index));
        }
        if ui
            .add_enabled(row.index > 0, egui::Button::new("\u{2191}").small())
            .on_hover_text("Move up")
            .clicked()
        {
            action = Some(RowAction::MoveUp(row.index));
        }
    }
    action
}

/// Apply a reorder or duplicate to `items`. Edit and Delete are left to the
/// list, which owns its edit form and delete side effects. Returns whether
/// the list changed.
pub fn apply_move_or_duplicate<T: Clone>(
    items: &mut Vec<T>,
    action: RowAction,
    rename_copy: impl FnOnce(&mut T),
) -> bool {
    match action {
        RowAction::MoveUp(i) if i > 0 && i < items.len() => {
            items.swap(i, i - 1);
            true
        }
        RowAction::MoveDown(i) if i + 1 < items.len() => {
            items.swap(i, i + 1);
            true
        }
        RowAction::Duplicate(i) if i < items.len() => {
            let mut copy = items[i].clone();
            rename_copy(&mut copy);
            items.insert(i + 1, copy);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_and_duplicates_apply_in_place() {
        let mut items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert!(apply_move_or_duplicate(
            &mut items,
            RowAction::MoveUp(1),
            |_| {}
        ));
        assert_eq!(items, ["b", "a", "c"]);
        assert!(apply_move_or_duplicate(
            &mut items,
            RowAction::MoveDown(1),
            |_| {}
        ));
        assert_eq!(items, ["b", "c", "a"]);
        assert!(apply_move_or_duplicate(
            &mut items,
            RowAction::Duplicate(0),
            |s| s.push_str(" copy")
        ));
        assert_eq!(items, ["b", "b copy", "c", "a"]);
    }

    #[test]
    fn out_of_range_moves_are_ignored() {
        let mut items = vec![1, 2];
        assert!(!apply_move_or_duplicate(
            &mut items,
            RowAction::MoveUp(0),
            |_| {}
        ));
        assert!(!apply_move_or_duplicate(
            &mut items,
            RowAction::MoveDown(1),
            |_| {}
        ));
        assert!(!apply_move_or_duplicate(
            &mut items,
            RowAction::Edit(0),
            |_| {}
        ));
        assert_eq!(items, [1, 2]);
    }
}
