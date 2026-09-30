//! The per-window overlay framework (UX.md Part III, OV1/OV2).
//!
//! Every surface that draws above the terminal and can own keys — dialogs,
//! pickers, menus, side panels, and modal modes — is an [`OverlayId`] with
//! an [`OverlayKind`]. The window's [`OverlayStack`] is the ordered set of
//! overlays open right now, bottom to top, and key routing reads it instead
//! of an enumerated list of dialogs.
//!
//! The stack is *derived*, not stored: [`WindowState::overlay_stack`]
//! rebuilds it from each surface's own visibility every time it is asked.
//! Surfaces keep their state where it already lives, so opening a dialog
//! through its existing entry point is all it takes to join the stack, and
//! the stack can never disagree with what is drawn.
//!
//! - [`routing`] is the pure key-routing contract (OV2) — decided from the
//!   stack and a key description alone, so it is unit-tested without a
//!   window.
//! - [`confirm`] is the shared `ConfirmDialog` (OV3).
//! - [`picker`] is the shared list/picker component (OV5).
//! - [`toast`] is the toast queue (OV7).

pub(crate) mod confirm;
pub(crate) mod inline_edit;
pub(crate) mod key_routing;
pub(crate) mod palette_rows;
pub(crate) mod picker;
pub(crate) mod routing;
pub(crate) mod stack_state;
pub(crate) mod theme;
pub(crate) mod toast;

#[cfg(test)]
mod routing_live_tests;
#[cfg(test)]
mod stack_tests;

/// How an overlay takes part in key routing (OV2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlayKind {
    /// A dialog that must be answered: consumes every key it does not use,
    /// and no other overlay's chord replaces it.
    Modal,
    /// A transient chooser (palette, pickers, menus, history panels):
    /// consumes every key it does not use; another overlay's chord replaces
    /// it, and its own toggle chord closes it.
    Popup,
    /// A docked panel (assistant, profile drawer): owns keys only while one
    /// of its widgets holds keyboard focus.
    Panel,
    /// A keyboard mode over the terminal (copy mode, pane hints, resize,
    /// demote pick, action prefix): consumes the keys it defines.
    Mode,
}

impl OverlayKind {
    /// Stacking layer: higher layers sit above lower ones whatever the
    /// open order.
    pub(crate) const fn layer(self) -> u8 {
        match self {
            OverlayKind::Panel => 0,
            OverlayKind::Mode => 1,
            OverlayKind::Popup => 2,
            OverlayKind::Modal => 3,
        }
    }
}

/// Every overlay the stack knows. The order here is only the tie-break for
/// overlays first seen open in the same query; the stack itself orders by
/// layer, then by open order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum OverlayId {
    // Panels
    AiInspector,
    ProfileDrawer,
    // Modes
    CopyMode,
    CustomActionPrefix,
    PluginOverlayFocus,
    PaneHints,
    ResizeMode,
    DemotePick,
    // Popups
    Help,
    ClipboardHistory,
    CommandHistory,
    PasteSpecial,
    Search,
    CommandPalette,
    AgentUsage,
    PaneContextMenu,
    TreePicker,
    SessionPicker,
    TabContextMenu,
    NewTabProfileMenu,
    AppMenu,
    PaneRename,
    SshConnect,
    // Modals
    ShaderInstall,
    Integrations,
    RemoteShellInstall,
    UpdateDialog,
    TriggerConfirm,
    AgentCommandConfirm,
    CloseConfirm,
    MuxLastTab,
    QuitConfirm,
}

