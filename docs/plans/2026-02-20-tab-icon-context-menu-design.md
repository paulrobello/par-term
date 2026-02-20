# Tab Icon via Context Menu — Design

**Date**: 2026-02-20
**Status**: Approved

## Goal

Allow users to set a custom icon on any tab via the right-click context menu, matching the existing ability to rename tabs and set tab colors.

## Data Model

- Add `custom_icon: Option<String>` to `Tab` struct
- Rendering priority: `custom_icon.or(profile_icon)` — custom always wins
- Custom icon persists until explicitly cleared, unaffected by profile auto-matching

## Context Menu UI

- Add "Set Icon" item in context menu (near rename and color options)
- Opens the existing Nerd Font icon picker (grid + free-text field)
- Show "Clear Icon" option when a custom icon is set
- New `TabBarAction::SetTabIcon(TabId, Option<String>)` variant

## Persistence

| Path | Change |
|------|--------|
| `TabSnapshot` (arrangements) | Add `custom_icon: Option<String>` |
| Arrangement capture | Capture `tab.custom_icon` |
| Arrangement restore | Restore `custom_icon` on tab |
| Tab duplicate | Copy `custom_icon` from source |

## Files to Modify

1. `src/tab/mod.rs` — add `custom_icon` field
2. `src/tab_bar_ui.rs` — action variant, context menu UI, rendering priority
3. `src/app/tab_ops.rs` — handle `SetTabIcon` action
4. `par-term-settings-ui/src/arrangements.rs` — `TabSnapshot` field
5. `src/arrangements/capture.rs` — capture `custom_icon`
6. `src/app/window_manager.rs` — restore `custom_icon`
7. `src/tab/manager.rs` — copy in `duplicate_tab_by_id`
