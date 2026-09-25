# par-mux Integration

par-term can attach to [par-mux](https://github.com/paulrobello/par-mux) sessions: a par-term window becomes a client of a session owned by the par-mux daemon, with one tab per session window, so your windows, panes, and running programs keep running when you close the tab, the window, or par-term itself. Reattach later — even after a crash — and every pane is reseeded with its live screen before new output arrives.

On top of sessions, par-mux carries an **agent roster**: what coding agents (Claude Code, Codex, Grok, pi, omp, …) are running inside the session's panes and whether each is working, blocked, or idle. par-term surfaces that roster in a status-bar widget and a command-palette picker.

## Table of Contents

- [What par-mux gives you](#what-par-mux-gives-you)
  - [How par-mux maps onto par-term](#how-par-mux-maps-onto-par-term)
- [Requirements](#requirements)
- [Setup](#setup)
- [Attaching](#attaching)
- [Working in a mux pane](#working-in-a-mux-pane)
- [Pane environment](#pane-environment)
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

### How par-mux maps onto par-term

par-mux has three levels, session → window → pane (tmux's model, with ids `$N`, `@N`, `%N`). par-term maps them the same way as its tmux integration:

| par-mux | par-term |
|---|---|
| daemon (one per socket name) | nothing — it runs outside par-term and outlives it |
| session `$N` | one par-term **window** (the connection lives on the window) |
| window `@N` | a **tab** in that window |
| pane `%N` | a split pane in that tab |

Opening a profile with `mux_session_name` in a window starts the attach; every window in the session then gets its own tab in that par-term window, and new session windows arrive as new tabs.

A par-term window holds one par-mux session at a time. Opening a second mux profile in a window that is already attached (or still attaching) does nothing; open it in a new par-term window instead. There is no workspace or tab level above the session — to keep separate groups of sessions apart, run separate daemons (`par-mux <name>` gives each its own socket).

## Requirements

- **The `mux` feature** — on by default in every par-term build since the core library's 0.50 release.
- The **par-mux daemon binary** (`par-mux`), looked up next to the par-term executable first, then on `PATH`. Release packages bundle it (inside the macOS `.app`, and in the `par-term-bundle-*` archives on Linux and Windows); for a from-source run, `scripts/build-par-mux.sh target/dev-release/par-mux` stages it (see `CLAUDE.md`, "par-mux daemon for local runs"). The attach surfaces a visible error when it is missing or stale.
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

## Pane environment

A mux pane gets the same shell environment a local tab does. On attach, par-term hands the daemon the environment it builds for local tabs, and the daemon applies it on top of its own environment for every pane spawned in that session:

- `TERM_PROGRAM=iTerm.app`, `TERM_PROGRAM_VERSION`, `LC_TERMINAL=iTerm2`, `LC_TERMINAL_VERSION`, and `__PAR_TERM=1` — these replace the daemon's `TERM_PROGRAM=kitty` default, since par-term is the renderer.
- A UTF-8 locale (`LANG` falls back to `en_US.UTF-8` when none is inherited).
- par-term's augmented `PATH` (the extra tool directories added for Finder/Dock launches).
- Everything in your `shell_env` config.
- `ITERM_SESSION_ID`, unique per pane: par-term restamps it before every split it requests.

When it is sent and refreshed:

- **Creating a session** sends the environment with `new-session -e`, so the session's first pane already has it.
- **Every attach and reattach** resends it with `set-environment`. Panes created afterwards see the current values, so a par-term update or a `shell_env` change reaches new panes after a reattach. Panes already running keep the environment they started with (tmux semantics) — restart the shell in a pane to pick up a change.
- A variable you **remove** from `shell_env` stays in the session environment: reattach only sets values, it never unsets them. Kill and recreate the session to drop one.
- `ITERM_SESSION_ID` is only fresh for panes par-term asks for. A pane spawned by something else — an agent running `par-mux` itself, or the daemon restoring panes after a restart — reuses the most recent value.

A daemon older than the session-environment protocol ignores all of this, and its panes get the daemon's own environment. Creating a session on such a daemon is silent (it drops `-e` without complaint). Reattaching logs a warning naming the refused variables (names only, never values). Restart the daemon to fix it (see [the stale-daemon check](#daemon-lifecycle-and-the-stale-daemon-check)). A `shell_env` value containing a newline cannot travel over the line-based protocol, so it is skipped with its own warning.

Values can carry secrets (tokens in `shell_env`). They travel over the owner-only daemon socket, persist in the daemon's owner-only (`0600`) state file, and are never logged.

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
| Attach fails with a visible error | Check the daemon binary next to the par-term executable and on `PATH`; for a from-source build see `CLAUDE.md`, "par-mux daemon for local runs". |
| Roster is empty | Install the hook/extension installers (see [Agent roster](#agent-roster)); roster entries only exist for agents reporting through par-mux. |
| Pane looks frozen | The daemon owns the pane; unfocused panes redraw as data arrives. If a pane stops updating, reattach to force a reseed. |
