//! Render implementation for [`SettingsWindow`].
//!
//! Contains the `render()` method, which drives the egui frame, submits GPU
//! commands, and translates settings-UI results into [`SettingsWindowAction`]
//! values returned to the caller.

use wgpu::SurfaceError;

use super::{SettingsWindow, SettingsWindowAction};
use crate::settings_ui::UpdateCheckResult;

impl SettingsWindow {
    /// Render the settings window.
    ///
    /// Called from `handle_window_event` when a `RedrawRequested` event arrives.
    /// Returns a [`SettingsWindowAction`] describing any user-driven change that
    /// the caller (window manager) must act upon.
    pub(super) fn render(&mut self) -> SettingsWindowAction {
        if !self.ready {
            return SettingsWindowAction::None;
        }

        // Get surface texture
        let output = match self.surface.get_current_texture() {
            Ok(output) => output,
            Err(SurfaceError::Lost | SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.surface_config);
                return SettingsWindowAction::None;
            }
            Err(SurfaceError::Timeout) => {
                log::warn!("Settings window surface timeout");
                return SettingsWindowAction::None;
            }
            Err(e) => {
                log::error!("Settings window surface error: {:?}", e);
                return SettingsWindowAction::None;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Track settings results
        let mut config_to_save = None;
        let mut config_for_live = None;
        let mut shader_apply = None;
        let mut cursor_shader_apply = None;

        // Run egui
        let mut raw_input = self.egui_state.take_egui_input(&self.window);

        // Inject pending events from menu accelerators (Cmd+V/C/X/A intercepted by muda)
        if let Some(text) = self.pending_paste.take() {
            raw_input.events.push(egui::Event::Paste(text));
        }
        raw_input.events.append(&mut self.pending_events);

        let egui_output = self.egui_ctx.run(raw_input, |ctx| {
            // Show the settings UI as a panel (not a nested window) and capture results
            let (save, live, shader, cursor_shader) = self.settings_ui.show_as_panel(ctx);
            config_to_save = save;
            config_for_live = live;
            shader_apply = shader;
            cursor_shader_apply = cursor_shader;
        });

        // Handle platform output (clipboard, cursor)
        // Manually handle clipboard copy as a fallback for macOS menu accelerator issues.
        // In egui 0.33, copy commands are in platform_output.commands as OutputCommand::CopyText.
        for cmd in &egui_output.platform_output.commands {
            match cmd {
                egui::OutputCommand::CopyText(text) => {
                    if let Ok(mut clipboard) = arboard::Clipboard::new()
                        && let Err(e) = clipboard.set_text(text)
                    {
                        log::warn!("Settings window: failed to copy to clipboard: {}", e);
                    }
                }
                egui::OutputCommand::CopyImage(_) => {}
                _ => {}
            }
        }
        self.egui_state
            .handle_platform_output(&self.window, egui_output.platform_output.clone());

        // Tessellate shapes
        let paint_jobs = self
            .egui_ctx
            .tessellate(egui_output.shapes, self.egui_ctx.pixels_per_point());

        // Upload egui textures
        for (id, delta) in &egui_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Settings Window Encoder"),
            });

        // Screen descriptor
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        // Update buffers
        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        // Render pass
        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Settings Window Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.094,
                            g: 0.094,
                            b: 0.094,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Convert to 'static lifetime as required by egui_renderer.render()
            let mut render_pass = render_pass.forget_lifetime();

