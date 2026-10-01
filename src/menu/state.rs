//! Live menu state (UX.md MN2): which items are enabled, checked, and
//! relabelled for a window.
//!
//! The model ([`super::model`]) says which rule applies to an item; this
//! module holds the facts the rules read, gathered once per tick from the
//! window by `WindowState::menu_state`. Both renderers apply the same
//! [`MenuState`], so the native menu and the in-app menu cannot disagree
//! about what is available.

use super::actions::MenuAction;
use super::model::{MenuEntry, MenuItemSpec, MenuSection};
use crate::arrangements::ArrangementId;
use muda::accelerator::{Accelerator, Code};
use par_term_keybindings::KeybindingRegistry;
use std::borrow::Cow;
use std::collections::BTreeSet;

/// When an item applies. It is disabled everywhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requires {
    /// Always applicable.
    Always,
    /// The active tab has more than one pane.
    MultiplePanes,
    /// The window has more than one visible tab.
    MultipleTabs,
    /// More than one window is open.
    MultipleWindows,
    /// This build has par-mux support.
    MuxAvailable,
    /// The window is attached to a par-mux or tmux session.
    SessionAttached,
    /// The window is attached to a par-mux session.
    MuxAttached,
    /// The active tab can move to another window: it is neither attached
    /// to a par-mux session nor a tmux gateway tab.
    TabMovable,
    /// [`Self::TabMovable`], and the window keeps a tab behind.
    TabMovableAway,
    /// Visible tab N (1-based) exists.
    Tab(usize),
    /// A window holds number N.
    Window(usize),
    /// At least one window arrangement is saved.
    Arrangements,
}

/// The state a toggle's checkmark mirrors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Check {
    Fullscreen,
    PaneZoom,
    ProfileDrawer,
    AssistantPanel,
    AgentUsage,
    FpsOverlay,
    BackgroundShader,
    ShaderAnimation,
    ShaderReadability,
    CursorShader,
    Throughput,
    AlwaysOnTop,
    BroadcastTab,
    PaneBroadcastExcluded,
    OutputRecording,
    CopyMode,
    TabProfilePinned,
}

/// Why the active tab cannot move to another window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveBlock {
    /// The tab mirrors a par-mux daemon window.
    Attached,
    /// The window is a tmux gateway.
    Gateway,
}

/// Longest tab or window title a menu label carries before it is cut.
const MAX_TITLE_CHARS: usize = 40;

/// Facts about a window the menu rules read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuState {
    /// A modal dialog is open: the key handler drops every chord but
    /// F1/F2/F3/Escape before the registry, so the menu disables the
    /// commands those chords would run.
    pub modal_open: bool,
    /// The active tab has more than one pane.
    pub multiple_panes: bool,
    /// Number of visible tabs.
    pub tab_count: usize,
    /// Titles of the first nine visible tabs, in tab-bar order.
    pub tab_titles: Vec<String>,
    /// `(window number, active tab title)` for every window, by number.
    pub windows: Vec<(usize, String)>,
    /// This build has par-mux support.
    pub mux_available: bool,
    /// Attached to a par-mux or tmux session.
    pub session_attached: bool,
    /// Attached to a par-mux session.
    pub mux_attached: bool,
    /// Why the active tab cannot move, if it cannot.
    pub move_block: Option<MoveBlock>,
    /// Saved window arrangements, in display order.
    pub arrangements: Vec<(ArrangementId, String)>,
    /// The toggles that are on.
    pub checks: BTreeSet<Check>,
    /// Registry ids of the toggles whose panel or mode is open now. Their
    /// chord keeps working while the keyboard is captured, so the chord that
    /// opened a panel also closes it.
    pub open_toggles: BTreeSet<&'static str>,
}

impl MenuState {
    /// Whether `requires` holds.
    pub fn satisfies(&self, requires: Requires) -> bool {
        match requires {
            Requires::Always => true,
            Requires::MultiplePanes => self.multiple_panes,
            Requires::MultipleTabs => self.tab_count > 1,
            Requires::MultipleWindows => self.windows.len() > 1,
            Requires::MuxAvailable => self.mux_available,
            Requires::SessionAttached => self.session_attached,
            Requires::MuxAttached => self.mux_attached,
            Requires::TabMovable => self.move_block.is_none(),
            Requires::TabMovableAway => self.move_block.is_none() && self.tab_count > 1,
            Requires::Tab(n) => n >= 1 && n <= self.tab_count,
            Requires::Window(n) => self.windows.iter().any(|(number, _)| *number == n),
            Requires::Arrangements => !self.arrangements.is_empty(),
        }
    }

    /// Whether the item can be activated now.
    pub fn enabled(&self, spec: &MenuItemSpec) -> bool {
        if self.modal_open && !self.passes(&spec.action, spec.accelerator.as_ref()) {
            return false;
        }
        self.satisfies(spec.requires)
    }

    /// Whether `action` may run while the keyboard belongs to a dialog: the
    /// [`passes_key_block`] commands, and the toggle of a panel that is open.
    pub fn passes(&self, action: &MenuAction, accelerator: Option<&Accelerator>) -> bool {
        passes_key_block(action, accelerator) || self.closes_open_panel(action)
    }

