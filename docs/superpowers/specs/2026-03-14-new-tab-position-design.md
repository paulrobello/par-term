# New Tab Position — Design Spec

**Date:** 2026-03-14
**Status:** Approved

## Problem

All new tabs are always appended to the end of the tab bar, regardless of which tab is currently active. Many terminal users prefer new tabs to open adjacent to their current context, matching the behavior of browsers and other tabbed applications.

## Goal

Add a `new_tab_position` config option that controls where newly created tabs appear: at the end of the bar (current behavior, default) or immediately to the right of the currently active tab.

## Scope

### Respects `new_tab_position`
- Plain new tab (Cmd+T / Ctrl+Shift+T)
- Clicking "+" in the tab bar
- Profile picker — both "Default" and named profile selections
- `CustomActionConfig::NewTab` custom actions / snippets

These all flow through `WindowState::new_tab()` or `WindowState::open_profile()`, so updating those two functions covers every user-initiated new-tab path automatically.

### Does NOT respect `new_tab_position`
- Session undo / reopen closed tab — restores to original saved index
- Arrangement / session restore — restores to saved index
- Duplicate tab — always opens immediately right of source tab (intentional existing behavior)

## Config

Add `NewTabPosition` to `par-term-config/src/types/tab_bar.rs` (alongside `TabBarMode`, `TabBarPosition`, etc.):

```rust
/// Controls where newly created tabs are inserted in the tab bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NewTabPosition {
    /// Append to the end of the tab bar (default).
    #[default]
    End,
    /// Insert immediately to the right of the currently active tab.
    AfterActive,
}

impl NewTabPosition {
    /// Human-readable label for display in the settings UI.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::End => "End of tab bar",
            Self::AfterActive => "After active tab",
        }
    }

    /// All variants, in display order.
    pub fn all() -> &'static [Self] {
        &[Self::End, Self::AfterActive]
    }
}
```

Add field to `Config` struct in `par-term-config/src/config/config_struct/mod.rs`:

```rust
/// Where to insert new tabs in the tab bar.
#[serde(default)]
pub new_tab_position: NewTabPosition,
```

`config.yaml` serializes as `new_tab_position: end` or `new_tab_position: after_active`.

## Logic

The positioning logic lives **inside** `WindowState::new_tab()` and `WindowState::open_profile()`. No changes are needed to `snippet_actions.rs`, `action_handlers/tab_bar.rs`, or any other callers — they all get the behavior for free.

**Key invariant:** `tab_manager.active_tab_index()` must be captured **before** the `new_tab()` call, because the active tab switches to the new tab on creation.

**Pattern in `WindowState::new_tab()` (`lifecycle.rs`)**:

```rust
// Capture BEFORE creation — active switches to new tab after new_tab()
let prior_active_idx = self.tab_manager.active_tab_index();

match self.tab_manager.new_tab(...) {
    Ok(tab_id) => {
        // Reposition if configured — inside Ok arm, so only runs on successful creation
        if self.config.new_tab_position == NewTabPosition::AfterActive {
            if let Some(idx) = prior_active_idx {
                self.tab_manager.move_tab_to_index(tab_id, idx + 1);
            }
        }

        // ... existing tab bar resize + refresh logic unchanged ...
    }
    Err(e) => { ... }
}
```

**Same pattern in `WindowState::open_profile()` (`profile_ops.rs`)**, capturing before `tab_manager.new_tab_from_profile()` and moving inside its `Ok` arm.

The move is always inside the `Ok(tab_id)` arm, so it is naturally gated on successful tab creation. If `new_tab()` returns early (e.g., max-tabs limit), no move is attempted.

## Settings UI

In `par-term-settings-ui/src/window_tab/tab_bar_behavior.rs`, add a combo box near the existing `tab_inherit_cwd` checkbox using the `NewTabPosition::all()` and `display_name()` methods — matching the pattern used for `TabBarMode`, `TabBarPosition`, and other enums in the same file.

```
New Tab Position   [End of tab bar ▾]
                   [After active tab]
```

Set `settings.has_changes = true` and `*changes_this_frame = true` on change.

Add search keywords in `par-term-settings-ui/src/window_tab/mod.rs` → `keywords()` function:
- `"new tab position"`
- `"after active"`
- `"tab order"`
- `"insert tab"`

## Files Changed

| File | Change |
|------|--------|
| `par-term-config/src/types/tab_bar.rs` | Add `NewTabPosition` enum with derives, `display_name()`, `all()` |
| `par-term-config/src/config/config_struct/mod.rs` | Add `new_tab_position: NewTabPosition` field |
| `src/app/tab_ops/lifecycle.rs` | Capture prior index, conditional move in `new_tab()` Ok arm |
| `src/app/tab_ops/profile_ops.rs` | Capture prior index, conditional move in `open_profile()` Ok arm |
| `par-term-settings-ui/src/window_tab/tab_bar_behavior.rs` | Combo box UI control |
| `par-term-settings-ui/src/window_tab/mod.rs` | Search keywords in `keywords()` |

## Backward Compatibility

`#[serde(default)]` on the field and `#[default]` on `End` variant ensures existing configs without `new_tab_position` deserialize cleanly with `End` behavior — no migration needed.

## Testing

- `end`: new tabs always append (existing behavior unchanged)
- `after_active`: open 5 tabs, switch to tab 2, open new tab → new tab appears at position 3
- Profile picker (named profile): same positioning
- "+" button / Cmd+T: same positioning
- Custom action `NewTab` snippet: same positioning (flows through `new_tab()`)
- Reopen closed tab: always restores to original index regardless of setting
- Max-tabs limit hit: `new_tab()` returns early, `Ok` arm not reached, no move attempted
- No active tab (edge case): `prior_active_idx` is `None`, `if let Some` guard skips move → tab appends to end gracefully
