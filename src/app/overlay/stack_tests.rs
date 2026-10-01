//! Stack membership, proven on a live `WindowState` — each surface is opened
//! through its real entry point and the derived stack and the MP0 modal
//! guard are read back. These replace the source-scan pins that grepped the
//! body of `any_modal_ui_visible` for field names: the guard is now derived
//! from the stack, so what matters is behavior, not spelling.

use super::{OverlayId, OverlayKind, OverlayStack};
use crate::app::window_state::{PendingTriggerAction, WindowState};
use crate::config::Config;
use std::sync::Arc;

pub(super) fn window() -> WindowState {
    let runtime = Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime"),
    );
    WindowState::new(Config::default(), runtime)
}

/// Open `id` on `ws` through the entry point the user interaction uses,
/// then query the stack the way the next frame would — so opens in one
/// test land in the stack in the order they happened.
pub(super) fn open(ws: &mut WindowState, id: OverlayId) {
    open_raw(ws, id);
    let _ = ws.overlay_stack();
}

fn open_raw(ws: &mut WindowState, id: OverlayId) {
    let o = &mut ws.overlay_ui;
    match id {
        OverlayId::AiInspector => o.ai_inspector.open = true,
        OverlayId::ProfileDrawer => o.profile_drawer_ui.expanded = true,
        OverlayId::CopyMode => ws.copy_mode.active = true,
        OverlayId::CustomActionPrefix => ws.custom_action_prefix_state.enter(),
        OverlayId::Leader => ws.leader.arm(
            std::time::Instant::now(),
            crate::app::leader::ArmedBy::Leader,
            crate::app::leader::LeaderTiming::from_config(&ws.config.load().input),
        ),
        // Focus needs a live plugin overlay (the host ignores a focus
        // request for an unknown plugin); covered by the plugin overlay
        // tests and the kind/guard table test.
        OverlayId::PluginOverlayFocus => {}
        OverlayId::PaneHints => {
            ws.pane_hint_select = crate::app::pane_hint_select::PaneHintSelectState::Selecting {
                tab_id: 1,
                assignments: Vec::new(),
                typed: String::new(),
            }
        }
        OverlayId::ResizeMode => {
            ws.pane_resize_mode =
                crate::app::pane_resize_mode::PaneResizeModeState::Resizing { tab_id: 1 }
        }
        OverlayId::DemotePick => {
            ws.pane_transfer_state =
                crate::app::tab_ops::pane_transfer::PaneTransferState::DemotePickTab {
                    source_tab_id: 1,
                }
        }
        OverlayId::Help => o.help_ui.visible = true,
        OverlayId::ClipboardHistory => o.clipboard_history_ui.visible = true,
        OverlayId::CommandHistory => o.command_history_ui.open(),
        OverlayId::PasteSpecial => o.paste_special_ui.open("text".to_string()),
        OverlayId::Search => o.search_ui.open(),
        OverlayId::CommandPalette => o
            .command_palette
            .open(Vec::new(), &par_term_keybindings::KeybindingRegistry::new()),
        OverlayId::AgentUsage => o.agent_usage_panel.open(),
        OverlayId::PaneContextMenu => o.pane_context_menu.open(1, egui::Pos2::ZERO),
        OverlayId::TreePicker => o.tree_picker_ui.open(),
        OverlayId::SessionPicker => o.tmux_session_picker_ui.visible = true,
        OverlayId::TabContextMenu => ws.tab_bar_ui.test_open_context_menu(1),
        OverlayId::ProfileLauncher => o.profile_launcher_ui.open(Vec::new(), None),
        // The in-app menu's open flag is private to the menu widget and only
        // flips inside an egui frame; its membership is covered by the
        // kind/guard table test instead.
        OverlayId::AppMenu => {}
        OverlayId::PaneRename => o.pane_rename_ui.open(1, "pane", egui::Pos2::ZERO),
        OverlayId::SshConnect => o.ssh_connect_ui.open(false, 0),
        OverlayId::ShaderInstall => o.shader_install_ui.show_dialog(),
        OverlayId::Integrations => o.integrations_ui.show_dialog(),
        OverlayId::RemoteShellInstall => o.remote_shell_install_ui.show_dialog(),
        OverlayId::UpdateDialog => {
            ws.update_state.show_dialog = true;
            ws.update_state.last_result =
                Some(par_term_update::update_checker::UpdateCheckResult::UpToDate);
        }
        OverlayId::TriggerConfirm => {
            ws.trigger_state
                .pending_trigger_actions
                .push(PendingTriggerAction {
                    trigger_id: 0,
                    trigger_name: "t".to_string(),
                    action: par_term_emu_core_rust::terminal::ActionResult::RunCommand {
                        trigger_id: 0,
                        command: "echo".to_string(),
                        args: Vec::new(),
                    },
                    description: "d".to_string(),
                    target: None,
                })
        }
        OverlayId::AgentCommandConfirm => {
            let file = par_term_config::agent_commands::AgentCommandFile {
                created_by: par_term_config::agent_commands::CommandAuthor::User,
                source_agent: None,
                created_at: None,
                action: par_term_config::CustomActionConfig::ShellCommand {
                    id: "stack-test".to_string(),
                    title: "stack test".to_string(),
                    command: "echo".to_string(),
                    args: Vec::new(),
                    notify_on_success: false,
                    timeout_secs: 30,
                    capture_output: false,
                    keybinding: None,
                    prefix_char: None,
                    keybinding_enabled: true,
                    description: None,
                },
            };
            ws.agent_commands.request_confirmation(file);
        }
        OverlayId::CloseConfirm => o.close_confirmation_ui.show_for_tab(1, "t", "sleep"),
        OverlayId::MuxLastTab => o.mux_last_tab_ui.show_for_session("s"),
        OverlayId::QuitConfirm => o.quit_confirmation_ui.show_confirmation(1, None),
    }
}

