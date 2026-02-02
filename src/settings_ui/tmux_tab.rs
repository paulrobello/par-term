//! Settings UI tab for tmux integration configuration
//!
//! This tab provides controls for:
//! - Enabling/disabling tmux integration
//! - tmux executable path
//! - Default session settings
//! - Auto-attach configuration

use super::SettingsUI;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("tmux Integration", |ui| {
        ui.label("Configure tmux control mode integration");
        ui.add_space(8.0);

        // Enable/Disable
        if ui
            .checkbox(&mut settings.config.tmux_enabled, "Enable tmux integration")
            .on_hover_text("Use tmux control mode for session management and split panes")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if !settings.config.tmux_enabled {
            ui.label(egui::RichText::new("tmux integration is disabled").italics());
            return;
        }

        ui.add_space(12.0);

        // tmux Path
        ui.heading("Executable");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("tmux path:");
            if ui
                .text_edit_singleline(&mut settings.config.tmux_path)
                .on_hover_text("Path to tmux executable (default: 'tmux' uses PATH)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(12.0);

        // Session Settings
        ui.heading("Sessions");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Default session name:");
            let mut session_name = settings
                .config
                .tmux_default_session
                .clone()
                .unwrap_or_default();
            if ui
                .text_edit_singleline(&mut session_name)
                .on_hover_text("Name for new tmux sessions (leave empty for tmux default)")
                .changed()
            {
                settings.config.tmux_default_session = if session_name.is_empty() {
                    None
                } else {
                    Some(session_name)
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(12.0);

        // Auto-attach
        ui.heading("Auto-Attach");
        ui.add_space(4.0);

        if ui
            .checkbox(
                &mut settings.config.tmux_auto_attach,
                "Auto-attach on startup",
            )
            .on_hover_text("Automatically attach to a tmux session when par-term starts")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if settings.config.tmux_auto_attach {
            ui.horizontal(|ui| {
                ui.label("Session to attach:");
                let mut attach_session = settings
                    .config
                    .tmux_auto_attach_session
                    .clone()
                    .unwrap_or_default();
                if ui
                    .text_edit_singleline(&mut attach_session)
                    .on_hover_text("Session name to auto-attach (leave empty for most recent)")
                    .changed()
                {
                    settings.config.tmux_auto_attach_session = if attach_session.is_empty() {
                        None
                    } else {
                        Some(attach_session)
                    };
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        ui.add_space(12.0);

        // Clipboard Sync
        ui.heading("Clipboard");
        ui.add_space(4.0);

        if ui
            .checkbox(
                &mut settings.config.tmux_clipboard_sync,
                "Sync clipboard with tmux",
            )
            .on_hover_text("When copying, also update tmux's paste buffer via set-buffer")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(12.0);

        // Status Bar
        ui.heading("Status Bar");
        ui.add_space(4.0);

        if ui
            .checkbox(
                &mut settings.config.tmux_show_status_bar,
                "Show tmux status bar",
            )
            .on_hover_text(
                "Display tmux status bar at bottom when connected (feature in development)",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(12.0);

        // Prefix Key
        ui.heading("Prefix Key");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Prefix key:");
            if ui
                .text_edit_singleline(&mut settings.config.tmux_prefix_key)
                .on_hover_text(
                    "Key combination for tmux commands in control mode.\n\
                     Format: C-b (Ctrl+B, default), C-Space (Ctrl+Space), C-a (Ctrl+A)\n\
                     Press prefix + command key (e.g., prefix + c = new window)",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.label("Common prefix + command combinations:");
        ui.label("  prefix + c = New window");
        ui.label("  prefix + % = Horizontal split");
        ui.label("  prefix + \" = Vertical split");
        ui.label("  prefix + d = Detach session");
        ui.label("  prefix + n = Next window");
        ui.label("  prefix + p = Previous window");

        ui.add_space(12.0);

        // Profile (placeholder for future)
        ui.heading("Profile");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("tmux profile:");
            let mut profile = settings.config.tmux_profile.clone().unwrap_or_default();
            if ui
                .text_edit_singleline(&mut profile)
                .on_hover_text(
                    "Profile to switch to when connected to tmux (requires profiles feature)",
                )
                .changed()
            {
                settings.config.tmux_profile = if profile.is_empty() {
                    None
                } else {
                    Some(profile)
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.label(
            egui::RichText::new("Note: Profiles feature not yet implemented")
                .italics()
                .weak(),
        );

        ui.add_space(12.0);

        // Info section
        ui.heading("About tmux Control Mode");
        ui.add_space(4.0);

        ui.label("tmux control mode (-CC) provides:");
        ui.label("• Native split pane rendering");
        ui.label("• Session persistence across disconnects");
        ui.label("• Shared sessions between terminals");
        ui.label("• Remote session attachment");
        ui.label("• Broadcast input to all panes");

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Note:");
            ui.label(
                egui::RichText::new("Requires tmux 2.1+ installed on your system")
                    .italics()
                    .weak(),
            );
        });
    });
}
