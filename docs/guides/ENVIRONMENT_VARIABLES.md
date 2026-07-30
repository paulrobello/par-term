# Environment Variables Reference

This document lists all environment variables recognized by par-term. Variables are grouped by function.

## Table of Contents

- [Logging and Debug](#logging-and-debug)
- [Shell and Terminal](#shell-and-terminal)
- [User and Home](#user-and-home)
- [XDG Base Directories](#xdg-base-directories)
- [Editor and Pager](#editor-and-pager)
- [Display and Windowing](#display-and-windowing)
- [MCP IPC (Internal)](#mcp-ipc-internal)
- [Git and Badge Variables](#git-and-badge-variables)
- [Config Variable Substitution](#config-variable-substitution)

## Overview

par-term reads environment variables at startup for two purposes:

1. **Runtime behavior** — variables like `DEBUG_LEVEL` and `RUST_LOG` affect logging and debugging.
2. **Config substitution** — the config file supports `${VAR_NAME}` placeholders that are expanded at load time. Only an explicit allowlist of variables (plus any `PAR_TERM_*` and `LC_*` prefixed names) are substituted by default. Set `allow_all_env_vars: true` in your config to bypass this restriction.

---

## Logging and Debug

| Variable | Default | Description |
|----------|---------|-------------|
| `DEBUG_LEVEL` | `0` (off) | Controls verbosity of par-term's custom debug macros. Accepts `0`–`4`: `0` = off, `1` = error, `2` = info, `3` = debug, `4` = trace. Logs are written to `$TMPDIR/par_term_debug.log`. |
| `RUST_LOG` | unset | Controls verbosity of the standard `log` crate. Accepts `error`, `warn`, `info`, `debug`, `trace`, or module-level filters such as `par_term=debug`. When set, log output is also mirrored to stderr. |

### Log File Location

The debug log is written to `<temp_dir>/par_term_debug.log`:

- **macOS/Linux**: `$TMPDIR/par_term_debug.log` (typically `/tmp/par_term_debug.log`)
- **Windows**: `%TEMP%\par_term_debug.log`

Use `make tail-log` or `tail -f /tmp/par_term_debug.log` to monitor the log in real time.

### Debug Level Values

| Value | Macro Level | What Is Logged |
|-------|-------------|---------------|
| `0` | Off | Nothing (default) |
| `1` | Error | Fatal and error conditions |
| `2` | Info | Major lifecycle events |
| `3` | Debug | Detailed operational events (most useful for development) |
| `4` | Trace | Highest-frequency events (rendering, input per frame) |

---

## Shell and Terminal

| Variable | Default | Description |
|----------|---------|-------------|
| `SHELL` | `/bin/bash` | Path to the shell to launch in new terminal tabs. Falls back to `/bin/bash` if unset. |
| `TERM` | — | Terminal type reported to child processes. Also used in config variable substitution. |
| `TERM_PROGRAM` | `iTerm.app` | Set by par-term in every child process environment for maximum protocol compatibility. Tools that check `TERM_PROGRAM` for feature detection (OSC 8 hyperlinks, OSC 9;4 progress bars, OSC 52 clipboard, OSC 1337 file transfer) will enable those features. |
| `TERM_PROGRAM_VERSION` | `3.6.6` | Set by par-term alongside `TERM_PROGRAM` to advertise the iTerm2 protocol version. |
| `LC_TERMINAL` | `iTerm2` | Set by par-term for tools that check this variable for iTerm2 feature compatibility. |
| `LC_TERMINAL_VERSION` | `3.6.6` | Set by par-term alongside `LC_TERMINAL`. |
| `ITERM_SESSION_ID` | `w0t0p0:<uuid>` | Set by par-term per session. Used by Claude Code and other tools for OSC 52 clipboard detection. Format: `w{window}t{tab}p{pane}:{UUID}`. |
| `__PAR_TERM` | `1` | Set by par-term as an identity marker. Shell integration scripts use this to detect they are running inside par-term. |
| `PATH` | — | System executable search path. Read at startup to locate the shell and other programs. par-term augments `PATH` with common tool directories (e.g., `/opt/homebrew/bin`, `~/.cargo/bin`) when launching child processes. |
| `LANG` | `en_US.UTF-8` | Locale setting inherited from the parent environment and forwarded to the child shell. If no locale variables (`LANG`, `LC_ALL`, `LC_CTYPE`) are set in the parent environment (e.g., when launched from Finder/Dock), defaults to `en_US.UTF-8`. |
| `COLORTERM` | — | Color capability hint (`truecolor` or `24bit`). Forwarded to child processes. |

---

## User and Home

| Variable | Default | Description |
|----------|---------|-------------|
| `HOME` | — | User's home directory. Used for path expansion in config and shell integration. |
| `USER` | — | Current username. Used in badge variables and snippet substitution. |
| `USERNAME` | — | Windows equivalent of `USER`. |
| `LOGNAME` | — | Alternative username variable, used as fallback after `USER`. |
| `USERPROFILE` | — | Windows home directory path (equivalent to `HOME` on Unix). |
| `HOSTNAME` | — | System hostname. Used in snippet variable substitution. |
| `HOST` | — | Short hostname. Used as fallback after `HOSTNAME`. |

---

## XDG Base Directories

par-term honours **`XDG_CONFIG_HOME` only**. It does not implement the full [XDG Base Directory specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html): all per-user data lives together under the config directory rather than being split across separate config, data, state and cache roots.

| Variable | Default | Description |
|----------|---------|-------------|
| `XDG_CONFIG_HOME` | `~/.config` | Base directory for par-term's configuration **and all other per-user data**. Config is at `$XDG_CONFIG_HOME/par-term/config.yaml`; profiles, arrangements, command history, session state, sounds and discovered ACP agents sit beside it. |

The value must be an **absolute** path. As the specification requires, a relative or empty `XDG_CONFIG_HOME` is treated as invalid and ignored, falling back to `~/.config`.

`XDG_DATA_HOME`, `XDG_STATE_HOME`, `XDG_CACHE_HOME` and `XDG_RUNTIME_DIR` are **not** read for path resolution. They remain on the substitution allowlist, so `${XDG_DATA_HOME}` still expands inside your config file — but setting them does not move any par-term file.

On Windows, `%APPDATA%\par-term` is used and the XDG variables are ignored entirely.

> **Changing `XDG_CONFIG_HOME` after first run?** par-term looks for an existing `~/.config/par-term/` and moves it to the new location on next launch, without overwriting anything already there. See the [Migration Guide](MIGRATION.md).

| Variable | Default | Description |
|----------|---------|-------------|
| `APPDATA` | — | Windows roaming app data directory (e.g., `C:\Users\<user>\AppData\Roaming`). Config is stored at `%APPDATA%\par-term\config.yaml`. |
| `LOCALAPPDATA` | — | Windows local app data directory. |

---

## Editor and Pager

| Variable | Default | Description |
|----------|---------|-------------|
| `EDITOR` | — | Preferred text editor command. Used when opening files from semantic history or URL detection. `VISUAL` is checked as a fallback. |
| `VISUAL` | — | Visual editor command. Checked as a fallback for `EDITOR`. |
| `PAGER` | — | Pager command. Available for config variable substitution. |

---

## Display and Windowing

| Variable | Default | Description |
|----------|---------|-------------|
| `DISPLAY` | — | X11 display server connection string (Linux). Required on X11 systems for GPU rendering. |
| `WAYLAND_DISPLAY` | — | Wayland compositor socket name (Linux). Used when running under a Wayland compositor. |
| `TMPDIR` / `TEMP` / `TMP` | — | Temporary directory override. Used to locate or create the debug log file. |

---

## MCP IPC (Internal)

These variables are set by par-term itself when launching the MCP server subprocess. They configure the file-based IPC handshake between the MCP server and the GUI application. You do not normally need to set these manually.

| Variable | Default | Description |
|----------|---------|-------------|
| `PAR_TERM_CONFIG_UPDATE_PATH` | `<config_dir>/.config-update.json` | Path for the MCP `config_update` tool to write configuration changes that the GUI app picks up. |
| `PAR_TERM_SCREENSHOT_REQUEST_PATH` | `<config_dir>/.screenshot-request.json` | Path where the MCP server writes a screenshot request. |
| `PAR_TERM_SCREENSHOT_RESPONSE_PATH` | `<config_dir>/.screenshot-response.json` | Path where the GUI app writes the screenshot response. |
| `PAR_TERM_SCREENSHOT_FALLBACK_PATH` | unset | Optional static fallback image path. Used by the ACP harness for testing the screenshot tool without a running GUI. |
| `PAR_TERM_SHADER_DIAGNOSTICS_REQUEST_PATH` | `<config_dir>/.shader-diagnostics-request.json` | Path where the MCP server writes a shader diagnostics request. |
| `PAR_TERM_SHADER_DIAGNOSTICS_RESPONSE_PATH` | `<config_dir>/.shader-diagnostics-response.json` | Path where the GUI app writes the shader diagnostics response. |
| `PAR_TERM_MCP_AUTH_TOKEN` | unset | Opt-in per-process session auth token for the MCP server (SEC-006 hardening). Unlike the rows above, this is **not** set by par-term — operators set it on the spawned `par-term mcp-server` process. When set to a non-empty value, the server requires clients to echo it back as `_meta.parTermAuthToken` in the `initialize` handshake and rejects `tools/list` / `tools/call` (`-32001` error) until they do. When unset (the default), auth is disabled and all calls are allowed, preserving existing ACP flows. |

> **Security:** `PAR_TERM_MCP_AUTH_TOKEN` is OPT-IN. par-term does not spawn the MCP server itself (the agent host does), so it cannot inject a token automatically. Operators who want the hardening must set this env var on the spawned `par-term mcp-server` process AND configure their agent host to forward the same value in `_meta.parTermAuthToken`. Token comparison uses constant-time comparison as defense-in-depth; the threat model is local-process access control.

---

## Git and Badge Variables

These optional environment variables can be set by external scripts or shell integrations to provide contextual information displayed in the tab bar and status line.

| Variable | Default | Description |
|----------|---------|-------------|
| `GIT_BRANCH` | (computed) | Current git branch name. If unset, par-term runs `git rev-parse --abbrev-ref HEAD` as a fallback. Used in snippet variable substitution (`${git_branch}`). |
| `GIT_COMMIT` | (computed) | Current git commit hash (short form). If unset, par-term runs `git rev-parse --short HEAD` as a fallback. Used in snippet variable substitution (`${git_commit}`). |
| `TTY` | — | TTY device path for the terminal session. Read by the badge system for display in the status line. |

---

## Config Variable Substitution

Inside `config.yaml`, you can reference environment variables using `${VAR_NAME}` syntax:

```yaml
font_family: "${PAR_TERM_FONT:-JetBrains Mono}"
shell: "${SHELL}"
```

### Syntax

| Form | Behavior |
|------|----------|
| `${VAR}` | Replaced with the value of `VAR`. Left unchanged if `VAR` is unset. |
| `${VAR:-default}` | Replaced with the value of `VAR`, or `default` if `VAR` is unset. |
| `$${VAR}` | Escaped form — produces the literal string `${VAR}` without substitution. |

### Allowlist

By default, only the following variable categories are substituted. References to other variables are left unchanged and a warning is logged.

- The standard allowlist defined in `par-term-config/src/config/env_vars.rs` (`ALLOWED_ENV_VARS`): `HOME`, `USER`, `USERNAME`, `LOGNAME`, `USERPROFILE`, `SHELL`, `TERM`, `LANG`, `COLORTERM`, `TERM_PROGRAM`, the five `XDG_*` directories, `PATH`, `TMPDIR`, `TEMP`, `TMP`, `DISPLAY`, `WAYLAND_DISPLAY`, `HOSTNAME`, `HOST`, `EDITOR`, `VISUAL`, `PAGER`, `APPDATA`, and `LOCALAPPDATA`.
- Any variable prefixed with `PAR_TERM_`
- Any variable prefixed with `LC_` (locale variables)

> **Note:** `TERM_PROGRAM_VERSION`, `ITERM_SESSION_ID`, `__PAR_TERM`, `TTY`, `GIT_BRANCH`, and `GIT_COMMIT` are read by par-term at runtime but are **not** on the config-substitution allowlist. (`LC_TERMINAL` and `LC_TERMINAL_VERSION` are substituted via the `LC_` prefix rule above.)

To allow substitution of all environment variables (including secrets — use with caution):

```yaml
allow_all_env_vars: true
```

---

## Related Documentation

- [Configuration Reference](../CONFIG_REFERENCE.md) — All configuration options
- [Getting Started](GETTING_STARTED.md) — Installation and first-launch walkthrough
- [Debug Logging](../LOGGING.md) — Detailed logging documentation