/// The set `any_modal_ui_visible()` covered before the stack existed
/// (MP0, B61). Deriving the guard from the stack must not add or drop one:
/// its callers route paste, IME, Tab-key focus, and hover focus on it.
const MP0_GUARD_SET: &[OverlayId] = &[
    OverlayId::Help,
    OverlayId::ClipboardHistory,
    OverlayId::CommandHistory,
    OverlayId::Search,
    OverlayId::CommandPalette,
    OverlayId::AgentUsage,
    OverlayId::SessionPicker,
    OverlayId::TreePicker,
    OverlayId::PaneContextMenu,
    OverlayId::ShaderInstall,
    OverlayId::Integrations,
    OverlayId::SshConnect,
    OverlayId::RemoteShellInstall,
    OverlayId::QuitConfirm,
    OverlayId::CloseConfirm,
    OverlayId::MuxLastTab,
    OverlayId::TriggerConfirm,
    OverlayId::AgentCommandConfirm,
    OverlayId::UpdateDialog,
    OverlayId::TabContextMenu,
    OverlayId::ProfileLauncher,
    OverlayId::DemotePick,
];

#[test]
fn the_guard_set_is_exactly_the_mp0_set() {
    let derived: Vec<OverlayId> = OverlayId::ALL
        .iter()
        .copied()
        .filter(|id| id.guards_terminal())
        .collect();
    for id in MP0_GUARD_SET {
        assert!(
            derived.contains(id),
            "{id:?} dropped out of the modal guard"
        );
    }
    for id in &derived {
        assert!(
            MP0_GUARD_SET.contains(id),
            "{id:?} joined the modal guard — its callers (paste routing, IME, \
             Tab focus) would change behavior"
        );
    }
}

#[test]
fn every_overlay_opened_for_real_joins_the_stack_and_the_guard_follows_its_kind() {
    for &id in OverlayId::ALL {
        if matches!(id, OverlayId::AppMenu | OverlayId::PluginOverlayFocus) {
            continue;
        }
        let mut ws = window();
        assert!(
            ws.overlay_stack().entries().is_empty(),
            "a fresh window has no overlays"
        );
        assert!(!ws.any_modal_ui_visible());
        open(&mut ws, id);
        let stack = ws.overlay_stack();
        assert_eq!(stack.entries(), &[id], "{id:?} must be the only entry");
        assert_eq!(
            ws.any_modal_ui_visible(),
            id.guards_terminal(),
            "{id:?}: the modal guard must follow the overlay's kind"
        );
    }
}

