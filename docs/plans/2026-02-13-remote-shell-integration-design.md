# Design: Install Shell Integration on Remote Host

**Issue**: #135
**Date**: 2026-02-13

## Summary

Add a menu option that sends a curl command to the active terminal to download and install par-term shell integration scripts on a remote host.

## Approach

Simple command injection with confirmation dialog. The existing install script at `https://paulrobello.github.io/par-term/install-shell-integration.sh` already handles shell detection, curl/wget fallback, RC file persistence, and cross-platform support.

### Alternatives considered

- **Shell detection first**: Inject `echo $SHELL` before install to target a specific shell. Rejected: the install script already detects the shell, and reading PTY output is complex/unreliable.
- **Embed script in binary**: Inject the install script as a heredoc. Rejected: fragile across different shells/environments, and the GitHub Pages URL already works.

## Components

1. **`MenuAction::InstallShellIntegrationRemote`** - New variant in `menu/actions.rs`
2. **Shell submenu** - New "Shell" menu in `menu/mod.rs` with this item
3. **`RemoteShellInstallUI`** - egui confirmation dialog in `src/remote_shell_install_ui.rs`
4. **Command injection** - Write curl command to active tab's PTY

## Command

```sh
curl -sSL https://paulrobello.github.io/par-term/install-shell-integration.sh | sh
```

## Dialog

- Title: "Install Shell Integration on Remote Host"
- Body: explains this sends a command to the active terminal
- Read-only code block with the exact command
- Warning: should only be used when SSH'd to a remote host
- Buttons: "Install" / "Cancel"

## Error handling

- No active tab: show error message in dialog or disable menu item
- Install script handles missing curl/wget and unsupported shells on the remote
