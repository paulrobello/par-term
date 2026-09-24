# par-mux Integration

par-term can attach to [par-mux](https://github.com/paulrobello/par-mux) sessions: a par-term tab becomes a client of a session owned by the par-mux daemon, so your windows, panes, and running programs keep running when you close the tab, the window, or par-term itself. Reattach later — even after a crash — and every pane is reseeded with its live screen before new output arrives.

On top of sessions, par-mux carries an **agent roster**: what coding agents (Claude Code, Codex, Grok, pi, omp, …) are running inside the session's panes and whether each is working, blocked, or idle. par-term surfaces that roster in a status-bar widget and a command-palette picker.

## Table of Contents

- [What par-mux gives you](#what-par-mux-gives-you)
- [Requirements](#requirements)
- [Setup](#setup)
- [Attaching](#attaching)
- [Working in a mux pane](#working-in-a-mux-pane)
- [Detach and reattach](#detach-and-reattach)
- [Daemon lifecycle and the stale-daemon check](#daemon-lifecycle-and-the-stale-daemon-check)
- [Agent roster](#agent-roster)
- [Troubleshooting](#troubleshooting)

## What par-mux gives you

| | par-mux session | ordinary par-term tab |
|---|---|---|
| Panes survive closing the tab/window | Yes — the daemon owns the session | No |
| Panes survive quitting par-term | Yes | No |
| Splits, focus, and resize negotiated with other clients | Yes | n/a |
| Agent roster (who is running, blocked vs working) | Yes | No |

A par-mux session is the direct analogue of a tmux session: the daemon is the counterpart of the tmux server, and the integration reuses par-term's tmux control-mode sync layer. If you know tmux sessions, you know the model.

## Requirements

- A par-term build with the **`mux` feature** enabled. The flag is currently **default-off** and only buildable while the core library's unpublished `mux` feature is vendored locally — build with `make with-local-core` (see `CLAUDE.md`, "Vendoring the core from the local checkout"). Once the core publishes 0.50, the feature ships by default.
- The **par-mux daemon binary** (`par-mux`) next to the par-term executable. `make with-local-core` stages the newest daemon from the local core checkout next to every built par-term binary; the attach surfaces a visible error when it is missing.
- Agents reporting into the roster is optional and needs the installers under [Agent roster](#agent-roster).

## Setup

par-mux attach is a **profile** property. In Settings → Profiles → the profile editor, the **par-mux Auto-Attach** section (separate from the tmux section) has one field, **Session Name** (empty = disabled):

| Setting | Config key | Meaning |
|---|---|---|
| Session Name | `mux_session_name` | Session to create-or-attach when this profile opens |

`mux_session_name` is independent of the profile's `tmux_session_name`, so one profile can carry both a tmux and a par-mux session without them interfering.

In `config.yaml`:

```yaml
profiles:
  - name: Dev
    mux_session_name: dev
```

Window arrangements also persist `mux_session_name`, so restoring an arrangement reattaches the session rather than opening a plain tab.

## Attaching

Opening a profile with `mux_session_name` set attaches:

1. **Stale-daemon check** — attach first queries the daemon's build; a daemon that predates the client is reported up front, because a version mismatch is the cheapest explanation for everything downstream misbehaving.
2. **Create-or-attach** — a missing session is created; an existing session is joined.
3. **Tabs for existing windows** — the daemon's existing windows each get a par-term tab.
4. **Seed** — each pane is seeded with the pane's current screen (a clear plus a replay), so you see the live state rather than a blank pane or stale bytes.
5. **Roster fill** — the initial agent roster is read with `list-agents`.

## Working in a mux pane

While attached, the pane is daemon-driven:

- Input goes to the daemon's **focused** pane; splits, divider drags, and pane closes are routed **daemon-side** so the layout stays negotiated with the session, not just local.
- Paste is routed daemon-side, mouse reports reach mouse-aware TUIs (htop, vim with mouse support), and resizes push to the daemon.
- The scrollbar draws in a reserved strip, so pane content never renders under it.

## Detach and reattach

Detach closes the par-term side and leaves the session running in the daemon:

- **Command palette** — the palette carries an explicit **Detach par-mux session** row when a session is attached.
- **Action** — `detach_mux_session` is a bindable action (`action: detach_mux_session` in your keybindings config).
- Windows arrangements that captured `mux_session_name` reattach on restore.

Reattach by opening the profile (or restoring an arrangement) again: the stale-daemon check runs, existing windows become tabs, and every pane is reseeded. Sessions also survive a par-term crash for the same reason — the daemon kept them.

## Daemon lifecycle and the stale-daemon check

The daemon (`par-mux`) outlives clients and can serve multiple clients at once. The build tooling stages it next to the app binary, so it does not have to be on `PATH`.

par-term queries the daemon's build when attaching. If the daemon predates the client (an old daemon left running from before an upgrade), attach **surfaces the stale daemon** instead of attaching into mysterious breakage — quit the old daemon and retry.

## Agent roster

While attached, par-term shows what agents are running in the session:

- **Agent Roster status-bar widget** (`status_bar: agent_roster`, disabled by default): a summary like `👥 2 blocked, 1~ working`, self-hides without an attached session, hover lists each agent with its provenance (reported by the agent itself vs detected), click opens the command palette.
- **Roster picker in the command palette**: runtime rows, blocked-first, each jumping to that agent's pane.
- **Feed the roster honestly**: install the par-mux session hooks for Claude Code, Codex, or Grok (`par-term install-mux-hooks`), or agent-state extensions for pi/omp (`par-term install-mux-extensions`) — see [INTEGRATIONS.md](INTEGRATIONS.md#par-mux-agent-extensions) for details.

## Troubleshooting

| Symptom | Fix |
|---|---|
| Attach reports a stale daemon | An old daemon from before an upgrade is still running. Quit it and reattach. |
| Attach fails with a visible error | Check the daemon binary next to the par-term executable; `make with-local-core` stages it. |
| Roster is empty | Install the hook/extension installers (see [Agent roster](#agent-roster)); roster entries only exist for agents reporting through par-mux. |
| Pane looks frozen | The daemon owns the pane; unfocused panes redraw as data arrives. If a pane stops updating, reattach to force a reseed. |
