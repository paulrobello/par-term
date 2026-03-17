# Per-Pane Title Tracking — Design Spec

**Date:** 2026-03-16
**Status:** Approved

---

## Problem

`Tab::update_title()` always reads the OSC title from `self.terminal` — the primary
pane's `TerminalManager`. In split-pane tabs (native or tmux), secondary panes have their
own `Arc<RwLock<TerminalManager>>` with their own OSC title state, but it is never
consulted. Whichever pane last sent an OSC sequence while it happened to be primary
"wins" the tab title, regardless of which pane is currently focused.

---

## Goal

Each pane stores its own last-known title. The tab bar always shows the focused pane's
title. Switching focus instantly shows the newly focused pane's last-known title without
waiting for a new OSC sequence.

---

## Decisions

| Question | Answer |
|---|---|
| Focus switch display | Immediately show last-known pane title |
| New pane with no OSC title yet | CWD fallback (last path component, `TabTitleMode::Auto`) |
| User-named tabs | Still freeze the whole tab title (no per-pane override) |
| Tmux panes | Included — same code path, each tmux pane has its own `TerminalManager` |

---

## Data Model

Add two fields to `Pane` (`src/pane/types/pane.rs`):

```rust
/// Title set by OSC sequences or CWD fallback (last-known; empty if never set)
pub title: String,
/// True when pane still has its default/fallback title (never set by OSC or CWD)
pub has_default_title: bool,
```

All four `Pane` constructors (`new`, `new_with_command`, `new_wrapping_terminal`,
`new_for_tmux`) initialize these as:
- `title: String::new()`
- `has_default_title: true`

The existing `tab.title` and `tab.has_default_title` remain. They are now *derived
output* updated from the focused pane each frame, not primary storage.

`tab.user_named` stays tab-level and continues to freeze the whole tab title (early exit
in `update_title()`).

---

## Title Update Logic

`Tab::update_title()` in `src/tab/profile_tracking.rs` is restructured as follows:

### Step 1 — Early exit if `user_named`
Unchanged. Returns immediately; `tab.title` is not touched.

### Step 2 — Snapshot focused pane ID (borrow-safety)
Before the mutable loop, capture the focused pane ID to avoid a borrow conflict
when reading it back after the loop:
```rust
let focused_id = self.pane_manager.as_ref().and_then(|pm| pm.focused_pane_id());
```

### Step 3 — Iterate all panes, update per-pane titles
For each `&mut Pane` from `pane_manager.all_panes_mut()`:
- Attempt `try_write()` on `pane.terminal`. Skip on contention (non-blocking contract).
- Same priority logic as current, applied per-pane:
  - Remote host → format from hostname/username/cwd (or OSC if `remote_osc_priority`)
  - Local with OSC title → use it
  - Local, no OSC, `TabTitleMode::Auto` → last CWD path component
  - Nothing → keep existing `pane.title` unchanged this frame
- Update `pane.has_default_title` accordingly.

The mutable borrow of `pane_manager` ends when the loop completes.

### Step 4 — Derive `tab.title` from focused pane (immutable re-borrow)
After the loop, re-borrow `pane_manager` immutably to look up the focused pane by the
snapshotted `focused_id`:
```rust
if let Some((focused_id, pm)) = focused_id.zip(self.pane_manager.as_ref()) {
    if let Some(pane) = pm.get_pane(focused_id) {
        self.title = pane.title.clone();
        self.has_default_title = pane.has_default_title;
    }
}
```

The tab-level hostname/CWD tracking (`detected_hostname`, `detected_cwd`) continues to
read from `self.terminal` — those drive profile auto-switching, not title display.

---

## `set_title()` — Single Write Point

`set_title()` is updated to write BOTH `tab.title` and the focused pane's `title` field.
This ensures that any explicit title override is not silently overwritten by the next
`update_title()` frame (which derives `tab.title` from `pane.title`):

```rust
pub fn set_title(&mut self, title: &str) {
    self.title = title.to_string();
    self.has_default_title = false;
    if let Some(pane) = self.pane_manager.as_mut().and_then(|pm| pm.focused_pane_mut()) {
        pane.title = title.to_string();
        pane.has_default_title = false;
    }
}
```

All existing direct `tab.title = ...` writes (see call-site table below) must be routed
through `set_title()` or handled as described.

---

## Direct `tab.title` Write-Site Policy

These sites currently write `tab.title` directly. Under the new design, `tab.title` is
derived from `pane.title` each frame, so un-synced direct writes would be overwritten
immediately. Policy for each:

