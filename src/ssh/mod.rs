//! SSH subsystem re-exports from the `par-term-ssh` crate.

pub use par_term_ssh::{SshHost, SshHostSource, discover_local_hosts};

// Re-export submodules for backward compatibility
pub use par_term_ssh::config_parser;
pub use par_term_ssh::discovery;
pub use par_term_ssh::history;
pub use par_term_ssh::known_hosts;
pub use par_term_ssh::mdns;
pub use par_term_ssh::types;
