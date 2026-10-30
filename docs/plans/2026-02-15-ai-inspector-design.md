# AI Terminal Inspector Panel — Design Document

**Issue**: #149
**Date**: 2026-02-15
**Status**: Approved

## Overview

A DevTools-style right-side panel that displays structured terminal state (commands, zones, environment metadata) for visual inspection, export, and interactive ACP agent integration. Three use cases:

1. **Visual inspection** — Browse commands, zones, and output in configurable views
2. **Copy/export** — Structured JSON snapshots for AI assistants or documentation
3. **ACP agent chat** — Connect to coding agents (Claude Code, etc.) via Agent Communication Protocol for interactive terminal assistance

External tool API (Unix socket/HTTP) is deferred to a follow-up issue.

## Layout

The panel slides in from the right edge, shrinking the terminal to make room. Seven regions stacked vertically:

```
┌──────────────────────────────┬──────────────────────────┐
│                              │ AI Inspector       ─  ✕  │
│                              ├──────────────────────────┤
│                              │ Scope: [Visible ▾]       │
│                              │ View:  [Cards ▾]  ⏸  🔄  │
│                              ├──────────────────────────┤
│        Terminal              │ user@host ~/project      │
│        (reflows to fit)      │ zsh  │  3 commands       │
│                              ├──────────────────────────┤
│                              │ ┌─ $ cargo build ─────┐  │
│                              │ │ ✅ 0  ⏱ 12.3s       │  │
│                              │ │ ▶ Output (24 lines)  │  │
│                              │ └──────────────────────┘  │
│                              │ ┌─ $ cargo test ──────┐  │
│                              │ │ ❌ 1  ⏱ 8.1s        │  │
│                              │ │ ▼ Output (12 lines)  │  │
│                              │ │  test foo ... FAIL   │  │
│                              │ └──────────────────────┘  │
│                              ├──────────────────────────┤
│                              │ 🤖 Claude Code ● Connected│
│                              │   [Auto ▾]  [⚡ Yolo]    │
│                              │──────────────────────────│
│                              │ Agent: Build succeeded.  │
│                              │ Try running tests next:  │
│                              │  ▸ cargo test            │
│                              │                          │
│                              │ User: What failed?       │
│                              │                          │
│                              │ Agent: The test in       │
│                              │ src/foo.rs line 42...    │
│                              │  ▸ cargo test foo        │
│                              │──────────────────────────│
│                              │ [💬 Ask...         ] [📎] │
│                              ├──────────────────────────┤
│                              │ [📋 Copy JSON] [💾 Save]  │
└──────────────────────────────┴──────────────────────────┘
```

1. **Title bar** — "AI Inspector" label + minimize/close buttons
2. **Controls bar** — Scope dropdown + View mode dropdown + Live/Paused toggle + Refresh button
3. **Environment strip** — Compact: user@host, CWD, shell, command count
4. **Zone content** — Scrollable area with command entries in selected view mode
5. **Agent section** — Agent header (name + status + context mode + yolo toggle) + scrollable chat + input field
6. **Action bar** — Copy JSON + Save to File buttons

### Sizing

- Default width: 300px, min 200px, max 50% of window width
- Left edge draggable to resize
- Double-click left edge to reset to default (300px)
- Width persisted in config
- Terminal reflows columns on panel open/close/resize

## View Modes

Four configurable views for the zone content area, switchable via dropdown. Zone display can also be disabled entirely for a compact export-only mode.

### Cards (default)

Each command is a self-contained card. Header: command text + exit code badge (green/red) + duration. Subheader: CWD. Body: collapsible output (collapsed by default, click to expand). Most recent command at top.

### Timeline

Vertical scrollable log, flatter than cards — no card borders, separator lines between entries. More compact, fits more commands on screen. Collapsible output.

### Tree

Collapsible tree nodes. Top level = command text. Children = Prompt, Command, Output sub-nodes. Most structured view for understanding zone boundaries.

### List + Detail

Panel splits horizontally. Top half: compact command list (one line per command: text + exit code). Bottom half: full details of selected command (output, CWD, timing). Click to select.

### Zones Disabled

Zone content area hidden entirely. Panel shows only environment strip, agent chat, and action bar — compact agent-only or export-only mode.

## Interactions & Keybindings

### Panel toggle

- `Cmd+I` (macOS) / `Ctrl+Shift+I` (other) — toggle panel open/close
- Escape closes panel when it has focus
- Optional status bar button (when status bar enabled)

### Panel resize

- Drag left edge to resize
- Double-click left edge to reset to default width

### Within the panel

- Click card/entry to expand/collapse output
- `Cmd+Shift+C` / `Ctrl+Shift+C` while focused — copy JSON to clipboard
- Right-click command entry for context menu: Copy Command, Copy Output, Copy as JSON

### Live/Paused toggle

- Button in controls bar toggles live auto-update vs frozen snapshot
- Live mode: new commands appear at top, panel auto-scrolls to latest
- Paused mode: content frozen, "Paused" badge visible, click refresh or unpause to update

