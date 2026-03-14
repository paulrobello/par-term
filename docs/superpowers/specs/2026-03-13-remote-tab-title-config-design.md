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
In `lib.rs`, append `RemoteTabTitleFormat` inside the **existing** `pub use types::{...}`
block (do not add a separate `pub use` statement — that causes duplicate re-export lints).

Add `display_name()` and `all()` methods to the enum, consistent with every other
combo-box enum in this file (`TabStyle`, `TabBarPosition`, `WindowType`):

```rust
impl RemoteTabTitleFormat {
    pub fn display_name(&self) -> &'static str {
        match self {
            RemoteTabTitleFormat::UserAtHost => "user@host",
            RemoteTabTitleFormat::Host       => "host",
            RemoteTabTitleFormat::HostAndCwd => "host:~/cwd",
        }
    }

    pub fn all() -> &'static [RemoteTabTitleFormat] {
        &[
            RemoteTabTitleFormat::UserAtHost,
            RemoteTabTitleFormat::Host,
            RemoteTabTitleFormat::HostAndCwd,
        ]
    }
}
```

### New fields — `par-term-config/src/config/config_struct/mod.rs`

Add `RemoteTabTitleFormat` to the `use crate::types::{...}` import at the top of the file
(it already imports `TabTitleMode` from that list).

```rust
/// Format for tab title when shell integration detects a remote host
#[serde(default)]
pub remote_tab_title_format: RemoteTabTitleFormat,

/// When true, explicit OSC title sequences override remote_tab_title_format
#[serde(default = "crate::defaults::bool_true")]
pub remote_tab_title_osc_priority: bool,
```

Use `crate::defaults::bool_true` (not a local helper) — this is the existing convention
for boolean fields in `Config` (e.g. `tab_show_close_button`).

### `par-term-config/src/config/config_struct/default_impl.rs`

Add both fields to the `impl Default for Config` block:

```rust
remote_tab_title_format: RemoteTabTitleFormat::default(),   // UserAtHost
remote_tab_title_osc_priority: true,
```

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

All four values (`osc_title`, `hostname`, `username`, `cwd`) must be read inside a
**single `try_write()` block** — the same pattern as the existing implementation. Do not
acquire the lock multiple times.

Use a nested scope `{ }` or `drop(term)` to release the lock **before** any mutation of
`self.title`, so the borrow checker is satisfied and the critical section is minimal:

```rust
if self.user_named { return; }

if let Ok(term) = self.terminal.try_write() {
    // Collect all values while holding the lock
    let osc_title = term.get_title();
    let hostname  = term.shell_integration_hostname();
    let username  = term.shell_integration_username();
    let cwd       = term.shell_integration_cwd();
    drop(term);  // release lock before mutating self.title

    let is_remote = hostname.is_some();

    if is_remote {
        if remote_osc_priority && !osc_title.is_empty() {
            self.title = osc_title;             // explicit OSC wins
            self.has_default_title = false;
        } else {
            self.title = format_remote_title(hostname, username, cwd, remote_format);
            self.has_default_title = false;
        }
    } else if !osc_title.is_empty() {
        self.title = osc_title;                 // local OSC title (unchanged)
        self.has_default_title = false;
    } else if title_mode == TabTitleMode::Auto {
        if let Some(cwd) = cwd {
            // Use last path component (unchanged local CWD fallback)
            ...
            self.has_default_title = false;
        }
    }
    // else: keep existing title (unchanged)
}
```

### `format_remote_title()` helper (private, same file)

For `HostAndCwd`, the remote CWD is an absolute path on the remote machine (e.g.
`/home/remoteuser/projects`). The local `$HOME` never matches, so local-home abbreviation
is a no-op on remote paths. Instead, abbreviate using the remote username: replace
`/home/<username>` (or `/Users/<username>`) with `~` if the username is known.

```rust
fn format_remote_title(
    hostname: Option<String>,
    username: Option<String>,
    cwd: Option<String>,
    format: par_term_config::RemoteTabTitleFormat,
) -> String {
    use par_term_config::RemoteTabTitleFormat;
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
                // Try to abbreviate the remote home dir using the remote username.
                // Remote home is typically /home/<user> (Linux) or /Users/<user> (macOS).
                let abbrev = if let Some(ref user) = username {
                    let linux_home = format!("/home/{}", user);
                    let macos_home = format!("/Users/{}", user);
                    if cwd.starts_with(&linux_home) {
                        cwd.replacen(&linux_home, "~", 1)
                    } else if cwd.starts_with(&macos_home) {
                        cwd.replacen(&macos_home, "~", 1)
                    } else {
                        cwd
                    }
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
    for tab in &mut self.tabs {   // Vec<Tab>, not HashMap
        tab.update_title(title_mode, remote_format, remote_osc_priority);
    }
}
```

Update all call sites to pass `config.remote_tab_title_format` and
`config.remote_tab_title_osc_priority`. There are two known call sites:

1. `src/app/render_pipeline/frame_setup.rs` — calls `update_all_titles()` each frame.
2. `src/app/window_state/action_handlers/tab_bar.rs` — calls `tab.update_title()` directly
   on the "clear tab rename / revert to auto" path. This direct call site must also be
   updated to the new three-argument signature.

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

### `par-term-settings-ui/src/window_tab/mod.rs` — `keywords()` and `section_matches()`

The keyword list lives in `pub fn keywords()` in `window_tab/mod.rs` (not `sidebar.rs`).
Add to that slice:
```
"remote tab title", "ssh title", "remote host", "user at host", "remote format"
```

Also add the same keywords to the `"Tab Bar"` arm of `section_matches()` in the same file,
so that searching for "remote tab title" highlights the Tab Bar section.

---

## Files to Change

| File | Change |
|------|--------|
| `par-term-config/src/types/tab_bar.rs` | Add `RemoteTabTitleFormat` enum |
| `par-term-config/src/types/mod.rs` | Export `RemoteTabTitleFormat` |
| `par-term-config/src/lib.rs` | Re-export `RemoteTabTitleFormat` |
| `par-term-config/src/config/config_struct/mod.rs` | Add two new fields; add `RemoteTabTitleFormat` to `use crate::types::{…}` import |
| `par-term-config/src/config/config_struct/default_impl.rs` | Add `remote_tab_title_format` and `remote_tab_title_osc_priority` to `impl Default for Config` |
| `src/tab/profile_tracking.rs` | Update `update_title()` (3 params, single lock block), add `format_remote_title()` |
| `src/tab/manager.rs` | Update `update_all_titles()` signature + call sites |
| `src/app/render_pipeline/frame_setup.rs` | Update `update_all_titles()` call to pass new args |
| `src/app/window_state/action_handlers/tab_bar.rs` | Update direct `tab.update_title()` call to 3-arg signature |
| `par-term-settings-ui/src/window_tab/tab_bar_behavior.rs` | Add two UI controls |
| `par-term-settings-ui/src/window_tab/mod.rs` | Add keywords to `keywords()` and `section_matches()` |

---

## Backward Compatibility

- Existing configs without the new fields deserialize cleanly via `#[serde(default)]`.
- Default behaviour (`UserAtHost`, OSC priority on) is strictly better than the current
  accidental "last path component" display — a safe default change.

---

## Testing

- `make build` verifies no compile errors across the workspace.
- Manual: SSH to a remote host with shell integration installed; verify tab shows
  `user@host` by default, changes with config, and OSC title override works.
- Existing tests unaffected (no PTY-dependent changes).
