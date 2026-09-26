# Agent Usage Panel

par-term can display subscription usage for AI coding agents (Claude, Codex,
fireworks, …) as a status-bar widget and a popup detail panel. par-term itself
ships **no collectors** — it renders whatever appears in a records directory
that external collectors write. The file contract is omarchy's
(`omarchy/bin/omarchy-agent-usage-*` collectors are the port reference): keep
the records directory fresh is a collector's job, watch and render it is
par-term's.

## Records directory

One `<agent-id>.json` file per agent:

| Platform | Default |
|----------|---------|
| Linux / macOS | `~/.local/state/par-term/agents/usage/` |
| Windows | `%LOCALAPPDATA%\par-term\agents\usage\` |

The `PAR_TERM_AGENT_USAGE_RECORDS_DIR` environment variable overrides the
default (see [Environment Variables](../guides/ENVIRONMENT_VARIABLES.md)) —
used for hermetic `--ui-test` runs and debugging, the same isolation pattern
as `XDG_CONFIG_HOME` for the config.

## Record contract

The wire shape is omarchy's collector output: camelCase JSON with the
top-level identity/limit fields and a stats block the collectors merge in
**flat** (`record.update(stats)`), so `todayPrompts`, `recentDays`,
`modelUsage` etc. sit at the top level alongside `schemaVersion`. A minimal
but displayable record:

```json
{
    "schemaVersion": 1,
    "id": "claude",
    "name": "Claude",
    "updatedAt": "2026-09-20T17:06:14Z",
    "ready": true,
    "tierLabel": "Max 20x",
    "limits": [
        {"label": "Session (5-hour)", "percent": 42.0,
         "resetsAt": "2026-09-20T19:00:00Z"},
        {"label": "Weekly (7-day)", "percent": 71.5,
         "resetsAt": "2026-09-22T00:00:00Z"}
    ],
    "recentDays": [
        {"date": "2026-09-19", "messageCount": 6100000},
        {"date": "2026-09-20", "messageCount": 8200000}
    ],
    "modelUsage": {
        "opus-5": {"inputTokens": 1000, "outputTokens": 2000,
                   "cacheReadInputTokens": 3000,
                   "cacheCreationInputTokens": 4000}
    }
}
```

Beyond the fields shown: `hasLocalStats`, `usageStatusText` and
`authHelpText` (auth-failure records carry a status line instead of limits),
`balance` (prepaid agents — `{remaining, funded, spent, currency, estimated}`
— carried instead of limits), `retryAdvised`, and the all-time/today stats
(`totalPrompts`, `totalSessions`, `activeDays`, `activeDates`,
`todayPrompts`, `todaySessions`, `todayTotalTokens`, `todayTokensByModel`).

Tolerance rules — a collector change never takes the panel down:

- **Unknown fields are ignored** and **every field defaults when absent**, so
  a newer `schemaVersion` or a gained key still parses.
- A file that does not parse at all is skipped and reported as an error line
  in the panel (`claude.json: parse failed`); every other file still loads.
- Wire quirks are absorbed: the per-day key `messageCount` is actually a
  token total (the omarchy collectors say so) and is surfaced as tokens; a
  limit's display name arrives as `label` on most collectors but `title` on
  scoped limits — both spellings are accepted.

Merge rules:

- Stats are merged **flat** into the record by the collector (above).
- `activeDays` across records union their traveling `activeDates` lists and
  take the widest of that union and either side's bare count — a source that
  only knows a count still bounds the answer from below. This rule is
  reserved for v2 cross-device sync; v1 never merges records.

## Self-hiding

A record renders only when it has something to show and is not hidden:
`ready == true` and its id not in `agent_usage_hidden_agents`. A not-ready
agent produces no widget and no panel tab; a records directory with nothing
displayable (or missing entirely) hides the widget rather than showing an
empty placeholder. Deleting the records directory clears the display — it
never freezes stale data on screen.

## Status-bar widget

The `agent_usage` widget (**disabled by default**) shows one
line:

| Line | When |
|------|------|
| `◆ 71%` | Tightest rate-limit percentage across all displayable agents |
| `◆ $12.50` | No limits, first prepaid balance (USD) |
| `◆ 12.50 EUR` | Prepaid balance in another currency |
| `◆ active` | Ready agents with neither limits nor balance |

Clicking the widget opens the panel (works in every bar section).

## Popup panel

Opened by clicking the widget or the `toggle_agent_usage_panel` keybinding
action (bind it under `keybindings:` — the panel has no built-in chord).

| Key | Action |
|-----|--------|
| `r` | Refresh — rescan the records directory now (a local read, instant) |
| `h` / `l` | Switch to the previous / next agent |
| `Escape` | Close |

The panel shows the selected agent's tier and status line, each rate limit
with its percentage, reset countdown and meter bar, the prepaid balance when
present, a tokens-per-day chart over the last 7 `recentDays`, per-model token
totals (input + output + cache reads + cache writes, largest first), the
record's `updatedAt`, and one error line per unparseable file. While open it
registers with the modal guard — keystrokes cannot reach the terminal.

## Configuration

Top-level YAML keys (all optional):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `agent_usage_enabled` | `bool` | `true` | Master arm for the subsystem (directory watch + panel) |
| `agent_usage_update_command` | `string` | (none) | Optional refresh command (see below) |
| `agent_usage_refresh_interval_sec` | `u64` | `300` | Seconds between periodic rescans (clamped to a 30 s floor) |
| `agent_usage_hidden_agents` | `array` | `[]` | Agent ids to hide from widget and panel |

## Keeping records fresh

By default par-term is in **pure watch mode**: it watches the records
directory (with a poll fallback) and reacts to file changes, running nothing
itself. Setting `agent_usage_update_command` opts into omarchy's division of
labor: the command runs through `sh -c` on every refresh interval and on
manual refresh (`r`), off the UI thread, at most one instance at a time,
killed after 60 s. It comes from your own config (same trust level as
`custom_shell`) — nothing is ever interpolated into it. It is expected to
rewrite the records directory.

## Runnable ui-test recipe

The panel is fully drivable through the [`--ui-test`
harness](../guides/AGENT_UI_VERIFICATION.md) — including the letter keys,
since single letters `a`–`z` are pressable. Hermetic end-to-end run:

```bash
# 1. Fixture records directory (one displayable record)
mkdir -p /tmp/pt-agent-usage/{cfg/par-term,records,ui}
cat > /tmp/pt-agent-usage/records/claude.json <<'EOF'
{"schemaVersion":1,"id":"claude","name":"Claude",
 "updatedAt":"2026-09-20T17:06:14Z","ready":true,"tierLabel":"Max 20x",
 "limits":[{"label":"Session (5-hour)","percent":42.0}]}