impl OverlayId {
    /// Every overlay.
    pub(crate) const ALL: &'static [OverlayId] = &[
        OverlayId::AiInspector,
        OverlayId::ProfileDrawer,
        OverlayId::CopyMode,
        OverlayId::CustomActionPrefix,
        OverlayId::PluginOverlayFocus,
        OverlayId::PaneHints,
        OverlayId::ResizeMode,
        OverlayId::DemotePick,
        OverlayId::Help,
        OverlayId::ClipboardHistory,
        OverlayId::CommandHistory,
        OverlayId::PasteSpecial,
        OverlayId::Search,
        OverlayId::CommandPalette,
        OverlayId::AgentUsage,
        OverlayId::PaneContextMenu,
        OverlayId::TreePicker,
        OverlayId::SessionPicker,
        OverlayId::TabContextMenu,
        OverlayId::NewTabProfileMenu,
        OverlayId::AppMenu,
        OverlayId::PaneRename,
        OverlayId::SshConnect,
        OverlayId::ShaderInstall,
        OverlayId::Integrations,
        OverlayId::RemoteShellInstall,
        OverlayId::UpdateDialog,
        OverlayId::TriggerConfirm,
        OverlayId::AgentCommandConfirm,
        OverlayId::CloseConfirm,
        OverlayId::MuxLastTab,
        OverlayId::QuitConfirm,
    ];

    /// The overlay's routing kind.
    pub(crate) const fn kind(self) -> OverlayKind {
        use OverlayId::*;
        match self {
            AiInspector | ProfileDrawer => OverlayKind::Panel,
            CopyMode | CustomActionPrefix | PluginOverlayFocus | PaneHints | ResizeMode
            | DemotePick => OverlayKind::Mode,
            Help | ClipboardHistory | CommandHistory | PasteSpecial | Search | CommandPalette
            | AgentUsage | PaneContextMenu | TreePicker | SessionPicker | TabContextMenu
            | NewTabProfileMenu | AppMenu | PaneRename | SshConnect => OverlayKind::Popup,
            ShaderInstall | Integrations | RemoteShellInstall | UpdateDialog | TriggerConfirm
            | AgentCommandConfirm | CloseConfirm | MuxLastTab | QuitConfirm => OverlayKind::Modal,
        }
    }

    /// The stable name reported by the `--ui-test` observation (`modals`)
    /// and used in logs. Names predate the stack and are kept verbatim.
    pub(crate) const fn name(self) -> &'static str {
        use OverlayId::*;
        match self {
            AiInspector => "ai_inspector",
            ProfileDrawer => "profile_drawer",
            CopyMode => "copy_mode",
            CustomActionPrefix => "custom_action_prefix",
            PluginOverlayFocus => "plugin_overlay_focus",
            PaneHints => "pane_hints",
            ResizeMode => "resize_mode",
            DemotePick => "demote_chooser",
            Help => "help_ui",
            ClipboardHistory => "clipboard_history_ui",
            CommandHistory => "command_history_ui",
            PasteSpecial => "paste_special_ui",
            Search => "search_ui",
            CommandPalette => "command_palette",
            AgentUsage => "agent_usage_panel",
            PaneContextMenu => "pane_context_menu",
            TreePicker => "tree_picker_ui",
            SessionPicker => "tmux_session_picker_ui",
            TabContextMenu => "tab_context_menu",
            NewTabProfileMenu => "new_tab_profile_menu",
            AppMenu => "app_menu",
            PaneRename => "pane_rename",
            SshConnect => "ssh_connect_ui",
            ShaderInstall => "shader_install_ui",
            Integrations => "integrations_ui",
            RemoteShellInstall => "remote_shell_install_ui",
            UpdateDialog => "update_dialog",
            TriggerConfirm => "trigger_confirm",
            AgentCommandConfirm => "agent_command_confirm",
            CloseConfirm => "close_confirmation_ui",
            MuxLastTab => "mux_last_tab_ui",
            QuitConfirm => "quit_confirmation_ui",
        }
    }

    /// Whether this overlay blocks terminal input as a whole while open —
    /// the membership of `any_modal_ui_visible()` (the MP0 guard).
    ///
    /// Every Modal and Popup is guarded except three that own keys another
    /// way: the Linux in-app menu and the pane rename field (egui keyboard
    /// focus, the way a panel does) and paste special (its key layer
    /// consumes every key). Of the
    /// modes, only the demote pick is guarded: copy mode, pane hints,
    /// resize, and the action prefix consume their keys through their own
    /// handlers, and guarding them would re-route paste and IME to egui
    /// while they run.
    pub(crate) const fn guards_terminal(self) -> bool {
        match self.kind() {
            OverlayKind::Modal => true,
            OverlayKind::Popup => !matches!(
                self,
                OverlayId::AppMenu | OverlayId::PasteSpecial | OverlayId::PaneRename
            ),
            OverlayKind::Panel => false,
            OverlayKind::Mode => matches!(self, OverlayId::DemotePick),
        }
    }

    /// Whether egui is asked for keyboard ownership while this overlay is
    /// open (`is_egui_using_keyboard`): every guarding overlay, the docked
    /// panels, and the two focus-owning popups (pane rename, in-app menu).
    /// Paste special and the keyboard modes own keys through their own
    /// handlers instead.
    pub(crate) const fn may_hold_egui_focus(self) -> bool {
        self.guards_terminal()
            || matches!(self.kind(), OverlayKind::Panel)
            || matches!(self, OverlayId::PaneRename | OverlayId::AppMenu)
    }

    /// The registry action whose chord toggles this overlay, if any. While
    /// the overlay is on top, that chord closes it (OV2).
    pub(crate) const fn toggle_action(self) -> Option<&'static str> {
        use OverlayId::*;
        Some(match self {
            AiInspector => "toggle_ai_inspector",
            ProfileDrawer => "toggle_profile_drawer",
            CopyMode => "toggle_copy_mode",
            Help => "toggle_help",
            ClipboardHistory => "toggle_clipboard_history",
            CommandHistory => "toggle_command_history",
            Search => "toggle_search",
            CommandPalette => "toggle_command_palette",
            AgentUsage => "toggle_agent_usage_panel",
            TreePicker => "toggle_tree_picker",
            SessionPicker => "toggle_session_picker",
            AppMenu => "toggle_menu",
            _ => return None,
        })
    }

    /// The overlay a registry action opens, if it opens one — used to let
    /// another overlay's chord replace the top Popup (OV2).
    pub(crate) fn opened_by_action(action: &str) -> Option<OverlayId> {
        OverlayId::ALL
            .iter()
            .copied()
            .find(|id| id.toggle_action() == Some(action))
            .or(match action {
                "ssh_quick_connect" => Some(OverlayId::SshConnect),
                "paste_special" => Some(OverlayId::PasteSpecial),
                _ => None,
            })
    }
}

