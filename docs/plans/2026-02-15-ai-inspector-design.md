# AI Terminal Inspector Panel — Design Document

**Issue**: #149
**Date**: 2026-02-15
**Status**: Approved

## Overview

A DevTools-style right-side panel that displays structured terminal state (commands, zones, environment metadata) for visual inspection and export. Designed for three use cases: direct visual inspection, copy/export to AI assistants, and future external tool API (deferred).

## Layout

The panel slides in from the right edge, shrinking the terminal to make room. Five regions stacked vertically:

```
┌─────────────────────────────────────┬──────────────────────────┐
│                                     │ AI Inspector       ─  ✕  │
│                                     ├──────────────────────────┤
│                                     │ Scope: [Visible ▾]      │
│                                     │ View:  [Cards ▾]  ⏸  🔄 │
│          Terminal                    ├──────────────────────────┤
│          (reflows to fit)           │ user@host ~/project      │
│                                     │ zsh  │  3 commands       │
│                                     ├──────────────────────────┤
│                                     │ ┌────────────────────┐   │
│                                     │ │ $ cargo build      │   │
│                                     │ │ ✅ 0  ⏱ 12.3s     │   │
│                                     │ │ 📁 ~/project       │   │
│                                     │ │ ▶ Output (24 lines)│   │
│                                     │ └────────────────────┘   │
│                                     │ ┌────────────────────┐   │
│                                     │ │ $ cargo test       │   │
│                                     │ │ ❌ 1  ⏱ 8.1s      │   │
│                                     │ │ 📁 ~/project       │   │
│                                     │ │ ▼ Output (12 lines)│   │
│                                     │ │  test foo ... FAIL │   │
│                                     │ │  test bar ... ok   │   │
│                                     │ └────────────────────┘   │
│                                     ├──────────────────────────┤
│                                     │ [📋 Copy JSON] [💾 Save] │
└─────────────────────────────────────┴──────────────────────────┘
```

1. **Title bar** — "AI Inspector" label + minimize/close buttons
2. **Controls bar** — Scope dropdown + View mode dropdown + Live/Paused toggle + Refresh button
3. **Environment strip** — Compact: user@host, CWD, shell, command count
4. **Zone content** — Scrollable area with command entries in selected view mode
5. **Action bar** — Copy JSON + Save to File buttons

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

Zone content area hidden entirely. Panel shows only environment strip and action bar — compact export-only mode.

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
- Panel does NOT steal focus on live auto-update

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

## Configuration

New fields in `Config` / `config.yaml`:

```yaml
ai_inspector_enabled: true           # Feature gate
ai_inspector_width: 300              # Panel width in pixels
ai_inspector_default_scope: "visible" # visible | recent_10 | recent_25 | full
ai_inspector_view_mode: "cards"      # cards | timeline | tree | list_detail
ai_inspector_live_update: true       # Default live vs paused
ai_inspector_show_zones: true        # Zone content visible vs export-only
```

All settings exposed in Settings UI (Integrations tab or dedicated AI Inspector tab).

Panel open/closed state is NOT persisted — always starts closed on launch. Width and view preferences are persisted.

## Implementation Notes

### New module: `src/ai_inspector/`

- `mod.rs` — `AIInspectorUI` struct, public API (toggle, show, is_open)
- `views.rs` — Rendering logic for each view mode
- `snapshot.rs` — `SnapshotData` struct, gathering logic, JSON serialization
- `export.rs` — Clipboard copy, file save

### Integration points

- `WindowState` — add `ai_inspector: AIInspectorUI` field, call `show()` in event loop
- Terminal reflow — adjust column count when panel opens/closes/resizes
- Config — add fields with serde defaults
- Settings UI — add controls to appropriate tab
- Input events — add keybinding handler for toggle

### Existing patterns to follow

- `SearchUI` for toggle/show/close lifecycle
- `SettingsUI` for complex egui panel with tabs
- `StatusBar` for egui Area positioning
- `search_highlight.rs` for terminal data access patterns

## Deferred (Future Issues)

- External tool API (Unix socket / HTTP) for MCP servers and scripts
- Streaming snapshot protocol
- AI assistant direct integration (send context to Claude/etc.)
