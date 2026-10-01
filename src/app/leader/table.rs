//! The leader key table (UX.md L-table, K9a).
//!
//! Every entry names a registry action, so a leader key and that action's
//! chord run the same code in local, par-mux, and tmux gateway tabs. Keys
//! mirror tmux wherever tmux has a convention.
//!
//! Not bound yet, because their actions do not exist: `$` (rename the
//! par-mux session, needs UP1), `(` / `)` (previous / next par-mux
//! session), `L` (switch back to the previous par-mux session; `S` under
//! `leader_vim_keys`), and `0` (par-term tabs are 1-based and nothing
//! targets a tenth tab). In a tmux gateway tab, `(`, `)`, and `L` still
//! reach tmux's own prefix table.

use crate::pane::NavigationDirection;
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// A key as the leader table sees it: letters keep their case (`E` is
/// not `e`), punctuation is the shifted glyph as typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TableKey {
    Char(char),
    Arrow {
        dir: NavigationDirection,
        shift: bool,
    },
    Space,
    Tab,
}

impl TableKey {
    /// The table key a press stands for, or `None` for a key no table
    /// entry can name. Ctrl, Alt, or Super on the follow-up key make it a
    /// different key (tmux's `C-n` is not `n`), so none of them maps.
    /// Letter case comes from Shift, not from the logical key, so a
    /// synthetic press carrying an upper-case letter without Shift (the
    /// `--ui-test` injector) reads the same as a real one.
    pub(crate) fn from_key(logical: &Key, mods: ModifiersState) -> Option<Self> {
        if mods.control_key() || mods.alt_key() || mods.super_key() {
            return None;
        }
        let shift = mods.shift_key();
        Some(match logical {
            Key::Character(text) => {
                let mut chars = text.chars();
                let c = chars.next()?;
                if chars.next().is_some() {
                    return None;
                }
                match c {
                    ' ' => TableKey::Space,
                    c if c.is_ascii_alphabetic() && shift => TableKey::Char(c.to_ascii_uppercase()),
                    c if c.is_ascii_alphabetic() => TableKey::Char(c.to_ascii_lowercase()),
                    c => TableKey::Char(c),
                }
            }
            Key::Named(NamedKey::Space) => TableKey::Space,
            Key::Named(NamedKey::Tab) => TableKey::Tab,
            Key::Named(NamedKey::ArrowLeft) => arrow(NavigationDirection::Left, shift),
            Key::Named(NamedKey::ArrowRight) => arrow(NavigationDirection::Right, shift),
            Key::Named(NamedKey::ArrowUp) => arrow(NavigationDirection::Up, shift),
            Key::Named(NamedKey::ArrowDown) => arrow(NavigationDirection::Down, shift),
            _ => return None,
        })
    }

    /// How the which-key overlay names the key.
    pub(crate) fn label(self) -> String {
        let dir = |d: NavigationDirection| match d {
            NavigationDirection::Left => "←",
            NavigationDirection::Right => "→",
            NavigationDirection::Up => "↑",
            NavigationDirection::Down => "↓",
        };
        match self {
            TableKey::Char(c) => c.to_string(),
            TableKey::Arrow {
                dir: d,
                shift: false,
            } => dir(d).to_string(),
            TableKey::Arrow {
                dir: d,
                shift: true,
            } => format!("Shift+{}", dir(d)),
            TableKey::Space => "Space".to_string(),
            TableKey::Tab => "Tab".to_string(),
        }
    }
}

const fn arrow(dir: NavigationDirection, shift: bool) -> TableKey {
    TableKey::Arrow { dir, shift }
}

/// One leader table entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LeaderBinding {
    pub(crate) key: TableKey,
    pub(crate) action: &'static str,
    /// Stays armed after running (K9).
    pub(crate) repeat: bool,
}

const fn bind(key: TableKey, action: &'static str) -> LeaderBinding {
    LeaderBinding {
        key,
        action,
        repeat: false,
    }
}

