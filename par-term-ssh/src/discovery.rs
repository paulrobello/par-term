//! SSH host discovery aggregator.

use super::config_parser;
use super::history;
use super::known_hosts;
use super::types::SshHost;
use std::collections::HashSet;

/// Discover SSH hosts from all local sources (config, known_hosts, history).
pub fn discover_local_hosts() -> Vec<SshHost> {
    let mut all_hosts = Vec::new();
    let mut seen = HashSet::new();

    if let Some(home) = dirs::home_dir() {
        let ssh_dir = home.join(".ssh");

        let config_path = ssh_dir.join("config");
        if config_path.exists() {
            for host in config_parser::parse_ssh_config(&config_path) {
                let key = dedup_key(&host);
                if seen.insert(key) {
                    all_hosts.push(host);
                }
            }
        }

        let known_hosts_path = ssh_dir.join("known_hosts");
        if known_hosts_path.exists() {
            for host in known_hosts::parse_known_hosts(&known_hosts_path) {
                let key = dedup_key(&host);
                if seen.insert(key) {
                    all_hosts.push(host);
                }
            }
        }
    }

    for host in history::scan_history() {
        let key = dedup_key(&host);
        if seen.insert(key) {
            all_hosts.push(host);
        }
    }

    all_hosts
}

fn dedup_key(host: &SshHost) -> String {
    let target = host.hostname.as_deref().unwrap_or(&host.alias);
    let port = host.port.unwrap_or(22);
    format!("{}:{}", target.to_lowercase(), port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SshHostSource;

    #[test]
    fn test_dedup_key() {
        let host = SshHost {
            alias: "myhost".to_string(),
            hostname: Some("MyHost.Example.COM".to_string()),
            user: None,
            port: None,
            identity_file: None,
            proxy_jump: None,
            source: SshHostSource::Config,
        };
        assert_eq!(dedup_key(&host), "myhost.example.com:22");
    }

    #[test]
    fn test_dedup_key_with_port() {
        let host = SshHost {
            alias: "myhost".to_string(),
            hostname: Some("myhost.example.com".to_string()),
            user: None,
            port: Some(2222),
            identity_file: None,
            proxy_jump: None,
            source: SshHostSource::Config,
        };
        assert_eq!(dedup_key(&host), "myhost.example.com:2222");
    }
}
