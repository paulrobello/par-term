//! Parser for ~/.ssh/known_hosts files.
//!
//! Extracts hostnames from known_hosts entries. Handles both plain and
//! hashed hostname formats, as well as bracketed [host]:port entries.

use super::types::{SshHost, SshHostSource};
use std::collections::HashSet;
use std::path::Path;

/// Parse a known_hosts file and return discovered hosts.
pub fn parse_known_hosts(path: &Path) -> Vec<SshHost> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    parse_known_hosts_str(&content)
}

/// Parse known_hosts from a string (for testing).
pub fn parse_known_hosts_str(content: &str) -> Vec<SshHost> {
    let mut seen = HashSet::new();
    let mut hosts = Vec::new();

    for line in content.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') || line.starts_with('@') {
            continue;
        }

        let host_field = match line.split_whitespace().next() {
            Some(f) => f,
            None => continue,
        };

        if host_field.starts_with("|1|") {
            continue;
        }

        for entry in host_field.split(',') {
            let (hostname, port) = parse_host_entry(entry);

            if hostname.is_empty() {
                continue;
            }

            let key = format!("{}:{}", hostname, port.unwrap_or(22));
            if seen.contains(&key) {
                continue;
            }
            seen.insert(key);

            hosts.push(SshHost {
                alias: hostname.clone(),
                hostname: Some(hostname),
                user: None,
                port,
                identity_file: None,
                proxy_jump: None,
                source: SshHostSource::KnownHosts,
            });
        }
    }

    hosts
}

fn parse_host_entry(entry: &str) -> (String, Option<u16>) {
    if entry.starts_with('[') {
        if let Some(bracket_end) = entry.find(']') {
            let hostname = entry[1..bracket_end].to_string();
            let port = entry[bracket_end + 1..]
                .strip_prefix(':')
                .and_then(|p| p.parse().ok());
            (hostname, port)
        } else {
            (entry.to_string(), None)
        }
    } else {
        (entry.to_string(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_entries() {
        let content = "github.com ssh-ed25519 AAAAC3...\nbitbucket.org ssh-rsa AAAAB3...\n";
        let hosts = parse_known_hosts_str(content);
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0].alias, "github.com");
        assert_eq!(hosts[1].alias, "bitbucket.org");
    }

    #[test]
    fn test_skip_hashed_entries() {
        let content = "|1|abc123|def456 ssh-rsa AAAAB3...\ngithub.com ssh-ed25519 AAAA...\n";
        let hosts = parse_known_hosts_str(content);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].alias, "github.com");
    }

    #[test]
    fn test_bracketed_port() {
        let content = "[myhost.example.com]:2222 ssh-rsa AAAAB3...\n";
        let hosts = parse_known_hosts_str(content);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].alias, "myhost.example.com");
        assert_eq!(hosts[0].port, Some(2222));
    }

    #[test]
    fn test_comma_separated_hostnames() {
        let content = "host1.example.com,192.168.1.1 ssh-rsa AAAAB3...\n";
        let hosts = parse_known_hosts_str(content);
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0].alias, "host1.example.com");
        assert_eq!(hosts[1].alias, "192.168.1.1");
    }

    #[test]
    fn test_dedup() {
        let content = "host.com ssh-rsa AAAA...\nhost.com ssh-ed25519 AAAA...\n";
        let hosts = parse_known_hosts_str(content);
        assert_eq!(hosts.len(), 1);
    }

    #[test]
    fn test_skip_comments_and_markers() {
        let content =
            "# comment\n@cert-authority *.example.com ssh-rsa AAAA...\nreal.host ssh-rsa AAAA...\n";
        let hosts = parse_known_hosts_str(content);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].alias, "real.host");
    }

    #[test]
    fn test_empty() {
        let hosts = parse_known_hosts_str("");
        assert!(hosts.is_empty());
    }
}
