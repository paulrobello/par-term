//! SSH settings section for the settings UI.
//!
//! Rendered as a collapsing section inside the Integrations tab.

use crate::SettingsUI;
use crate::search::SearchTag;
use crate::section::collapsing_section;
use std::collections::HashSet;

impl SettingsUI {
    /// Render the SSH settings as a collapsing section (used inside Integrations tab).
    pub(crate) fn show_ssh_tab_as_section(
        &mut self,
        ui: &mut egui::Ui,
        changes_this_frame: &mut bool,
        collapsed: &mut HashSet<String>,
    ) {
        collapsing_section(ui, "SSH", "integrations_ssh", true, collapsed, |ui| {
            ui.group(|ui| {
                ui.label(egui::RichText::new("Profile Auto-Switching").strong());
                ui.add_space(4.0);

                if ui
                    .checkbox(
                        &mut self.config.ssh.ssh_auto_profile_switch,
                        "Auto-switch profile on SSH connection",
                    )
                    .search_tag(&["ssh_auto_profile_switch"])
                    .changed()
                {
                    self.has_changes = true;
                    *changes_this_frame = true;
                }
                ui.label(
                    egui::RichText::new(
                        "Automatically switch to a matching profile when an SSH hostname is detected. \
                         The hostname is reported by the remote shell, so if the matched profile \
                         defines a command you will be asked to confirm it before it runs.",
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
                    .search_tag(&["ssh_revert_profile_on_disconnect"])
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

            ui.add_space(8.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("mDNS/Bonjour Discovery").strong());
                ui.add_space(4.0);

                if ui
                    .checkbox(
                        &mut self.config.ssh.enable_mdns_discovery,
                        "Enable mDNS host discovery",
                    )
                    .search_tag(&["enable_mdns_discovery"])
                    .changed()
                {
                    self.has_changes = true;
                    *changes_this_frame = true;
                }
                ui.label(
                    egui::RichText::new(
                        "Discover SSH hosts on the local network via Bonjour/mDNS.",
                    )
                    .weak()
                    .size(11.0),
                );

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("Scan timeout:");
                    let mut timeout = self.config.ssh.mdns_scan_timeout_secs as f32;
                    if ui
                        .add(
                            egui::Slider::new(&mut timeout, 1.0..=10.0)
                                .suffix(" s")
                                .integer(),
                        )
                        .search_tag(&["mdns_scan_timeout_secs"])
                        .changed()
                    {
                        self.config.ssh.mdns_scan_timeout_secs = timeout as u32;
                        self.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            });

            ui.add_space(8.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("Quick Connect").strong());
                ui.add_space(4.0);
                ui.label(
                    match crate::live_binding::binding_for(&self.config, "ssh_quick_connect") {
                        Some(chord) => format!("Press {chord} to open the SSH Quick Connect dialog."),
                        None => "Bind the \"SSH Quick Connect\" action in Keys › Key Bindings \
                                 to open the SSH Quick Connect dialog."
                            .to_string(),
                    },
                );
                ui.label(
                    egui::RichText::new(
                        "The dialog shows hosts from SSH config, known_hosts, shell history, and mDNS.",
                    )
                    .weak()
                    .size(11.0),
                );
            });
        });
    }
}
