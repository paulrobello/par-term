//! Overlay UI panel state for a window.
//!
//! Groups all transient overlay/modal/side-panel UI components together.
//! This reduces WindowState's field count and localises all panel visibility
//! logic to one struct.

use crate::ai_inspector::panel::AIInspectorPanel;
use crate::clipboard_history_ui::ClipboardHistoryUI;
use crate::close_confirmation_ui::CloseConfirmationUI;
use crate::command_history::CommandHistory;
use crate::command_history_ui::CommandHistoryUI;
use crate::config::Config;
use crate::help_ui::HelpUI;
use crate::integrations_ui::IntegrationsUI;
use crate::paste_special_ui::PasteSpecialUI;
use crate::profile::{ProfileManager, storage as profile_storage};
use crate::profile_drawer_ui::ProfileDrawerUI;
use crate::quit_confirmation_ui::QuitConfirmationUI;
use crate::remote_shell_install_ui::RemoteShellInstallUI;
use crate::search::SearchUI;
use crate::shader_install_ui::ShaderInstallUI;
use crate::ssh_connect_ui::SshConnectUI;
use crate::tmux_session_picker_ui::TmuxSessionPickerUI;
use crate::tmux_status_bar_ui::TmuxStatusBarUI;
use anyhow::Result;

/// All transient overlay / modal / side-panel UI state for a window.
pub(crate) struct OverlayUiState {
    pub(crate) help_ui: HelpUI,
    pub(crate) clipboard_history_ui: ClipboardHistoryUI,
    pub(crate) command_history_ui: CommandHistoryUI,
    /// Persistent command history model (backing command_history_ui)
    pub(crate) command_history: CommandHistory,
    /// Commands already synced from marks (avoids repeated adds).
    /// `pub(crate)` rather than fully private because access goes through
    /// `self.overlay_ui.synced_commands` from `impl WindowState` methods.
    pub(crate) synced_commands: std::collections::HashSet<String>,
    pub(crate) paste_special_ui: PasteSpecialUI,
    pub(crate) tmux_session_picker_ui: TmuxSessionPickerUI,
    pub(crate) tmux_status_bar_ui: TmuxStatusBarUI,
    pub(crate) search_ui: SearchUI,
    pub(crate) ai_inspector: AIInspectorPanel,
    /// Last known AI Inspector panel consumed width (logical pixels).
    pub(crate) last_inspector_width: f32,
    pub(crate) shader_install_ui: ShaderInstallUI,
    /// Receiver for shader installation results (from background thread)
    pub(crate) shader_install_receiver: Option<std::sync::mpsc::Receiver<Result<usize, String>>>,
    pub(crate) integrations_ui: IntegrationsUI,
    pub(crate) close_confirmation_ui: CloseConfirmationUI,
    pub(crate) quit_confirmation_ui: QuitConfirmationUI,
    pub(crate) remote_shell_install_ui: RemoteShellInstallUI,
    pub(crate) ssh_connect_ui: SshConnectUI,
    pub(crate) profile_drawer_ui: ProfileDrawerUI,
    pub(crate) profile_manager: ProfileManager,
}

impl OverlayUiState {
    pub(crate) fn new(config: &Config) -> Self {
        let command_history_max = config.command_history_max_entries;
        let profile_manager = match profile_storage::load_profiles() {
            Ok(manager) => manager,
            Err(e) => {
                log::warn!("Failed to load profiles: {}", e);
                ProfileManager::new()
            }
        };
        Self {
            help_ui: HelpUI::new(),
            clipboard_history_ui: ClipboardHistoryUI::new(),
            command_history_ui: CommandHistoryUI::new(),
            command_history: {
                let mut ch = CommandHistory::new(command_history_max);
                ch.load();
                ch
            },
            synced_commands: std::collections::HashSet::new(),
            paste_special_ui: PasteSpecialUI::new(),
            tmux_session_picker_ui: TmuxSessionPickerUI::new(),
            tmux_status_bar_ui: TmuxStatusBarUI::new(),
            search_ui: SearchUI::new(),
            ai_inspector: AIInspectorPanel::new(config),
            last_inspector_width: 0.0,
            shader_install_ui: ShaderInstallUI::new(),
            shader_install_receiver: None,
            integrations_ui: IntegrationsUI::new(),
            close_confirmation_ui: CloseConfirmationUI::new(),
            quit_confirmation_ui: QuitConfirmationUI::new(),
            remote_shell_install_ui: RemoteShellInstallUI::new(),
            ssh_connect_ui: SshConnectUI::new(),
            profile_drawer_ui: ProfileDrawerUI::new(),
            profile_manager,
        }
    }
}
