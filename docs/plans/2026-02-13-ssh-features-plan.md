# SSH Features Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement SSH host profiles, quick connect dialog, profile auto-switching, and mDNS discovery for par-term (issue #134).

**Architecture:** New `src/ssh/` module for config parsing, host discovery, and mDNS. Quick connect UI as egui overlay. Profile system extended with SSH-specific fields. Auto-switching hooks into existing `sync_badge_shell_integration()`.

**Tech Stack:** Rust, egui (UI), mdns-sd (mDNS discovery), serde (config), existing profile/tab/config infrastructure.

---

### Task 1: Add mdns-sd dependency

**Files:**
- Modify: `Cargo.toml:103`

**Step 1: Add the dependency**

In `Cargo.toml`, after line 103 (`sysinfo = "0.37.2"`), add:
```toml
mdns-sd = "0.17"  # mDNS/Bonjour service discovery for SSH host auto-detection
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: Compiles without errors.

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add mdns-sd dependency for SSH host discovery"
```

---

### Task 2: Create SSH config parser module

**Files:**
- Create: `src/ssh/mod.rs`
- Create: `src/ssh/types.rs`
- Create: `src/ssh/config_parser.rs`
- Modify: `src/lib.rs:57` (add `pub mod ssh;`)

**Step 1: Create `src/ssh/types.rs`**

```rust
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

        // Add port if non-default
        if let Some(port) = self.port {
            if port != 22 {
                args.push("-p".to_string());
                args.push(port.to_string());
            }
        }

        // Add identity file
        if let Some(ref identity) = self.identity_file {
            args.push("-i".to_string());
            args.push(identity.clone());
        }

        // Add proxy jump
        if let Some(ref proxy) = self.proxy_jump {
            args.push("-J".to_string());
            args.push(proxy.clone());
        }

        // Add user@host or just host
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
```

**Step 2: Create `src/ssh/config_parser.rs`**

```rust
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

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Split on first whitespace or '='
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
                // Save previous host block
                flush_host_block(
                    &current_aliases,
                    &hostname,
                    &user,
                    &port,
                    &identity_file,
                    &proxy_jump,
                    &mut hosts,
                );

                // Parse new host aliases (can be space-separated)
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
                // Expand ~ to home directory
                let expanded = if value.starts_with("~/") {
                    if let Some(home) = dirs::home_dir() {
                        format!("{}/{}", home.display(), &value[2..])
                    } else {
                        value.to_string()
                    }
                } else {
                    value.to_string()
                };
                identity_file = Some(expanded);
            }
            "proxyjump" => proxy_jump = Some(value.to_string()),
            _ => {} // Ignore other directives
        }
    }

    // Flush final block
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
        // Default port should not be included
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
```

**Step 3: Create `src/ssh/mod.rs`**

```rust
//! SSH subsystem for host management, discovery, and quick connect.
//!
//! Provides SSH config parsing, known_hosts scanning, shell history extraction,
//! and mDNS/Bonjour discovery for SSH hosts.

pub mod config_parser;
pub mod types;

pub use types::{SshHost, SshHostSource};
```

**Step 4: Add module declaration in `src/lib.rs`**

After line 57 (`pub mod shell_quote;`), add:
```rust
pub mod ssh;
```

**Step 5: Run tests**

Run: `cargo test ssh::config_parser`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add src/ssh/ src/lib.rs
git commit -m "feat(ssh): add SSH config parser module"
```

---

### Task 3: Create known_hosts parser

**Files:**
- Create: `src/ssh/known_hosts.rs`
- Modify: `src/ssh/mod.rs`

**Step 1: Create `src/ssh/known_hosts.rs`**

```rust
//! Parser for ~/.ssh/known_hosts files.
//!
//! Extracts hostnames from known_hosts entries. Handles both plain and
//! hashed hostname formats, as well as bracketed [host]:port entries.

use super::types::{SshHost, SshHostSource};
use std::collections::HashSet;
use std::path::Path;