### Scope selector

- Dropdown: Visible, Recent 5 / 10 / 25 / 50, Full
- Changing scope immediately refreshes zone content

### Focus behavior

- Clicking panel gives it focus (keyboard events go to panel, not terminal)
- Clicking terminal returns focus to terminal
- Panel does NOT steal focus on live auto-update or agent messages

## Data Flow

### Snapshot generation

Panel calls `TerminalManager` methods to gather data:
- `core_command_history()` — commands + exit codes + timing
- `scrollback_marks()` — zone boundaries
- `shell_integration_*()` — environment metadata (hostname, username, CWD, shell)
- `cursor_position()` — cursor state

Data assembled into a `SnapshotData` struct that all view modes render from.

- **Live mode**: snapshot regenerated on terminal content change, debounced (~500ms or on new command detection)
- **Paused mode**: snapshot frozen until user refreshes

### Export: Copy JSON

Produces structured JSON:

```json
{
  "timestamp": "2026-02-15T10:30:00Z",
  "scope": "recent_10",
  "environment": {
    "hostname": "macbook",
    "username": "paul",
    "cwd": "~/Repos/par-term",
    "shell": "zsh"
  },
  "terminal": {
    "cols": 80,
    "rows": 24,
    "cursor": [12, 5]
  },
  "commands": [
    {
      "command": "cargo build",
      "exit_code": 0,
      "duration_ms": 12300,
      "cwd": "~/Repos/par-term",
      "output": "Compiling par-term v0.16.0\n..."
    }
  ]
}
```

Copies to system clipboard via arboard.

### Export: Save to file

- Native file dialog (rfd or similar)
- Default filename: `par-term-snapshot-YYYY-MM-DD-HHMMSS.json`
- Writes same JSON structure to disk

### Performance

- Full scope on large scrollback: gather asynchronously, show spinner
- Recent(N) and Visible scopes: near-instant
- Output text in cards truncated to ~50 lines with "Show all (N lines)" expand

## ACP Agent Integration

### Protocol

