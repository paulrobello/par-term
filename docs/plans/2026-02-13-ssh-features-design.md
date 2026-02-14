# SSH Host Profiles, Quick Connect & Auto-Discovery

**Issue**: #134
**Date**: 2026-02-13

## Overview

Implement SSH host management, quick connect dialog, profile auto-switching, and mDNS discovery for par-term. All features are frontend-only — no core library changes needed.

## 1. SSH Config Parser (`src/ssh/`)

New module with sub-modules:

### `config_parser.rs`
Parses `~/.ssh/config` into structured `SshHost` entries. Extracts: `Host`, `HostName`, `User`, `Port`, `IdentityFile`, `ProxyJump`, `ForwardAgent`, `LocalForward`, `RemoteForward`. Supports wildcard hosts and `Host *` defaults.

### `known_hosts.rs`
Parses `~/.ssh/known_hosts` to discover previously-connected hosts. Handles hashed and plain hostname entries.

### `discovery.rs`
Aggregates hosts from SSH config, known_hosts, shell history (`~/.bash_history`, `~/.zsh_history`) scanning for `ssh` commands, and mDNS.

### Data structure
```rust
pub enum SshHostSource {
    Config,
    KnownHosts,
    History,
    Mdns,
}

pub struct SshHost {
    pub alias: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub proxy_jump: Option<String>,
    pub source: SshHostSource,
}
```

## 2. Quick Connect Dialog (`src/ssh_connect_ui.rs`)

Egui modal overlay opened with **Cmd+Shift+S**.

- Search bar at top with fuzzy filtering by alias, hostname, user, tags
- Scrollable host list grouped by source (Config, Known Hosts, History, mDNS)
- Optional profile override dropdown
- Keyboard navigation: Up/Down to select, Enter to connect, Escape to dismiss

### Action flow
Selecting a host opens a new tab with `ssh [user@]host [-p port]`. If the host matches a profile's `hostname_patterns`, that profile is auto-applied.

```rust
pub enum SshConnectAction {
    None,
    Connect { host: SshHost, profile: Option<ProfileId> },
    Cancel,
}

pub struct SshConnectUI {
    visible: bool,
    search_query: String,
    hosts: Vec<SshHost>,
    selected_index: usize,
    selected_profile: Option<ProfileId>,
}
```

## 3. Host Profiles — SSH-Specific Profile Fields

Extend existing `Profile` struct with optional SSH fields:

```rust
pub ssh_host: Option<String>,
pub ssh_user: Option<String>,
pub ssh_port: Option<u16>,
pub ssh_identity_file: Option<String>,
pub ssh_extra_args: Option<String>,
```

Profiles with `ssh_host` set appear in the quick connect dialog alongside discovered hosts. Opening such a profile launches `ssh [user@]host [-p port] [-i identity] [extra_args]` in a new tab.

Settings UI: New collapsible "SSH" section in profile editor modal.

## 4. Profile Auto-Switching

### Triggers

1. **User-based** (OSC 1337 RemoteHost): In `sync_badge_shell_integration()`, when hostname changes, look up matching profile via `profile_manager.find_by_hostname()`.

2. **Command-based**: When `current_command` starts with `"ssh"`, extract target host and look up matching profile. When SSH exits, revert.

### Revert mechanism

New fields on `Tab`:
```rust
pub pre_switch_profile: Option<ProfileId>,
pub auto_switched: bool,
```

Auto-switch saves previous profile; trigger clearing restores it. Manual selection clears `auto_switched`.

### Priority (highest to lowest)
1. Manual user selection
2. Hostname pattern match (OSC 1337 / OSC 7)
3. Command-based match (running `ssh` process)
4. Directory pattern match (CWD)
5. Default profile

## 5. Bonjour/mDNS Discovery (`src/ssh/mdns.rs`)

Uses `mdns-sd` crate to browse `_ssh._tcp.local.` services.

```rust
pub struct MdnsDiscovery {
    receiver: Option<mpsc::Receiver<SshHost>>,
    discovered: Vec<SshHost>,
    scanning: bool,
}
```

- Starts lazily when quick connect dialog opens
- Runs on tokio task with configurable timeout (default 3s)
- Results cached until next refresh
- Config: `enable_mdns_discovery: bool` (default false, opt-in)

## 6. Settings UI & Config

### New settings tab: SSH (`settings_ui/ssh_tab.rs`)
- `enable_mdns_discovery` toggle
- `mdns_scan_timeout_secs` slider
- `ssh_auto_profile_switch` toggle
- `ssh_revert_profile_on_disconnect` toggle

### Config additions
```rust
pub enable_mdns_discovery: bool,          // default false
pub mdns_scan_timeout_secs: u32,          // default 3
pub ssh_auto_profile_switch: bool,        // default true
pub ssh_revert_profile_on_disconnect: bool, // default true
```

### Keybinding
`Cmd+Shift+S` → `action:ssh_quick_connect` (built-in default)

### Sidebar search keywords
`ssh`, `remote`, `host`, `connect`, `mdns`, `bonjour`, `discovery`, `auto-switch`

## 7. Testing

### Unit tests
- SSH config parser: wildcards, ProxyJump, multi-host blocks, comments, edge cases
- Known hosts parser: hashed and plain entries, malformed lines
- Discovery aggregation and deduplication
- Profile auto-switching: priority order, revert, manual override protection

### Integration tests
- Quick connect → tab creation with correct SSH command
- SSH profile fields → correct command line construction
- Config round-trip for SSH fields
- mDNS: mock service resolution (no network in CI)