EOF

# 2. Isolated config: capture shell, no first-run prompts, panel chord,
#    and the agent_usage widget enabled (a partial widget list is fine —
#    defaults are merged in).
cat > /tmp/pt-agent-usage/cfg/par-term/config.yaml <<'EOF'
custom_shell: /bin/sh
shell_args:
  - -c
  - cat > /tmp/pt-agent-usage/ui/pty-capture.txt
shader_install_prompt: never
shell_integration_state: never
status_bar_widgets:
  - id: agent_usage
    enabled: true
    section: right
    order: 90
keybindings:
  - key: "Ctrl+Alt+Cmd+U"
    action: "toggle_agent_usage_panel"
EOF

# 3. Script: ready assert, chord open, guard, refresh, Escape close,
#    PTY-leak sink.
cat > /tmp/pt-agent-usage/ui/script.json <<'EOF'
{
  "steps": [
    {"assert": "agent_usage_ready", "wait_ms": 700},
    {"chord": "Ctrl+Alt+Cmd+U", "wait_ms": 400},
    {"assert": "agent_usage_panel_open"},
    {"assert": "modal_guard"},
    {"press": "r", "wait_ms": 400},
    {"press": "Escape", "wait_ms": 400},
    {"assert_not": "agent_usage_panel_open"},
    {"assert_eq": ["file_empty", "/tmp/pt-agent-usage/ui/pty-capture.txt"]}
  ]
}
EOF

# 4. Build and run (dev-release profile — `make build` writes
#    target/dev-release/, not target/release/).
make build
XDG_CONFIG_HOME=/tmp/pt-agent-usage/cfg \
PAR_TERM_AGENT_USAGE_RECORDS_DIR=/tmp/pt-agent-usage/records \
  ./target/dev-release/par-term --ui-test /tmp/pt-agent-usage/ui/script.json \
  --ui-test-report /tmp/pt-agent-usage/ui/report.json

# 5. Verdict
python3 -c "import json;print(json.load(open('/tmp/pt-agent-usage/ui/report.json'))['all_passed'])"
```

`agent_usage_ready` asserts the store has at least one displayable record
(the fixture provides it via `PAR_TERM_AGENT_USAGE_RECORDS_DIR`); the final
`file_empty` asserts no keystroke leaked past the modal guard to the shell.
The operands are `agent_usage_panel_open` and `agent_usage_ready`; every step
observation also carries both.

## Related Documentation

- [Status Bar](STATUS_BAR.md) — the `agent_usage` widget and its sections
- [Environment Variables](../guides/ENVIRONMENT_VARIABLES.md) — `PAR_TERM_AGENT_USAGE_RECORDS_DIR`
- [Agent UI Verification](../guides/AGENT_UI_VERIFICATION.md) — the `--ui-test` harness
- [Configuration Reference](../CONFIG_REFERENCE.md) — all settings