| File | Lines | Current action | Fix |
|---|---|---|---|
| `profile_auto_switch.rs` | 60, 236, 355 | Restore `pre_profile_title` to `tab.title` | Use `tab.set_title(&original)` |
| `profile_auto_switch.rs` | 124, 296 | Apply profile name to `tab.title` | Use `tab.set_title(&name)` |
| `gateway_profile.rs` | 97 | Apply gateway profile name | Use `tab.set_title(&name)` |
| `arrangements.rs` | 159 | Restore user-named title (also sets `user_named=true`) | Use `tab.set_title(&user_title)` — `user_named=true` keeps `update_title` from overwriting |
| `window_session.rs` | 123 | Restore session title (also sets `user_named=true`) | Use `tab.set_title(&user_title)` — same |
| `tab_reopen.rs` | 113 | Restore reopened tab title | Use `tab.set_title(&info.title)` |
| `tab_bar.rs` | 81 | Apply user-typed name (`RenameTab`) | Use `tab.set_title(&name)` |
| `tab_bar.rs` | 72–73 | Clear user name (`user_named=false`, `has_default_title=true`) | Also set focused pane: `pane.title = ""; pane.has_default_title = true;` |

The last row (`tab_bar.rs` clear case) is the only one that cannot go through `set_title()`
as-is since it resets to default rather than setting a specific title. Add a small helper
or inline the pane reset alongside the existing field writes.

---

## `clear_auto_profile()` — Title Restore Fix

In `profile_tracking.rs`, `clear_auto_profile()` currently restores `tab.title` directly:

```rust
if let Some(original) = self.profile.pre_profile_title.take() {
    self.title = original;  // ← will be overwritten next frame
}
```

Change to:
```rust
if let Some(original) = self.profile.pre_profile_title.take() {
    self.set_title(&original);  // writes both tab.title and pane.title
}
```

`pre_profile_title` is saved from `tab.title.clone()` before overwriting — since
`tab.title` is kept in sync with the focused pane each frame, the save value is correct.
No change needed to the save side.

---

## `set_default_title()` — Fix Required

Under the new design, Step 4 of `update_title()` derives `tab.title` from `pane.title`
every frame. A brand-new pane has `pane.title = ""`. If `set_default_title()` only writes
`tab.title = "Tab N"`, the next frame immediately overwrites it with `""` from the pane.

**Fix:** `set_default_title()` must also write the pane's `title` field for all panes
that still have `has_default_title = true`:

```rust
pub fn set_default_title(&mut self, tab_number: usize) {
    if self.has_default_title {
        let title = format!("Tab {}", tab_number);
        self.title = title.clone();
        if let Some(pm) = self.pane_manager.as_mut() {
            for pane in pm.all_panes_mut() {
                if pane.has_default_title {
                    pane.title = title.clone();
                }
            }
        }
    }
}
```

Only panes with `has_default_title = true` are written — panes that have already set a
real title (e.g., "vim") are not overwritten. On the next frame, Step 3 of
`update_title()` finds no OSC or CWD data, leaves `pane.title = "Tab N"` unchanged, and
Step 4 derives `tab.title = "Tab N"` correctly.

---

## Focus Switch

No special code. `update_title()` runs every frame. When focus changes, the next frame
reads the new focused pane's stored `title` field. One-frame latency, imperceptible.

---

## Tmux Panes

No special handling. Each tmux pane already has its own `Arc<RwLock<TerminalManager>>`.
The all-pane iteration in `update_title()` covers them automatically.

---

## Files Changed

| File | Change |
|---|---|
| `src/pane/types/pane.rs` | Add `title` + `has_default_title` fields; initialize in all 4 constructors |
| `src/tab/profile_tracking.rs` | Restructure `update_title()` (two-phase borrow); update `set_title()` to sync pane; fix `clear_auto_profile()` |
| `src/app/tab_ops/profile_auto_switch.rs` | Route 5 direct `tab.title` writes through `set_title()` |
| `src/app/tmux_handler/gateway_profile.rs` | Route 1 direct `tab.title` write through `set_title()` |
| `src/app/window_manager/arrangements.rs` | Route 1 direct `tab.title` write through `set_title()` |
| `src/app/window_manager/window_session.rs` | Route 1 direct `tab.title` write through `set_title()` |
| `src/app/tab_ops/tab_reopen.rs` | Route 1 direct `tab.title` write through `set_title()` |
| `src/app/window_state/action_handlers/tab_bar.rs` | Route `RenameTab` write through `set_title()`; inline pane reset for clear case |
| `src/tab/manager.rs` | Update `set_default_title()` to also write pane titles for all default-titled panes |

**Tmux notification callers (no code change needed):** The following files already call
`tab.set_title()` rather than writing `tab.title` directly. Once `set_title()` is updated
to sync the pane, these callers automatically gain per-pane title sync with no changes:
`notifications/layout_new_tab.rs`, `notifications/session.rs`,
`notifications/flow_control.rs`, `notifications/window.rs`, `notifications/output.rs`.

---

## Out of Scope

- Displaying per-pane titles within split-pane borders (future feature)
- Per-pane `user_named` (tab-level freeze is sufficient)
- Changes to profile auto-switching logic (reads primary terminal, unchanged)
