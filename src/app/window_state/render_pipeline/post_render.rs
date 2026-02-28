//! Post-render action dispatch for the render pipeline.
//!
//! `update_post_render_state` consumes the `PostRenderActions` collected
//! during `submit_gpu_frame` and dispatches them to the appropriate handlers
//! (tab bar, clipboard, search, AI inspector, tmux, shader install, etc.).

use super::{PostRenderActions, ShaderInstallPrompt};
use crate::app::window_state::WindowState;
use crate::close_confirmation_ui::CloseConfirmAction;
use crate::command_history_ui::CommandHistoryAction;
use crate::paste_special_ui::PasteSpecialAction;
use crate::profile_drawer_ui::ProfileDrawerAction;
use crate::quit_confirmation_ui::QuitConfirmAction;
use crate::remote_shell_install_ui::{RemoteShellInstallAction, RemoteShellInstallUI};
use crate::shader_install_ui::ShaderInstallResponse;
use crate::ssh_connect_ui::SshConnectAction;
use crate::tmux_session_picker_ui::SessionPickerAction;

impl WindowState {
    /// Handle all actions collected during the render pass and finalize frame timing.
    pub(super) fn update_post_render_state(&mut self, actions: PostRenderActions) {
        let PostRenderActions {
            clipboard,
            command_history,
            paste_special,
            session_picker,
            tab_action,
            shader_install,
            integrations,
            search,
            inspector,
            profile_drawer,
            close_confirm,
            quit_confirm,
            remote_install,
            ssh_connect,
            save_config: _,
        } = actions;

        // Sync AI Inspector panel width after the render pass.
        // This catches drag-resize changes that update self.overlay_ui.ai_inspector.width during show().
        // Done here to avoid borrow conflicts with the renderer block above.
        self.sync_ai_inspector_width();

        // Handle tab bar actions collected during egui rendering
        self.handle_tab_bar_action_after_render(tab_action);

        // Handle clipboard actions collected during egui rendering
        self.handle_clipboard_history_action_after_render(clipboard);

        // Handle command history actions collected during egui rendering
        match command_history {
            CommandHistoryAction::Insert(command) => {
                self.paste_text(&command);
                log::info!(
                    "Inserted command from history: {}",
                    &command[..command.len().min(60)]
                );
            }
            CommandHistoryAction::None => {}
        }

        // Handle close confirmation dialog actions
        match close_confirm {
            CloseConfirmAction::Close { tab_id, pane_id } => {
                // User confirmed close - close the tab/pane
                if let Some(pane_id) = pane_id {
                    // Close specific pane
                    if let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                        && let Some(pm) = tab.pane_manager_mut()
                    {
                        pm.close_pane(pane_id);
                        log::info!("Force-closed pane {} in tab {}", pane_id, tab_id);
                    }
                } else {
                    // Close entire tab
                    self.tab_manager.close_tab(tab_id);
                    log::info!("Force-closed tab {}", tab_id);
                }
                self.focus_state.needs_redraw = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            CloseConfirmAction::Cancel => {
                // User cancelled - do nothing, dialog already hidden
                log::debug!("Close confirmation cancelled");
            }
            CloseConfirmAction::None => {}
        }

        // Handle quit confirmation dialog actions
        match quit_confirm {
            QuitConfirmAction::Quit => {
                // User confirmed quit - proceed with shutdown
                log::info!("Quit confirmed by user");
                self.perform_shutdown();
            }
            QuitConfirmAction::Cancel => {
                log::debug!("Quit confirmation cancelled");
            }
            QuitConfirmAction::None => {}
        }

        // Handle remote shell integration install action
        match remote_install {
            RemoteShellInstallAction::Install => {
                // Send the install command via paste_text() which uses the same
                // code path as Cmd+V paste — handles bracketed paste mode and
                // correctly forwards through SSH sessions.
                let command = RemoteShellInstallUI::install_command();
                // paste_text appends \r internally via term.paste()
                self.paste_text(&format!("{}\n", command));
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            RemoteShellInstallAction::Cancel => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            RemoteShellInstallAction::None => {}
        }

        // Handle SSH Quick Connect actions
        match ssh_connect {
            SshConnectAction::Connect {
                host,
                profile_override: _,
            } => {
                // Build SSH command and write it to the active terminal's PTY
                let args = host.ssh_args();
                let ssh_cmd = format!("ssh {}\n", args.join(" "));
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(term) = tab.terminal.try_write()
                {
                    let _ = term.write_str(&ssh_cmd);
                }
                log::info!(
                    "SSH Quick Connect: connecting to {}",
                    host.connection_string()
                );
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            SshConnectAction::Cancel => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            SshConnectAction::None => {}
        }

        // Handle paste special actions collected during egui rendering
        match paste_special {
            PasteSpecialAction::Paste(content) => {
                self.paste_text(&content);
                log::debug!("Pasted transformed text ({} chars)", content.len());
            }
            PasteSpecialAction::None => {}
        }

        // Handle search actions collected during egui rendering
        match search {
            crate::search::SearchAction::ScrollToMatch(offset) => {
                self.set_scroll_target(offset);
                self.focus_state.needs_redraw = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            crate::search::SearchAction::Close => {
                self.focus_state.needs_redraw = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            crate::search::SearchAction::None => {}
        }

        // Handle AI Inspector actions collected during egui rendering
        self.handle_inspector_action_after_render(inspector);

        // Handle tmux session picker actions collected during egui rendering
        // Uses gateway mode: writes tmux commands to existing PTY instead of spawning process
        match session_picker {
            SessionPickerAction::Attach(session_name) => {
                crate::debug_info!(
                    "TMUX",
                    "Session picker: attaching to '{}' via gateway",
                    session_name
                );
                if let Err(e) = self.attach_tmux_gateway(&session_name) {
                    log::error!("Failed to attach to tmux session '{}': {}", session_name, e);
                    self.show_toast(format!("Failed to attach: {}", e));
                } else {
                    crate::debug_info!("TMUX", "Gateway initiated for session '{}'", session_name);
                    self.show_toast(format!("Connecting to session '{}'...", session_name));
                }
                self.focus_state.needs_redraw = true;
            }
            SessionPickerAction::CreateNew(name) => {
                crate::debug_info!(
                    "TMUX",
                    "Session picker: creating new session {:?} via gateway",
                    name
                );
                if let Err(e) = self.initiate_tmux_gateway(name.as_deref()) {
                    log::error!("Failed to create tmux session: {}", e);
                    crate::debug_error!("TMUX", "Failed to initiate gateway: {}", e);
                    self.show_toast(format!("Failed to create session: {}", e));
                } else {
                    let msg = match name {
                        Some(ref n) => format!("Creating session '{}'...", n),
                        None => "Creating new tmux session...".to_string(),
                    };
                    crate::debug_info!("TMUX", "Gateway initiated: {}", msg);
                    self.show_toast(msg);
                }
                self.focus_state.needs_redraw = true;
            }
            SessionPickerAction::None => {}
        }

        // Check for shader installation completion from background thread
        if let Some(ref rx) = self.overlay_ui.shader_install_receiver
            && let Ok(result) = rx.try_recv()
        {
            match result {
                Ok(count) => {
                    log::info!("Successfully installed {} shaders", count);
                    self.overlay_ui
                        .shader_install_ui
                        .set_success(&format!("Installed {} shaders!", count));

                    // Update config to mark as installed
                    self.config.shader_install_prompt = ShaderInstallPrompt::Installed;
                    if let Err(e) = self.save_config_debounced() {
                        log::error!("Failed to save config after shader install: {}", e);
                    }
                }
                Err(e) => {
                    log::error!("Failed to install shaders: {}", e);
                    self.overlay_ui.shader_install_ui.set_error(&e);
                }
            }
            self.overlay_ui.shader_install_receiver = None;
            self.focus_state.needs_redraw = true;
        }

        // Handle shader install responses
        match shader_install {
            ShaderInstallResponse::Install => {
                log::info!("User requested shader installation");
                self.overlay_ui
                    .shader_install_ui
                    .set_installing("Downloading shaders...");
                self.focus_state.needs_redraw = true;

                // Spawn installation in background thread so UI can show progress
                let (tx, rx) = std::sync::mpsc::channel();
                self.overlay_ui.shader_install_receiver = Some(rx);

                std::thread::spawn(move || {
                    let result = crate::shader_install_ui::install_shaders_headless();
                    let _ = tx.send(result);
                });

                // Request redraw so the spinner shows
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            ShaderInstallResponse::Never => {
                log::info!("User declined shader installation (never ask again)");
                self.overlay_ui.shader_install_ui.hide();

                // Update config to never ask again
                self.config.shader_install_prompt = ShaderInstallPrompt::Never;
                if let Err(e) = self.save_config_debounced() {
                    log::error!("Failed to save config after declining shaders: {}", e);
                }
            }
            ShaderInstallResponse::Later => {
                log::info!("User deferred shader installation");
                self.overlay_ui.shader_install_ui.hide();
                // Config remains "ask" - will prompt again on next startup
            }
            ShaderInstallResponse::None => {}
        }

        // Handle integrations welcome dialog responses
        self.handle_integrations_response(&integrations);

        // Handle profile drawer actions
        match profile_drawer {
            ProfileDrawerAction::OpenProfile(id) => {
                self.open_profile(id);
            }
            ProfileDrawerAction::ManageProfiles => {
                // Open settings window to Profiles tab instead of terminal-embedded modal
                self.overlay_state.open_settings_window_requested = true;
                self.overlay_state.open_settings_profiles_tab = true;
            }
            ProfileDrawerAction::None => {}
        }

        if let Some(start) = self.debug.render_start {
            let total = start.elapsed();
            if total.as_millis() > 10 {
                log::debug!(
                    "TIMING: AbsoluteTotal={:.2}ms (from function start to end)",
                    total.as_secs_f64() * 1000.0
                );
            }
        }
    }
}
