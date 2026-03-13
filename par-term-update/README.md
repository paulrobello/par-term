# par-term-update

Self-update and update-check system for the par-term terminal emulator.

This crate handles GitHub release polling, in-place binary replacement for standalone
installs, installation type detection, SHA256 asset verification, and bundled asset
manifest tracking for shaders and shell integration scripts.

## What This Crate Provides

- `update_checker` — polls the GitHub releases API to detect available updates, with
  configurable check frequency (daily, weekly, manual) and cooldown enforcement
- `self_updater` — in-place binary replacement for standalone installs; downloads the
  platform-appropriate asset, verifies its SHA256 checksum, and replaces the running binary
- `install_methods` — detects whether par-term is installed via Homebrew, `cargo install`,
  a macOS `.app` bundle, or as a standalone binary; provides platform-specific upgrade paths
- `binary_ops` — asset name resolution, download URL construction, and SHA256 verification
- `manifest` — tracks installed bundled assets (shaders, shell integration scripts) for
  upgrade and rollback
- `http` — shared HTTP utilities for release API and asset download requests

## Key Types

| Type | Purpose |
|------|---------|
| `UpdateChecker` | Polls GitHub releases API with frequency control |
| `UpdateCheckResult` | Outcome: `UpToDate`, `UpdateAvailable`, `Disabled`, `Skipped`, `Error` |
| `UpdateCheckInfo` | Available update details: version, release notes, URL |

These types are also available via `par-term-settings-ui` re-exports used by the settings
UI to display update status and trigger installation.

## Security Note

SHA256 checksums are verified against `.sha256` release assets before installation. When no
checksum asset is present, installation proceeds with a warning. See [SECURITY.md](../SECURITY.md)
for the full security policy on self-updates.

## Workspace Position

Layer 2 in the dependency graph. Depends on `par-term-config` for update frequency and
skipped-version settings. Used directly by the root `par-term` crate.

## Related Documentation

- [Self-Update](../docs/SELF_UPDATE.md) — update configuration and installation methods
- [Config Reference](../docs/CONFIG_REFERENCE.md) — update configuration options
- [Security Policy](../SECURITY.md) — checksum verification and update security
- [Crate Structure](../docs/CRATE_STRUCTURE.md) — dependency layers
