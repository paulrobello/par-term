# par-mux Integration

> A **window** holds **tabs**; a tab holds **panes**. A window can be **attached** to one par-mux **session**, in which case its tabs live in the daemon and survive quitting par-term. **Detach** leaves them running; **End session** ends them.

par-term can attach to [par-mux](https://github.com/paulrobello/par-mux) sessions: a par-term window becomes a client of a session owned by the par-mux daemon, with one tab per session window, so your windows, panes, and running programs keep running when you close the window, detach, or quit par-term itself. Reattach later — even after a crash — and every pane is reseeded with its live screen before new output arrives. Closing a **tab** closes its session window (the tmux semantic — the session itself survives while any window or the daemon lives); closing the **last attached tab** instead asks first — **Detach** (the default) leaves the session running in the daemon for reattach, **End session** kills every window and pane in it, and Cancel keeps the tab. Closing a tab's last **pane** instead closes just the tab and leaves the session window running for reattach.

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

par-term's own labels always say **tab** for a daemon window, and use **session** only for a par-mux or tmux session. The same bridging table, seen from par-term's side:

| par-term says | par-mux / tmux says |
|---|---|
| Window (attached) | Client |
| Tab | Window |
| Pane | Pane |
| Session | Session |

The full vocabulary, including which old labels were renamed, is in the [documentation style guide](../DOCUMENTATION_STYLE_GUIDE.md#par-term-vocabulary-uxmd-t1-t8).

The **session chip** at the start of the tab bar (top of a vertical bar) shows the attached session's name, `N hidden` for tabs whose last pane you closed (they keep running daemon-side), and the daemon's health when it is not plain connected (attaching, not responding, out of date). A par-mux error — a failed split, a lost daemon connection, a refused attach — also stays on the chip, in red, until you dismiss it with its ×, so a two-second toast cannot hide it. Click the chip to open the session picker. Each attached tab carries its own 🔗 badge (hover names the session); a local tab in the same window has none.

Opening a profile with `mux_session_name` in a window starts the attach; every window in the session then gets its own tab in that par-term window, and new session windows arrive as new tabs. While attached, tab operations map onto daemon windows: **Cmd+T / New Tab** asks the daemon for a `new-window` (the tab arrives as the daemon's `%window-add`), a tab's title carries the daemon window's name (renaming a tab sends `rename-window`, so names survive detach; clearing the name sends the tab's automatic title, since the daemon keeps no empty name), reordering a tab moves its daemon window (`move-window`, so the order survives reattach), **Clear Scrollback** clears the pane you see (the daemon has no command to drop its own history, so a reattach brings that history back), and **Move Tab to Another Window** is blocked for mux tabs — the tab is a mirror with no transport of its own, so moving it would strand the mirror (detach first). An attached tab shows a link glyph between its icon and title (hover names the session), decided per tab, so mux tabs are distinguishable from local ones at a glance; it appears and disappears on the frame after attach or detach.

A par-term window holds one par-mux session at a time. Opening a second mux profile in a window that is already attached (or still attaching) explains itself with a toast naming the attached (or attaching) session; open it in a new par-term window instead. There is no workspace or tab level above the session — to keep separate groups of sessions apart, run separate daemons (`par-mux <name>` gives each its own socket).

A par-term launched **from inside a par-mux pane** refuses to attach to the session that owns that pane (it would render the session inside itself — a display feedback loop); the refusal surfaces as a toast. Attaching to a *different* daemon from inside a pane is allowed when that daemon is already running; starting one there follows the core's nesting rule (`PAR_MUX_ALLOW_NESTED=1` overrides, same as for the daemon itself).

## Requirements

- **The `mux` feature** — on by default in every par-term build since the core library's 0.50 release.
- The **par-mux daemon binary** (`par-mux`), looked up next to the par-term executable first, then on `PATH`. Release packages bundle it (inside the macOS `.app`, and in the `par-term-bundle-*` archives on Linux and Windows); for a from-source run, `scripts/build-par-mux.sh target/dev-release/par-mux` stages it (see `CLAUDE.md`, "par-mux daemon for local runs"). The attach surfaces a visible error when it is missing or stale.
- Agents reporting into the roster is optional and needs the installers under [Agent roster](#agent-roster).

## Setup

No setup is needed to attach: the **session picker** (`toggle_session_picker`, `Cmd + Ctrl + S` on macOS, `Ctrl + Alt + S` elsewhere) lists every running par-mux session and attaches, switches, creates, renames, ends, or detaches without touching a profile. The command palette lists the same sessions as **Attach par-mux Session: <name>** rows, plus **New par-mux Session** and **Detach from par-mux Session**. From the command line, `par-term --attach <session>` attaches a window at launch (creating the session when it does not exist), and the `mux_auto_attach: <session>` config key does the same on every launch (Settings → Sessions → par-mux). With windows restored on launch, it attaches the focused window unless that window is already reattaching its own session, in which case it takes the first window that is not; when every restored window is reattaching, nothing more is attached.

Sessions are grouped by daemon: par-term starts one daemon per session it creates, named after it. A session held by a differently named daemon (created by `par-mux` itself, say) still attaches through that daemon; the picker marks it with its daemon's name because such a session is not reattached on the next launch.

par-mux attach can also be a **profile** property. In Settings → Profiles → Profiles → the profile editor's **Session** sub-tab, the **par-mux Auto-Attach** section (separate from the tmux section) has one field, **Session Name** (empty = disabled):

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
4. **Seed** — each pane is seeded with the pane's current screen (a clear plus a replay), so you see the live state rather than a blank pane or stale bytes. The replay carries the pane's scrollback too, so the scrollbar and mouse wheel reach history written while you were detached, up to your `scrollback_lines` limit (the newest lines are kept when the daemon holds more).
5. **Roster fill** — the initial agent roster is read with `list-agents`.

## Working in a mux pane

While attached, the pane is daemon-driven:

- Input goes to the daemon's **focused** pane; splits, divider drags, and pane closes are routed **daemon-side** so the layout stays negotiated with the session, not just local.
- With `confirm_close_running_jobs` on, closing a pane asks the daemon what is running in it, and closing a tab asks about every pane in the tab (on a connection of its own, so a slow command already in flight does not delay the answer); either close asks first when it finds a job. If the daemon does not answer within a second, the close asks anyway, saying the check could not be made, instead of closing unchecked.
- Paste is routed daemon-side, mouse reports reach mouse-aware TUIs (htop, vim with mouse support), and resizes push to the daemon.
- The scrollbar draws in a reserved strip, so pane content never renders under it.
- **Exited panes are held, not closed.** When a pane's process exits, the daemon keeps the pane with its frozen screen, and par-term shows a **Process exited (code N)** banner over it (no code for a signal death). Press **Enter** in the pane, click **Restart** on the banner, or pick **Restart Pane Process (par-mux)** in the command palette to run it again in place (`respawn-pane`; on a pane whose process is still running, the palette row restarts it with `-k`). Other keys are ignored while the pane is held. Close a held pane like any other, with one difference: closing the last pane of a tab normally hides the tab and keeps its daemon window running, but when every pane in that tab is held the close ends the daemon window instead (`kill-window`), so no window of dead panes is left behind. If that window is the session's only one, the close opens the last-tab dialog (**Detach** / **End session** / **Cancel**) rather than ending the session. A session whose panes have all exited keeps its windows, so it is saved and restored like a live one, and its panes come back as fresh shells. A daemon older than the held-pane surface still closes the pane on exit.
- **Promote Pane to Tab** moves the focused pane into its own daemon window (`break-pane`), and **Demote Tab to Pane** joins a single-pane mux tab's pane into another mux tab (`join-pane`), so both keep the pane's process and daemon link. Demote is refused, and greyed out in the tab menu with the reason on hover, for a mux tab with several panes, and between a mux tab and a local tab.
- Moves made elsewhere (the `par-mux` CLI, another client) are mirrored the same way: a window that appears gets its tab and panes, and a pane that moves between windows follows into the right tab with its screen.

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

- **Command palette** — **Detach from par-mux Session** in the palette.
- **Action** — `detach` is a bindable action (`action: detach` in your keybindings config); the older id `mux-detach` still works.
- Windows arrangements that captured `mux_session_name` reattach on restore.

Reattach by opening the profile (or restoring an arrangement) again: the stale-daemon check runs, existing windows become tabs, and every pane is reseeded. Sessions also survive a par-term crash for the same reason — the daemon kept them.

## Daemon lifecycle and the stale-daemon check

The daemon (`par-mux`) outlives clients and can serve multiple clients at once. The build tooling stages it next to the app binary, so it does not have to be on `PATH`.

par-term queries the daemon's build when attaching. If the daemon predates the client (an old daemon left running from before an upgrade), attach **surfaces the stale daemon** instead of attaching into mysterious breakage — quit the old daemon and retry.

## Agent roster

While attached, par-term shows what agents are running in the session:

- **Agent Roster status-bar widget** (`status_bar: agent_roster`, enabled by default; the status bar itself is off until `status_bar_enabled: true`): a summary like `👥 2 blocked, 1 done, 1~ working`, self-hides without an attached session, hover lists each agent with its provenance (reported by the agent itself vs detected), click opens the command palette.
- **Roster picker in the command palette**: runtime rows, blocked and done-unseen first, each jumping to that agent's pane.
- **Badges on tabs and pane title bars**: a tab shows its most urgent agent (🟠 blocked, 🟢 done and not yet seen, ⋯ working), and a pane's title bar leads with its own agent's badge.
- **Next agent needing attention** (`focus_next_attention_agent`, `Cmd + Opt + A` / `Ctrl + Alt + A`): jumps to the next blocked agent, then to agents that finished while you were elsewhere, across tabs and in tab order. Focusing a finished agent marks it seen, so the next press moves on.
- **Done, unseen**: when an agent goes from `working` to `idle` in a pane you are not looking at, the roster shows it as `done` (widget), `done (unseen)` (hover), and `done ✓` (palette) until you focus that pane. Focusing the pane clears the mark; an agent that finishes in the pane you are watching is never marked.

States are shown as the agent reports them (`working`, `blocked`, `idle`); `done` is the only state par-term derives. The roster is passive: a state change updates the widget and palette, but par-term sends no desktop notification, toast, or bell for an agent that blocks or finishes. An agent that wants a desktop notification must emit one itself (OSC 9/777/99, see [NOTIFICATIONS.md](NOTIFICATIONS.md)).
- **Feed the roster honestly**: install the par-mux session hooks for Claude Code, Codex, or Grok (`par-term install-mux-hooks`), or agent-state extensions for pi/omp (`par-term install-mux-extensions`) — see [INTEGRATIONS.md](INTEGRATIONS.md#par-mux-agent-extensions) for details.

Both surfaces are scoped to the attached session. An entry disappears as soon as its pane closes, or when the agent itself exits — the hooks and extensions send `pane.release_agent` on claude `SessionEnd` and pi/omp shutdown (the codex and grok hooks have no release, so those entries stay until the pane closes), so a quit agent leaves the roster at once instead of showing "working" until the pane dies. The widget and picker never count a closed pane or offer a row they cannot focus.

## Socket location contract

The daemon's socket lives at a stable, per-user path that does not depend on the par-mux or par-term version, so a par-term upgrade never re-homes a live daemon:

| Platform | Default path | Notes |
|---|---|---|
| Linux | `$XDG_RUNTIME_DIR/par-mux-<name>.sock`, else `$TMPDIR/par-mux-<uid>/par-mux-<name>.sock` | The per-UID directory (tmux's `/tmp/tmux-<uid>` defense) keeps the socket out of the shared temp dir. |
| macOS | `$TMPDIR/par-mux-<uid>/par-mux-<name>.sock` | `$XDG_RUNTIME_DIR` is usually unset on macOS, so the per-UID temp dir is the norm. |
| Windows | `%TEMP%\par-mux-<name>.sock` | The file is the marker the named pipe name is derived from; the transport itself is a named pipe. |

An explicit `--socket <path>` always wins. An upgrade never strands a live daemon: `connect_or_spawn` probes the pre-0.52 legacy path (`$TMPDIR/par-mux-<name>.sock`, Unix only) before spawning a replacement, and only follows a socket file it owns — a planted regular file or symlink is never probed. When the daemon and client build stamps disagree, par-term attaches anyway and toasts `par-mux --restart` (all platforms, bundles inside the .app / `par-term-bundle-*` archives), which stops the daemon, saves its state, starts a fresh one, and restores the tree.

## Troubleshooting

| Symptom | Fix |
|---|---|
| Attach reports a stale daemon | Run `par-mux --restart` (works on all platforms; `pkill -f par-mux` is macOS/Linux only). Your session tree is restored. |
| Attach fails with a visible error | Check the daemon binary next to the par-term executable and on `PATH`; for a from-source build see `CLAUDE.md`, "par-mux daemon for local runs". |
| Roster is empty | Install the hook/extension installers (see [Agent roster](#agent-roster)); roster entries only exist for agents reporting through par-mux. |
| Pane looks frozen | The daemon owns the pane; unfocused panes redraw as data arrives. If a pane stops updating, reattach to force a reseed. |
