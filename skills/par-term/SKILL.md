---
name: par-term
description: Operate, configure, and extend the par-term GPU terminal emulator for your user. Covers editing config.yaml safely (hot-reload semantics), profiles, adding keybindings, installing and enabling plugins (land-disabled trust model), creating agent-authored commands over MCP or as files, attaching par-mux sessions and reading the agent roster, and verifying every change with the --ui-test harness. Use when asked to configure par-term, change a terminal setting or keybinding, install a plugin, add a palette command or agent command, attach or inspect a par-mux session, or verify terminal UI behavior without touching the user's real config.
---

# par-term agent skill

You are operating par-term on the user's machine. This skill tells you where
the knobs are and how to prove a change worked. Every section names the doc
that owns the details — read it before doing anything the summary here does
not fully specify.

## Where things live

| Path | Contents |
|---|---|
| `~/.config/par-term/config.yaml` | Main config (Windows: `%APPDATA%\par-term\config.yaml`). `XDG_CONFIG_HOME` is honored — the isolation trick below depends on it |
| `~/.config/par-term/commands/<id>.yaml` | Agent/user-authored commands (one `CustomActionConfig` per file) |
| `~/.config/par-term/plugins/<id>/` | Plugin directories (`manifest.json` + files) |
| `~/.config/par-term/agents/*.toml` | Custom ACP agent definitions |
| `~/.config/par-term/shaders/` | User-installed custom shaders |

Docs root: [docs/README.md](../../docs/README.md) in any checkout, or the
repo at <https://github.com/paulrobello/par-term>.

## Config editing

- `config.yaml` is **watched and hot-reloaded** (500 ms poll). An edit
  applies to the running app without a restart. Write atomically
  (temp file + rename) so the watcher only ever sees complete YAML.
- The full key reference is [CONFIG_REFERENCE.md](../../docs/CONFIG_REFERENCE.md);
  profiles are [PROFILES.md](../../docs/PROFILES.md) (a profile is a named
  config overlay with its own shell, cwd, tmux/par-mux session, and keybinds).
- Safe-to-edit as an agent: appearance, fonts, keybindings, snippets/actions,
  status bar, plugins state, profiles. Never rewrite the whole file to change
  one key — read, modify, write.
- Two top-level keys suppress first-run prompts and matter in every test
  config: `shader_install_prompt: never` and `shell_integration_state: never`
  (lowercase values, top-level keys).

## Keybindings

List form under `keybindings:` — each entry `{key, action}`:

```yaml
keybindings:
  - key: "Ctrl+Alt+Cmd+Y"
    action: "toggle_command_palette"
```

Action names, chord syntax, and the full catalog:
[KEYBOARD_SHORTCUTS.md](../../docs/guides/KEYBOARD_SHORTCUTS.md). Snippets
and actions bind as `snippet:<id>` / `action:<id>` ([SNIPPETS.md](../../docs/features/SNIPPETS.md)),
agent commands as `agent-cmd:<id>`.

## Plugins

A plugin is a subprocess — never in-process. **Land-disabled**: installing
never runs anything; a plugin runs only once enabled, and the enable toggle
in **Settings → Automation → Plugins** is the consent step. The state also
lives in config as `plugins: [{id: …, enabled: true}]`.

- Install from git: `par-term plugin add <url>` (lands disabled, fast-forward
  updates only). Or copy a directory to
  `~/.config/par-term/plugins/<plugin-id>` (dir name must equal the manifest id).
- Example plugins to learn from or test with live in
  `scripts/examples/plugins/` of a checkout (`com.example.clock` is the
  reference widget).
- Full manifest format, security model, and a runnable clock install recipe:
  [PLUGINS.md](../../docs/features/PLUGINS.md).

## Agent-authored commands

One YAML file per command under `~/.config/par-term/commands/<id>.yaml`,
wrapping exactly one action variant; `id` must match the filename stem and
match `[a-z0-9-]+`:

```yaml
created_by: agent
source_agent: claude-code
created_at: 2026-09-25T12:00:00Z
action:
  type: shell_command
  id: deploy-staging
  title: Deploy staging
  command: ./deploy.sh
  args: ["staging"]
```

- Prefer the MCP tools when you have them (`par-term mcp-server`):
  `command_create` / `command_list` / `command_delete` — validated, atomic,
  and they **refuse to touch user-authored files**. Never edit or delete a
  command file whose `created_by` is `user`.
- Commands appear in the palette as `agent-cmd:<id>`, bind to chords, and run
  from the CLI as `par-term <id> [args…]` (scripts confirm on first run
  unless `--yes`).
- Full format, trust model, and an end-to-end MCP recipe:
  [AGENT_COMMANDS.md](../../docs/features/AGENT_COMMANDS.md).

## par-mux sessions

par-mux is the daemon-backed session layer: panes survive closing the tab,
window, or app. Attach is a **profile** property — set `mux_session_name` in
a profile (Settings → Profiles → par-mux Auto-Attach, or in the profile's
YAML) and opening that profile attaches, one tab per session window. The
agent roster (which coding agents are working/blocked/idle) surfaces as a
status-bar widget and palette picker, populated via the daemon's
`list-agents`. Model, detach/reattach, daemon lifecycle:
[MUX.md](../../docs/features/MUX.md).

## Verifying changes — `--ui-test`

Never experiment against the user's real config. `--ui-test` drives the live
app's UI through the real keybinding layer and reports JSON — no host
permissions needed ([AGENT_UI_VERIFICATION.md](../../docs/guides/AGENT_UI_VERIFICATION.md)):

```bash
ROOT=/tmp/pt-agent-skill; rm -rf "$ROOT"; mkdir -p "$ROOT/cfg/par-term"
cat > "$ROOT/cfg/par-term/config.yaml" <<'EOF'
custom_shell: /bin/sh
shell_args: ["-c", "cat > /tmp/pt-agent-skill/pty-capture.txt"]
shader_install_prompt: never
shell_integration_state: never
EOF
cat > "$ROOT/script.json" <<'EOF'
{"steps": [
  {"chord": "Ctrl+Alt+Cmd+P", "wait_ms": 400},
  {"assert": "palette_open"},
  {"press": "Escape", "wait_ms": 300},
  {"assert_not": "palette_open"},
  {"assert_eq": ["file_empty", "/tmp/pt-agent-skill/pty-capture.txt"]}
]}
EOF
XDG_CONFIG_HOME="$ROOT/cfg" par-term --ui-test "$ROOT/script.json" --ui-test-report "$ROOT/report.json"
```

`all_passed: true` in the report is the verdict. The `cat > file` shell is a
PTY-capture sink: `file_empty` at the end proves nothing leaked to the shell.
Asserts cover palette/search/settings/plugins (`plugins_loaded`,
`plugin_widget_set`) and more — see the operand table in the doc. For driving
a real agent end-to-end, the ACP harness recipe is in
[ACP_HARNESS.md](../../docs/ACP_HARNESS.md).
