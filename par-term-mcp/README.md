# par-term-mcp

MCP (Model Context Protocol) stdio server for the par-term terminal emulator.

This crate implements a minimal JSON-RPC 2.0 server over stdin/stdout that exposes par-term terminal tools to ACP agents (Claude, Ollama) via the Model Context Protocol.

## What This Crate Provides

- A line-delimited JSON-RPC 2.0 stdin/stdout server loop
- MCP tool registration and dispatch
- `config_update` tool — writes configuration changes to a file for the main app to pick up via its config watcher
- `terminal_screenshot` tool — requests a live terminal screenshot via a file-based IPC handshake

## Key Modules

| Module | Description |
|--------|-------------|
| `jsonrpc` | JSON-RPC 2.0 wire types, response helpers, and stdout framing |
| `ipc` | IPC path resolution, atomic writes, and restricted-permission file helpers |
| `tools` | Tool registration, descriptors, and dispatch entry point |
| `tools::config_update` | `config_update` tool implementation |
| `tools::screenshot` | `terminal_screenshot` tool implementation |

## IPC File Locations

All IPC files are written to `~/.config/par-term/` (Linux/macOS) or `%APPDATA%\par-term\` (Windows) with restrictive permissions (`0o600` on Unix).

## Workspace Position

Layer 0 in the dependency graph. This crate has no internal workspace dependencies and can be updated independently.

## Related Documentation

- [Assistant Panel](../docs/ASSISTANT_PANEL.md) — ACP agent integration user documentation
- [ACP Harness](../docs/ACP_HARNESS.md) — debugging the ACP/MCP integration
- [Architecture Overview](../docs/ARCHITECTURE.md) — workspace structure
