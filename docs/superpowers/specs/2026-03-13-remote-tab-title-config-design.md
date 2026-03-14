# Remote Tab Title Format — Design Spec

**Date:** 2026-03-13
**Status:** Approved

---

## Problem

When the par-term shell integration is active and the user SSH-es to a remote host, the tab
title in `Auto` mode shows only the last component of the remote CWD (e.g. `remoteuser`),
because:

1. The remote shell integration script emits OSC 7 with the full remote path
   (`file://remote-hostname/home/remoteuser`).
2. `update_title()` abbreviates against the *local* `$HOME`, which never matches, so
   no `~` substitution occurs.
3. The last path component (`remoteuser`) is used as the tab title.

Users want a configurable format that makes remote sessions immediately identifiable.

---

## Goal

Add a separate config field that controls the tab title format when a remote host is
detected via shell integration, with `user@host` as the default.

---

## Non-Goals

- Changing local (non-remote) tab title behavior.
- Modifying the existing `TabTitleMode` enum.
- SSH detection via command parsing (this relies on OSC 7 hostname detection).

---

## Config Changes (`par-term-config`)

### New enum — `par-term-config/src/types/tab_bar.rs`

```rust
/// Controls the tab title format when shell integration detects a remote host
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTabTitleFormat {
    /// "user@host" — SSH-style identifier (default)
    #[default]
    UserAtHost,
    /// "host" — remote hostname only
    Host,
    /// "host:~/dir" — hostname plus abbreviated CWD
    HostAndCwd,
}
```

Export from `par-term-config/src/types/mod.rs` and `par-term-config/src/lib.rs`.

### New fields — `par-term-config/src/config/config_struct/mod.rs`

```rust
/// Format for tab title when shell integration detects a remote host
#[serde(default)]
pub remote_tab_title_format: RemoteTabTitleFormat,

/// When true, explicit OSC title sequences override remote_tab_title_format
#[serde(default = "default_true")]
pub remote_tab_title_osc_priority: bool,
```

`default_true` is an existing helper (`fn default_true() -> bool { true }`); verify it
exists or add it.

Default for `remote_tab_title_format` comes from `#[default]` on the enum variant.
Default for `remote_tab_title_osc_priority` is `true` (OSC wins).

Both use `#[serde(default …)]` so existing configs deserialize cleanly with no migration.

---

## Logic Changes (`src/tab/profile_tracking.rs`)

### `update_title()` signature

Add two parameters:

```rust
pub fn update_title(
    &mut self,
    title_mode: par_term_config::TabTitleMode,
    remote_format: par_term_config::RemoteTabTitleFormat,
    remote_osc_priority: bool,
)
```

### Updated decision tree

```
if self.user_named → return (unchanged)

lock terminal (try_write, skip frame on contention — unchanged)

osc_title = term.get_title()
hostname  = term.shell_integration_hostname()   // Some(_) means remote
is_remote = hostname.is_some()

if is_remote:
    if remote_osc_priority && !osc_title.is_empty():
        self.title = osc_title                  // explicit OSC wins
        self.has_default_title = false
    else:
        self.title = format_remote_title(
            hostname, username, cwd, remote_format)
        self.has_default_title = false

elif !osc_title.is_empty():
    self.title = osc_title                      // local OSC title (unchanged)
    self.has_default_title = false

elif title_mode == TabTitleMode::Auto
     && cwd = term.shell_integration_cwd() is Some:
    self.title = last_cwd_component(cwd)        // local CWD fallback (unchanged)
    self.has_default_title = false

// else: keep existing title (unchanged)
```

### `format_remote_title()` helper (private, same file)

```rust
fn format_remote_title(
    hostname: Option<String>,
    username: Option<String>,
    cwd: Option<String>,
    format: RemoteTabTitleFormat,
) -> String {
    let host = hostname.unwrap_or_default();
    match format {
        RemoteTabTitleFormat::UserAtHost => {
            if let Some(user) = username {
                format!("{}@{}", user, host)
            } else {
                host
            }
        }
        RemoteTabTitleFormat::Host => host,
        RemoteTabTitleFormat::HostAndCwd => {
            if let Some(cwd) = cwd {
                // Abbreviate against local $HOME (best effort on remote)
                let abbrev = if let Some(home) = dirs::home_dir() {
                    cwd.replace(&home.to_string_lossy().to_string(), "~")
                } else {
                    cwd
                };
                format!("{}:{}", host, abbrev)
            } else {
                host
            }
        }
    }
}
```

### `update_all_titles()` — `src/tab/manager.rs`

Pass the two new fields from the caller:

```rust
pub fn update_all_titles(
    &mut self,
    title_mode: par_term_config::TabTitleMode,
    remote_format: par_term_config::RemoteTabTitleFormat,
    remote_osc_priority: bool,
) {
    for tab in self.tabs.values_mut() {
        tab.update_title(title_mode, remote_format, remote_osc_priority);
    }
}
```

Update all call sites to pass `config.remote_tab_title_format` and
`config.remote_tab_title_osc_priority`.

---

## UI Changes (`par-term-settings-ui`)

### `par-term-settings-ui/src/window_tab/tab_bar_behavior.rs`

Directly below the existing "Tab title mode" combo box, add:

**Remote format combo box:**
```
Remote tab title format: [ComboBox]
  • user@host   — "Show username and hostname (e.g. paul@server)"
  • host        — "Show hostname only"
  • host:~/cwd  — "Show hostname and current directory"
```

**OSC priority checkbox:**
```
[✓] OSC title takes priority on remote hosts
    hover: "When checked, explicit OSC title sequences override the remote format"
```

Both controls call `settings.has_changes = true` and `*changes_this_frame = true` on change.

### `par-term-settings-ui/src/sidebar.rs` — `tab_search_keywords()`

Add to the Window/Tab Bar keyword list:
```
"remote tab title", "ssh title", "remote host", "user at host", "remote format"
```

---

## Files to Change

| File | Change |
|------|--------|
| `par-term-config/src/types/tab_bar.rs` | Add `RemoteTabTitleFormat` enum |
| `par-term-config/src/types/mod.rs` | Export `RemoteTabTitleFormat` |
| `par-term-config/src/lib.rs` | Re-export `RemoteTabTitleFormat` |
| `par-term-config/src/config/config_struct/mod.rs` | Add two new fields |
| `par-term-config/src/config/config_struct/default_impl.rs` | Add defaults |
| `src/tab/profile_tracking.rs` | Update `update_title()`, add `format_remote_title()` |
| `src/tab/manager.rs` | Update `update_all_titles()` signature + call sites |
| `par-term-settings-ui/src/window_tab/tab_bar_behavior.rs` | Add two UI controls |
| `par-term-settings-ui/src/sidebar.rs` | Add search keywords |

---

## Backward Compatibility

- Existing configs without the new fields deserialize cleanly via `#[serde(default)]`.
- Default behaviour (`UserAtHost`, OSC priority on) is strictly better than the current
  accidental "last path component" display — a safe default change.

---

## Testing

- `make build` verifies no compile errors.
- Manual: SSH to a remote host with shell integration installed; verify tab shows
  `user@host` by default, changes with config, and OSC title override works.
- Existing tests unaffected (no PTY-dependent changes).