ACP (Agent Communication Protocol) uses JSON-RPC 2.0 over stdio. Par-term spawns the agent as a subprocess and communicates via stdin/stdout pipes. No external SDK required — par-term implements the ACP client directly (following toad's pattern).

### Agent Connection Lifecycle

1. User opens panel → agent dropdown shows last-used agent (remembered in config)
2. User selects agent (e.g., "Claude Code") → panel spawns agent subprocess (e.g., `claude-code-acp`)
3. ACP `initialize()` handshake → exchange protocol version and capabilities
4. `session/new()` creates a new conversation session
5. Agent status shows "● Connected" in header
6. Closing the panel kills the agent subprocess
7. Config remembers selected agent for next session

### Agent Discovery

Agent definitions stored as TOML files:
- **Bundled defaults**: `<app-resources>/agents/*.toml` — common agents (Claude Code, OpenHands, Gemini CLI, etc.)
- **User overrides**: `~/.config/par-term/agents/*.toml` — custom agent configs

Agent TOML schema (following toad's convention):
```toml
identity = "claude.com"
name = "Claude Code"
short_name = "claude"
protocol = "acp"
type = "coding"

[run_command]
"*" = "claude-code-acp"
macos = "claude-code-acp"

[actions."*".install]
command = "npm install -g @zed-industries/claude-code-acp"
description = "Install Claude Code ACP adapter"
```

### Context Feeding

Configurable auto/manual mode, toggled via dropdown in agent header:

**Auto mode (default):** On each command completion (detected via shell integration OSC 133 exit code), par-term automatically sends `session/prompt()` with content blocks:
- The command that ran
- Its exit code
- Its output (truncated to `ai_inspector_context_max_lines`)
- Current CWD

Agent processes this and may proactively respond with analysis or suggestions.

**Manual mode:** User explicitly types a question in the chat input or clicks the attach button (📎) to send current screen context. Agent only sees what user shares.

### Command Suggestion Flow

1. Agent response includes a command suggestion (detected by convention or markup in agent output)
2. Rendered as a styled `▸ command` block in the chat area
3. User clicks it → command text written to terminal input line via `TerminalManager::write()`
4. User reviews the text in their terminal, can edit it, then hits Enter to execute
5. If auto mode: shell integration detects command completion → result fed back to agent automatically

### Permission System

When agent sends `session/request_permission` (e.g., to read a file):

**Normal mode:** Panel shows an inline permission prompt in the chat area with the agent's requested options (Allow Once, Allow Always, Deny). User selects an option.

**Yolo mode (⚡):** All permission requests auto-approved. The chat area shows a subtle inline note (e.g., "Auto-approved: read src/foo.rs") for visibility. Warning badge `⚡ Yolo` visible in agent header when active.

### ACP Capabilities Supported

| ACP Method | Supported | Notes |
|---|---|---|
| `initialize` | Yes | Handshake with protocol version + capabilities |
| `session/new` | Yes | Create new conversation |
| `session/load` | Yes | Resume previous session (if agent supports) |
| `session/prompt` | Yes | Send user message + terminal context |
| `session/cancel` | Yes | Cancel current agent operation |
| `session/update` | Yes | Receive agent messages, tool calls, thoughts, plans |
| `session/request_permission` | Yes | Permission dialog (or auto-approve in yolo mode) |
| `session/set_mode` | Yes | Switch agent mode if supported |
| `fs/read_text_file` | Yes | Agent reads files (with permission prompt or yolo) |
| `fs/write_text_file` | No | Read-only — deferred to future scope |
| `terminal/create` | No | Par-term manages its own terminals |
| `terminal/kill` | No | Deferred |
| `terminal/output` | No | Deferred |

### Chat Area Rendering

Agent messages rendered with appropriate styling:
- **Agent text**: Normal message styling, supports markdown rendering
- **Command suggestions**: Styled `▸ command` blocks, clickable to write to terminal
- **Tool calls**: Collapsible entries showing what the agent did (e.g., "Read file: src/foo.rs")
- **Thinking/reasoning**: Collapsed by default, expandable, dimmed styling
- **Plans**: Rendered as a checklist if agent sends plan updates
- **Permission prompts**: Inline buttons (unless yolo mode auto-approves)
- **User messages**: Distinct styling, right-aligned or different background

## Configuration

New fields in `Config` / `config.yaml`:

```yaml
# AI Inspector - Panel
ai_inspector_enabled: true            # Feature gate (shows/hides keybinding + status bar button)
ai_inspector_width: 317               # Panel width in pixels
ai_inspector_default_scope: "visible" # visible | recent_10 | recent_25 | full
ai_inspector_view_mode: "tree"        # cards | timeline | tree | list_detail
ai_inspector_live_update: true        # Default to live mode vs paused
ai_inspector_show_zones: true         # Zone content visible vs export-only mode

# AI Inspector - Agent
ai_inspector_agent: "claude.com"      # Selected agent identity (default: Claude Code)
ai_inspector_auto_launch: true        # Auto-launch agent when panel opens
ai_inspector_auto_context: false      # Auto-send terminal context on command completion
ai_inspector_context_max_lines: 200   # Max output lines sent to agent per command
ai_inspector_auto_approve: false      # Yolo mode — auto-approve all agent permission requests
```

All settings exposed in Settings UI (dedicated AI Inspector tab).

Panel open/closed state is NOT persisted — always starts closed on launch. Width, view preferences, and agent selection are persisted.

### Agent Auto-Launch Behavior

When `ai_inspector_auto_launch` is `true` (default):
- Opening the panel automatically connects to the configured agent (`ai_inspector_agent`, defaults to Claude Code)
- No manual "Connect" step needed — panel opens and agent is ready
- If the agent process isn't installed, show an inline error with the install command from the agent's TOML config

When `ai_inspector_auto_launch` is `false`:
- Panel opens with agent disconnected
- User must manually select and connect via the agent dropdown

## Implementation Notes

### New modules

**`src/ai_inspector/`** — Inspector panel UI
- `mod.rs` — `AIInspectorUI` struct, public API (toggle, show, is_open)
- `views.rs` — Rendering logic for each view mode (cards, timeline, tree, list+detail)
- `snapshot.rs` — `SnapshotData` struct, gathering logic, JSON serialization
- `export.rs` — Clipboard copy, file save

**`src/acp/`** — ACP protocol implementation
- `mod.rs` — Public API, agent lifecycle management
- `jsonrpc.rs` — JSON-RPC 2.0 client/server over stdio
- `protocol.rs` — ACP message type definitions (TypedDict-style structs)
- `agent.rs` — Agent subprocess management, message routing, context feeding
- `agents.rs` — Agent discovery from TOML files
- `messages.rs` — Internal message types for UI communication

### Integration points

- `WindowState` — add `ai_inspector: AIInspectorUI` field, call `show()` in event loop
- Terminal reflow — adjust column count when panel opens/closes/resizes
- Config — add fields with serde defaults
- Settings UI — add AI Inspector tab
- Input events — add keybinding handler for panel toggle
- Shell integration — hook into command completion for auto-context feeding
- `TerminalManager::write()` — used by agent to write suggested commands to input line

### Existing patterns to follow

- `SearchUI` for toggle/show/close lifecycle
- `SettingsUI` for complex egui panel with sections
- `StatusBar` for egui Area positioning
- `search_highlight.rs` for terminal data access patterns
- Toad's `acp/agent.py` for ACP protocol implementation reference <!-- doc-path: example -->

## Deferred (Future Issues)

- External tool API (Unix socket / HTTP) for MCP servers and scripts
- Streaming snapshot protocol
- `fs/write_text_file` support (agent file writing)
- `terminal/*` ACP methods (agent-managed terminals)
- Session persistence and resume across par-term restarts
- Multiple simultaneous agent connections
