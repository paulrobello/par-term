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
use crate::quit_confirmation_ui::QuitConfirmAction;
use crate::remote_shell_install_ui::{RemoteShellInstallAction, RemoteShellInstallUI};
use crate::shader_install_ui::ShaderInstallResponse;
use crate::ssh_connect_ui::SshConnectAction;
use crate::tmux_session_picker_ui::SessionPickerAction;
use par_term_config::text::truncate_chars;

impl WindowState {
    /// Handle all actions collected during the render pass and finalize frame timing.
    pub(super) fn update_post_render_state(&mut self, actions: PostRenderActions) {
        let PostRenderActions {
            clipboard,
            command_history,
            command_palette,
            paste_special,
            session_picker,
            pane_menu,
            tab_action,
            shader_install,
            integrations,
            search,
            inspector,
            profile_launcher,
            close_confirm,
            mux_last_tab,
            quit_confirm,
            remote_install,
            ssh_connect,
            save_config,
            demote,
            toast_events,
        } = actions;

        // Toast clicks (UX.md OV7): dismiss, or run the toast's action.
        for event in toast_events {
            match event {
                crate::app::overlay::toast::ToastEvent::Dismissed(id) => {
                    self.overlay_state.toasts.dismiss(id);
                }
                crate::app::overlay::toast::ToastEvent::Action { id, action_id } => {
                    self.overlay_state.toasts.dismiss(id);
                    self.execute_keybinding_action(&action_id);
                }
            }
            self.focus_state.needs_redraw = true;
        }

        // "Skip This Version" changed the config; the window manager writes it.
        if save_config {
            let version = self.config.load().updates.skipped_version.clone();
            self.render_loop
                .external_config_changes
                .push(crate::app::window_state::ExternalConfigChange::SkippedVersion(version));
        }

        // Handle demote direction-choice overlay action
        match demote {
            super::types::DemoteAction::Execute {
                source_tab_id,
                target_tab_id,
                target_pane_id,
                direction,
            } => {
                self.execute_demote(source_tab_id, target_tab_id, target_pane_id, direction);
            }
            super::types::DemoteAction::None => {}
        }

        // Invoke a palette selection through the one dispatch entry point, so
        // it takes the same miss-path (display table, then snippet:/action:/
        // restore_arrangement: prefixes) that a keybinding would.
        if let Some(action_id) = command_palette {
            self.execute_keybinding_action(&action_id);
        }

        // Sync AI Inspector panel width after the render pass.
        // This catches drag-resize changes that update self.overlay_ui.ai_inspector.width during show().
        // Done here to avoid borrow conflicts with the renderer block above.
        self.sync_ai_inspector_width();

        // Handle tab bar actions collected during egui rendering.
        // If egui didn't detect a tab click but a focus-click landed on a known
        // tab (stored in pending_focus_tab_switch), apply the switch now as a
        // fallback.  This covers the case where egui's pointer state was stale
        // when the window was unfocused and clicked_by() didn't fire.
        let effective_tab_action = if tab_action != crate::tab_bar_ui::TabBarAction::None {
            self.focus_state.pending_focus_tab_switch = None;
            tab_action
        } else if let Some(tab_id) = self.focus_state.pending_focus_tab_switch.take() {
            crate::tab_bar_ui::TabBarAction::SwitchTo(tab_id)
        } else {
            tab_action
        };
        self.handle_tab_bar_action_after_render(effective_tab_action);

        // Handle clipboard actions collected during egui rendering
        self.handle_clipboard_history_action_after_render(clipboard);

        // Handle command history actions collected during egui rendering
        match command_history {
            CommandHistoryAction::Insert(command) => {
                self.paste_text(&command);
                log::info!(
                    "Inserted command from history: {}",
                    truncate_chars(&command, 60)
                );
            }
            CommandHistoryAction::None => {}
        }

        // Handle close confirmation dialog actions
        match close_confirm {
            CloseConfirmAction::Close { tab_id, pane_id } => {
                // "Don't ask again" (UX.md OV3): turn the setting off here and
                // queue it for the window manager to write.
                if self.overlay_ui.close_confirmation_ui.take_dont_ask_again() {
                    let change =
                        crate::app::window_state::ExternalConfigChange::StopConfirmingRunningJobClose;
                    self.config.rcu(|old| {
                        let mut new = (**old).clone();
                        change.apply(&mut new);
                        std::sync::Arc::new(new)
                    });
                    self.render_loop.external_config_changes.push(change);
                }
                // Route through the proper cleanup path so session-undo capture,
                // tab-bar resize, alert sounds, and is_shutting_down are all handled.
                let prev_active = self.tab_manager.active_tab_id();
                self.tab_manager.switch_to(tab_id);
                let was_last = if let Some(pane_id) = pane_id {
                    // Focus the confirmed pane then close it via the normal path.
                    if let Some(tab) = self.tab_manager.active_tab_mut()
                        && let Some(pm) = tab.pane_manager_mut()
                    {
                        pm.focus_pane(pane_id);
                    }
                    let was_last = self.close_focused_pane_confirmed();
                    log::info!("Force-closed pane {} in tab {}", pane_id, tab_id);
                    was_last
                } else {
                    let was_last = self.close_current_tab_immediately();
                    log::info!("Force-closed tab {}", tab_id);
                    was_last
                };
                if was_last {
                    self.is_shutting_down = true;
                } else if let Some(prev) = prev_active
                    && prev != tab_id
                    && self.tab_manager.get_tab(prev).is_some()
                {
                    // UX.md B13/TW1: a confirmed close of a background tab
                    // or pane must not move the user either.
                    self.tab_manager.switch_to(prev);
                }
            }
            CloseConfirmAction::Cancel => {
                // User cancelled - do nothing, dialog already hidden
                log::debug!("Close confirmation cancelled");
            }
            CloseConfirmAction::None => {}
        }

        // Handle par-mux last-tab close dialog actions (UX.md M1)
        #[cfg(feature = "mux")]
        self.handle_mux_last_tab_action(mux_last_tab);
        // Without the mux feature there is no session to detach or end.
        #[cfg(not(feature = "mux"))]
        let _ = mux_last_tab;

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
                self.request_redraw();
            }
            RemoteShellInstallAction::Cancel => {
                self.request_redraw();
            }
            RemoteShellInstallAction::None => {}
        }

        // Handle SSH Quick Connect actions
        match ssh_connect {
            SshConnectAction::Connect {
                host,
                profile_override: _,
            } => {
                // SEC-003: the command is written into the active pane's shell,
                // and discovery sources are untrusted (an mDNS responder on the
                // LAN picks its own hostname). `ssh_command_line` validates every
                // component and quotes them; never `ssh_args().join(" ")`.
                match host.ssh_command_line() {
                    Ok(ssh_cmd) => {
                        // The trailing newline is what submits the command.
                        // A mux tab's `tab.terminal` is a hidden login shell —
                        // route the daemon pane first.
                        let cmd_bytes = format!("{ssh_cmd}\n");
                        if let Some(tab) = self.tab_manager.active_tab()
                            && !self.route_mux_tab_write(tab, cmd_bytes.as_bytes())
                            && let Ok(term) = tab.terminal.try_read()
                            && let Err(e) = term.write_str(&cmd_bytes)
                        {
                            crate::debug_error!(
                                "TAB_ACTION",
                                "PTY write failed (SSH quick connect): {e}"
                            );
                        }
                        log::info!(
                            "SSH Quick Connect: connecting to {}",
                            host.connection_string()
                        );
                    }
                    Err(e) => {
                        log::warn!(
                            "[SEC-003] refusing SSH Quick Connect to {:?}: {}",
                            host.display_name(),
                            e
                        );
                        self.post_toast(
                            crate::app::overlay::toast::ToastKind::Error,
                            format!("Cannot connect to this host: {}", e),
                            None,
                        );
                    }
                }
                self.request_redraw();
            }
            SshConnectAction::Cancel => {
                self.request_redraw();
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
                self.request_redraw();
            }
            crate::search::SearchAction::Close => {
                self.focus_state.needs_redraw = true;
                self.request_redraw();
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
                    self.post_toast(
                        crate::app::overlay::toast::ToastKind::Error,
                        format!("Failed to attach: {}", e),
                        None,
                    );
                } else {
                    crate::debug_info!("TMUX", "Gateway initiated for session '{}'", session_name);
                    self.show_toast(format!("Connecting to tmux session '{}'...", session_name));
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
                    self.post_toast(
                        crate::app::overlay::toast::ToastKind::Error,
                        format!("Failed to create tmux session: {}", e),
                        None,
                    );
                } else {
                    let msg = match name {
                        Some(ref n) => format!("Creating tmux session '{}'...", n),
                        None => "Creating new tmux session...".to_string(),
                    };
                    crate::debug_info!("TMUX", "Gateway initiated: {}", msg);
                    self.show_toast(msg);
                }
                self.focus_state.needs_redraw = true;
            }
            SessionPickerAction::Mux(request) => {
                #[cfg(feature = "mux")]
                self.handle_mux_session_request(request);
                #[cfg(not(feature = "mux"))]
                let _ = request;
                self.focus_state.needs_redraw = true;
            }
            SessionPickerAction::None => {}
        }

        // Pane context menu choice (UX.md V10): the action runs on the
        // pane the menu was opened on.
        if let Some((pane_id, action)) = pane_menu {
            if let Some(pm) = self
                .tab_manager
                .active_tab_mut()
                .and_then(|t| t.pane_manager_mut())
                && pm.focused_pane_id() != Some(pane_id)
            {
                pm.focus_pane(pane_id);
                self.after_user_pane_focus();
            }
            self.is_shutting_down |= action == "close_pane" && self.close_focused_pane();
            if action != "close_pane" {
                self.execute_keybinding_action(action);
            }
            self.focus_state.needs_redraw = true;
        }

        // Session chip click (UX.md V1)
        #[cfg(feature = "mux")]
        if let Some(chip_action) = self.tab_bar_ui.chip_action.take() {
            self.handle_session_chip_action(chip_action);
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
                    self.config.rcu(|old| {
                        let mut new = (**old).clone();
                        new.integrations.shader_install_prompt = ShaderInstallPrompt::Installed;
                        std::sync::Arc::new(new)
                    });
                    self.queue_integrations_change();
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
                self.request_redraw();
            }
            ShaderInstallResponse::Never => {
                log::info!("User declined shader installation (never ask again)");
                self.overlay_ui.shader_install_ui.hide();

                // Update config to never ask again
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.integrations.shader_install_prompt = ShaderInstallPrompt::Never;
                    std::sync::Arc::new(new)
                });
                self.queue_integrations_change();
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

        // Open Profiles… / the Profiles drawer (UX.md PR1/PR2): the same
        // registry actions a bound shortcut runs. The drawer's rows follow
        // profile and chord changes (and fill after its edge button opens it).
        if let Some(choice) = profile_launcher {
            self.run_launcher_choice(choice);
        }
        self.refresh_profiles_drawer_rows();

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
