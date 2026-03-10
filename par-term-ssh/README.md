# par-term-ssh

SSH host management and discovery for the par-term terminal emulator.

## What This Crate Provides

- SSH config file parsing (`~/.ssh/config`) for host extraction
- mDNS/Bonjour host discovery for zero-configuration LAN SSH targets
- Known hosts file parsing
- Connection history tracking
- SSH host types shared between the terminal frontend and UI components

## Key Modules

| Module | Description |
|--------|-------------|
| `config_parser` | Parse `~/.ssh/config` to extract `Host` entries with their options |
| `discovery` | Aggregate SSH hosts from config, history, and mDNS sources |
| `history` | Track recently connected SSH hosts for quick-connect |
| `known_hosts` | Parse `~/.ssh/known_hosts` for host verification |
| `mdns` | mDNS/Bonjour discovery for local network SSH services |
| `types` | Shared SSH host and connection types |

## Workspace Position

Layer 0 in the dependency graph. This crate has no internal workspace dependencies and can be updated independently.

## Related Documentation

- [SSH Support](../docs/SSH.md) — user-facing documentation for SSH features
- [Architecture Overview](../docs/ARCHITECTURE.md) — workspace structure