const fn repeat(key: TableKey, action: &'static str) -> LeaderBinding {
    LeaderBinding {
        key,
        action,
        repeat: true,
    }
}

const fn ch(c: char) -> TableKey {
    TableKey::Char(c)
}

use NavigationDirection::{Down, Left, Right, Up};

/// The default table, in which-key display order.
pub(crate) const BASE_TABLE: &[LeaderBinding] = &[
    // Tabs
    bind(ch('c'), "new_tab"),
    bind(ch('n'), "next_tab"),
    bind(ch('p'), "prev_tab"),
    bind(ch('l'), "last_tab"),
    bind(ch('1'), "switch_to_tab_1"),
    bind(ch('2'), "switch_to_tab_2"),
    bind(ch('3'), "switch_to_tab_3"),
    bind(ch('4'), "switch_to_tab_4"),
    bind(ch('5'), "switch_to_tab_5"),
    bind(ch('6'), "switch_to_tab_6"),
    bind(ch('7'), "switch_to_tab_7"),
    bind(ch('8'), "switch_to_tab_8"),
    bind(ch('9'), "switch_to_tab_9"),
    bind(ch(','), "rename_tab"),
    bind(ch('&'), "close_tab"),
    // Pickers and par-mux sessions
    bind(ch('w'), "toggle_tree_picker"),
    bind(ch('s'), "toggle_session_picker"),
    bind(ch('d'), "detach"),
    // Panes
    bind(ch('%'), "split_right"),
    bind(ch('|'), "split_right"),
    bind(ch('"'), "split_down"),
    bind(ch('-'), "split_down"),
    repeat(arrow(Left, false), "navigate_pane_left"),
    repeat(arrow(Right, false), "navigate_pane_right"),
    repeat(arrow(Up, false), "navigate_pane_up"),
    repeat(arrow(Down, false), "navigate_pane_down"),
    repeat(arrow(Left, true), "swap_pane_left"),
    repeat(arrow(Right, true), "swap_pane_right"),
    repeat(arrow(Up, true), "swap_pane_up"),
    repeat(arrow(Down, true), "swap_pane_down"),
    repeat(ch('o'), "next_pane"),
    bind(ch(';'), "last_pane"),
    bind(ch('q'), "select_pane_hint"),
    bind(ch('z'), "toggle_pane_zoom"),
    bind(ch('E'), "equalize_panes"),
    bind(TableKey::Space, "cycle_layout"),
    bind(ch('r'), "enter_resize_mode"),
    bind(ch('x'), "close_pane"),
    bind(ch('!'), "promote_pane_to_tab"),
    bind(ch('@'), "demote_tab_to_pane"),
    bind(ch('R'), "restart_pane"),
    bind(ch('b'), "toggle_broadcast_input"),
    bind(ch('a'), "focus_next_attention_agent"),
    // Window and app
    bind(ch('N'), "new_window"),
    bind(ch('P'), "toggle_profile_drawer"),
    bind(ch(':'), "toggle_command_palette"),
    bind(ch('?'), "toggle_help"),
];

/// `leader_vim_keys: true` (K9a): `h j k l` focus and `H J K L` swap, and
/// last tab moves from `l` to `Tab`.
pub(crate) const VIM_TABLE: &[LeaderBinding] = &[
    repeat(ch('h'), "navigate_pane_left"),
    repeat(ch('j'), "navigate_pane_down"),
    repeat(ch('k'), "navigate_pane_up"),
    repeat(ch('l'), "navigate_pane_right"),
    repeat(ch('H'), "swap_pane_left"),
    repeat(ch('J'), "swap_pane_down"),
    repeat(ch('K'), "swap_pane_up"),
    repeat(ch('L'), "swap_pane_right"),
    bind(TableKey::Tab, "last_tab"),
];