/// The overlays open in one window, bottom to top.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct OverlayStack {
    entries: Vec<OverlayId>,
}

impl OverlayStack {
    /// Build a stack from the open overlays, given in the order they were
    /// opened (oldest first). Layers stack by kind — panels, then modes,
    /// then popups, then modal dialogs, matching the egui draw order where
    /// dialogs float above everything — and within a layer the most
    /// recently opened overlay is on top.
    pub(crate) fn from_open(open: impl IntoIterator<Item = OverlayId>) -> Self {
        let mut entries: Vec<OverlayId> = open.into_iter().collect();
        entries.sort_by_key(|id| id.kind().layer());
        Self { entries }
    }

    /// The open overlays, bottom to top.
    pub(crate) fn entries(&self) -> &[OverlayId] {
        &self.entries
    }

    /// The topmost overlay that is a routing target: Modal, Popup, and Mode
    /// always are; a Panel only while it holds keyboard focus (OV2 — an
    /// open but unfocused assistant panel must not eat Escape from vim).
    pub(crate) fn top_key_owner(&self, panel_focused: bool) -> Option<OverlayId> {
        self.entries
            .iter()
            .rev()
            .copied()
            .find(|id| id.kind() != OverlayKind::Panel || panel_focused)
    }

    /// Names of the guarding overlays, bottom to top (the `--ui-test`
    /// `modals` observation).
    pub(crate) fn guard_names(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|id| id.guards_terminal())
            .map(|id| id.name().to_string())
            .collect()
    }
}
