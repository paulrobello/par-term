//! SSH subsystem for host management, discovery, and quick connect.
//!
//! Provides SSH config parsing, known_hosts scanning, shell history extraction,
//! and mDNS/Bonjour discovery for SSH hosts.

pub mod config_parser;
pub mod discovery;
pub mod history;
pub mod known_hosts;
pub mod mdns;
pub mod types;

pub use discovery::discover_local_hosts;
pub use types::{SshHost, SshHostSource};
