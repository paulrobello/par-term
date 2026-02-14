//! SSH subsystem for host management, discovery, and quick connect.
//!
//! Provides SSH config parsing, known_hosts scanning, shell history extraction,
//! and mDNS/Bonjour discovery for SSH hosts.

pub mod config_parser;
pub mod types;

pub use types::{SshHost, SshHostSource};