            self.egui_renderer
                .render(&mut render_pass, &paint_jobs, &screen_descriptor);
        } // render_pass dropped here

        // Submit
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // Free textures
        for id in &egui_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        // Check for test notification request
        if self.settings_ui.take_test_notification_request() {
            return SettingsWindowAction::TestNotification;
        }

        // Check for profile save request
        if let Some(profiles) = self.settings_ui.take_profile_save_request() {
            self.window.request_redraw();
            return SettingsWindowAction::SaveProfiles(profiles);
        }

        // Check for profile open request
        if let Some(id) = self.settings_ui.take_profile_open_request() {
            self.window.request_redraw();
            return SettingsWindowAction::OpenProfile(id);
        }

        // Check for open log file request
        if self.settings_ui.open_log_requested {
            self.settings_ui.open_log_requested = false;
            return SettingsWindowAction::OpenLogFile;
        }

        // Check for update check request
        if self.settings_ui.check_now_requested {
            self.settings_ui.check_now_requested = false;
            self.window.request_redraw();
            return SettingsWindowAction::ForceUpdateCheck;
        }

        // Check for update install request
        if self.settings_ui.update_install_requested {
            self.settings_ui.update_install_requested = false;
            // Extract the version from the last_update_result
            if let Some(UpdateCheckResult::UpdateAvailable(ref info)) =
                self.settings_ui.last_update_result
            {
                let version = info
                    .version
                    .strip_prefix('v')
                    .unwrap_or(&info.version)
                    .to_string();
                self.settings_ui
                    .start_self_update_with(version.clone(), |v| {
                        crate::self_updater::perform_update(v, crate::VERSION).map(|r| {
                            crate::settings_ui::UpdateResult {
                                old_version: r.old_version,
                                new_version: r.new_version,
                                install_path: r.install_path.display().to_string(),
                                needs_restart: r.needs_restart,
                            }
                        })
                    });
                self.window.request_redraw();
                return SettingsWindowAction::InstallUpdate(version);
            }
        }

        // Poll for update install completion
        self.settings_ui.poll_update_install_status();

        // Check for coprocess start/stop actions
        if let Some((index, start)) = self.settings_ui.pending_coprocess_actions.pop() {
            log::info!(
                "Settings window: popped coprocess action index={} start={}",
                index,
                start
            );
            // Request another redraw to process remaining actions (if any) and config changes
            self.window.request_redraw();
            return if start {
                SettingsWindowAction::StartCoprocess(index)
            } else {
                SettingsWindowAction::StopCoprocess(index)
            };
        }

        // Check for script start/stop actions
        if let Some((index, start)) = self.settings_ui.pending_script_actions.pop() {
            crate::debug_info!(
                "SCRIPT",
                "Settings window: popped script action index={} start={}",
                index,
                start
            );
            self.window.request_redraw();
            return if start {
                SettingsWindowAction::StartScript(index)
            } else {
                SettingsWindowAction::StopScript(index)
            };
        }

        // Check for arrangement actions
        if let Some(action) = self.settings_ui.pending_arrangement_actions.pop() {
            self.window.request_redraw();
            return action;
        }

        // Check for identify panes request
        if self.settings_ui.identify_panes_requested {
            self.settings_ui.identify_panes_requested = false;
            self.window.request_redraw();
            return SettingsWindowAction::IdentifyPanes;
        }

        // Check for shell integration install/uninstall actions
        if let Some(action) = self.settings_ui.shell_integration_action.take() {
            self.window.request_redraw();
            return match action {
                crate::settings_ui::integrations_tab::ShellIntegrationAction::Install => {
                    SettingsWindowAction::InstallShellIntegration
                }
                crate::settings_ui::integrations_tab::ShellIntegrationAction::Uninstall => {
                    SettingsWindowAction::UninstallShellIntegration
                }
            };
        }

        // Determine action based on settings UI results
        if let Some(config) = config_to_save {
            return SettingsWindowAction::SaveConfig(config);
        }
        if let Some(shader_result) = shader_apply {
            return SettingsWindowAction::ApplyShader(shader_result);
        }
        if let Some(cursor_shader_result) = cursor_shader_apply {
            return SettingsWindowAction::ApplyCursorShader(cursor_shader_result);
        }
        if let Some(config) = config_for_live {
            return SettingsWindowAction::ApplyConfig(config);
        }

        SettingsWindowAction::None
    }
}