    /// [`Self::passes`] for an action whose chord is read from `registry`
    /// (a native activation names only the action).
    pub fn passes_with(&self, action: &MenuAction, registry: &KeybindingRegistry) -> bool {
        let accelerator = action
            .keybinding_action()
            .and_then(|id| registry.chord_for_action(&id))
            .and_then(|combo| super::registry_accel::accelerator_from_combo(&combo));
        self.passes(action, accelerator.as_ref())
    }

    /// Whether `action` toggles a panel or mode that is open now.
    fn closes_open_panel(&self, action: &MenuAction) -> bool {
        matches!(action, MenuAction::Action(id) if self.open_toggles.contains(id))
    }

    /// Whether dynamic entries (profiles, arrangements) can be activated.
    pub fn dynamic_enabled(&self) -> bool {
        !self.modal_open
    }

    /// Whether a toggle is on.
    pub fn checked(&self, check: Check) -> bool {
        self.checks.contains(&check)
    }

    /// The item's label now: tab and window items carry the title, and a
    /// blocked move says why (UX.md MN2).
    pub fn label(&self, spec: &MenuItemSpec) -> Cow<'static, str> {
        match spec.requires {
            Requires::Tab(n) => match self.tab_titles.get(n.wrapping_sub(1)) {
                Some(title) if !title.trim().is_empty() => {
                    Cow::Owned(format!("Tab {n}: {}", shorten(title)))
                }
                _ => Cow::Borrowed(spec.label),
            },
            Requires::Window(n) => match self.windows.iter().find(|(number, _)| *number == n) {
                Some((_, title)) if !title.trim().is_empty() => {
                    Cow::Owned(format!("Window {n}: {}", shorten(title)))
                }
                _ => Cow::Borrowed(spec.label),
            },
            Requires::TabMovable | Requires::TabMovableAway => match self.move_block {
                Some(MoveBlock::Attached) => Cow::Owned(format!("{} (attached tab)", spec.label)),
                Some(MoveBlock::Gateway) => Cow::Owned(format!("{} (tmux tab)", spec.label)),
                None => Cow::Borrowed(spec.label),
            },
            _ => Cow::Borrowed(spec.label),
        }
    }
}

/// Whether a command may run while a dialog owns the keyboard. Mirrors the
/// key handler's modal guard: clipboard commands are routed into the
/// focused text field, app- and window-level commands touch no terminal,
/// and chords on F1/F2/F3/Escape are the ones the guard lets through.
pub fn passes_key_block(action: &MenuAction, accelerator: Option<&Accelerator>) -> bool {
    matches!(
        action,
        MenuAction::Copy
            | MenuAction::Paste
            | MenuAction::SelectAll
            | MenuAction::Quit
            | MenuAction::OpenSettings
            | MenuAction::About
            | MenuAction::OpenDocs
            | MenuAction::Minimize
            | MenuAction::Zoom
    ) || accelerator
        .is_some_and(|a| matches!(a.key(), Code::F1 | Code::F2 | Code::F3 | Code::Escape))
}

/// The accelerators the native menu keeps while the keyboard is captured:
/// `None` keeps all of them; `Some(open)` keeps only the
/// [`passes_key_block`] chords and the toggles of the panels in `open`.
pub type Capture = Option<BTreeSet<&'static str>>;

/// Drop every accelerator a dialog or text field must receive instead.
///
/// On macOS a menu key equivalent fires before the window sees the key —
/// even on a disabled item, which swallows it — so while the keyboard
/// belongs to a dialog, a text field, the Settings window, or a modal mode
/// (copy mode, pane hints, a prefix key) the native menu is built without
/// them. The [`passes_key_block`] chords stay (Copy, Paste, Quit, F1, …),
/// and so do the toggles of the panels in `open`, so the chord that opened
/// a panel still closes it.
pub fn release_captured_accelerators(sections: &mut [MenuSection], open: &BTreeSet<&'static str>) {
    fn walk(entries: &mut [MenuEntry], open: &BTreeSet<&'static str>) {
        for entry in entries {
            match entry {
                MenuEntry::Item(spec) => {
                    let keeps = passes_key_block(&spec.action, spec.accelerator.as_ref())
                        || matches!(spec.action, MenuAction::Action(id) if open.contains(id));
                    if !keeps {
                        spec.accelerator = None;
                    }
                }
                MenuEntry::Submenu(sub) => walk(&mut sub.entries, open),
                _ => {}
            }
        }
    }
    for section in sections {
        walk(&mut section.entries, open);
    }
}

/// Whether a native item's label can change with the state (tab and window
/// titles, a blocked move), so the per-tick apply only rewrites those.
pub fn label_varies(requires: Requires) -> bool {
    matches!(
        requires,
        Requires::Tab(_) | Requires::Window(_) | Requires::TabMovable | Requires::TabMovableAway
    )
}

/// Cut a title to [`MAX_TITLE_CHARS`] characters.
fn shorten(title: &str) -> String {
    let title = title.trim();
    if title.chars().count() <= MAX_TITLE_CHARS {
        return title.to_string();
    }
    let mut cut: String = title.chars().take(MAX_TITLE_CHARS - 1).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