/// The live table, display order: the vim keys first when enabled, then
/// every base entry they do not displace.
pub(crate) fn entries(vim_keys: bool) -> Vec<LeaderBinding> {
    let vim: &[LeaderBinding] = if vim_keys { VIM_TABLE } else { &[] };
    vim.iter()
        .copied()
        .chain(
            BASE_TABLE
                .iter()
                .copied()
                .filter(|b| !vim.iter().any(|v| v.key == b.key)),
        )
        .collect()
}

pub(crate) fn lookup(key: TableKey, vim_keys: bool) -> Option<LeaderBinding> {
    let vim: &[LeaderBinding] = if vim_keys { VIM_TABLE } else { &[] };
    vim.iter().chain(BASE_TABLE).find(|b| b.key == key).copied()
}

/// Keys that stay par-term's in a tmux gateway tab: they open par-term
/// overlays, which list tmux windows and sessions too. tmux's versions
/// (command-prompt, list-keys, choose-tree, display-panes) draw into a pane
/// or nowhere at all for a control-mode client.
const PAR_TERM_UI_KEYS: &[char] = &[':', '?', 'w', 's', 'q', 'P', ','];

/// tmux prefix keys that open a tmux command prompt or status message,
/// which a control-mode client never shows: never routed to tmux.
const TMUX_PROMPT_KEYS: &[char] = &['$', '.', 'f', 'i'];

/// Tab navigation stays par-term's in a gateway tab, as the matching
/// chords do: par-term does not follow tmux's current window
/// (`%session-window-changed` is dropped in `par_term_tmux::parser_bridge`),
/// so tmux's `next-window` would move tmux and leave the visible tab put.
const TAB_NAV_KEYS: &[char] = &[
    'n', 'p', 'l', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
];

/// The key to run through tmux's prefix table in a tmux gateway tab
/// (UX.md K4: the tmux table is merged, so tmux muscle memory carries
/// over), or `None` when the key stays a par-term action. A key tmux's
/// table does not know stays par-term's too.
pub(crate) fn tmux_key(key: TableKey, vim_keys: bool) -> Option<Key> {
    let arrow_key = |dir: NavigationDirection| {
        Key::Named(match dir {
            Left => NamedKey::ArrowLeft,
            Right => NamedKey::ArrowRight,
            Up => NamedKey::ArrowUp,
            Down => NamedKey::ArrowDown,
        })
    };
    let candidate = match key {
        TableKey::Char('h') if vim_keys => arrow_key(Left),
        TableKey::Char('j') if vim_keys => arrow_key(Down),
        TableKey::Char('k') if vim_keys => arrow_key(Up),
        TableKey::Char('l') if vim_keys => arrow_key(Right),
        // tmux has no directional swap: these stay par-term's swap.
        TableKey::Char('H' | 'J' | 'K' | 'L') if vim_keys => return None,
        TableKey::Char('S') if vim_keys => Key::Character("L".into()),
        TableKey::Char(c)
            if PAR_TERM_UI_KEYS.contains(&c)
                || TMUX_PROMPT_KEYS.contains(&c)
                || TAB_NAV_KEYS.contains(&c) =>
        {
            return None;
        }
        TableKey::Char(c) => Key::Character(c.to_string().into()),
        TableKey::Arrow { dir, shift: false } => arrow_key(dir),
        TableKey::Arrow { shift: true, .. } | TableKey::Tab => return None,
        TableKey::Space => Key::Named(NamedKey::Space),
    };
    crate::tmux::translate_command_key(&candidate, ModifiersState::empty(), None)
        .is_some()
        .then_some(candidate)
}

/// tmux prefix keys with no par-term leader entry, listed in the which-key
/// overlay of a tmux gateway tab so they stay discoverable.
pub(crate) fn tmux_only_keys(vim_keys: bool) -> Vec<TableKey> {
    "()LS[]{}thjk"
        .chars()
        .map(TableKey::Char)
        .filter(|k| lookup(*k, vim_keys).is_none() && tmux_key(*k, vim_keys).is_some())
        .collect()
}
