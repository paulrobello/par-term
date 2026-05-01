# par-term-config

Configuration system for the par-term terminal emulator.

This crate defines all configuration types, loading, saving, and default values used across the par-term workspace. It is the foundation crate (Layer 1) that all other workspace crates depend on.

## What This Crate Provides

- `Config` struct — the main configuration struct with 200+ fields covering fonts, rendering, colors, keybindings, automation, and more
- Theme and color scheme types
- Shader configuration and metadata types
- Snippet and custom action configuration
- Automation trigger and coprocess configuration
- Profile configuration types
- Status bar widget configuration
- Scrollback mark types
- Configuration file watching (behind the `watcher` feature flag)
- Cell type shared between terminal and renderer crates
- Shell detection for profile shell selection

## Configuration File Location

| Platform | Path |
|----------|------|
| Linux / macOS | `~/.config/par-term/config.yaml` |
| Windows | `%APPDATA%\par-term\config.yaml` |

All fields are optional. Omitting a field uses its documented default value. See [docs/CONFIG_REFERENCE.md](../docs/CONFIG_REFERENCE.md) for the complete reference.

## Environment Variable Substitution

String config values support `${VAR}` substitution. By default, only a safe allowlist of variables is substituted (HOME, USER, SHELL, XDG_*, PAR_TERM_*, LC_*). Set `allow_all_env_vars: true` in your config to allow all environment variables.

## Feature Flags

| Flag | Description |
|------|-------------|
| `watcher` | Enables `notify`-based config file watching for hot reload |
| `wgpu-types` | Re-exports wgpu color types for GPU rendering integration |

## Installation

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
par-term-config = { version = "0.10.1" }
```

Enable optional features as needed:

```toml
par-term-config = { version = "0.10.1", features = ["watcher", "wgpu-types"] }
```

## Usage

```rust
use par_term_config::Config;

// Load configuration from the default platform-specific path
let config = Config::load()?;

// Access configuration values
println!("Font: {} ({})", config.font_family, config.font_size);
println!("Theme: {}", config.theme);

// Save configuration
config.save()?;
```

## Workspace Position

Layer 1 in the dependency graph. All Layer 2 and higher crates depend on this crate.

## Related Documentation

- [Config Reference](../docs/CONFIG_REFERENCE.md) — complete field reference
- [Architecture Overview](../docs/ARCHITECTURE.md) — workspace structure
- [Crate Structure](../docs/CRATE_STRUCTURE.md) — dependency layers and version bumps
