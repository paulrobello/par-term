# Agent-Authored Commands

Agents — external MCP clients and par-term's built-in ACP agent — can create
commands at runtime with no restart, the same contract the MCP `config_update`
tool established for shaders. Design: `docs/plans/2026-09-24-agent-authored-commands-design.md`
(approved 2026-09-24).

## What a command is

One file per command under `<config_dir>/commands/<id>.yaml`, wrapping exactly
one `CustomActionConfig` variant (the same action language as config.yaml
`actions`):

```yaml
created_by: agent             # "agent" | "user" (absent = user)
source_agent: claude-code     # required when created_by: agent
created_at: 2026-09-24T12:00:00Z
action:                       # one CustomActionConfig variant, serde-tagged
  type: shell_command         #   shell_command = script kind
  id: deploy-staging          #   [a-z0-9-]+, must equal the filename stem
  title: Deploy staging
  command: ./deploy.sh
  args: ["staging"]
  timeout_secs: 300
```

- **script** (`type: shell_command`) runs a shell command as a subprocess.
- **macro** (any other variant: `new_tab`, `insert_text`, `split_pane`,
  `key_sequence`, `sequence`) replays built-in actions.

`id` and `title` live inside `action` — every variant embeds both as required
fields. Validation on load: id charset (`[a-z0-9-]+`, ≤64 bytes) and
`action.id == <filename stem>` (this doubles as the path-traversal guard).

## Surfaces

- **Command palette** — rows appear automatically as `agent-cmd:<id>`
  (label: `<title> · agent (<source_agent>)` for agent commands, plain
  `<title>` for user commands), hot-reloaded by a directory watcher.
- **Keybindings** — bind `agent-cmd:<id>` to any chord.
- **CLI** — `par-term <id> [args...]` (external-subcommand fallthrough; real
  subcommands always win). Extra args append to a script's stored args;
  macros refuse with a pointer at the palette.
- **Settings → Actions → Agent Commands** — lists every valid command file
  (id, title, script/macro, author) and deletes one behind a two-step
  confirm. Deleting here acts with the user's authority, so it removes
  user-authored files too, and it prunes the id's confirmation-ledger entry.
  The list refreshes on first display and on **Refresh**; editing is by hand.

## Trust model (D4b/D4c)

- **First-run confirmation for scripts**: `<config_dir>/commands/.confirmations.json`
  maps id → approved SHA-256 of the action payload. An unconfirmed script
  shows its full body and asks Run / Cancel (in-app) or prints the body and
  exits non-zero (CLI, unless `--yes`). Any body change resets confirmation.
  Macros need no confirmation.
- **Provenance lives in the file**: agents may freely modify/delete
  `created_by: agent` files; `command_create`/`command_delete` **refuse**
  to touch user-authored files (the user edits or deletes those by hand).
- **MCP write path validation**: id charset/length caps, 64 KiB payload cap,
  200-file count cap, atomic writes (temp + rename) so the watcher only
  ever sees complete files, `source_agent` required for agent files, and
  `command_create` rejects `created_by: user` payloads outright.

## Script runtime inputs

- **Args** — stored `args` plus CLI extras (`par-term <id> a b c`).
- **Environment** — inherited, plus `PAR_TERM_COMMAND_ID` and (for agent
  commands) `PAR_TERM_COMMAND_SOURCE_AGENT`.
- Scripts driving par-term use the documented IPC paths under `config_dir`.

## MCP tools (`par-term mcp-server`)

| Tool | Effect |
|------|--------|
| `command_create` | Upsert one agent command (validated, atomic, refuses user files) |
| `command_list` | Enumerate: id / title / kind / created_by / source_agent |
| `command_delete` | Delete by id (refuses user files) |

The built-in ACP agent reaches the same tools through its `par-term-config`
MCP server descriptor (`build_mcp_server_descriptor`).

## Code map

| Area | File |
|------|------|
| File format, validation, ledger | `par-term-config/src/agent_commands.rs` |
| App store + watcher + confirmation queue | `src/agent_commands_store.rs` |
| Palette merge + `agent-cmd:` dispatch | `src/app/input_events/keybinding_actions.rs` |
| First-run confirmation dialog | `src/app/render_pipeline/egui_overlays.rs` |
| MCP tools | `par-term-mcp/src/tools/agent_commands.rs` |
| CLI fallthrough | `src/cli/mod.rs` (`Commands::External`) |
| Settings list + delete | `par-term-settings-ui/src/actions_tab/agent_commands_section.rs` |

## Out of scope (filed as follow-ups on the board)

- Editing commands in Settings (list + delete shipped; edit is by hand)
- User-approval flow for agent-modify-of-user-command (v1 refuses)
- Command parameters / prompts beyond the first-run gate
- Per-project command scope (global per D4c)
- `command_run` MCP tool
