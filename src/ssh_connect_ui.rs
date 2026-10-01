//! SSH Quick Connect dialog.
//!
//! An egui modal overlay for browsing and connecting to SSH hosts.
//! Opened via Cmd+Shift+S (macOS) or Ctrl+Shift+S (Linux/Windows).

use crate::profile::ProfileId;
use egui::Context;
use par_term_ssh::mdns::MdnsDiscovery;
use par_term_ssh::{SshHost, discover_local_hosts};

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

/// Rows drawn before the host list scrolls.
const VISIBLE_ROWS: usize = 12;

/// SSH Quick Connect UI state.
pub struct SshConnectUI {
    visible: bool,
    search_query: String,
    hosts: Vec<SshHost>,
    /// Selection and drawn window, on the shared picker (UX.md OV5).
    nav: crate::app::overlay::picker::ListNav,
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
            nav: Default::default(),
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
        self.nav.reset();
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

    /// Indices into `hosts` matching the search query, in list order.
    fn filtered(&self) -> Vec<usize> {
        let query = self.search_query.to_lowercase();
        self.hosts
            .iter()
            .enumerate()
            .filter(|(_, h)| {
                query.is_empty()
                    || h.alias.to_lowercase().contains(&query)
                    || h.hostname
                        .as_deref()
                        .is_some_and(|n| n.to_lowercase().contains(&query))
                    || h.user
                        .as_deref()
                        .is_some_and(|u| u.to_lowercase().contains(&query))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Draw the dialog and return the user's choice.
    ///
    /// The host list, filter, keys, and footer come from the shared picker
    /// (UX.md OV5): arrows, PageUp/PageDown, Home/End, Enter connects,
    /// Escape cancels. Hosts are grouped under a heading per source.
    pub fn show(&mut self, ctx: &Context) -> SshConnectAction {
        use crate::app::overlay::picker::{self, ListConfig, ListHooks, ListOutcome};

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

        let filtered = self.filtered();
        let config = ListConfig {
            id: "SSH Quick Connect",
            hint: "Search hosts...",
            visible_rows: VISIBLE_ROWS,
            width: crate::app::overlay::theme::WIDTH_LARGE,
            empty_text: "No hosts found.",
            enter_verb: "connect",
            // No toggle action: the overlay stack has no chord that closes it.
            toggle_chord: None,
            alternates: false,
            alternate_labels: None,
            extra_keys: &[],
            multi_select: false,
        };
        let scanning = self.mdns.is_scanning();
        let mut show_scan = |ui: &mut egui::Ui| {
            ui.horizontal(|ui| {
                ui.label("Connect to an SSH host");
                if scanning {
                    ui.spinner();
                    ui.label(egui::RichText::new("Scanning...").weak().small());
                }
            });
        };
        let mut cancel_clicked = false;
        let mut cancel_button = |ui: &mut egui::Ui| {
            if ui.button("Cancel").clicked() {
                cancel_clicked = true;
            }
        };
        let hosts = &self.hosts;
        let (outcome, query_changed) = picker::show_list_with(
            ctx,
            &config,
            ListHooks {
                keys_follow_filter: false,
                above: Some(&mut show_scan),
                below: Some(&mut cancel_button),
            },
            &mut self.search_query,
            &mut self.request_focus,
            &mut self.nav,
            filtered.len(),
            |ui, index, selected| {
                let host = &hosts[filtered[index]];
                let new_group = index == 0 || hosts[filtered[index - 1]].source != host.source;
                if new_group {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(host.source.to_string())
                            .strong()
                            .small(),
                    );
                }
                ui.selectable_label(
                    selected,
                    format!("{}  {}", host.alias, host.connection_string()),
                )
                .clicked()
            },
        );
        if query_changed {
            self.nav.reset();
        }
        let action = if cancel_clicked {
            SshConnectAction::Cancel
        } else {
            match outcome {
                ListOutcome::Chosen { index, .. } => match filtered.get(index) {
                    Some(&host_idx) => SshConnectAction::Connect {
                        host: self.hosts[host_idx].clone(),
                        profile_override: self.selected_profile,
                    },
                    None => SshConnectAction::None,
                },
                ListOutcome::Closed => SshConnectAction::Cancel,
                ListOutcome::Open | ListOutcome::ToggleMark(_) => SshConnectAction::None,
            }
        };

        match &action {
            SshConnectAction::Cancel | SshConnectAction::Connect { .. } => self.close(),
            SshConnectAction::None => {}
        }

        action
    }
}

impl crate::traits::OverlayComponent for SshConnectUI {
    type Action = SshConnectAction;

    fn show(&mut self, ctx: &egui::Context) -> Self::Action {
        SshConnectUI::show(self, ctx)
    }

    fn is_visible(&self) -> bool {
        self.is_visible()
    }

    fn set_visible(&mut self, visible: bool) {
        if !visible {
            self.close();
        }
        // Note: setting visible=true requires mdns_enabled and mdns_timeout parameters.
        // Use open(mdns_enabled, mdns_timeout) to show this dialog.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use par_term_ssh::SshHostSource;

    fn host(alias: &str) -> SshHost {
        SshHost {
            alias: alias.to_string(),
            hostname: Some(format!("{alias}.example.com")),
            user: None,
            port: None,
            identity_file: None,
            proxy_jump: None,
            source: SshHostSource::Config,
        }
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }
    }

    fn frame(ctx: &egui::Context, ui: &mut SshConnectUI, key: egui::Key) -> SshConnectAction {
        ctx.begin_pass(egui::RawInput {
            events: vec![self::key(key)],
            ..Default::default()
        });
        let action = ui.show(ctx);
        ctx.end_pass().textures_delta.clear();
        action
    }

    fn open_with(hosts: Vec<SshHost>) -> SshConnectUI {
        let mut ui = SshConnectUI::new();
        ui.visible = true;
        ui.hosts = hosts;
        ui
    }

    #[test]
    fn enter_connects_the_selected_host_and_escape_cancels() {
        let ctx = egui::Context::default();
        let mut ui = open_with(vec![host("alpha"), host("beta")]);
        let _ = frame(&ctx, &mut ui, egui::Key::ArrowDown);
        match frame(&ctx, &mut ui, egui::Key::Enter) {
            SshConnectAction::Connect { host, .. } => assert_eq!(host.alias, "beta"),
            other => panic!("expected Connect, got {other:?}"),
        }
        assert!(!ui.is_visible());

        let mut ui = open_with(vec![host("alpha")]);
        assert!(matches!(
            frame(&ctx, &mut ui, egui::Key::Escape),
            SshConnectAction::Cancel
        ));
        assert!(!ui.is_visible());
    }

    #[test]
    fn the_search_query_filters_by_alias_host_and_user() {
        let mut ui = open_with(vec![host("alpha"), host("beta")]);
        ui.search_query = "BET".into();
        assert_eq!(ui.filtered(), vec![1]);
    }
}
