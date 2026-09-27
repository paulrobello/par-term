---
name: par-mux
description: Use when managing par-mux (the terminal multiplexer bundled with par-term) from a script or agent — starting/stopping/restarting the par-mux daemon, creating sessions/windows/panes, spawning panes for dev servers or long-running commands, sending keys or text to a pane, capturing pane output, reading the agent roster (list-agents), reporting agent state via hook reports, resizing/renaming/titling panes, or debugging "no daemon running" and stale-daemon errors. Covers the `par-mux` CLI (client mode) and the pane env contract.
---

# par-mux

## What it is

par-mux is the tmux-control-mode multiplexer bundled with par-term: a daemon
(`par-mux`) owns PTY-backed panes in a `session → window → pane` tree and
serves a local Unix socket. The **`par-mux --cmd '<command>'`** client mode is
how an agent drives it — one command per invocation, reply printed, exit code 0
on success and 1 on a daemon-reported error (`%error` prints to stderr). It
**never starts a daemon**: a `--cmd` with nothing running fails immediately
with `no daemon running on <path>`.

Ids are typed and stable: sessions `$N`, windows `@N`, panes `%N`. Prefer ids
over names — a name that matches two targets is an error listing the candidates,
never a silent pick. Ids are monotonic and never reused, even across daemon
restarts (a restored tree keeps its old ids) — always re-query, never assume
`%0` or `%1`.

Every client call targets a socket: `--socket PATH` overrides it, and the
default is `$XDG_RUNTIME_DIR/par-mux-<name>.sock` (a per-user fallback dir
below the temp dir on Linux/macOS). The `no daemon running on <path>` error
names the exact path it tried — that is your first debug signal.

## Daemon lifecycle

```sh
par-mux default                 # start (named; "default" may be omitted)
par-mux --restart               # stop + start detached on the same socket — the
                                #   routine fix after rebuilding par-mux (an old
                                #   daemon keeps serving old code until restarted)
par-mux --stop                  # stop cleanly and wait
par-mux <name> --cmd version    # build stamp — compare against the client build
                                #   to detect a stale daemon
```

`--restart` works when nothing is running (prints a notice, still exits 0).
The daemon detaches itself; no `&` needed. It ignores terminal hangup, exits
when it has held zero sessions and zero clients for 5 s, and persists the tree
(state, scrollback, paste buffer) across restarts — panes respawn with fresh
processes in their saved cwd. Panes it spawns are marked with `PAR_MUX_ENV=1`
and starting a daemon from inside a pane is refused (nested-daemon guard) unless
`PAR_MUX_ALLOW_NESTED=1`.

## Orient → act → confirm

```sh
par-mux --cmd list-sessions     # "$N: name" lines
par-mux --cmd list-windows      # "@N: name" lines
par-mux --cmd list-panes        # "%N" lines, global
par-mux --cmd list-agents       # agent roster: "%N <agent> <state> <source> [message]"
```

Then act on ids captured from those listings, and confirm the effect by
capturing the pane (below).

## Command map

