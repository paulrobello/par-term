# Agent-Authored Commands — Design Document

**Date**: 2026-09-24
**Status**: Draft — for owner review
**Board card**: `01a0d1cbcb3276c19983cdc9d7bc097c` (par-term, high)
**Prior decisions**: D4, D4a, D4b, D4c (2026-09-23, recorded on the card)

## Overview

Agents — external MCP clients and par-term's built-in ACP agent — can create
new commands at runtime with no restart, the way the MCP `config_update` tool
lets an agent write and hot-apply shaders today
(`par-term-mcp/src/tools/config_update.rs`, applied by the config watcher).

A command is one of two kinds (D4a):

- **script** — a shell script run as a subprocess. Script commands can drive
  par-term through the existing script JSON protocol.
- **macro** — a declarative sequence over existing action types (new tab,
  split pane, insert text, key sequence, nested sequence).

Commands surface in the command palette and as `par-term <id>` on the CLI.

## Relationship to existing systems

The macro vocabulary already exists: `CustomActionConfig`
(`par-term-config/src/snippets.rs:337`) defines `ShellCommand`, `NewTab`,
`InsertText`, `SplitPane`, `KeySequence`, `Sequence`, each with a working
executor in `src/app/input_events/snippet_actions/`. Agent commands reuse
these instead of inventing a second action language:

- **script** = a `ShellCommand` variant payload
- **macro** = any of the other five variants, or a `Sequence` of them

Runtime row plumbing also exists: the palette already merges runtime rows
from plugins (`plugin_palette_entries`, wire ids `plugin-action:...`) and the
keybinding dispatcher already routes prefixed ids (`snippet:`, `action:`,
`plugin-action:`, `restore_arrangement:`). Agent commands add one more of
each, not a new mechanism.

Today snippets and config actions are reachable only via keybindings; the
palette does not list them. Agent commands are palette-first.

## Storage

One file per command, next to `config.yaml` (same `config_dir()` — XDG on
Unix, `%APPDATA%\par-term` on Windows):

```
<config_dir>/commands/<id>.yaml
```

YAML to match `config.yaml`. File format:

```yaml
created_by: agent             # "agent" | "user"
source_agent: claude-code     # required when created_by: agent
created_at: 2026-09-24T12:00:00Z
action:                       # one CustomActionConfig variant, serde-tagged
  type: shell_command
  id: deploy-staging          # [a-z0-9-]+, == filename stem
  title: Deploy staging
  command: ./deploy.sh
  args: ["staging"]
  timeout_secs: 300
```

- `id` and `title` live **inside** `action`: every
  `CustomActionConfig` variant embeds both as required fields (no serde
  default), so the wrapper has no duplicate top-level copy. Validation
  requires `action.id == <filename stem>` on load.
- `kind` is not stored: script vs macro is derivable from
  `action.type == shell_command`.
- Files whose `created_by` is `user` (or the field is absent) are
  user-authored commands. Users can create them by hand-editing the same
  directory; a Settings UI is follow-up scope.
- The existing `config.yaml` `actions` array is untouched and remains the
  legacy surface; no migration.

## Identity & trust (D4b, D4c)

**Body hash.** On load, par-term computes `body_hash` = SHA-256 over the
canonical serialization of the `action` payload. It is derived, never stored.

**Confirmation ledger.** `<config_dir>/commands/.confirmations.json` maps
`id` → approved `body_hash`. A script command whose current hash is not in
the ledger is *unconfirmed*:

- First run shows the full script body with **Run** / **Cancel**. Run
  records the hash; later runs of the same body need no confirmation.
- Any change to `action` produces a new hash → unconfirmed again.

Macros (non-`shell_command`) need no confirmation, per D4b.

**Provenance rules (D4c).** Enforced by the MCP tools and by par-term on
directory events:

- Agents may create, modify, and delete files tagged `created_by: agent`
  freely.
- Writes that would create or modify a **user-authored** command are
  refused by `command_create` with a message directing the user to approve
  or do it manually. (An approval-prompt flow is follow-up scope; refusal is
  the v1 behavior.)
- Provenance lives in the file, not inferable from the path.

