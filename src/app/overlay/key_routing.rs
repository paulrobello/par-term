//! `WindowState` side of overlay key routing (UX.md OV2): describe a key
//! press as [`KeyFacts`], ask [`route_key`], and apply the route. The pure
//! decision lives in [`super::routing`]; this module owns only the effects.

use super::OverlayId;
use super::routing::{KeyFacts, KeyRoute, route_key};
use crate::app::window_state::WindowState;
use winit::keyboard::{Key, NamedKey, PhysicalKey};

impl WindowState {
    /// Describe a key press for routing. The registry lookup is the same
    /// one terminal dispatch performs, so the chord that would run an
    /// action with no overlay open is the chord routing sees.
    pub(crate) fn overlay_key_facts(
        &self,
        logical_key: &Key,
        physical_key: PhysicalKey,
        modifiers: &winit::event::Modifiers,
    ) -> KeyFacts {
        let config = self.config.load();
        let bound_action = self
            .keybinding_registry
            .lookup_with_key_fields(
                logical_key,
                physical_key,
                modifiers,
                &config.input.modifier_remapping,
                config.input.use_physical_keys,
            )
            .filter(|action| *action != par_term_keybindings::PASS_TO_TERMINAL)
            .map(str::to_string);
        let state = modifiers.state();
        let is_function_key = matches!(
            logical_key,
            Key::Named(
                NamedKey::F1
                    | NamedKey::F2
                    | NamedKey::F3
                    | NamedKey::F4
                    | NamedKey::F5
                    | NamedKey::F6
                    | NamedKey::F7
                    | NamedKey::F8
                    | NamedKey::F9
                    | NamedKey::F10
                    | NamedKey::F11
                    | NamedKey::F12
            )
        );
        KeyFacts {
            is_escape: matches!(logical_key, Key::Named(NamedKey::Escape)),
            bound_action,
            is_command_chord: state.super_key()
                || state.control_key()
                || state.alt_key()
                || is_function_key,
        }
    }

    /// Route one key press through the overlay stack.
    pub(crate) fn route_overlay_key(&self, key: &KeyFacts) -> KeyRoute {
        let egui_focused = self
            .egui
            .ctx
            .as_ref()
            .is_some_and(|ctx| ctx.egui_wants_keyboard_input());
        route_key(&self.overlay_stack(), key, egui_focused)
    }

    /// Route a winit key event. Only a first press acts on the stack: a
    /// release or auto-repeat the stack would have resolved is consumed
    /// instead, so releasing or holding Escape cannot close a second
    /// overlay (and the release of the chord that opened an overlay cannot
    /// close it again).
    pub(crate) fn route_window_key(&self, event: &winit::event::KeyEvent) -> KeyRoute {
        let facts = self.overlay_key_facts(
            &event.logical_key,
            event.physical_key,
            &self.input_handler.modifiers,
        );
        let route = self.route_overlay_key(&facts);
        if event.state == winit::event::ElementState::Pressed && !event.repeat {
            return route;
        }
        match route {
            KeyRoute::CloseTop(_) | KeyRoute::Replace { .. } | KeyRoute::OpenAbove { .. } => {
                KeyRoute::Consume
            }
            other => other,
        }
    }

    /// Apply the stack's part of a route: close the top overlay, or close
    /// it and run the chord that replaces it. Returns whether the route was
    /// fully handled here (the key press is then done).
    pub(crate) fn apply_overlay_route(&mut self, route: &KeyRoute) -> bool {
        match route {
            KeyRoute::CloseTop(id) => {
                self.close_overlay(*id);
                true
            }
            KeyRoute::Replace { close, action } => {
                self.close_overlay(*close);
                self.execute_keybinding_action(action);
                true
            }
            KeyRoute::OpenAbove { action } => {
                self.execute_keybinding_action(action);
                true
            }
            KeyRoute::Terminal | KeyRoute::ToOverlay(_) | KeyRoute::Consume => false,
        }
    }

    /// Close overlay `id` the way its own Cancel / close path does. A
    /// dialog closed from here is a Cancel: it produces no action.
    pub(crate) fn close_overlay(&mut self, id: OverlayId) {
        let o = &mut self.overlay_ui;
        match id {
            OverlayId::AiInspector => {
                crate::app::input_events::keybinding_view_actions::toggle_ai_inspector(self);
            }
            OverlayId::ProfileDrawer => o.profile_drawer_ui.expanded = false,
            OverlayId::CopyMode => self.exit_copy_mode(),
            OverlayId::CustomActionPrefix => self.custom_action_prefix_state.exit(),
            OverlayId::Leader => self.disarm_leader(),
            OverlayId::PluginOverlayFocus => self.resolve_focused_overlay_key(true),
            OverlayId::PaneHints => {
                self.resolve_pane_hint_select(None);
            }
            OverlayId::ResizeMode => self.exit_pane_resize_mode(),
            OverlayId::DemotePick => self.cancel_pane_transfer(),
            OverlayId::Help => o.help_ui.visible = false,
            OverlayId::ClipboardHistory => o.clipboard_history_ui.visible = false,
            OverlayId::CommandHistory => o.command_history_ui.close(),
            OverlayId::PasteSpecial => o.paste_special_ui.close(),
            OverlayId::Search => o.search_ui.close(),
            OverlayId::CommandPalette => o.command_palette.close(),
            OverlayId::AgentUsage => o.agent_usage_panel.close(),
            OverlayId::PaneContextMenu => o.pane_context_menu.close(),
            OverlayId::TreePicker => o.tree_picker_ui.close(),
            OverlayId::SessionPicker => o.tmux_session_picker_ui.hide(),
            OverlayId::TabContextMenu => self.tab_bar_ui.close_context_menu(),
            OverlayId::ProfileLauncher => o.profile_launcher_ui.close(),
            OverlayId::AppMenu => crate::menu::request_toggle(),
            OverlayId::PaneRename => o.pane_rename_ui.cancel(),
            OverlayId::SshConnect => o.ssh_connect_ui.close(),
            OverlayId::ShaderInstall => {
                if !o.shader_install_ui.installing {
                    o.shader_install_ui.hide();
                }
            }
            OverlayId::Integrations => {
                if !o.integrations_ui.installing {
                    o.integrations_ui.hide();
                }
            }
            OverlayId::RemoteShellInstall => o.remote_shell_install_ui.cancel(),
            // Escape is inert on these (see `escape_behavior`); they have no
            // toggle chord, so nothing routes a close to them.
            OverlayId::UpdateDialog
            | OverlayId::TriggerConfirm
            | OverlayId::AgentCommandConfirm => {}
            OverlayId::CloseConfirm => o.close_confirmation_ui.hide(),
            OverlayId::MuxLastTab => o.mux_last_tab_ui.hide(),
            OverlayId::QuitConfirm => o.quit_confirmation_ui.hide(),
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }
}
