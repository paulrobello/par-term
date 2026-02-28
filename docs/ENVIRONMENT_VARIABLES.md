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
| `TERM_PROGRAM` | — | Terminal program identifier. Populated by par-term shell integration. |
| `PATH` | — | System executable search path. Read at startup to locate the shell and other programs. |
| `LANG` | — | Locale setting forwarded to the child shell. |
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

par-term follows the [XDG Base Directory specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html) for configuration and data storage on Linux and macOS.

| Variable | Default | Description |
|----------|---------|-------------|
| `XDG_CONFIG_HOME` | `~/.config` | Base directory for user configuration files. Config is stored at `$XDG_CONFIG_HOME/par-term/config.yaml`. |
| `XDG_DATA_HOME` | `~/.local/share` | Base directory for user data files. |
| `XDG_STATE_HOME` | `~/.local/state` | Base directory for user state files. |
| `XDG_CACHE_HOME` | `~/.cache` | Base directory for user cache files. |
| `XDG_RUNTIME_DIR` | — | Runtime directory for socket files and other per-session data. |

On Windows, `APPDATA` and `LOCALAPPDATA` are used instead of XDG directories.

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

- Variables in the standard allowlist (all variables listed in this document under Shell, User/Home, XDG, Editor, Display, and Temp sections)
- Any variable prefixed with `PAR_TERM_`
- Any variable prefixed with `LC_` (locale variables)

To allow substitution of all environment variables (including secrets — use with caution):

```yaml
allow_all_env_vars: true
```

---

## Related Documentation

- [Configuration Reference](CONFIG_REFERENCE.md) — All configuration options
- [Getting Started](GETTING_STARTED.md) — Installation and first-launch walkthrough
- [Debug Logging](LOGGING.md) — Detailed logging documentation