/// Parse a known_hosts file and return discovered hosts.
///
/// Hashed hostnames (starting with `|1|`) are skipped since the original
/// hostname cannot be recovered. Non-standard ports in `[host]:port` format
/// are captured.
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

        // Skip comments, empty lines, and revoked/cert markers
        if line.is_empty() || line.starts_with('#') || line.starts_with('@') {
            continue;
        }

        // First field is comma-separated list of hostnames
        let host_field = match line.split_whitespace().next() {
            Some(f) => f,
            None => continue,
        };

        // Skip hashed hostnames (|1|base64|base64)
        if host_field.starts_with("|1|") {
            continue;
        }

        // Parse comma-separated host entries
        for entry in host_field.split(',') {
            let (hostname, port) = parse_host_entry(entry);

            if hostname.is_empty() {
                continue;
            }

            // Skip IP addresses that are just numbers (less useful as display names)
            // but still include them as hosts
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

/// Parse a single host entry, handling [host]:port format.
fn parse_host_entry(entry: &str) -> (String, Option<u16>) {
    if entry.starts_with('[') {
        // [hostname]:port format
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
        let content = "# comment\n@cert-authority *.example.com ssh-rsa AAAA...\nreal.host ssh-rsa AAAA...\n";
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
```

**Step 2: Add to `src/ssh/mod.rs`**

```rust
pub mod known_hosts;
```

**Step 3: Run tests**

Run: `cargo test ssh::known_hosts`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add src/ssh/known_hosts.rs src/ssh/mod.rs
git commit -m "feat(ssh): add known_hosts parser"
```

---

### Task 4: Create shell history scanner and host discovery aggregator

**Files:**
- Create: `src/ssh/history.rs`
- Create: `src/ssh/discovery.rs`
- Modify: `src/ssh/mod.rs`

**Step 1: Create `src/ssh/history.rs`**

```rust
//! Shell history scanner for SSH commands.
//!
//! Scans bash, zsh, and fish history files for `ssh` command invocations
//! to discover previously-connected hosts.

use super::types::{SshHost, SshHostSource};
use std::collections::HashSet;
use std::path::Path;

/// Scan shell history files for SSH commands.
///
/// Checks `~/.bash_history`, `~/.zsh_history`, and `~/.local/share/fish/fish_history`.
pub fn scan_history() -> Vec<SshHost> {
    let mut hosts = Vec::new();
    let mut seen = HashSet::new();

    if let Some(home) = dirs::home_dir() {
        // Bash history
        let bash_hist = home.join(".bash_history");
        if bash_hist.exists() {
            scan_history_file(&bash_hist, false, &mut hosts, &mut seen);
        }

        // Zsh history
        let zsh_hist = home.join(".zsh_history");
        if zsh_hist.exists() {
            scan_history_file(&zsh_hist, true, &mut hosts, &mut seen);
        }

        // Fish history
        let fish_hist = home.join(".local/share/fish/fish_history");
        if fish_hist.exists() {
            scan_fish_history(&fish_hist, &mut hosts, &mut seen);
        }
    }

    hosts
}

/// Scan a bash/zsh history file for SSH commands.
fn scan_history_file(
    path: &Path,
    is_zsh: bool,
    hosts: &mut Vec<SshHost>,
    seen: &mut HashSet<String>,
) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    for line in content.lines() {
        let line = if is_zsh {
            // Zsh extended history format: ": timestamp:0;command"
            line.split_once(';').map(|(_, cmd)| cmd).unwrap_or(line)
        } else {
            line
        };

        if let Some(host) = parse_ssh_command(line) {
            let key = host.connection_string();
            if !seen.contains(&key) {
                seen.insert(key);
                hosts.push(host);
            }
        }
    }
}

/// Scan a fish history file for SSH commands.
///
/// Fish history uses YAML-like format:
/// ```text
/// - cmd: ssh user@host
///   when: 1234567890
/// ```
fn scan_fish_history(path: &Path, hosts: &mut Vec<SshHost>, seen: &mut HashSet<String>) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    for line in content.lines() {
        let line = line.trim();
        if let Some(cmd) = line.strip_prefix("- cmd: ") {
            if let Some(host) = parse_ssh_command(cmd) {
                let key = host.connection_string();
                if !seen.contains(&key) {
                    seen.insert(key);
                    hosts.push(host);
                }
            }
        }
    }
}

/// Parse an SSH command line and extract host info.
///
/// Handles common patterns:
/// - `ssh hostname`
/// - `ssh user@hostname`
/// - `ssh -p port hostname`
/// - `ssh -p port user@hostname`
pub fn parse_ssh_command(line: &str) -> Option<SshHost> {
    let parts: Vec<&str> = line.split_whitespace().collect();

    // Must start with "ssh" (or "ssh " after some prefix)
    let ssh_idx = parts.iter().position(|&p| p == "ssh")?;
    let args = &parts[ssh_idx + 1..];

    if args.is_empty() {
        return None;
    }

    let mut port: Option<u16> = None;
    let mut identity: Option<String> = None;
    let mut target: Option<&str> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i] {
            "-p" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().ok();
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "-i" => {
                if i + 1 < args.len() {
                    identity = Some(args[i + 1].to_string());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "-J" => {
                // Skip proxy jump arg
                i += 2;
            }
            arg if arg.starts_with('-') => {
                // Skip other flags; some take values, some don't.
                // Flags that take values: -b -c -D -E -e -F -I -L -l -m -O -o -Q -R -S -W -w
                let takes_value = matches!(
                    arg,
                    "-b" | "-c"
                        | "-D"
                        | "-E"
                        | "-e"
                        | "-F"
                        | "-I"
                        | "-L"
                        | "-l"
                        | "-m"
                        | "-O"
                        | "-o"
                        | "-Q"
                        | "-R"
                        | "-S"
                        | "-W"
                        | "-w"
                );
                if takes_value && i + 1 < args.len() {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            arg => {
                // First non-flag argument is the target
                if target.is_none() {
                    target = Some(arg);
                }
                i += 1;
            }
        }
    }

    let target = target?;

    // Skip if target looks like a command or path, not a hostname
    if target.starts_with('/') || target.starts_with('.') || target.contains('=') {
        return None;
    }

    // Parse user@host
    let (user, hostname) = if let Some((u, h)) = target.split_once('@') {
        (Some(u.to_string()), h.to_string())
    } else {
        (None, target.to_string())
    };

    // Skip empty or obviously invalid hostnames
    if hostname.is_empty() || hostname.starts_with('-') {
        return None;
    }

    Some(SshHost {
        alias: hostname.clone(),
        hostname: Some(hostname),
        user,
        port,
        identity_file: identity,
        proxy_jump: None,
        source: SshHostSource::History,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_ssh() {
        let host = parse_ssh_command("ssh myhost.com").unwrap();
        assert_eq!(host.alias, "myhost.com");
        assert_eq!(host.user, None);
        assert_eq!(host.port, None);
    }

    #[test]
    fn test_parse_user_at_host() {
        let host = parse_ssh_command("ssh deploy@myhost.com").unwrap();
        assert_eq!(host.alias, "myhost.com");
        assert_eq!(host.user.as_deref(), Some("deploy"));
    }

    #[test]
    fn test_parse_with_port() {
        let host = parse_ssh_command("ssh -p 2222 myhost.com").unwrap();
        assert_eq!(host.port, Some(2222));
    }

    #[test]
    fn test_parse_with_identity() {
        let host = parse_ssh_command("ssh -i ~/.ssh/id_work myhost.com").unwrap();
        assert_eq!(host.identity_file.as_deref(), Some("~/.ssh/id_work"));
    }

    #[test]
    fn test_parse_complex_command() {
        let host = parse_ssh_command("ssh -p 2222 -i ~/.ssh/key deploy@server.example.com")
            .unwrap();
        assert_eq!(host.alias, "server.example.com");
        assert_eq!(host.user.as_deref(), Some("deploy"));
        assert_eq!(host.port, Some(2222));
        assert_eq!(host.identity_file.as_deref(), Some("~/.ssh/key"));
    }

    #[test]
    fn test_skip_non_ssh() {
        assert!(parse_ssh_command("ls -la").is_none());
        assert!(parse_ssh_command("git push").is_none());
    }

    #[test]
    fn test_skip_ssh_only() {
        assert!(parse_ssh_command("ssh").is_none());
    }

    #[test]
    fn test_zsh_history_format() {
        // Simulating the parse after zsh prefix strip
        let host = parse_ssh_command("ssh myhost.com").unwrap();
        assert_eq!(host.alias, "myhost.com");
    }

    #[test]
    fn test_ssh_with_preceding_command() {
        // e.g., "TERM=xterm ssh myhost"
        let host = parse_ssh_command("TERM=xterm ssh myhost.com").unwrap();
        assert_eq!(host.alias, "myhost.com");
    }
}
```

**Step 2: Create `src/ssh/discovery.rs`**

```rust
//! SSH host discovery aggregator.
//!
//! Combines hosts from multiple sources: SSH config, known_hosts,
//! shell history, and mDNS. Deduplicates by hostname.

use super::config_parser;
use super::history;
use super::known_hosts;
use super::types::SshHost;
use std::collections::HashSet;

/// Discover SSH hosts from all local sources (config, known_hosts, history).
///
/// Does NOT include mDNS results — those are provided separately via
/// the async `MdnsDiscovery` system.
pub fn discover_local_hosts() -> Vec<SshHost> {
    let mut all_hosts = Vec::new();
    let mut seen = HashSet::new();

    if let Some(home) = dirs::home_dir() {
        let ssh_dir = home.join(".ssh");

        // 1. SSH config (highest priority — most complete info)
        let config_path = ssh_dir.join("config");
        if config_path.exists() {
            for host in config_parser::parse_ssh_config(&config_path) {
                let key = dedup_key(&host);
                if seen.insert(key) {
                    all_hosts.push(host);
                }
            }
        }

        // 2. Known hosts (second priority — confirmed connections)
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

    // 3. Shell history (lowest priority — may have stale entries)
    for host in history::scan_history() {
        let key = dedup_key(&host);
        if seen.insert(key) {
            all_hosts.push(host);
        }
    }

    all_hosts
}

/// Create a deduplication key from hostname (or alias) + port.
fn dedup_key(host: &SshHost) -> String {
    let target = host.hostname.as_deref().unwrap_or(&host.alias);
    let port = host.port.unwrap_or(22);
    format!("{}:{}", target.to_lowercase(), port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::types::SshHostSource;

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
```

**Step 3: Update `src/ssh/mod.rs`**

```rust
//! SSH subsystem for host management, discovery, and quick connect.
//!
//! Provides SSH config parsing, known_hosts scanning, shell history extraction,
//! and mDNS/Bonjour discovery for SSH hosts.

pub mod config_parser;
pub mod discovery;
pub mod history;
pub mod known_hosts;
pub mod types;

pub use discovery::discover_local_hosts;
pub use types::{SshHost, SshHostSource};
```

**Step 4: Run all SSH tests**

Run: `cargo test ssh::`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add src/ssh/
git commit -m "feat(ssh): add history scanner and host discovery aggregator"
```

---

### Task 5: Create mDNS discovery module

**Files:**
- Create: `src/ssh/mdns.rs`
- Modify: `src/ssh/mod.rs`

**Step 1: Create `src/ssh/mdns.rs`**

```rust
//! mDNS/Bonjour discovery for SSH hosts on the local network.
//!
//! Uses the `mdns-sd` crate to browse for `_ssh._tcp.local.` services.
//! Discovery runs asynchronously and sends results via an mpsc channel.

use super::types::{SshHost, SshHostSource};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::sync::mpsc;
use std::time::Duration;

/// mDNS discovery state.
pub struct MdnsDiscovery {
    /// Discovered hosts from mDNS
    discovered: Vec<SshHost>,
    /// Whether a scan is currently running
    scanning: bool,
    /// Receiver for hosts from background scan
    receiver: Option<mpsc::Receiver<SshHost>>,
}

impl Default for MdnsDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl MdnsDiscovery {
    /// Create a new mDNS discovery instance.
    pub fn new() -> Self {
        Self {
            discovered: Vec::new(),
            scanning: false,
            receiver: None,
        }
    }

    /// Start an mDNS scan for SSH services.
    ///
    /// The scan runs in a background thread for `timeout_secs` seconds.
    /// Call `poll()` periodically to collect results.
    pub fn start_scan(&mut self, timeout_secs: u32) {
        if self.scanning {
            return;
        }

        self.scanning = true;
        self.discovered.clear();

        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);

        let timeout = Duration::from_secs(timeout_secs as u64);

        std::thread::spawn(move || {
            run_mdns_scan(tx, timeout);
        });
    }

    /// Poll for newly discovered hosts. Returns true if new hosts were found.
    pub fn poll(&mut self) -> bool {
        let receiver = match &self.receiver {
            Some(r) => r,
            None => return false,
        };

        let mut found_new = false;

        // Drain all available results
        while let Ok(host) = receiver.try_recv() {
            // Dedup by hostname
            let dominated = self.discovered.iter().any(|h| {
                h.hostname == host.hostname
                    && h.port == host.port
            });
            if !dominated {
                self.discovered.push(host);
                found_new = true;
            }
        }

        // Check if scan is complete (channel closed)
        if receiver.try_recv() == Err(mpsc::TryRecvError::Disconnected) && !found_new {
            self.scanning = false;
            self.receiver = None;
        }

        found_new
    }

    /// Get all discovered hosts.
    pub fn hosts(&self) -> &[SshHost] {
        &self.discovered
    }

    /// Whether a scan is currently running.
    pub fn is_scanning(&self) -> bool {
        self.scanning
    }

    /// Clear discovered hosts and stop any running scan.
    pub fn clear(&mut self) {
        self.discovered.clear();
        self.scanning = false;
        self.receiver = None;
    }
}

/// Run the mDNS scan in a background thread.
fn run_mdns_scan(tx: mpsc::Sender<SshHost>, timeout: Duration) {
    let daemon = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            log::warn!("Failed to start mDNS daemon: {}", e);
            return;
        }
    };

    let receiver = match daemon.browse("_ssh._tcp.local.") {
        Ok(r) => r,
        Err(e) => {
            log::warn!("Failed to browse mDNS: {}", e);
            let _ = daemon.shutdown();
            return;
        }
    };

    let deadline = std::time::Instant::now() + timeout;

    loop {
        if std::time::Instant::now() >= deadline {
            break;
        }

        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        match receiver.recv_timeout(remaining.min(Duration::from_millis(500))) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let hostname = info
                    .get_hostname()
                    .trim_end_matches('.')
                    .to_string();
                let port = info.get_port();
                let service_name = info.get_fullname()
                    .split("._ssh._tcp")
                    .next()
                    .unwrap_or(&hostname)
                    .to_string();

                let host = SshHost {
                    alias: service_name,
                    hostname: Some(hostname),
                    user: None,
                    port: if port == 22 { None } else { Some(port) },
                    identity_file: None,
                    proxy_jump: None,
                    source: SshHostSource::Mdns,
                };

                if tx.send(host).is_err() {
                    break; // Receiver dropped
                }
            }
            Ok(_) => {} // Ignore other events
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let _ = daemon.shutdown();
}
```

**Step 2: Update `src/ssh/mod.rs` — add `pub mod mdns;`**

**Step 3: Run check**

Run: `cargo check`
Expected: Compiles without errors.

**Step 4: Commit**

```bash
git add src/ssh/mdns.rs src/ssh/mod.rs
git commit -m "feat(ssh): add mDNS/Bonjour host discovery"
```

---

### Task 6: Add SSH fields to Profile struct

**Files:**
- Modify: `src/profile/types.rs:120` (add fields before closing brace)
- Modify: `src/profile/types.rs:126-154` (update `new()`)
- Modify: `src/profile/types.rs:157-185` (update `with_id()`)

**Step 1: Add SSH fields to Profile struct**

In `src/profile/types.rs`, before line 121 (the closing `}` of the struct), add:

```rust
    // ========================================================================
    // SSH connection fields (issue #134)
    // ========================================================================
    /// SSH hostname for direct connection (profile acts as SSH bookmark)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_host: Option<String>,

    /// SSH user for direct connection
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_user: Option<String>,

    /// SSH port for direct connection
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_port: Option<u16>,

    /// SSH identity file path for direct connection
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_identity_file: Option<String>,

    /// Extra SSH arguments (e.g., "-o StrictHostKeyChecking=no")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_extra_args: Option<String>,
```

**Step 2: Update `Profile::new()` (line 126-154)**

Add after `badge_max_height: None,`:
```rust
            ssh_host: None,
            ssh_user: None,
            ssh_port: None,
            ssh_identity_file: None,
            ssh_extra_args: None,
```

**Step 3: Update `Profile::with_id()` (line 157-185)**

Same additions after `badge_max_height: None,`.

**Step 4: Add builder methods**

After the existing builder methods (around line 220+), add:

```rust
    /// Builder method to set SSH host
    pub fn ssh_host(mut self, host: impl Into<String>) -> Self {
        self.ssh_host = Some(host.into());
        self
    }

    /// Builder method to set SSH user
    pub fn ssh_user(mut self, user: impl Into<String>) -> Self {
        self.ssh_user = Some(user.into());
        self
    }

    /// Builder method to set SSH port
    pub fn ssh_port(mut self, port: u16) -> Self {
        self.ssh_port = Some(port);
        self
    }

    /// Build the SSH command arguments for this profile's SSH connection.
    /// Returns None if ssh_host is not set.
    pub fn ssh_command_args(&self) -> Option<Vec<String>> {
        let host = self.ssh_host.as_ref()?;
        let mut args = Vec::new();

        if let Some(port) = self.ssh_port {
            if port != 22 {
                args.push("-p".to_string());
                args.push(port.to_string());
            }
        }

        if let Some(ref identity) = self.ssh_identity_file {
            args.push("-i".to_string());
            args.push(identity.clone());
        }

        if let Some(ref extra) = self.ssh_extra_args {
            args.extend(extra.split_whitespace().map(String::from));
        }

        let target = if let Some(ref user) = self.ssh_user {
            format!("{}@{}", user, host)
        } else {
            host.clone()
        };
        args.push(target);

        Some(args)
    }
```

**Step 5: Run tests**

Run: `cargo test profile`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add src/profile/types.rs
git commit -m "feat(ssh): add SSH connection fields to Profile"
```

---

### Task 7: Add SSH fields to Profile Modal UI

**Files:**
- Modify: `src/profile_modal_ui.rs` (add temp fields, clear_form, load, save, render)

**Step 1: Add temp fields to `ProfileModalUI` struct**

After `badge_section_expanded: bool,` (line 108), add:

```rust
    /// Whether SSH settings section is expanded
    ssh_section_expanded: bool,
    // SSH temp fields
    temp_ssh_host: String,
    temp_ssh_user: String,
    temp_ssh_port: String,
    temp_ssh_identity_file: String,
    temp_ssh_extra_args: String,
```

**Step 2: Initialize in `new()`**

After `badge_section_expanded: false,` (line 151), add:

```rust
            ssh_section_expanded: false,
            temp_ssh_host: String::new(),
            temp_ssh_user: String::new(),
            temp_ssh_port: String::new(),
            temp_ssh_identity_file: String::new(),
            temp_ssh_extra_args: String::new(),
```

**Step 3: Update `clear_form()`**

After `self.temp_badge_max_height = None;` (line 230), add:

```rust
        self.temp_ssh_host.clear();
        self.temp_ssh_user.clear();
        self.temp_ssh_port.clear();
        self.temp_ssh_identity_file.clear();
        self.temp_ssh_extra_args.clear();
```

**Step 4: Update `load_profile_to_form()`**

After `self.temp_badge_max_height = profile.badge_max_height;` (line 264), add:

```rust
        // SSH fields
        self.temp_ssh_host = profile.ssh_host.clone().unwrap_or_default();
        self.temp_ssh_user = profile.ssh_user.clone().unwrap_or_default();
        self.temp_ssh_port = profile.ssh_port.map(|p| p.to_string()).unwrap_or_default();
        self.temp_ssh_identity_file = profile.ssh_identity_file.clone().unwrap_or_default();
        self.temp_ssh_extra_args = profile.ssh_extra_args.clone().unwrap_or_default();
```

**Step 5: Update `form_to_profile()`**

After `profile.badge_max_height = self.temp_badge_max_height;` (line 345), add:

```rust
        // SSH fields
        if !self.temp_ssh_host.is_empty() {
            profile.ssh_host = Some(self.temp_ssh_host.clone());
        }
        if !self.temp_ssh_user.is_empty() {
            profile.ssh_user = Some(self.temp_ssh_user.clone());
        }
        if !self.temp_ssh_port.is_empty() {
            profile.ssh_port = self.temp_ssh_port.parse().ok();
        }
        if !self.temp_ssh_identity_file.is_empty() {
            profile.ssh_identity_file = Some(self.temp_ssh_identity_file.clone());
        }
        if !self.temp_ssh_extra_args.is_empty() {
            profile.ssh_extra_args = Some(self.temp_ssh_extra_args.clone());
        }
```

**Step 6: Add SSH section to the edit form UI**

Find the edit form rendering method (look for `show_edit_form` or where `badge_section_expanded` is used in `CollapsingHeader`). Add a similar collapsible SSH section:

```rust
        // SSH Connection section
        egui::CollapsingHeader::new("SSH Connection")
            .default_open(self.ssh_section_expanded)
            .show(ui, |ui| {
                self.ssh_section_expanded = true;
                ui.horizontal(|ui| {
                    ui.label("Host:");
                    ui.text_edit_singleline(&mut self.temp_ssh_host);
                });
                ui.horizontal(|ui| {
                    ui.label("User:");
                    ui.text_edit_singleline(&mut self.temp_ssh_user);
                });
                ui.horizontal(|ui| {
                    ui.label("Port:");
                    ui.add(egui::TextEdit::singleline(&mut self.temp_ssh_port).desired_width(60.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Identity File:");
                    ui.text_edit_singleline(&mut self.temp_ssh_identity_file);
                });
                ui.horizontal(|ui| {
                    ui.label("Extra Args:");
                    ui.text_edit_singleline(&mut self.temp_ssh_extra_args);
                });
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("When SSH Host is set, opening this profile connects via SSH instead of launching a shell.")
                        .weak()
                        .size(11.0),
                );
            });
```

**Step 7: Run check**

Run: `cargo check`
Expected: Compiles without errors.

**Step 8: Commit**

```bash
git add src/profile_modal_ui.rs
git commit -m "feat(ssh): add SSH fields to profile editor UI"
```

---

### Task 8: Add SSH config fields and settings tab

**Files:**
- Modify: `src/config/mod.rs` (add SSH config fields)
- Create: `src/settings_ui/ssh_tab.rs`
- Modify: `src/settings_ui/mod.rs` (add `pub mod ssh_tab;`)
- Modify: `src/settings_ui/sidebar.rs` (add SSH tab to enum + keywords)

**Step 1: Add config fields**

In `src/config/mod.rs`, in the `Config` struct, add (near other feature toggles):

```rust
    /// Enable mDNS/Bonjour discovery for SSH hosts
    #[serde(default = "defaults::bool_false")]
    pub enable_mdns_discovery: bool,

    /// mDNS scan timeout in seconds
    #[serde(default = "defaults::mdns_timeout")]
    pub mdns_scan_timeout_secs: u32,

    /// Enable automatic profile switching based on SSH hostname
    #[serde(default = "defaults::bool_true")]
    pub ssh_auto_profile_switch: bool,

    /// Revert profile when SSH session disconnects
    #[serde(default = "defaults::bool_true")]
    pub ssh_revert_profile_on_disconnect: bool,
```

In `src/config/defaults.rs`, add:

```rust
pub fn mdns_timeout() -> u32 {
    3
}
```

**Step 2: Add SSH tab to SettingsTab enum**

In `src/settings_ui/sidebar.rs`, add `Ssh` variant to the `SettingsTab` enum (after `Profiles`):

```rust
    Ssh,
```

Update `display_name()`:
```rust
Self::Ssh => "SSH",
```

Update `icon()`:
```rust
Self::Ssh => "🔗",
```

Update `all()` — add `Self::Ssh` after `Self::Profiles`.

Update `tab_search_keywords()`:
```rust
SettingsTab::Ssh => &[
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
],
```

Update `tab_contents_summary()` — add:
```rust
SettingsTab::Ssh => "SSH connection settings, mDNS discovery, auto-switch behavior",
```

**Step 3: Create `src/settings_ui/ssh_tab.rs`**

```rust
//! SSH settings tab for the settings UI.

use crate::settings_ui::SettingsUI;

impl SettingsUI {
    /// Render the SSH settings tab.
    pub(crate) fn show_ssh_tab(&mut self, ui: &mut egui::Ui, changes_this_frame: &mut bool) {
        ui.heading("SSH Settings");
        ui.add_space(8.0);

        // Auto-switch section
        ui.group(|ui| {
            ui.label(egui::RichText::new("Profile Auto-Switching").strong());
            ui.add_space(4.0);

            if ui
                .checkbox(
                    &mut self.config.ssh_auto_profile_switch,
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
                    &mut self.config.ssh_revert_profile_on_disconnect,
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

        // mDNS section
        ui.group(|ui| {
            ui.label(egui::RichText::new("mDNS/Bonjour Discovery").strong());
            ui.add_space(4.0);

            if ui
                .checkbox(
                    &mut self.config.enable_mdns_discovery,
                    "Enable mDNS host discovery",
                )
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
                ui.label("Scan timeout (seconds):");
                let mut timeout = self.config.mdns_scan_timeout_secs as f32;
                if ui
                    .add(egui::Slider::new(&mut timeout, 1.0..=10.0).integer())
                    .changed()
                {
                    self.config.mdns_scan_timeout_secs = timeout as u32;
                    self.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        });

        ui.add_space(12.0);

        // Quick connect shortcut info
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
```

**Step 4: Add `pub mod ssh_tab;` to `src/settings_ui/mod.rs`**

After `pub mod profiles_tab;` (line 24), add:
```rust
pub mod ssh_tab;
```

**Step 5: Wire SSH tab rendering into the settings UI show method**

Find the main `show()` method in SettingsUI where tabs are rendered (pattern: `match self.current_tab { ... }`). Add:
```rust
SettingsTab::Ssh => self.show_ssh_tab(ui, &mut changes_this_frame),
```

**Step 6: Run check**

Run: `cargo check`
Expected: Compiles without errors.

**Step 7: Commit**

```bash
git add src/config/ src/settings_ui/
git commit -m "feat(ssh): add SSH config options and settings tab"
```

---

### Task 9: Create Quick Connect UI

**Files:**
- Create: `src/ssh_connect_ui.rs`
- Modify: `src/lib.rs` (add `pub mod ssh_connect_ui;`)

**Step 1: Create `src/ssh_connect_ui.rs`**

```rust
//! SSH Quick Connect dialog.
//!
//! An egui modal overlay for browsing and connecting to SSH hosts.
//! Opened via Cmd+Shift+S. Shows hosts from SSH config, known_hosts,
//! shell history, and optionally mDNS.

use crate::profile::ProfileId;
use crate::ssh::{SshHost, SshHostSource, discover_local_hosts};
use crate::ssh::mdns::MdnsDiscovery;

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
    /// Whether the dialog is visible
    visible: bool,
    /// Search query for filtering hosts
    search_query: String,
    /// All discovered hosts
    hosts: Vec<SshHost>,
    /// Currently selected host index
    selected_index: usize,
    /// Optional profile override for the connection
    selected_profile: Option<ProfileId>,
    /// mDNS discovery state
    mdns: MdnsDiscovery,
    /// Whether mDNS is enabled
    mdns_enabled: bool,
    /// Whether hosts have been loaded
    hosts_loaded: bool,
    /// Whether the search field should get focus
    request_focus: bool,
}

impl Default for SshConnectUI {
    fn default() -> Self {
        Self::new()
    }
}

impl SshConnectUI {
    /// Create a new quick connect UI.
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

    /// Show the quick connect dialog.
    pub fn open(&mut self, mdns_enabled: bool, mdns_timeout: u32) {
        self.visible = true;
        self.search_query.clear();
        self.selected_index = 0;
        self.selected_profile = None;
        self.mdns_enabled = mdns_enabled;
        self.request_focus = true;

        // Load hosts from local sources
        self.hosts = discover_local_hosts();
        self.hosts_loaded = true;

        // Start mDNS scan if enabled
        if mdns_enabled {
            self.mdns.start_scan(mdns_timeout);
        }
    }

    /// Close the dialog.
    pub fn close(&mut self) {
        self.visible = false;
        self.hosts.clear();
        self.mdns.clear();
        self.hosts_loaded = false;
    }

    /// Whether the dialog is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Render the dialog and return any action.
    pub fn show(&mut self, ctx: &egui::Context) -> SshConnectAction {
        if !self.visible {
            return SshConnectAction::None;
        }

        // Poll mDNS for new hosts
        if self.mdns.poll() {
            // Add new mDNS hosts
            for host in self.mdns.hosts() {
                let dominated = self.hosts.iter().any(|h| {
                    h.hostname == host.hostname && h.port == host.port
                });
                if !dominated {
                    self.hosts.push(host.clone());
                }
            }
        }

        let mut action = SshConnectAction::None;

        let screen_rect = ctx.screen_rect();
        let dialog_width = (screen_rect.width() * 0.5).min(500.0).max(350.0);
        let dialog_height = (screen_rect.height() * 0.6).min(500.0).max(300.0);

        egui::Area::new(egui::Id::new("ssh_connect_overlay"))
            .fixed_pos(egui::pos2(
                (screen_rect.width() - dialog_width) / 2.0,
                (screen_rect.height() - dialog_height) / 2.5,
            ))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(16.0)
                    .shadow(egui::epaint::Shadow {
                        spread: 8.0,
                        blur: 16.0,
                        color: egui::Color32::from_black_alpha(100),
                        offset: egui::Vec2::new(0.0, 4.0),
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
                            [dialog_width - 32.0, 24.0],
                            egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text("Search hosts...")
                                .desired_width(dialog_width - 32.0),
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

                        // Clamp selection
                        if !filtered.is_empty() {
                            self.selected_index = self.selected_index.min(filtered.len() - 1);
                        }

                        // Handle keyboard navigation
                        let mut enter_pressed = false;
                        if search_response.has_focus() {
                            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                                if self.selected_index + 1 < filtered.len() {
                                    self.selected_index += 1;
                                }
                            }
                            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                                if self.selected_index > 0 {
                                    self.selected_index -= 1;
                                }
                            }
                            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                enter_pressed = true;
                            }
                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                action = SshConnectAction::Cancel;
                            }
                        }

                        // Host list
                        egui::ScrollArea::vertical()
                            .max_height(dialog_height - 100.0)
                            .show(ui, |ui| {
                                if filtered.is_empty() {
                                    ui.label(
                                        egui::RichText::new("No hosts found.")
                                            .weak()
                                            .italics(),
                                    );
                                    return;
                                }

                                let mut current_source: Option<&SshHostSource> = None;
                                for (display_idx, &host_idx) in filtered.iter().enumerate() {
                                    let host = &self.hosts[host_idx];

                                    // Section header when source changes
                                    if current_source != Some(&host.source) {
                                        current_source = Some(&host.source);
                                        ui.add_space(4.0);
                                        ui.label(
                                            egui::RichText::new(host.source.to_string())
                                                .strong()
                                                .size(11.0)
                                                .color(egui::Color32::from_rgb(140, 140, 180)),
                                        );
                                        ui.separator();
                                    }

                                    let is_selected = display_idx == self.selected_index;

                                    let response = ui.add_sized(
                                        [dialog_width - 48.0, 28.0],
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "  {}  {}",
                                                host.alias,
                                                egui::RichText::new(host.connection_string())
                                                    .weak()
                                            )),
                                        )
                                        .fill(if is_selected {
                                            egui::Color32::from_rgb(50, 50, 70)
                                        } else {
                                            egui::Color32::TRANSPARENT
                                        }),
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

                        // Bottom bar
                        ui.add_space(8.0);
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Cancel").clicked() {
                                action = SshConnectAction::Cancel;
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new("↑↓ Navigate  ⏎ Connect  Esc Cancel")
                                        .weak()
                                        .size(10.0),
                                );
                            });
                        });
                    });
            });

        // Handle cancel/close
        match &action {
            SshConnectAction::Cancel => self.close(),
            SshConnectAction::Connect { .. } => self.close(),
            SshConnectAction::None => {}
        }

        action
    }
}
```

**Step 2: Add module to `src/lib.rs`**

After `pub mod shell_quote;`:
```rust
pub mod ssh_connect_ui;
```

**Step 3: Run check**

Run: `cargo check`
Expected: Compiles without errors.

**Step 4: Commit**

```bash
git add src/ssh_connect_ui.rs src/lib.rs
git commit -m "feat(ssh): add quick connect dialog UI"
```

---

### Task 10: Wire Quick Connect into app and keybindings

**Files:**
- Modify: `src/app/window_state.rs` (add `ssh_connect_ui` field)
- Modify: `src/app/input_events.rs:1596` (add `ssh_quick_connect` action)
- Modify: `src/app/handler.rs` or the about_to_wait render path (render dialog and handle actions)

**Step 1: Add SshConnectUI to WindowState**

In `src/app/window_state.rs`, add field:
```rust
pub ssh_connect_ui: crate::ssh_connect_ui::SshConnectUI,
```

Initialize it in the constructor with `SshConnectUI::new()`.

**Step 2: Add keybinding action**

In `src/app/input_events.rs`, in `execute_keybinding_action()`, before the `_ =>` catch-all at line 1596, add:

```rust
            "ssh_quick_connect" => {
                self.ssh_connect_ui.open(
                    self.config.enable_mdns_discovery,
                    self.config.mdns_scan_timeout_secs,
                );
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!("SSH Quick Connect opened via keybinding");
                true
            }
```

**Step 3: Register default keybinding**

Find where default keybindings are generated (look for `generate_snippet_action_keybindings` or the default keybinding list in config). Add:

```rust
KeyBinding {
    key: "Cmd+Shift+S".to_string(),
    action: "ssh_quick_connect".to_string(),
}
```

**Step 4: Render the dialog and handle actions**

In the egui rendering path (where other overlays like `close_confirmation_ui` are rendered), add:

```rust
// SSH Quick Connect dialog
let ssh_action = self.ssh_connect_ui.show(ctx);
match ssh_action {
    crate::ssh_connect_ui::SshConnectAction::Connect { host, profile_override } => {
        // Open new tab with SSH command
        let ssh_args = host.ssh_args();
        // Create a new tab running `ssh <args>`
        self.new_tab_with_ssh_command(&ssh_args, profile_override);
    }
    crate::ssh_connect_ui::SshConnectAction::Cancel => {
        // Dialog already closed itself
    }
    crate::ssh_connect_ui::SshConnectAction::None => {}
}
```

**Step 5: Implement `new_tab_with_ssh_command` on WindowState**

In `src/app/tab_ops.rs`, add a method to create a tab running an SSH command:

```rust
    /// Create a new tab with an SSH command
    pub fn new_tab_with_ssh_command(
        &mut self,
        ssh_args: &[String],
        profile_override: Option<crate::profile::ProfileId>,
    ) {
        // Build command: ssh <args>
        let command = "ssh".to_string();
        let args: Vec<String> = ssh_args.to_vec();

        // Create tab with this command
        // ... (uses existing tab creation flow with command override)
    }
```

This should use the existing tab creation patterns — the exact integration will depend on how the current `new_tab()` and `Tab::new_from_profile()` work together. The implementer should look at how `Profile.command` is used in `Tab::new_from_profile()` and follow that same pattern.

**Step 6: Run check**

Run: `cargo check`
Expected: Compiles without errors.

**Step 7: Commit**

```bash
git add src/app/ src/ssh_connect_ui.rs
git commit -m "feat(ssh): wire quick connect dialog to keybindings and tab creation"
```

---

### Task 11: Implement profile auto-switching

**Files:**
- Modify: `src/tab/mod.rs:358` (add `pre_switch_profile` and `auto_switched` fields)
- Modify: `src/app/handler.rs:86-136` (add auto-switch logic to `sync_badge_shell_integration`)

**Step 1: Add fields to Tab struct**

In `src/tab/mod.rs`, before line 359 (the `}` closing the Tab struct), add:

```rust
    /// Profile saved before SSH auto-switch (for revert on disconnect)
    pub pre_ssh_switch_profile: Option<crate::profile::ProfileId>,
    /// Whether current profile was auto-applied due to SSH hostname detection
    pub ssh_auto_switched: bool,
```

Initialize both in `Tab::new()` and `Tab::new_from_profile()`:
```rust
            pre_ssh_switch_profile: None,
            ssh_auto_switched: false,
```

**Step 2: Add auto-switch logic to `sync_badge_shell_integration()`**

In `src/app/handler.rs`, after the hostname change detection (around line 120-124), add:

```rust
            // Auto-switch profile based on hostname change
            if self.config.ssh_auto_profile_switch {
                if let Some(ref host) = hostname {
                    // Hostname changed — try to find a matching profile
                    if let Some(matching_profile) = self.profile_manager.find_by_hostname(host) {
                        let profile_id = matching_profile.id;
                        let tab = self.tab_manager.active_tab_mut().unwrap();
                        // Only auto-switch if not already on this profile and not manually selected
                        if tab.auto_applied_profile_id != Some(profile_id) && !tab.ssh_auto_switched {
                            // Save current profile for revert
                            tab.pre_ssh_switch_profile = tab.auto_applied_profile_id;
                            tab.ssh_auto_switched = true;
                            tab.auto_applied_profile_id = Some(profile_id);
                            // Apply profile settings (icon, badge, etc.)
                            // ... use existing profile application pattern
                            log::info!("SSH auto-switched to profile {:?} for host {}", profile_id, host);
                        }
                    }
                } else if hostname.is_none() && self.config.ssh_revert_profile_on_disconnect {
                    // Hostname cleared — SSH disconnected, revert profile
                    let tab = self.tab_manager.active_tab_mut().unwrap();
                    if tab.ssh_auto_switched {
                        tab.auto_applied_profile_id = tab.pre_ssh_switch_profile.take();
                        tab.ssh_auto_switched = false;
                        log::info!("SSH auto-switch reverted to previous profile");
                    }
                }
            }
```

**Step 3: Add command-based switching**

Also in `sync_badge_shell_integration()`, after checking `current_command`:

```rust
            // Command-based SSH detection
            if self.config.ssh_auto_profile_switch {
                if let Some(ref cmd) = current_command {
                    if cmd == "ssh" || cmd.starts_with("ssh ") {
                        // SSH is running — try to match the target host
                        if let Some(host) = crate::ssh::history::parse_ssh_command(cmd) {
                            if let Some(hostname) = &host.hostname {
                                if let Some(matching_profile) = self.profile_manager.find_by_hostname(hostname) {
                                    let profile_id = matching_profile.id;
                                    let tab = self.tab_manager.active_tab_mut().unwrap();
                                    if tab.auto_applied_profile_id != Some(profile_id) && !tab.ssh_auto_switched {
                                        tab.pre_ssh_switch_profile = tab.auto_applied_profile_id;
                                        tab.ssh_auto_switched = true;
                                        tab.auto_applied_profile_id = Some(profile_id);
                                        log::info!("SSH command auto-switched to profile for host {}", hostname);
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // No command running — revert if SSH was auto-switched
                    let tab = self.tab_manager.active_tab_mut().unwrap();
                    if tab.ssh_auto_switched && self.config.ssh_revert_profile_on_disconnect {
                        tab.auto_applied_profile_id = tab.pre_ssh_switch_profile.take();
                        tab.ssh_auto_switched = false;
                        log::info!("SSH command ended — reverted to previous profile");
                    }
                }
            }
```

**Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add src/tab/mod.rs src/app/handler.rs
git commit -m "feat(ssh): implement profile auto-switching for SSH connections"
```

---

### Task 12: Update sidebar search keywords for Profiles tab

**Files:**
- Modify: `src/settings_ui/sidebar.rs`

**Step 1: Add SSH-related keywords to Profiles tab**

The Profiles tab at line 557 already has `"ssh"` and `"hostname"`. Add more:

```rust
        SettingsTab::Profiles => &[
            "profile",
            "profiles",
            "shell",
            "shell selection",
            "login shell",
            "login",
            "bash",
            "zsh",
            "fish",
            "powershell",
            "tags",
            "inheritance",
            "shortcut",
            "auto switch",
            "hostname",
            "ssh",
            "ssh host",
            "ssh user",
            "ssh port",
            "identity file",
            "remote",
            "connection",
            "profile drawer",
        ],
```

**Step 2: Run check**

Run: `cargo check`
Expected: Compiles without errors.

**Step 3: Commit**

```bash
git add src/settings_ui/sidebar.rs
git commit -m "feat(ssh): update search keywords for SSH settings"
```

---

### Task 13: Add SSH profile support to tab creation

**Files:**
- Modify: `src/tab/mod.rs` (in `new_from_profile`, detect SSH profile and launch SSH)

**Step 1: Update `Tab::new_from_profile()`**

In `src/tab/mod.rs`, in `new_from_profile()`, before the existing command/shell logic, add:

```rust
        // If this profile has SSH settings, launch SSH instead of a shell
        if let Some(ssh_args) = profile.ssh_command_args() {
            // Use SSH as the command
            let command = "ssh".to_string();
            // ... set up tab with ssh command and args
            // Follow existing pattern for profile.command
        }
```

This integrates with the existing profile-based tab creation so that profiles with `ssh_host` set automatically launch an SSH connection.

**Step 2: Run tests**

Run: `cargo test`
Expected: All tests pass.

**Step 3: Commit**

```bash
git add src/tab/mod.rs
git commit -m "feat(ssh): launch SSH from profiles with ssh_host set"
```

---

### Task 14: Final integration tests and cleanup

**Files:**
- Create: `tests/ssh_integration.rs` (integration tests)
- Run full test suite

**Step 1: Create integration tests**

```rust
//! Integration tests for the SSH subsystem.

use par_term::ssh::config_parser::parse_ssh_config_str;
use par_term::ssh::known_hosts::parse_known_hosts_str;
use par_term::ssh::history::parse_ssh_command;
use par_term::ssh::types::SshHostSource;

#[test]
fn test_ssh_config_roundtrip() {
    let config = r#"
Host production
    HostName prod.example.com
    User deploy
    Port 22
    IdentityFile ~/.ssh/id_prod
    ProxyJump bastion

Host staging
    HostName staging.example.com
    User staging
"#;
    let hosts = parse_ssh_config_str(config);
    assert_eq!(hosts.len(), 2);

    let prod = &hosts[0];
    assert_eq!(prod.alias, "production");
    let args = prod.ssh_args();
    assert!(args.contains(&"-J".to_string()));
    assert!(args.contains(&"bastion".to_string()));
    assert!(args.iter().any(|a| a.contains("deploy@")));
}

#[test]
fn test_profile_ssh_command_args() {
    use par_term::profile::Profile;

    let mut profile = Profile::new("SSH Test");
    profile.ssh_host = Some("server.example.com".to_string());
    profile.ssh_user = Some("admin".to_string());
    profile.ssh_port = Some(2222);
    profile.ssh_identity_file = Some("/home/user/.ssh/id_work".to_string());
    profile.ssh_extra_args = Some("-o StrictHostKeyChecking=no".to_string());

    let args = profile.ssh_command_args().unwrap();
    assert!(args.contains(&"-p".to_string()));
    assert!(args.contains(&"2222".to_string()));
    assert!(args.contains(&"-i".to_string()));
    assert!(args.iter().any(|a| a.contains("admin@server.example.com")));
    assert!(args.contains(&"-o".to_string()));
    assert!(args.contains(&"StrictHostKeyChecking=no".to_string()));
}

#[test]
fn test_host_source_display() {
    assert_eq!(SshHostSource::Config.to_string(), "SSH Config");
    assert_eq!(SshHostSource::KnownHosts.to_string(), "Known Hosts");
    assert_eq!(SshHostSource::History.to_string(), "History");
    assert_eq!(SshHostSource::Mdns.to_string(), "mDNS");
}

#[test]
fn test_connection_string_formats() {
    use par_term::ssh::types::SshHost;

    // Just hostname
    let host = SshHost {
        alias: "test".to_string(),
        hostname: Some("example.com".to_string()),
        user: None,
        port: None,
        identity_file: None,
        proxy_jump: None,
        source: SshHostSource::Config,
    };
    assert_eq!(host.connection_string(), "example.com");

    // User@host:port
    let host2 = SshHost {
        alias: "test".to_string(),
        hostname: Some("example.com".to_string()),
        user: Some("admin".to_string()),
        port: Some(2222),
        identity_file: None,
        proxy_jump: None,
        source: SshHostSource::Config,
    };
    assert_eq!(host2.connection_string(), "admin@example.com:2222");
}
```

**Step 2: Run all tests**

Run: `cargo test`
Expected: All tests pass.

**Step 3: Run full check suite**

Run: `make checkall`
Expected: fmt, lint, typecheck, tests all pass.

**Step 4: Commit**

```bash
git add tests/ssh_integration.rs
git commit -m "test(ssh): add integration tests for SSH subsystem"
```

---

### Task 15: Update CHANGELOG and MATRIX

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `MATRIX.md` (if it exists)

**Step 1: Add entry to CHANGELOG**

Add under the latest version section:

```markdown
### Added
- SSH config parser (`~/.ssh/config`) for host discovery
- SSH known_hosts parser for previously-connected hosts
- Shell history scanner for SSH commands
- mDNS/Bonjour SSH host discovery (opt-in)
- SSH Quick Connect dialog (Cmd+Shift+S) with search, keyboard navigation
- SSH-specific profile fields (host, user, port, identity file, extra args)
- Automatic profile switching on SSH connection/disconnection
- Command-based profile switching when `ssh` process is detected
- SSH settings tab in Settings UI
- Profile auto-revert when SSH session ends
```

**Step 2: Update MATRIX.md**

Mark completed items in the relevant sections (Network & Discovery, Profile Switching).

**Step 3: Commit**

```bash
git add CHANGELOG.md MATRIX.md
git commit -m "docs: update CHANGELOG and MATRIX for SSH features"
```
