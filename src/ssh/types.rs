//! SSH host types for the SSH subsystem.

use serde::{Deserialize, Serialize};

/// Source of an SSH host entry
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SshHostSource {
    /// Parsed from ~/.ssh/config
    Config,
    /// Found in ~/.ssh/known_hosts
    KnownHosts,
    /// Extracted from shell history
    History,
    /// Discovered via mDNS/Bonjour
    Mdns,
}

impl std::fmt::Display for SshHostSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config => write!(f, "SSH Config"),
            Self::KnownHosts => write!(f, "Known Hosts"),
            Self::History => write!(f, "History"),
            Self::Mdns => write!(f, "mDNS"),
        }
    }
}

/// A discovered SSH host with connection details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshHost {
    /// The Host alias from SSH config, or hostname from other sources
    pub alias: String,
    /// Resolved hostname or IP address
    pub hostname: Option<String>,
    /// SSH username
    pub user: Option<String>,
    /// SSH port (None means default 22)
    pub port: Option<u16>,
    /// Path to identity file
    pub identity_file: Option<String>,
    /// ProxyJump host
    pub proxy_jump: Option<String>,
    /// Where this host was discovered from
    pub source: SshHostSource,
}

impl SshHost {
    /// Get the display name for this host (alias or hostname)
    pub fn display_name(&self) -> &str {
        &self.alias
    }

    /// Get the connection target (hostname or alias)
    pub fn connection_target(&self) -> &str {
        self.hostname.as_deref().unwrap_or(&self.alias)
    }

    /// Build the ssh command arguments for connecting to this host
    pub fn ssh_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        if let Some(port) = self.port {
            if port != 22 {
                args.push("-p".to_string());
                args.push(port.to_string());
            }
        }

        if let Some(ref identity) = self.identity_file {
            args.push("-i".to_string());
            args.push(identity.clone());
        }

        if let Some(ref proxy) = self.proxy_jump {
            args.push("-J".to_string());
            args.push(proxy.clone());
        }

        let target = if let Some(ref user) = self.user {
            format!("{}@{}", user, self.connection_target())
        } else {
            self.connection_target().to_string()
        };
        args.push(target);

        args
    }

    /// Build a display string showing user@host:port
    pub fn connection_string(&self) -> String {
        let mut s = String::new();
        if let Some(ref user) = self.user {
            s.push_str(user);
            s.push('@');
        }
        s.push_str(self.connection_target());
        if let Some(port) = self.port {
            if port != 22 {
                s.push(':');
                s.push_str(&port.to_string());
            }
        }
        s
    }
}
