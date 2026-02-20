# Tab Naming & Title Mode Design

## Problem

Tab titles auto-update from shell integration CWD with no way to disable this.
Users cannot manually rename tabs. Manual names and custom colors are not
persisted across sessions.

## Design

### 1. Global Tab Title Mode

New enum `TabTitleMode` with two variants:

- **`auto`** (default) — OSC title first, then CWD from shell integration, then
  keep existing "Tab N"
- **`osc_only`** — only update from explicit OSC escape sequences; never
  auto-set from CWD

Config field: `tab_title_mode: TabTitleMode` with `#[serde(default)]`.

Settings UI: dropdown in Window > Tab Bar section.

### 2. Manual Tab Rename

Right-click context menu gains a "Rename Tab" item. Clicking it shows an inline
`TextEdit` field pre-filled with the current title. Behavior:

- **Enter or focus-lost with non-empty text** — sets `tab.title`, sets
  `tab.user_named = true`. Title becomes static (never auto-updated).
- **Enter or focus-lost with blank text** — sets `tab.user_named = false`,
  reverts to global `tab_title_mode` behavior.
- **Escape** — cancels rename, no changes.

New field on `Tab`: `user_named: bool` (default `false`).

In `update_title()`: if `user_named` is `true`, return immediately (no-op).

### 3. Session Persistence

`SessionTab` gains two new optional fields:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub custom_color: Option<[u8; 3]>,

#[serde(default, skip_serializing_if = "Option::is_none")]
pub user_title: Option<String>,
```

`TabSnapshot` (arrangements) gains the same two fields.

**Capture**: if `tab.user_named`, save `user_title = Some(tab.title.clone())`.
Always save `custom_color` if set.

**Restore**: if `user_title` is `Some`, set `tab.title` and `tab.user_named = true`.
If `custom_color` is `Some`, set `tab.custom_color`.

### 4. Profile `tab_name` Interaction

Tabs created from profiles with `tab_name` set already get
`has_default_title = false`. These will also set `user_named = true`, making
them static. Users can still clear the name (blank rename) to revert to auto.

## Files Affected

| File | Change |
|------|--------|
| `par-term-config/src/config.rs` | Add `TabTitleMode` enum, `tab_title_mode` field |
| `par-term-config/src/defaults.rs` | Default for `tab_title_mode` |
| `src/tab/mod.rs` | Add `user_named` field, update `update_title()` logic |
| `src/tab_bar_ui.rs` | Add `RenameTab` action, rename UI in context menu |
| `src/app/window_state.rs` | Handle `RenameTab` action |
| `src/session/mod.rs` | Add `custom_color`, `user_title` to `SessionTab` |
| `src/session/capture.rs` | Capture `user_named` tabs and custom colors |
| `src/session/restore.rs` | Restore user titles and custom colors |
| `src/app/window_manager.rs` | Apply restored user titles and colors |
| `par-term-settings-ui/src/arrangements.rs` | Add fields to `TabSnapshot` |
| `src/arrangements/capture.rs` | Capture user titles and colors |
| `par-term-settings-ui/src/window_tab.rs` | Add `tab_title_mode` dropdown |
| `par-term-settings-ui/src/sidebar.rs` | Add search keywords |
| `docs/TABS.md` | Document new features |
