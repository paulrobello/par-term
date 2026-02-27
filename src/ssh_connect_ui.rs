//! SSH Quick Connect dialog.
//!
//! An egui modal overlay for browsing and connecting to SSH hosts.
//! Opened via Cmd+Shift+S (macOS) or Ctrl+Shift+S (Linux/Windows).

use crate::profile::ProfileId;
use crate::ssh::mdns::MdnsDiscovery;
use crate::ssh::{SshHost, SshHostSource, discover_local_hosts};
use crate::ui_constants::{
    SSH_CONNECT_DIALOG_MAX_HEIGHT, SSH_CONNECT_DIALOG_MAX_WIDTH, SSH_CONNECT_DIALOG_MIN_HEIGHT,
    SSH_CONNECT_DIALOG_MIN_WIDTH, SSH_CONNECT_HOST_ROW_HEIGHT, SSH_CONNECT_INNER_MARGIN,
    SSH_CONNECT_LIST_BOTTOM_RESERVE, SSH_CONNECT_SEARCH_BAR_HEIGHT,
};
use egui::{Color32, Context, epaint::Shadow};

/// Action returned by the quick connect dialog.
#[derive(Debug, Clone)]
pub enum SshConnectAction {
    /// No action (dialog still showing)
    None,
    /// Connect to the selected host
    Connect {
        host: SshHost,
        profile_override: Option<ProfileId>,
    },
    /// Dialog was cancelled
    Cancel,
}

/// SSH Quick Connect UI state.
pub struct SshConnectUI {
    visible: bool,
    search_query: String,
    hosts: Vec<SshHost>,
    selected_index: usize,
    selected_profile: Option<ProfileId>,
    mdns: MdnsDiscovery,
    mdns_enabled: bool,
    hosts_loaded: bool,
    request_focus: bool,
}

impl Default for SshConnectUI {
    fn default() -> Self {
        Self::new()
    }
}

impl SshConnectUI {
    pub fn new() -> Self {
        Self {
            visible: false,
            search_query: String::new(),
            hosts: Vec::new(),
            selected_index: 0,
            selected_profile: None,
            mdns: MdnsDiscovery::new(),
            mdns_enabled: false,
            hosts_loaded: false,
            request_focus: false,
        }
    }

