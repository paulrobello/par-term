//! SSH settings tab for the settings UI.

use crate::SettingsUI;

impl SettingsUI {
    /// Render the SSH settings tab.
    pub(crate) fn show_ssh_tab(&mut self, ui: &mut egui::Ui, changes_this_frame: &mut bool) {
        ui.heading("SSH Settings");
        ui.add_space(8.0);

        ui.group(|ui| {
            ui.label(egui::RichText::new("Profile Auto-Switching").strong());
            ui.add_space(4.0);

            if ui
                .checkbox(
                    &mut self.config.ssh.ssh_auto_profile_switch,
                    "Auto-switch profile on SSH connection",
                )
                .changed()
            {
                self.has_changes = true;
                *changes_this_frame = true;
            }
            ui.label(
                egui::RichText::new(
                    "Automatically switch to a matching profile when an SSH hostname is detected.",
                )
                .weak()
                .size(11.0),
            );

            ui.add_space(4.0);

            if ui
                .checkbox(
                    &mut self.config.ssh.ssh_revert_profile_on_disconnect,
                    "Revert profile on SSH disconnect",
                )
                .changed()
            {
                self.has_changes = true;
                *changes_this_frame = true;
            }
            ui.label(
                egui::RichText::new(
                    "Switch back to the previous profile when the SSH session ends.",
                )
                .weak()
                .size(11.0),
            );
        });

        ui.add_space(12.0);

        ui.group(|ui| {
            ui.label(egui::RichText::new("mDNS/Bonjour Discovery").strong());
            ui.add_space(4.0);

            if ui
                .checkbox(
                    &mut self.config.ssh.enable_mdns_discovery,
                    "Enable mDNS host discovery",
                )
                .changed()
            {
                self.has_changes = true;
                *changes_this_frame = true;
            }
            ui.label(
                egui::RichText::new("Discover SSH hosts on the local network via Bonjour/mDNS.")
                    .weak()
                    .size(11.0),
            );

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("Scan timeout (seconds):");
                let mut timeout = self.config.ssh.mdns_scan_timeout_secs as f32;
                if ui
                    .add(egui::Slider::new(&mut timeout, 1.0..=10.0).integer())
                    .changed()
                {
                    self.config.ssh.mdns_scan_timeout_secs = timeout as u32;
                    self.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        });

        ui.add_space(12.0);

        ui.group(|ui| {
            ui.label(egui::RichText::new("Quick Connect").strong());
            ui.add_space(4.0);
            ui.label("Press Cmd+Shift+S to open the SSH Quick Connect dialog.");
            ui.label(
                egui::RichText::new(
                    "The dialog shows hosts from SSH config, known_hosts, shell history, and mDNS.",
                )
                .weak()
                .size(11.0),
            );
        });
    }
}

/// Search keywords for the SSH settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        "ssh",
        "remote",
        "host",
        "connect",
        "quick connect",
        "mdns",
        "bonjour",
        "discovery",
        "auto-switch",
        "auto switch",
        "profile switch",
        "hostname",
        "known hosts",
        // Auto-switch extras
        "revert profile",
        "disconnect",
        // mDNS extras
        "scan timeout",
    ]
}
