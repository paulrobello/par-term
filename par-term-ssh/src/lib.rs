//! SSH subsystem for par-term terminal emulator.
//!
//! Provides SSH config parsing, known_hosts scanning, shell history extraction,
//! and mDNS/Bonjour discovery for SSH hosts.
//!
//! Every source produces [`SshHost`] values tagged with an [`SshHostSource`].
//! [`discover_local_hosts`] merges the three file-based sources
//! (`~/.ssh/config`, `~/.ssh/known_hosts`, shell history) into one
//! deduplicated list; network discovery is separate and asynchronous, through
//! [`mdns::MdnsDiscovery`].
//!
//! Discovered hosts are untrusted input. Before a host reaches a command line,
//! call [`SshHost::validate_for_connect`] (for argv use) or
//! [`SshHost::ssh_command_line`] (for a shell command line), which reject
//! shell metacharacters and option-shaped values.
//!
//! ```
//! use par_term_ssh::config_parser::parse_ssh_config_str;
//!
//! let hosts = parse_ssh_config_str("Host web\n  HostName web.example.com\n  User deploy\n");
//! assert_eq!(hosts[0].connection_string(), "deploy@web.example.com");
//! assert_eq!(hosts[0].ssh_command_line().unwrap(), "ssh deploy@web.example.com");
//! ```

#![warn(missing_docs)]

pub mod config_parser;
pub mod discovery;
pub mod history;
pub mod known_hosts;
pub mod mdns;
pub mod types;

pub use discovery::discover_local_hosts;
pub use types::{SshHost, SshHostSource, UnsafeSshComponent};
