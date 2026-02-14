//! Shell history scanner for SSH commands.

use super::types::{SshHost, SshHostSource};
use std::collections::HashSet;
use std::path::Path;

/// Scan shell history files for SSH commands.
pub fn scan_history() -> Vec<SshHost> {
    let mut hosts = Vec::new();
    let mut seen = HashSet::new();

    if let Some(home) = dirs::home_dir() {
        let bash_hist = home.join(".bash_history");
        if bash_hist.exists() {
            scan_history_file(&bash_hist, false, &mut hosts, &mut seen);
        }

        let zsh_hist = home.join(".zsh_history");
        if zsh_hist.exists() {
            scan_history_file(&zsh_hist, true, &mut hosts, &mut seen);
        }

        let fish_hist = home.join(".local/share/fish/fish_history");
        if fish_hist.exists() {
            scan_fish_history(&fish_hist, &mut hosts, &mut seen);
        }
    }

    hosts
}

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

fn scan_fish_history(path: &Path, hosts: &mut Vec<SshHost>, seen: &mut HashSet<String>) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    for line in content.lines() {
        let line = line.trim();
        if let Some(cmd) = line.strip_prefix("- cmd: ")
            && let Some(host) = parse_ssh_command(cmd)
        {
            let key = host.connection_string();
            if !seen.contains(&key) {
                seen.insert(key);
                hosts.push(host);
            }
        }
    }
}

/// Parse an SSH command line and extract host info.
pub fn parse_ssh_command(line: &str) -> Option<SshHost> {
    let parts: Vec<&str> = line.split_whitespace().collect();

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
                i += 2;
            }
            arg if arg.starts_with('-') => {
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
                if target.is_none() {
                    target = Some(arg);
                }
                i += 1;
            }
        }
    }

    let target = target?;

    if target.starts_with('/') || target.starts_with('.') || target.contains('=') {
        return None;
    }

    let (user, hostname) = if let Some((u, h)) = target.split_once('@') {
        (Some(u.to_string()), h.to_string())
    } else {
        (None, target.to_string())
    };

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
        let host =
            parse_ssh_command("ssh -p 2222 -i ~/.ssh/key deploy@server.example.com").unwrap();
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
    fn test_ssh_with_preceding_command() {
        let host = parse_ssh_command("TERM=xterm ssh myhost.com").unwrap();
        assert_eq!(host.alias, "myhost.com");
    }
}