| Goal | Command |
| --- | --- |
| New session | `new-session [-s name] [-e KEY=VALUE]…` → replies `$N` |
| New window | `new-window -t $N [-n name] [-c dir]` → replies `@N` |
| Split a pane | `split-window -t %N [-h\|-v] [-p 1-99] [-c dir]` → replies `%N` |
| Send keys | `send-keys -t %N <keys…>` — key names (`Enter`, `C-c`, `BSpace`, arrows), `-l` literal, `-H` hex bytes |
| Run a command | `send-keys -t %N "make test" Enter` |
| Capture screen | `capture-pane -t %N` — visible screen; `-S -200` adds 200 history lines |
| Capture styled | `capture-pane -t %N -e` — one line per grid row with SGR escapes inline |
| Read a title | `pane-title -t %N` (empty reply = none set) |
| Set a sticky title | `select-pane -t %N -T 'dev server'`; `-T ''` clears |
| Pane geometry | `pane-info -t %N` → `%N @W COLSxROWS`; `resize-pane -t %N -x 80 -y 24` (or `-L\|-R\|-U\|-D [cells]`) |
| Paste buffer | `set-buffer <text>` / `show-buffer` / `paste-buffer -t %N` |
| Session env | `set-environment -t $N KEY VALUE` (applies to panes spawned *after*) |
| Kill pane/window | `kill-pane -t %N` / `kill-window -t @N` (a window's last pane cannot be killed) |
| Stop everything | `par-mux --stop` (or `kill-server`) — clean save, then exit |

`-h` places the new pane beside the target, `-v` (default) below; `-p` is the
percent given to the **new** pane. `new-window` without `-t` targets the
most-recently-created session. Quoted values (names, titles, `-c` dirs, env
values) may contain spaces; everything else is whitespace-split.

## The two gotchas

**1. `split-window -t` takes a pane id, not a window id.** You split *from* an
existing pane. `split-window -t @1` fails with `no such pane: @1`; use the pane
in that window (`%1`), then capture the new pane's id from the reply.

**2. The wire is line-based — never send a bare newline inside a payload.**
`send-keys` appends nothing: `Enter` is an expressible key you send explicitly.
To type multi-line text, send one line per call, or send raw bytes with `-H`,
which takes **one hex byte pair per token** (`send-keys -t %N -H 63 61 74 0a`
types `cat\n`; a longer hex string like `636174` is a parse error). Bare
`0xNN` tokens (e.g. `0x0a`) send single raw bytes in an otherwise-literal
payload. `set-buffer <content>` takes the rest of the line verbatim, so it
carries one line at a time; for multi-line content, send line-wise or set the
buffer once per line before each `paste-buffer`.

## Reading pane output

`capture-pane` returns what is on screen right now — for a long-running command
(dev server, test run, agent) capture repeatedly and diff, or send the command
with `Enter` and poll. To wait for an agent's verdict instead of polling its
screen, read the roster:

```sh
par-mux --cmd list-agents       # %N claude blocked hook <reason>
```

Agent panes spawn with an env contract a script inside the pane can use:

```sh
echo "pane=$PAR_MUX_PANE_ID socket=$PAR_MUX_SOCKET bin=$PAR_MUX_BIN"
"$PAR_MUX_BIN" --socket "$PAR_MUX_SOCKET" --cmd list-panes   # client mode without PATH
```

## Agent hook reports

A pane's process reports agent state by sending **one JSON line** to the socket
and reading **one JSON reply** (`{"id":…,"result":"ok"}`). No `--cmd` support —
this is a raw socket write:

```sh
printf '%s\n' "{\"id\":1,\"method\":\"pane.report_agent\",\"params\":{
  \"pane_id\":\"$PAR_MUX_PANE_ID\",\"agent\":\"claude\",\"seq\":$(date +%s),
  \"state\":\"blocked\",\"message\":\"waiting for review\"}}" | nc -U "$PAR_MUX_SOCKET"
```

- `state` is `working` / `blocked` / `idle` (`unknown` accepted, never stored);
  `seq` must increase per pane **per source** — stale reports are dropped
  silently. Use an epoch clock (`date +%s`); a shell-start counter like
  `$SECONDS` restarts near 0 for every invocation and gets dropped.
- `pane.report_agent_session` (id/transcript path + optional resume argv) and
  `pane.report_agent_telemetry` (versioned, display-only) exist too;
  `pane.release_agent` is the exit announcement and clears the whole claim.
- A hook report makes the pane **hook-authoritative** — the daemon's automatic
  screen-scrape for claude/codex/grok never overrides it. Panes whose agents
  never report a hook get scraped state instead (roster `source=scrape`).
- `nc -U` needs a netcat with Unix-socket support (macOS built-in works).

## Requirements

- The `par-mux` binary (ships with par-term: `make install` and `make bundle`
  place it next to par-term; check `which par-mux`).
- Full command reference, notifications, persistence semantics: `MUX.md` in the
  par-term-emu-core-rust repository (github.com/paulrobello/par-term-emu-core-rust);
  a par-term checkout carries a copy at `docs/features/MUX.md`.