    pub fn open(&mut self, mdns_enabled: bool, mdns_timeout: u32) {
        self.visible = true;
        self.search_query.clear();
        self.selected_index = 0;
        self.selected_profile = None;
        self.mdns_enabled = mdns_enabled;
        self.request_focus = true;
        self.hosts = discover_local_hosts();
        self.hosts_loaded = true;
        if mdns_enabled {
            self.mdns.start_scan(mdns_timeout);
        }
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.hosts.clear();
        self.mdns.clear();
        self.hosts_loaded = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, ctx: &Context) -> SshConnectAction {
        if !self.visible {
            return SshConnectAction::None;
        }

        // Poll mDNS for newly discovered hosts
        if self.mdns.poll() {
            for host in self.mdns.hosts() {
                let dominated = self
                    .hosts
                    .iter()
                    .any(|h| h.hostname == host.hostname && h.port == host.port);
                if !dominated {
                    self.hosts.push(host.clone());
                }
            }
        }

        let mut action = SshConnectAction::None;
        let screen_rect = ctx.content_rect();
        let dialog_width = (screen_rect.width() * 0.5)
            .clamp(SSH_CONNECT_DIALOG_MIN_WIDTH, SSH_CONNECT_DIALOG_MAX_WIDTH);
        let dialog_height = (screen_rect.height() * 0.6)
            .clamp(SSH_CONNECT_DIALOG_MIN_HEIGHT, SSH_CONNECT_DIALOG_MAX_HEIGHT);

        egui::Area::new(egui::Id::new("ssh_connect_overlay"))
            .fixed_pos(egui::pos2(
                (screen_rect.width() - dialog_width) / 2.0,
                (screen_rect.height() - dialog_height) / 2.5,
            ))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(SSH_CONNECT_INNER_MARGIN)
                    .shadow(Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 8,
                        color: Color32::from_black_alpha(100),
                    })
                    .show(ui, |ui| {
                        ui.set_width(dialog_width);
                        ui.set_max_height(dialog_height);

                        // Title
                        ui.horizontal(|ui| {
                            ui.heading("SSH Quick Connect");
                            if self.mdns.is_scanning() {
                                ui.spinner();
                                ui.label(egui::RichText::new("Scanning...").weak().size(11.0));
                            }
                        });
                        ui.add_space(8.0);

                        // Search bar
                        let search_response = ui.add_sized(
                            [
                                dialog_width - SSH_CONNECT_INNER_MARGIN * 2.0,
                                SSH_CONNECT_SEARCH_BAR_HEIGHT,
                            ],
                            egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text("Search hosts...")
                                .desired_width(dialog_width - SSH_CONNECT_INNER_MARGIN * 2.0),
                        );

                        if self.request_focus {
                            search_response.request_focus();
                            self.request_focus = false;
                        }

                        ui.add_space(8.0);

                        // Filter hosts by search query
                        let query_lower = self.search_query.to_lowercase();
                        let filtered: Vec<usize> = self
                            .hosts
                            .iter()
                            .enumerate()
                            .filter(|(_, h)| {
                                if query_lower.is_empty() {
                                    return true;
                                }
                                h.alias.to_lowercase().contains(&query_lower)
                                    || h.hostname
                                        .as_deref()
                                        .is_some_and(|n| n.to_lowercase().contains(&query_lower))
                                    || h.user
                                        .as_deref()
                                        .is_some_and(|u| u.to_lowercase().contains(&query_lower))
                            })
                            .map(|(i, _)| i)
                            .collect();

                        if !filtered.is_empty() {
                            self.selected_index = self.selected_index.min(filtered.len() - 1);
                        }

                        // Keyboard navigation
                        let mut enter_pressed = false;
                        if search_response.has_focus() {
                            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown))
                                && self.selected_index + 1 < filtered.len()
                            {
                                self.selected_index += 1;
                            }
                            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp))
                                && self.selected_index > 0
                            {
                                self.selected_index -= 1;
                            }
                            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                enter_pressed = true;
                            }
                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                action = SshConnectAction::Cancel;
                            }
                        }

                        // Host list grouped by source
                        egui::ScrollArea::vertical()
                            .max_height(dialog_height - SSH_CONNECT_LIST_BOTTOM_RESERVE)
                            .show(ui, |ui| {
                                if filtered.is_empty() {
                                    ui.label(
                                        egui::RichText::new("No hosts found.").weak().italics(),
                                    );
                                    return;
                                }

                                let mut current_source: Option<&SshHostSource> = None;
                                for (display_idx, &host_idx) in filtered.iter().enumerate() {
                                    let host = &self.hosts[host_idx];

                                    // Group header when source changes
                                    if current_source != Some(&host.source) {
                                        current_source = Some(&host.source);
                                        ui.add_space(4.0);
                                        ui.label(
                                            egui::RichText::new(host.source.to_string())
                                                .strong()
                                                .size(11.0)
                                                .color(Color32::from_rgb(140, 140, 180)),
                                        );
                                        ui.separator();
                                    }

                                    let is_selected = display_idx == self.selected_index;
                                    let response = ui.add_sized(
                                        [
                                            dialog_width - SSH_CONNECT_INNER_MARGIN * 3.0,
                                            SSH_CONNECT_HOST_ROW_HEIGHT,
                                        ],
                                        egui::Button::new(egui::RichText::new(format!(
                                            "  {}  {}",
                                            host.alias,
                                            host.connection_string()
                                        )))
                                        .fill(
                                            if is_selected {
                                                Color32::from_rgb(50, 50, 70)
                                            } else {
                                                Color32::TRANSPARENT
                                            },
                                        ),
                                    );

                                    if response.clicked() || (enter_pressed && is_selected) {
                                        action = SshConnectAction::Connect {
                                            host: host.clone(),
                                            profile_override: self.selected_profile,
                                        };
                                    }
                                    if response.hovered() {
                                        self.selected_index = display_idx;
                                    }
                                }
                            });

                        // Bottom bar with cancel button and keyboard hints
                        ui.add_space(8.0);
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() {
                                action = SshConnectAction::Cancel;
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(
                                            "Up/Down Navigate  Enter Connect  Esc Cancel",
                                        )
                                        .weak()
                                        .size(10.0),
                                    );
                                },
                            );
                        });
                    });
            });

        match &action {
            SshConnectAction::Cancel | SshConnectAction::Connect { .. } => self.close(),
            SshConnectAction::None => {}
        }

        action
    }
}
