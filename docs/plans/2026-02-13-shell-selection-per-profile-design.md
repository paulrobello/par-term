# Shell Selection Per Profile

**Issue**: #128
**Date**: 2026-02-13

## Problem

Users cannot configure a specific shell per profile. The only way to override the shell is via the global `custom_shell` config or the generic `command` field on profiles (which conflates shell selection with arbitrary command execution).

## Design

### Profile struct additions (`src/profile/types.rs`)

- `shell: Option<String>` — path to shell binary (e.g. `/bin/zsh`, `/usr/bin/fish`)
- `login_shell: Option<bool>` — `None` inherits global setting, `Some(true/false)` overrides

### Shell detection (`src/shell_detection.rs`)

New module providing platform-aware shell discovery:

- **Unix/macOS**: parse `/etc/shells`, verify each path exists via `std::fs::metadata`
- **Windows**: check known locations for PowerShell, cmd.exe, WSL distributions, Git Bash
- Returns `Vec<ShellInfo>` with display name and absolute path
- Results cached with `OnceLock` after first call

### Resolution priority (`Tab::new_from_profile()`)

1. Profile `command` set → use command + command_args (existing, unchanged)
2. Profile `shell` set → use as shell, apply profile `login_shell` if set (else global)
3. Neither → existing behavior (global `custom_shell` or `$SHELL` / `powershell.exe`)

### UI changes (`src/profile_modal_ui.rs`)

- ComboBox dropdown: "Default (inherit)" + all detected shells
- Checkbox: "Login shell" with tri-state semantics (inherit global / force on / force off)

### Search keywords (`src/settings_ui/sidebar.rs`)

Add: `shell`, `login`, `bash`, `zsh`, `fish`, `powershell` to profiles tab keywords.

## Files changed

1. `src/profile/types.rs` — add `shell`, `login_shell` fields
2. `src/shell_detection.rs` — new module
3. `src/tab/mod.rs` — update shell resolution in `new_from_profile()`
4. `src/profile_modal_ui.rs` — add dropdown + checkbox
5. `src/settings_ui/sidebar.rs` — search keywords
6. `src/main.rs` or `src/lib.rs` — register module
