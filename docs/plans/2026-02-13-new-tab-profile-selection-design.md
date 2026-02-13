# Design: Profile Selection on New Tab Button (#129)

**Date**: 2026-02-13
**Issue**: https://github.com/paulrobello/par-term/issues/129

## Problem

Users must open the profile drawer to create a tab from a profile. The new tab button (`+`) always creates a default tab with no way to select a profile inline.

## Solution: Split Button with Profile Dropdown

### UI

The `+` button becomes a split button:

- **Left portion** (`+`): Clicking creates a default tab (current behavior preserved)
- **Right portion** (`▾` chevron): Clicking opens a profile dropdown popup

The dropdown contains:

1. **"Default"** entry at the top — uses the global terminal config (no profile)
2. All user profiles listed below, each showing its icon (if set) and name

### Dropdown Behavior

- Rendered as an egui popup anchored below (horizontal) or beside (vertical) the chevron
- Single click on an item opens a new tab with that profile and dismisses the dropdown
- Clicking outside or pressing Escape dismisses it
- Works in both horizontal and vertical tab bar layouts

### Keyboard Shortcut Configuration

New config option:

```yaml
new_tab_shortcut_shows_profiles: false  # default
```

- `false`: Cmd+T / Ctrl+Shift+T creates a default tab instantly (current behavior)
- `true`: The shortcut opens the profile dropdown popup instead

### New Types

```rust
// Added to TabBarAction enum
TabBarAction::NewTabWithProfile(ProfileId)
TabBarAction::ShowNewTabProfileMenu
```

### Files Modified

| File | Change |
|------|--------|
| `src/tab_bar_ui.rs` | Split button UI, dropdown rendering, new state fields |
| `src/app/window_state.rs` | Handle `NewTabWithProfile` and `ShowNewTabProfileMenu` actions |
| `src/app/tab_ops.rs` | Wire profile-based tab creation from tab bar action |
| `src/app/input_events.rs` | Conditional shortcut behavior based on config |
| `src/config.rs` | `new_tab_shortcut_shows_profiles` field |
| `src/settings_ui/` | Expose the new setting in appropriate tab |
| `src/settings_ui/sidebar.rs` | Add search keywords |