## Palette integration

New runtime row source alongside `plugin_palette_entries`:

- `action_id`: `agent-cmd:<id>`
- `label`: `<title> · agent (<source_agent>)` for agent commands,
  `<title>` for user commands
- `priority`: 0, sorted with everything else by label

Dispatch extends the prefix chain in `keybinding_actions.rs` with
`agent-cmd:<id>`:

- load the command; if missing → log + no-op
- macro → execute through the existing `execute_custom_action` executor path
- script → check the confirmation ledger; unconfirmed opens the
  confirmation UI (full body, Run/Cancel); confirmed executes

Because dispatch is a keybinding-action id, users can also bind
`agent-cmd:<id>` to a chord in their keybindings config. No new UI for that.

## CLI

`Cli` gains an `#[command(external_subcommand)]` fallthrough: an argument
vector that matches no known subcommand resolves `<id>` against the commands
directory and runs it, passing remaining elements as positional arguments to
the script (`$1`, `$2`, ...). Macro commands ignore extra args. The same
confirmation rules apply; from a non-interactive CLI, an unconfirmed script
command prints the body and exits non-zero unless `--yes` is passed.

Collision behavior is clap-native: real subcommands (`install-shaders`,
`plugin`, ...) always win over the fallthrough.

## MCP tools (`par-term-mcp`)

Four tools, following the `config_update` precedent (atomic write,
restricted perms, validation before I/O):

- **command_list** — enumerate the commands directory, return id / title /
  kind / created_by / source_agent.
- **command_create** — upsert one command; callers repeat id/title inside
  `action` (required fields of every `CustomActionConfig` variant).
  Validation (SEC-005 class):
  id charset and length cap, field allowlist, payload size cap (64 KiB),
  `source_agent` required when `created_by: agent`. Refuses overwriting a
  user-authored command (D4c). Writes atomically (temp file + rename) so
  the watcher sees a complete file.
- **command_delete** — delete by id; refuses user-authored commands.
- **command_run** — *deferred*: an agent that wants to run a script can run
  it in its own pane; running inside par-term's UI adds confirmation-flow
  complexity for little gain. Revisit if agents need to drive the user's
  visible terminal.

## Directory watching

par-term watches `<config_dir>/commands/` with the same watcher pattern the
config file and plugins directory use (debounced, rename-event safe — an
atomic-rename is the create signal). Events reload the file and refresh the
palette snapshot; an invalid file is skipped with a log line, never a
dialog storm.

## Security notes

- Script bodies are arbitrary shell — the same risk class as the existing
  `ShellCommand` custom actions and trigger `run_command`s. The first-run
  confirmation (per body version) is the control; nothing here weakens the
  existing config_update key allowlist.
- The `commands/` directory is the only surface agents can author through
  this feature. `config.yaml` itself stays agent-unwritable.
- Validation caps on the MCP write path bound damage from a compromised
  agent: bounded file count (e.g. 200 commands), bounded size, strict id
  charset, no path traversal (id == filename stem, validated).

## Out of scope (follow-up cards, not this design)

- Settings UI for browsing/editing commands (v1 is files + palette)
- User-approval prompt flow for agent-modify-of-user-command (v1 refuses)
- Command parameters / prompts-before-run beyond the first-run gate
- Per-project (non-global) command scope — global per D4c
- command_run MCP tool

## Open questions for owner

1. **YAML vs JSON** for command files — YAML proposed (matches config.yaml).
2. **User commands in the same directory** — proposed: yes, `created_by:
   user`, hand-edited; config.yaml actions stay legacy. Alternative: the
   directory is agent-only and user commands stay in config.yaml.
3. **CLI args pass-through** — proposed: remaining args become script
   positional args (`$1..`); macros ignore them.
4. **Palette priority** — agent commands at priority 0 mixed in by label,
   or slightly boosted (like agent-roster rows) so new commands are easy to
   spot? Proposed: 0 (palette is not a notification surface).
5. **Confirmation UI copy** — two buttons (Run / Cancel) with the full body
   shown, or three (Run / Always for this version / Cancel)? The ledger
   already makes Run persistent for that body, so two buttons suffice.
