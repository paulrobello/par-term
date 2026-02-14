//! Parser for ~/.ssh/config files.
//!
//! Reads SSH config and extracts host entries with their connection parameters.

use super::types::{SshHost, SshHostSource};
use std::path::Path;

/// Parse an SSH config file and return discovered hosts.
///
/// Skips wildcard-only hosts (e.g., `Host *`) since they're defaults, not connectable targets.
/// Handles multi-host lines like `Host foo bar` by creating separate entries.
pub fn parse_ssh_config(path: &Path) -> Vec<SshHost> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    parse_ssh_config_str(&content)
}

/// Parse SSH config from a string (for testing).
pub fn parse_ssh_config_str(content: &str) -> Vec<SshHost> {
    let mut hosts = Vec::new();
    let mut current_aliases: Vec<String> = Vec::new();
    let mut hostname: Option<String> = None;
    let mut user: Option<String> = None;
    let mut port: Option<u16> = None;
    let mut identity_file: Option<String> = None;
    let mut proxy_jump: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let (key, value) = if let Some(eq_pos) = line.find('=') {
            let (k, v) = line.split_at(eq_pos);
            (k.trim(), v[1..].trim())
        } else if let Some(space_pos) = line.find(char::is_whitespace) {
            let (k, v) = line.split_at(space_pos);
            (k.trim(), v.trim())
        } else {
            continue;
        };

        match key.to_lowercase().as_str() {
            "host" => {
                flush_host_block(
                    &current_aliases,
                    &hostname,
                    &user,
                    &port,
                    &identity_file,
                    &proxy_jump,
                    &mut hosts,
                );

                current_aliases = value
                    .split_whitespace()
                    .filter(|a| !a.contains('*') && !a.contains('?'))
                    .map(String::from)
                    .collect();
                hostname = None;
                user = None;
                port = None;
                identity_file = None;
                proxy_jump = None;
            }
            "hostname" => hostname = Some(value.to_string()),
            "user" => user = Some(value.to_string()),
            "port" => port = value.parse().ok(),
            "identityfile" => {
                let expanded = if let Some(rest) = value.strip_prefix("~/") {
                    if let Some(home) = dirs::home_dir() {
                        format!("{}/{}", home.display(), rest)
                    } else {
                        value.to_string()
                    }
                } else {
                    value.to_string()
                };
                identity_file = Some(expanded);
            }
            "proxyjump" => proxy_jump = Some(value.to_string()),
            _ => {}
        }
    }

    flush_host_block(
        &current_aliases,
        &hostname,
        &user,
        &port,
        &identity_file,
        &proxy_jump,
        &mut hosts,
    );

    hosts
}

fn flush_host_block(
    aliases: &[String],
    hostname: &Option<String>,
    user: &Option<String>,
    port: &Option<u16>,
    identity_file: &Option<String>,
    proxy_jump: &Option<String>,
    hosts: &mut Vec<SshHost>,
) {
    for alias in aliases {
        hosts.push(SshHost {
            alias: alias.clone(),
            hostname: hostname.clone(),
            user: user.clone(),
            port: *port,
            identity_file: identity_file.clone(),
            proxy_jump: proxy_jump.clone(),
            source: SshHostSource::Config,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_host() {
        let config = r#"
Host myserver
    HostName 192.168.1.100
    User deploy
    Port 2222
"#;
        let hosts = parse_ssh_config_str(config);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].alias, "myserver");
        assert_eq!(hosts[0].hostname.as_deref(), Some("192.168.1.100"));
        assert_eq!(hosts[0].user.as_deref(), Some("deploy"));
        assert_eq!(hosts[0].port, Some(2222));
    }

    #[test]
    fn test_parse_multiple_hosts() {
        let config = r#"
Host web
    HostName web.example.com
    User www

Host db
    HostName db.example.com
    User postgres
    Port 5432
"#;
        let hosts = parse_ssh_config_str(config);
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0].alias, "web");
        assert_eq!(hosts[1].alias, "db");
    }

    #[test]
    fn test_skip_wildcard_hosts() {
        let config = r#"
Host *
    ServerAliveInterval 60

Host *.example.com
    User admin

Host myserver
    HostName 10.0.0.1
"#;
        let hosts = parse_ssh_config_str(config);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].alias, "myserver");
    }

    #[test]
    fn test_multi_alias_host_line() {
        let config = r#"
Host foo bar
    HostName shared.example.com
    User shared
"#;
        let hosts = parse_ssh_config_str(config);
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0].alias, "foo");
        assert_eq!(hosts[1].alias, "bar");
        assert_eq!(hosts[0].hostname, hosts[1].hostname);
    }

    #[test]
    fn test_proxy_jump() {
        let config = r#"
Host internal
    HostName 10.0.0.5
    ProxyJump bastion
"#;
        let hosts = parse_ssh_config_str(config);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].proxy_jump.as_deref(), Some("bastion"));
    }

    #[test]
    fn test_identity_file_tilde_expansion() {
        let config = r#"
Host myhost
    IdentityFile ~/.ssh/id_work
"#;
        let hosts = parse_ssh_config_str(config);
        assert_eq!(hosts.len(), 1);
        assert!(hosts[0].identity_file.is_some());
        assert!(!hosts[0].identity_file.as_ref().unwrap().starts_with("~"));
    }

    #[test]
    fn test_equals_syntax() {
        let config = r#"
Host eqhost
    HostName=eq.example.com
    User=equser
    Port=3022
"#;
        let hosts = parse_ssh_config_str(config);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].hostname.as_deref(), Some("eq.example.com"));
        assert_eq!(hosts[0].user.as_deref(), Some("equser"));
        assert_eq!(hosts[0].port, Some(3022));
    }

    #[test]
    fn test_comments_and_empty_lines() {
        let config = r#"
# This is a comment
Host server1
    # HostName commented.out
    HostName real.example.com

    User admin
"#;
        let hosts = parse_ssh_config_str(config);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].hostname.as_deref(), Some("real.example.com"));
    }

    #[test]
    fn test_empty_config() {
        let hosts = parse_ssh_config_str("");
        assert!(hosts.is_empty());
    }

    #[test]
    fn test_ssh_args_basic() {
        let host = SshHost {
            alias: "myhost".to_string(),
            hostname: Some("10.0.0.1".to_string()),
            user: Some("deploy".to_string()),
            port: Some(2222),
            identity_file: None,
            proxy_jump: None,
            source: SshHostSource::Config,
        };
        let args = host.ssh_args();
        assert_eq!(args, vec!["-p", "2222", "deploy@10.0.0.1"]);
    }

    #[test]
    fn test_ssh_args_default_port() {
        let host = SshHost {
            alias: "myhost".to_string(),
            hostname: Some("10.0.0.1".to_string()),
            user: None,
            port: Some(22),
            identity_file: None,
            proxy_jump: None,
            source: SshHostSource::Config,
        };
        let args = host.ssh_args();
        assert_eq!(args, vec!["10.0.0.1"]);
    }

    #[test]
    fn test_connection_string() {
        let host = SshHost {
            alias: "myhost".to_string(),
            hostname: Some("10.0.0.1".to_string()),
            user: Some("deploy".to_string()),
            port: Some(2222),
            identity_file: None,
            proxy_jump: None,
            source: SshHostSource::Config,
        };
        assert_eq!(host.connection_string(), "deploy@10.0.0.1:2222");
    }
}
