//! Derive a window's [`OverlayStack`] from the live visibility of every
//! surface. The single place that knows how to ask each surface "are you
//! open?" — add a new overlay here and in [`OverlayId::ALL`].

use super::{OverlayId, OverlayStack};
use crate::app::window_state::WindowState;

impl WindowState {
    /// Whether overlay `id` is open in this window right now.
    pub(crate) fn overlay_is_open(&self, id: OverlayId) -> bool {
        let o = &self.overlay_ui;
        match id {
            OverlayId::AiInspector => o.ai_inspector.open,
            OverlayId::ProfileDrawer => o.profile_drawer_ui.expanded,
            OverlayId::CopyMode => self.copy_mode.active,
            OverlayId::CustomActionPrefix => self.custom_action_prefix_state.is_active(),
            OverlayId::Leader => self.leader.is_armed(),
            OverlayId::PluginOverlayFocus => {
                self.status_bar_ui.plugin_host().focused_overlay().is_some()
            }
            OverlayId::PaneHints => self.pane_hint_select.is_active(),
            OverlayId::ResizeMode => self.pane_resize_mode.is_active(),
            OverlayId::DemotePick => self.pane_transfer_state.is_active(),
            OverlayId::Help => o.help_ui.visible,
            OverlayId::ClipboardHistory => o.clipboard_history_ui.visible,
            OverlayId::CommandHistory => o.command_history_ui.visible,
            OverlayId::PasteSpecial => o.paste_special_ui.visible,
            OverlayId::Search => o.search_ui.visible,
            OverlayId::CommandPalette => o.command_palette.visible,
            OverlayId::AgentUsage => o.agent_usage_panel.visible,
            OverlayId::PaneContextMenu => o.pane_context_menu.is_open(),
            OverlayId::TreePicker => o.tree_picker_ui.visible,
            OverlayId::SessionPicker => o.tmux_session_picker_ui.visible,
            OverlayId::TabContextMenu => self.tab_bar_ui.is_context_menu_open(),
            OverlayId::NewTabProfileMenu => self.tab_bar_ui.show_new_tab_profile_menu,
            OverlayId::AppMenu => self.tab_bar_ui.is_app_menu_open(),
            OverlayId::PaneRename => o.pane_rename_ui.is_open(),
            OverlayId::SshConnect => o.ssh_connect_ui.is_visible(),
            OverlayId::ShaderInstall => o.shader_install_ui.visible,
            OverlayId::Integrations => o.integrations_ui.visible,
            OverlayId::RemoteShellInstall => o.remote_shell_install_ui.is_visible(),
            OverlayId::UpdateDialog => {
                self.update_state.show_dialog && self.update_state.last_result.is_some()
            }
            OverlayId::TriggerConfirm => !self.trigger_state.pending_trigger_actions.is_empty(),
            OverlayId::AgentCommandConfirm => !self.agent_commands.pending_confirmations.is_empty(),
            OverlayId::CloseConfirm => o.close_confirmation_ui.is_visible(),
            OverlayId::MuxLastTab => o.mux_last_tab_ui.is_visible(),
            OverlayId::QuitConfirm => o.quit_confirmation_ui.is_visible(),
        }
    }

    /// This window's overlay stack, rebuilt from live state (see the module
    /// docs of [`crate::app::overlay`] for why membership is derived, not
    /// stored).
    ///
    /// Only the *order* is remembered: each query reconciles the recorded
    /// open order with the live set — overlays that closed drop out,
    /// overlays that opened since the last query join on top — so the most
    /// recently opened overlay of a layer is the one keys reach first.
    pub(crate) fn overlay_stack(&self) -> OverlayStack {
        let mut order = self.overlay_order.borrow_mut();
        order.retain(|id| self.overlay_is_open(*id));
        for &id in OverlayId::ALL {
            if !order.contains(&id) && self.overlay_is_open(id) {
                order.push(id);
            }
        }
        OverlayStack::from_open(order.iter().copied())
    }
}
