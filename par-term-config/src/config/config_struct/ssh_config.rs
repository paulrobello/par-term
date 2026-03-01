//! `SshConfig` — SSH settings.

use serde::{Deserialize, Serialize};

/// Settings controlling SSH discovery and automatic profile switching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConfig {
    /// Enable mDNS/Bonjour discovery for SSH hosts
    #[serde(default = "crate::defaults::bool_false")]
    pub enable_mdns_discovery: bool,

    /// mDNS scan timeout in seconds
    #[serde(default = "crate::defaults::mdns_timeout")]
    pub mdns_scan_timeout_secs: u32,

    /// Enable automatic profile switching based on SSH hostname
    #[serde(default = "crate::defaults::bool_true")]
    pub ssh_auto_profile_switch: bool,

    /// Revert profile when SSH session disconnects
    #[serde(default = "crate::defaults::bool_true")]
    pub ssh_revert_profile_on_disconnect: bool,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            enable_mdns_discovery: crate::defaults::bool_false(),
            mdns_scan_timeout_secs: crate::defaults::mdns_timeout(),
            ssh_auto_profile_switch: crate::defaults::bool_true(),
            ssh_revert_profile_on_disconnect: crate::defaults::bool_true(),
        }
    }
}
