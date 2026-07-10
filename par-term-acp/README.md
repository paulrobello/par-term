# par-term-acp

Agent Communication Protocol (ACP) implementation for the par-term terminal emulator.

This crate provides the core ACP protocol implementation for communicating with AI coding
agents (Claude Code, Codex CLI, Copilot, etc.) via JSON-RPC. It handles agent
lifecycle management, filesystem sandboxing, permission dispatch, and session management.

## What This Crate Provides

- `Agent` — agent lifecycle management: spawn, handshake, message routing, and dispatch
- `AgentConfig` / `discover_agents` — agent discovery and configuration loading from `~/.config/par-term/agents/`
- `JsonRpcClient` — JSON-RPC 2.0 client with async read/write over stdio
- `protocol` — ACP message types: `InitializeParams`, `SessionNewParams`, `RequestPermissionParams`, etc.
- `SafePaths` — path validation struct for sandboxing agent filesystem access
- `fs_ops` / `fs_tools` — sandboxed filesystem operations exposed to agents via `fs/*` tool calls
- `permissions` — permission request dispatch and auto-approval logic
- `message_handler` — background async task routing incoming JSON-RPC messages to the UI
- `session` — session-new parameter builders including MCP server descriptors and Claude wrapper metadata
- `harness` — testing harness for ACP smoke tests and transcript capture

## Key Types

| Type | Purpose |
|------|---------|
| `Agent` | Manages one AI agent process: spawn, send/receive, lifecycle |
| `AgentMessage` | Messages sent from the agent background task to the UI |
| `AgentStatus` | Current connection state of an agent |
| `AgentConfig` | Configuration for a single agent (executable path, capabilities, etc.) |
| `SafePaths` | Config and shaders directories the agent is allowed to access |
| `JsonRpcClient` | Low-level JSON-RPC 2.0 framing over stdin/stdout |

## Security Model

Agent filesystem access is sandboxed via `SafePaths` and `is_safe_write_path`. Write-class
tool calls are always validated before execution, even in `auto_approve` mode. Sensitive
paths (`~/.ssh/`, `~/.gnupg/`, `/etc/`) are blocked unconditionally. See
[docs/ASSISTANT_PANEL.md](../docs/ASSISTANT_PANEL.md) for the full permission model.

## Workspace Position

Layer 0 in the dependency graph (no internal workspace dependencies). Used directly by the
root `par-term` crate to drive the assistant panel.

## Related Documentation

- [Assistant Panel](../docs/ASSISTANT_PANEL.md) — user-facing ACP configuration and usage
- [ACP Harness](../docs/ACP_HARNESS.md) — smoke test harness for debugging agent behavior
- [Architecture Overview](../docs/ARCHITECTURE.md) — workspace structure
- [Crate Structure](../docs/CRATE_STRUCTURE.md) — dependency layers