#[test]
fn overlay_component_visibility_is_what_the_stack_reads() {
    // MP1 Q1: the stack is an OverlayId registry; for the surfaces that
    // implement traits::OverlayComponent, the registry's open state must
    // be exactly the component's is_visible(), closed and open.
    use crate::traits::OverlayComponent;
    fn visible(ws: &WindowState, id: OverlayId) -> bool {
        let o = &ws.overlay_ui;
        match id {
            OverlayId::SshConnect => OverlayComponent::is_visible(&o.ssh_connect_ui),
            OverlayId::CommandHistory => OverlayComponent::is_visible(&o.command_history_ui),
            OverlayId::MuxLastTab => OverlayComponent::is_visible(&o.mux_last_tab_ui),
            OverlayId::Integrations => OverlayComponent::is_visible(&o.integrations_ui),
            OverlayId::CloseConfirm => OverlayComponent::is_visible(&o.close_confirmation_ui),
            OverlayId::PasteSpecial => OverlayComponent::is_visible(&o.paste_special_ui),
            OverlayId::ClipboardHistory => OverlayComponent::is_visible(&o.clipboard_history_ui),
            OverlayId::RemoteShellInstall => {
                OverlayComponent::is_visible(&o.remote_shell_install_ui)
            }
            OverlayId::ShaderInstall => OverlayComponent::is_visible(&o.shader_install_ui),
            OverlayId::QuitConfirm => OverlayComponent::is_visible(&o.quit_confirmation_ui),
            other => unreachable!("{other:?} does not implement OverlayComponent"),
        }
    }
    for id in [
        OverlayId::SshConnect,
        OverlayId::CommandHistory,
        OverlayId::MuxLastTab,
        OverlayId::Integrations,
        OverlayId::CloseConfirm,
        OverlayId::PasteSpecial,
        OverlayId::ClipboardHistory,
        OverlayId::RemoteShellInstall,
        OverlayId::ShaderInstall,
        OverlayId::QuitConfirm,
    ] {
        let mut ws = window();
        assert!(
            !visible(&ws, id) && !ws.overlay_is_open(id),
            "{id:?} starts closed"
        );
        open(&mut ws, id);
        assert!(visible(&ws, id), "{id:?}: the real entry point shows it");
        assert!(ws.overlay_is_open(id), "{id:?}: the stack sees it");
        ws.close_overlay(id);
        assert_eq!(
            visible(&ws, id),
            ws.overlay_is_open(id),
            "{id:?}: the stack's close path and the component agree"
        );
    }
}

#[test]
fn b61_dialogs_block_the_terminal() {
    // The B61 list, opened for real (was a source-scan pin on the guard's
    // body in chord_tests): keys typed while any of these is open must not
    // reach the PTY (UX.md RT22). The e2e half is tests/ui/b61_modal_guard.json.
    for id in [
        OverlayId::CloseConfirm,
        OverlayId::MuxLastTab,
        OverlayId::TriggerConfirm,
        OverlayId::AgentCommandConfirm,
        OverlayId::UpdateDialog,
        OverlayId::TabContextMenu,
        OverlayId::ProfileLauncher,
        OverlayId::DemotePick,
        OverlayId::CommandPalette,
        OverlayId::AgentUsage,
    ] {
        let mut ws = window();
        open(&mut ws, id);
        assert!(ws.any_modal_ui_visible(), "{id:?} must block the terminal");
    }
}

#[test]
fn the_profile_drawer_is_a_panel_that_asks_egui_for_the_keyboard() {
    // B61: the drawer is a side panel, not a modal — it blocks keys only
    // while its tag filter holds egui focus, so it must be one of the
    // overlays `is_egui_using_keyboard` asks egui about.
    assert_eq!(OverlayId::ProfileDrawer.kind(), OverlayKind::Panel);
    assert!(!OverlayId::ProfileDrawer.guards_terminal());
    assert!(OverlayId::ProfileDrawer.may_hold_egui_focus());
}

#[test]
fn stack_order_is_bottom_to_top_whatever_the_open_order() {
    let stack = OverlayStack::from_open([
        OverlayId::QuitConfirm,
        OverlayId::AiInspector,
        OverlayId::CommandPalette,
    ]);
    assert_eq!(
        stack.entries(),
        &[
            OverlayId::AiInspector,
            OverlayId::CommandPalette,
            OverlayId::QuitConfirm
        ]
    );
}

#[test]
fn guard_names_keep_the_report_strings() {
    let mut ws = window();
    open(&mut ws, OverlayId::QuitConfirm);
    open(&mut ws, OverlayId::AiInspector);
    open(&mut ws, OverlayId::DemotePick);
    assert_eq!(
        ws.overlay_stack().guard_names(),
        vec![
            "demote_chooser".to_string(),
            "quit_confirmation_ui".to_string()
        ],
        "panels do not appear in the modal report; names are unchanged"
    );
}

#[test]
fn no_overlay_writes_the_global_style() {
    // UX.md OV1 / RT23: a per-panel ctx.set_global_style changed every
    // later panel's look (which one opened first decided it). Overlays
    // style their own `ui` via overlay::theme::solid_panel instead.
    let needle = ["set_global", "_style("].join("");
    for (path, source) in [
        ("help_ui.rs", include_str!("../../help_ui.rs")),
        ("search/mod.rs", include_str!("../../search/mod.rs")),
        (
            "tmux_session_picker_ui.rs",
            include_str!("../../tmux_session_picker_ui.rs"),
        ),
        (
            "shader_install_ui.rs",
            include_str!("../../shader_install_ui.rs"),
        ),
        (
            "integrations_ui.rs",
            include_str!("../../integrations_ui.rs"),
        ),
    ] {
        assert!(
            !source.contains(&needle),
            "{path} writes the global egui style again"
        );
    }
}
