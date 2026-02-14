# macOS Target Space Design

**Issue**: #140 — feat: open window in specific macOS Space
**Date**: 2026-02-13

## Summary

Add a config option to specify which macOS Space (virtual desktop) a window opens in. Uses private SkyLight Server (SLS) APIs via `dlopen`/`dlsym`, following the same pattern as `macos_blur.rs`.

## Config

```yaml
target_space: null  # null = OS decides, 1-16 = specific Space number
```

Field: `pub target_space: Option<u32>` with `#[serde(default)]`

## Architecture

### New File: `src/macos_space.rs`

Follows `macos_blur.rs` pattern:
- Inner module gated behind `#[cfg(target_os = "macos")]`
- Public `move_window_to_space(window, space_number)` with no-op on non-macOS
- Loads SLS functions via `dlopen`/`dlsym` with `OnceLock` caching
- Version-aware: detects macOS version at runtime

### SLS API Strategy

**macOS < 14.5** (legacy):
```
SLSMoveWindowsToManagedSpace(cid, window_list_cfarray, space_id)
```

**macOS 14.5+** (compat ID workaround):
```
SLSSpaceSetCompatID(cid, space_id, COMPAT_MAGIC)
SLSSetWindowListWorkspace(cid, &window_number, 1, COMPAT_MAGIC)
SLSSpaceSetCompatID(cid, space_id, 0)  // clear
```

### Supporting APIs

- `SLSMainConnectionID()` — get CGS connection
- `SLSGetSpaceManagementMode(cid, space_id)` — verify space exists
- `NSProcessInfo.operatingSystemVersion` — version detection
- `NSWindow.windowNumber` — get CGWindowID from winit window

### Integration Point

Called in `WindowManager::apply_window_positioning()` after monitor/edge positioning:
```rust
#[cfg(target_os = "macos")]
if let Some(space) = self.config.target_space {
    if let Err(e) = crate::macos_space::move_window_to_space(window, space) {
        log::warn!("Failed to move window to Space {}: {}", space, e);
    }
}
```

## Settings UI

Added to `window_tab.rs` below "Target monitor", same Auto checkbox + slider pattern:
- Auto checkbox (None = let OS decide)
- Slider 1-16 when not auto
- Only visible on macOS (`cfg!(target_os = "macos")`)
- Note: "Takes effect on next window creation"

## Sidebar Keywords

Add to Window tab: "space", "spaces", "mission control", "virtual desktop", "macos space"

## Error Handling

- SLS functions not found → log warning, skip silently
- Target Space doesn't exist → log warning, window stays on current Space
- All errors via `anyhow::Result`

## Files Changed

1. `src/macos_space.rs` — new file, SLS API wrapper
2. `src/config/mod.rs` — add `target_space: Option<u32>`
3. `src/app/window_manager.rs` — call `move_window_to_space()` in positioning
4. `src/settings_ui/window_tab.rs` — add UI control
5. `src/settings_ui/sidebar.rs` — add search keywords
6. `src/main.rs` or `src/lib.rs` — register `mod macos_space`
