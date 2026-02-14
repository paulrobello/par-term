# macOS Private APIs for Space (Virtual Desktop) Management

**Research Date**: 2026-02-13
**Last Updated**: 2026-02-13
**Target Platform**: macOS (Sonoma 14.5+, Sequoia 15.x)
**Primary Sources**: Yabai, Hammerspoon, CGSInternal

## Overview

This document provides comprehensive technical information on macOS private APIs for managing Spaces (virtual desktops/Mission Control). These APIs enable programmatic control over window placement across different Spaces, which is critical for terminal emulators, window managers, and productivity tools.

**Key Takeaway**: Apple removed public Space management APIs in macOS 10.7, forcing developers to use private CoreGraphics/SkyLight APIs (SLS/CGS) to achieve Space-aware window management. These private APIs work but carry App Store rejection risks and version compatibility concerns.

## API Evolution

### Historical Context

- **Pre-10.7**: Public APIs existed for Space management
- **10.7 (Lion)**: Apple made Space APIs private, removing official support
- **10.14 (Mojave)**: ProcessSerialNumber APIs deprecated
- **14.5 (Sonoma)**: Major API changes requiring `SLSSpaceSetCompatID` workaround
- **15.x (Sequoia)**: Current stable APIs continue to work with Sonoma 14.5+ patterns

### Current Status (2026)

The private SLS (SkyLight Server) APIs remain functional on modern macOS but require platform-version-specific handling. macOS 14.5+ introduced breaking changes requiring compatibility ID workarounds.

## Core API Functions

### Connection Management

**`SLSMainConnectionID()`**
```c
extern int SLSMainConnectionID(void);
```
- Returns the main connection ID to the Window Server
- Called once at application startup, stored globally
- Required as first parameter to all SLS functions

### Space Information

**`SLSManagedDisplayGetCurrentSpace()`**
```c
extern uint64_t SLSManagedDisplayGetCurrentSpace(int cid, CFStringRef display_uuid);
```
- Returns the currently active Space ID for a display
- `display_uuid`: UUID string from NSScreen (or main display UUID)
- Returns `uint64_t` Space ID

**`SLSCopyManagedDisplayForSpace()`**
```c
extern CFStringRef SLSCopyManagedDisplayForSpace(int cid, uint64_t sid);
```
- Returns the UUID of the display containing a Space
- Caller must release the returned `CFStringRef`

**`SLSCopyManagedDisplaySpaces()`**
```c
extern CFArrayRef SLSCopyManagedDisplaySpaces(int cid);
```
- Returns array of all Spaces across all displays
- Structure: Array of display dictionaries with Space arrays
- Caller must release the returned `CFArrayRef`

**`SLSSpaceGetType()`**
```c
extern int SLSSpaceGetType(int cid, uint64_t sid);
```
- Returns Space type: 0 = user Space, 4 = fullscreen Space
- Useful for filtering out fullscreen app Spaces

**`SLSSpaceCopyName()`**
```c
extern CFStringRef SLSSpaceCopyName(int cid, uint64_t sid);
```
- Returns the user-assigned name for a Space (if set in Mission Control)
- Caller must release the returned `CFStringRef`

**`SLSGetActiveSpace()`**
```c
extern uint64_t SLSGetActiveSpace(int cid);
```
- Returns the globally active Space ID
- Equivalent to current Space on the active display

### Window Management

**`SLSCopyWindowsWithOptionsAndTags()`**
```c
extern CFArrayRef SLSCopyWindowsWithOptionsAndTags(
    int cid,
    uint32_t owner,
    CFArrayRef spaces,
    uint32_t options,
    uint64_t *set_tags,
    uint64_t *clear_tags
);
```
- Retrieves window IDs for specific Spaces
- `owner`: 0 for all windows, or specific connection ID
- `spaces`: Array of Space IDs to query (or NULL for current Space)
- `options`: Filtering options (0x2 for on-screen windows)
- Returns array of `NSNumber` objects containing window IDs
- Caller must release the returned `CFArrayRef`

**`SLSCopySpacesForWindows()`**
```c
extern CFArrayRef SLSCopySpacesForWindows(
    int cid,
    int selector,
    CFArrayRef window_list
);
```
- Returns Space IDs containing the specified windows
- `selector`: 0x7 (common value, purpose unclear)
- `window_list`: Array of window IDs to query
- Returns array of Space ID arrays (one per window)

### Moving Windows Between Spaces

#### Modern API (macOS 14.5+, Sequoia)

**`SLSSpaceSetCompatID()` + `SLSSetWindowListWorkspace()`**

Since macOS Sonoma 14.5, the old `SLSMoveWindowsToManagedSpace` requires a compatibility ID workaround:

```c
extern CGError SLSSpaceSetCompatID(int cid, uint64_t sid, int workspace);
extern CGError SLSSetWindowListWorkspace(int cid, uint32_t *window_list, int window_count, int workspace);
```

**Usage Pattern (from Hammerspoon)**:
```c
// Magic compatibility ID (ASCII 'yabe' - used by yabai)
const int COMPAT_ID = 0x79616265;

// Temporary compatibility ID assignment
SLSSpaceSetCompatID(g_connection, target_space_id, COMPAT_ID);

// Move window(s) to the compatibility workspace
uint32_t window_id = 12345;
SLSSetWindowListWorkspace(g_connection, &window_id, 1, COMPAT_ID);

// Clear compatibility ID
SLSSpaceSetCompatID(g_connection, target_space_id, 0x0);
```

**Why This Works**:
- macOS 14.5+ requires explicit workspace ID matching
- The compatibility ID (`0x79616265` = "yabe") acts as a temporary marker
- This three-step dance bypasses the new Space isolation checks

#### Legacy API (macOS < 14.5)

**`SLSMoveWindowsToManagedSpace()`**
```c
extern void SLSMoveWindowsToManagedSpace(int cid, CFArrayRef window_list, uint64_t sid);
```
- `window_list`: CFArray of NSNumber objects (window IDs)
- `sid`: Target Space ID
- No return value (void)

**Example**:
```c
uint32_t window_id = 12345;
NSArray *windows = @[@(window_id)];
SLSMoveWindowsToManagedSpace(g_connection, (__bridge CFArrayRef)windows, target_space_id);
```

#### Platform-Agnostic Implementation

```c
void move_window_to_space(int cid, uint32_t wid, uint64_t sid) {
    if (is_macos_14_5_or_newer()) {
        // Modern approach
        SLSSpaceSetCompatID(cid, sid, 0x79616265);
        SLSSetWindowListWorkspace(cid, &wid, 1, 0x79616265);
        SLSSpaceSetCompatID(cid, sid, 0x0);
    } else {
        // Legacy approach
        NSArray *windows = @[@(wid)];
        SLSMoveWindowsToManagedSpace(cid, (__bridge CFArrayRef)windows, sid);
    }
}
```

### Process-Level Space Assignment

**`SLSProcessAssignToSpace()`**
```c
extern CGError SLSProcessAssignToSpace(int cid, pid_t pid, uint64_t sid);
```
- Assigns a process to a specific Space
- New windows from that process will open on the assigned Space

**`SLSProcessAssignToAllSpaces()`**
```c
extern CGError SLSProcessAssignToAllSpaces(int cid, pid_t pid);
```
- Makes process windows appear on all Spaces
- Equivalent to "Options → All Desktops" in Dock right-click menu

### Space Visibility

**`SLSShowSpaces()` / `SLSHideSpaces()`**
```c
extern void SLSShowSpaces(int cid, CFArrayRef space_list);
extern void SLSHideSpaces(int cid, CFArrayRef space_list);
```
- Controls Space visibility in Mission Control
- `space_list`: Array of Space IDs
- Used for programmatic Space hiding/showing

### Window Positioning

**`SLSGetWindowBounds()`**
```c
extern CGError SLSGetWindowBounds(int cid, uint32_t wid, CGRect *frame);
```
- Retrieves window frame in screen coordinates

**`SLSMoveWindow()`**
```c
extern CGError SLSMoveWindow(int cid, uint32_t wid, CGPoint *point);
```
- Moves window to specified screen coordinates

**`SLSOrderWindow()`**
```c
extern CGError SLSOrderWindow(int cid, uint32_t wid, int mode, uint32_t rel_wid);
```
- Changes window Z-order
- `mode`: -1 (below), 0 (out/remove), 1 (above)
- `rel_wid`: Reference window ID (or 0 for absolute ordering)

## Type Definitions

```c
// Connection to Window Server
typedef int CGSConnectionID;

// Window identifier
typedef uint32_t CGSWindowID;

// Space identifier (64-bit on modern macOS)
typedef uint64_t CGSSpaceID;

// Display UUID
typedef CFStringRef CGSDisplayUUID;

// Standard CoreGraphics error type
typedef int32_t CGError;
```

## Rust FFI Implementation

### Using objc2 and Core Foundation

The `objc2` crate provides CoreGraphics bindings but does **not** include SLS private APIs. You must declare them manually.

```rust
use core_foundation::{
    array::{CFArray, CFArrayRef},
    base::{CFType, TCFType},
    number::CFNumber,
    string::{CFString, CFStringRef},
};
use core_graphics::geometry::{CGPoint, CGRect};

// Link against CoreGraphics framework (contains SLS)
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    // Connection
    fn SLSMainConnectionID() -> i32;

    // Space queries
    fn SLSManagedDisplayGetCurrentSpace(cid: i32, display_uuid: CFStringRef) -> u64;
    fn SLSCopyManagedDisplayForSpace(cid: i32, sid: u64) -> CFStringRef;
    fn SLSCopyManagedDisplaySpaces(cid: i32) -> CFArrayRef;
    fn SLSSpaceGetType(cid: i32, sid: u64) -> i32;
    fn SLSGetActiveSpace(cid: i32) -> u64;

    // Window queries
    fn SLSCopyWindowsWithOptionsAndTags(
        cid: i32,
        owner: u32,
        spaces: CFArrayRef,
        options: u32,
        set_tags: *mut u64,
        clear_tags: *mut u64,
    ) -> CFArrayRef;

    fn SLSCopySpacesForWindows(
        cid: i32,
        selector: i32,
        window_list: CFArrayRef,
    ) -> CFArrayRef;

    // Window movement (legacy)
    fn SLSMoveWindowsToManagedSpace(cid: i32, window_list: CFArrayRef, sid: u64);

    // Window movement (modern)
    fn SLSSpaceSetCompatID(cid: i32, sid: u64, workspace: i32) -> i32;
    fn SLSSetWindowListWorkspace(cid: i32, window_list: *mut u32, count: i32, workspace: i32) -> i32;

    // Process assignment
    fn SLSProcessAssignToSpace(cid: i32, pid: i32, sid: u64) -> i32;
    fn SLSProcessAssignToAllSpaces(cid: i32, pid: i32) -> i32;
}

/// Get the Window Server connection ID
pub fn get_connection_id() -> i32 {
    unsafe { SLSMainConnectionID() }
}

/// Get current Space ID for main display
pub fn get_current_space(cid: i32) -> Option<u64> {
    unsafe {
        // Use main display UUID (CGDirectMainDisplay's UUID)
        // For simplicity, pass NULL to get current active space
        let sid = SLSGetActiveSpace(cid);
        if sid > 0 {
            Some(sid)
        } else {
            None
        }
    }
}

/// Move a window to a target Space (modern API)
pub fn move_window_to_space_modern(cid: i32, window_id: u32, space_id: u64) -> Result<(), i32> {
    unsafe {
        const COMPAT_ID: i32 = 0x79616265; // 'yabe'

        // Step 1: Set compatibility ID
        let err = SLSSpaceSetCompatID(cid, space_id, COMPAT_ID);
        if err != 0 {
            return Err(err);
        }

        // Step 2: Move window
        let mut wid = window_id;
        let err = SLSSetWindowListWorkspace(cid, &mut wid, 1, COMPAT_ID);
        if err != 0 {
            SLSSpaceSetCompatID(cid, space_id, 0); // Cleanup
            return Err(err);
        }

        // Step 3: Clear compatibility ID
        SLSSpaceSetCompatID(cid, space_id, 0);

        Ok(())
    }
}

/// Move a window to a target Space (legacy API)
pub fn move_window_to_space_legacy(cid: i32, window_id: u32, space_id: u64) {
    unsafe {
        let num = CFNumber::from(window_id as i64);
        let array = CFArray::from_CFTypes(&[num.as_CFType()]);
        SLSMoveWindowsToManagedSpace(cid, array.as_concrete_TypeRef(), space_id);
    }
}

/// Platform-agnostic window movement
pub fn move_window_to_space(cid: i32, window_id: u32, space_id: u64) -> Result<(), i32> {
    // Check macOS version (implement version detection)
    let is_14_5_or_newer = check_macos_version_14_5_or_newer();

    if is_14_5_or_newer {
        move_window_to_space_modern(cid, window_id, space_id)
    } else {
        move_window_to_space_legacy(cid, window_id, space_id);
        Ok(())
    }
}

/// Helper: Check if macOS 14.5+
fn check_macos_version_14_5_or_newer() -> bool {
    use std::process::Command;

    if let Ok(output) = Command::new("sw_vers").arg("-productVersion").output() {
        if let Ok(version_str) = String::from_utf8(output.stdout) {
            // Parse version string "14.5.0" -> (14, 5, 0)
            let parts: Vec<&str> = version_str.trim().split('.').collect();
            if let (Some(major), Some(minor)) = (parts.get(0), parts.get(1)) {
                if let (Ok(maj), Ok(min)) = (major.parse::<u32>(), minor.parse::<u32>()) {
                    return maj > 14 || (maj == 14 && min >= 5);
                }
            }
        }
    }

    // Default to modern API on unknown version
    true
}
```

### Getting Window IDs

To move windows, you need their window IDs. Use the Accessibility API or NSWindow methods:

```rust
use cocoa::appkit::NSWindow;
use cocoa::base::id;
use objc::{msg_send, sel, sel_impl};

/// Get window ID from NSWindow
pub fn get_window_id(ns_window: id) -> u32 {
    unsafe {
        let window_number: i32 = msg_send![ns_window, windowNumber];
        window_number as u32
    }
}
```

## Alternative Public API Approaches

### NSWindow.collectionBehavior

`NSWindow.CollectionBehavior.moveToActiveSpace` is a **public API** from macOS 10.5+:

```swift
window.collectionBehavior.insert(.moveToActiveSpace)
```

**Behavior**: When the window becomes active (user clicks it), macOS automatically moves it to the current Space instead of switching Spaces.

**Limitations**:
- Only works when window is activated by user
- Cannot programmatically move windows to specific Spaces
- Does not provide Space queries or multi-Space control

**Rust Equivalent**:
```rust
use cocoa::appkit::{NSWindow, NSWindowCollectionBehavior};
use cocoa::base::id;
use objc::{msg_send, sel, sel_impl};

pub fn set_move_to_active_space(window: id) {
    unsafe {
        let behavior: NSWindowCollectionBehavior = msg_send![window, collectionBehavior];
        let new_behavior = behavior | NSWindowCollectionBehavior::NSWindowCollectionBehaviorMoveToActiveSpace;
        let _: () = msg_send![window, setCollectionBehavior: new_behavior];
    }
}
```

### CGEvent-Based Workaround (App Store Safe)

From the article [Accessibility, Windows, and Spaces in OS X](https://ianyh.com/blog/accessibility-windows-and-spaces-in-os-x/), an App Store-compatible approach uses synthetic events:

**Concept**: Simulate user actions (mouse grab + Control+Arrow keys) to move windows.

**Advantages**:
- No private APIs (App Store safe)
- Works across macOS versions

**Disadvantages**:
- Requires Accessibility permissions
- Visually intrusive (brief cursor movement, Space animation)
- Unreliable with some applications (e.g., Xcode)
- Slow (~500ms for animation)

**Implementation Summary**:
1. Move cursor to window's title bar (near green zoom button)
2. Post mouse-down event to "grab" window
3. Post Control+Arrow keyboard events to switch Space (window follows)
4. Post mouse-up event to release window

This approach is suitable for user-facing tools but not ideal for background automation or terminal emulators.

## How Terminal Emulators Handle Spaces

### Alacritty
- **No native Space management**: Relies on external tools like Hammerspoon
- Users configure Hammerspoon scripts to move Alacritty windows to specific Spaces
- Example: [Guake-style terminal with Alacritty + Hammerspoon](https://world.hey.com/jonash/alacritty-drop-down-guake-quake-style-terminal-setup-on-macos-6eef7d73)

### Kitty
- **No documented Space API usage**
- Focuses on standard NSWindow behaviors
- Users rely on macOS window management or third-party tools

### iTerm2
- **No public Space management features**
- May use private APIs internally (closed source, unconfirmed)
- Users typically use Hammerspoon or Aerospace for advanced Space control

### Ghostty
- **SwiftUI + Native macOS Integration**: Ghostty is a native Swift app with full macOS windowing support
- **No apparent Space API usage**: GitHub discussions show users pairing Ghostty with Aerospace window manager
- **Related Issues**:
  - [Quick Terminal not visible in all Aerospace workspaces](https://github.com/ghostty-org/ghostty/discussions/3512)
  - [Multi-monitor window placement issues](https://github.com/ghostty-org/ghostty/discussions/8673)
- **Architecture**: Main entry point is Swift, links to `libghostty` (likely Zig/C++) for terminal emulation
- **Takeaway**: Even modern native terminals don't implement custom Space management—they rely on external window managers

### Common Pattern
**Terminal emulators avoid implementing Space management directly**. Instead:
- Provide standard NSWindow behaviors
- Let users configure external tools (Hammerspoon, Aerospace, Yabai)
- Focus on terminal emulation, not window management

## Third-Party Window Managers

### Yabai
- **C implementation** using SLS private APIs
- Full source available: [koekeishiya/yabai](https://github.com/koekeishiya/yabai)
- Requires partial SIP disable for full functionality
- Uses `SLSMoveWindowsToManagedSpace` (pre-14.5) and compatibility ID workaround (14.5+)
- Primary reference: [`src/misc/extern.h`](https://github.com/asmvik/yabai/blob/master/src/misc/extern.h)

### Hammerspoon
- **Lua-scriptable macOS automation**
- `hs.spaces` module wraps SLS APIs
- Full source: [Hammerspoon/hammerspoon](https://github.com/Hammerspoon/hammerspoon)
- Reference implementation: [`extensions/spaces/libspaces.m`](https://github.com/Hammerspoon/hammerspoon/blob/master/extensions/spaces/libspaces.m)
- **Sequoia compatibility**: [Issue #3698](https://github.com/Hammerspoon/hammerspoon/issues/3698) shows macOS 15.0 initially broke `moveWindowToSpace`, later fixed

### Aerospace
- **Rust-based** tiling window manager inspired by i3
- Uses SLS APIs (source not directly linked in search results)
- Actively maintained for modern macOS
- Ghostty users commonly pair Aerospace for workspace management

## Risks and Limitations

### App Store Rejection
**Risk Level**: **HIGH**

Using private APIs (SLS/CGS) will result in **automatic rejection** from the Mac App Store. Apple scans for private API usage during review.

**Workarounds**:
- Distribute outside App Store (direct download, Homebrew)
- Use CGEvent-based workaround (slow, requires Accessibility permissions)
- Use `NSWindow.collectionBehavior.moveToActiveSpace` (limited functionality)

### API Stability
**Risk Level**: **MEDIUM**

Private APIs can change without notice:
- **macOS 14.5 (Sonoma)**: Broke `SLSMoveWindowsToManagedSpace`, required compatibility ID workaround
- **macOS 15.0 (Sequoia)**: Initially broke Hammerspoon's `moveWindowToSpace`, later stabilized
- **Future versions**: No guarantees of continued compatibility

**Mitigation**:
- Implement platform version detection
- Provide fallback behaviors
- Monitor window manager projects (Yabai, Hammerspoon) for compatibility updates
- Abstract SLS calls behind a compatibility layer

### System Integrity Protection (SIP)
**Risk Level**: **MEDIUM**

Some Space operations (e.g., Yabai's scripting addition injection into Dock.app) require **partial SIP disable**. Moving windows to Spaces does **not** require SIP changes—only reading/writing Space metadata requires elevated access.

**SIP-Free Operations**:
- `SLSMoveWindowsToManagedSpace` (and modern equivalents)
- `SLSProcessAssignToSpace`
- `SLSGetActiveSpace`
- Window queries

**SIP-Protected Operations**:
- Injecting code into Dock.app
- Modifying Space creation/deletion
- Advanced Space reordering

### Process Permissions
**Risk Level**: **LOW**

No special entitlements needed for basic Space operations. Accessibility permissions are **not** required for SLS APIs (unlike the CGEvent workaround).

### Deprecation of ProcessSerialNumber
**Risk Level**: **LOW** (No longer relevant)

Legacy function `CGSGetConnectionIDForPSN()` used `ProcessSerialNumber`, which is deprecated. Modern code uses `SLSMainConnectionID()` (no PSN needed) or process IDs (PIDs).

## macOS Version Compatibility Matrix

| macOS Version | `SLSMoveWindowsToManagedSpace` | Modern API (CompatID) | Notes |
|---------------|--------------------------------|-----------------------|-------|
| 10.13-14.4 | ✅ Works | ❌ Not needed | Use legacy API directly |
| 14.5-14.x | ⚠️ Requires workaround | ✅ Required | Must use 3-step CompatID pattern |
| 15.x (Sequoia) | ⚠️ Requires workaround | ✅ Required | Same as Sonoma 14.5+ |
| Future | ❓ Unknown | ❓ Unknown | Monitor Yabai/Hammerspoon for updates |

## Best Practices

### For Terminal Emulators

1. **Don't implement Space management unless absolutely necessary**
   - Most users prefer external window managers (Aerospace, Yabai, Hammerspoon)
   - Focus on terminal emulation quality

2. **If implementing, use a compatibility layer**
   - Abstract SLS APIs behind a versioned abstraction
   - Gracefully degrade on API failures
   - Provide user-facing errors when Space operations fail

3. **Expose public APIs where possible**
   - Allow users to set `NSWindow.collectionBehavior` via config
   - Document integration with external window managers
   - Provide window management hooks for scripting (if supported)

4. **Consider distribution model**
   - If targeting App Store: **Do not use SLS APIs**
   - If distributing via Homebrew/direct: Private APIs are viable

### For Development

1. **Version Detection is Critical**
   ```rust
   fn check_macos_version_14_5_or_newer() -> bool {
       // Parse sw_vers output or use NSProcessInfo
   }
   ```

2. **Handle Errors Gracefully**
   ```rust
   match move_window_to_space(cid, wid, sid) {
       Ok(_) => println!("Window moved successfully"),
       Err(e) => eprintln!("Failed to move window: CGError {}", e),
   }
   ```

3. **Test Across macOS Versions**
   - Use VMs or physical hardware for testing
   - Monitor Hammerspoon/Yabai issue trackers for breakage reports

4. **Abstract SLS APIs**
   ```rust
   trait SpaceManager {
       fn move_window_to_space(&self, window_id: u32, space_id: u64) -> Result<(), String>;
   }

   struct SLSSpaceManager { cid: i32 }
   struct NoOpSpaceManager; // Fallback for non-macOS
   ```

## Debugging and Testing

### Logging SLS Calls

Enable CoreGraphics debug logging:
```bash
defaults write -g CGDebugOptions -int 16
# Restart application
```

### Inspecting Spaces

Use Hammerspoon console:
```lua
hs.spaces.allSpaces()  -- List all Spaces
hs.spaces.activeSpaces()  -- Currently active Spaces per display
hs.spaces.windowsForSpace(space_id)  -- Windows on a Space
```

### Verifying Window Movement

Check Space ownership after moving:
```rust
let spaces = SLSCopySpacesForWindows(cid, 0x7, window_array);
// Parse returned CFArray to verify window is on target Space
```

## Complete Example: Moving par-term Window to Specific Space

```rust
#[cfg(target_os = "macos")]
mod macos_spaces {
    use core_foundation::array::{CFArray, CFArrayRef};
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFStringRef;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn SLSMainConnectionID() -> i32;
        fn SLSGetActiveSpace(cid: i32) -> u64;
        fn SLSSpaceSetCompatID(cid: i32, sid: u64, workspace: i32) -> i32;
        fn SLSSetWindowListWorkspace(cid: i32, window_list: *mut u32, count: i32, workspace: i32) -> i32;
        fn SLSMoveWindowsToManagedSpace(cid: i32, window_list: CFArrayRef, sid: u64);
    }

    pub struct SpaceManager {
        connection_id: i32,
    }

    impl SpaceManager {
        pub fn new() -> Self {
            Self {
                connection_id: unsafe { SLSMainConnectionID() },
            }
        }

        pub fn current_space(&self) -> u64 {
            unsafe { SLSGetActiveSpace(self.connection_id) }
        }

        pub fn move_window_to_space(&self, window_id: u32, space_id: u64) -> Result<(), i32> {
            if Self::is_macos_14_5_or_newer() {
                self.move_window_modern(window_id, space_id)
            } else {
                self.move_window_legacy(window_id, space_id);
                Ok(())
            }
        }

        fn move_window_modern(&self, window_id: u32, space_id: u64) -> Result<(), i32> {
            unsafe {
                const COMPAT_ID: i32 = 0x79616265;

                let err = SLSSpaceSetCompatID(self.connection_id, space_id, COMPAT_ID);
                if err != 0 {
                    return Err(err);
                }

                let mut wid = window_id;
                let err = SLSSetWindowListWorkspace(self.connection_id, &mut wid, 1, COMPAT_ID);
                if err != 0 {
                    SLSSpaceSetCompatID(self.connection_id, space_id, 0);
                    return Err(err);
                }

                SLSSpaceSetCompatID(self.connection_id, space_id, 0);
                Ok(())
            }
        }

        fn move_window_legacy(&self, window_id: u32, space_id: u64) {
            unsafe {
                let num = CFNumber::from(window_id as i64);
                let array = CFArray::from_CFTypes(&[num.as_CFType()]);
                SLSMoveWindowsToManagedSpace(
                    self.connection_id,
                    array.as_concrete_TypeRef(),
                    space_id
                );
            }
        }

        fn is_macos_14_5_or_newer() -> bool {
            use std::process::Command;

            if let Ok(output) = Command::new("sw_vers").arg("-productVersion").output() {
                if let Ok(version_str) = String::from_utf8(output.stdout) {
                    let parts: Vec<&str> = version_str.trim().split('.').collect();
                    if let (Some(major), Some(minor)) = (parts.get(0), parts.get(1)) {
                        if let (Ok(maj), Ok(min)) = (major.parse::<u32>(), minor.parse::<u32>()) {
                            return maj > 14 || (maj == 14 && min >= 5);
                        }
                    }
                }
            }

            true // Default to modern API
        }
    }
}

#[cfg(target_os = "macos")]
fn example_usage() {
    use cocoa::appkit::NSWindow;
    use cocoa::base::id;
    use objc::{msg_send, sel, sel_impl};

    // Get window ID from winit or NSWindow
    let ns_window: id = /* your NSWindow */;
    let window_number: i32 = unsafe { msg_send![ns_window, windowNumber] };
    let window_id = window_number as u32;

    // Initialize Space manager
    let manager = macos_spaces::SpaceManager::new();

    // Get target Space ID (example: Space 3)
    let target_space_id = 3; // Replace with actual Space ID query

    // Move window
    match manager.move_window_to_space(window_id, target_space_id) {
        Ok(_) => println!("Successfully moved window to Space {}", target_space_id),
        Err(e) => eprintln!("Failed to move window: CGError {}", e),
    }
}
```

## Sources

### Primary API Documentation
- [CGSInternal/CGSConnection.h](https://github.com/NUIKit/CGSInternal/blob/master/CGSConnection.h) - Comprehensive CGS API header collection
- [yabai/src/misc/extern.h](https://github.com/asmvik/yabai/blob/master/src/misc/extern.h) - Complete SLS function declarations
- [Hammerspoon/libspaces.m](https://github.com/Hammerspoon/hammerspoon/blob/master/extensions/spaces/libspaces.m) - Reference implementation with macOS 14.5+ workarounds

### Implementation Examples
- [Yabai GitHub Repository](https://github.com/koekeishiya/yabai) - Full-featured window manager using SLS APIs
- [Hammerspoon GitHub Repository](https://github.com/Hammerspoon/hammerspoon) - Lua-scriptable automation with `hs.spaces` module
- [hs._asm.undocumented.spaces](https://github.com/asmagill/hs._asm.undocumented.spaces) - Legacy Hammerspoon Spaces module

### Articles and Discussions
- [Accessibility, Windows, and Spaces in OS X](https://ianyh.com/blog/accessibility-windows-and-spaces-in-os-x/) - CGEvent-based workaround (App Store safe)
- [Move your application windows between spaces](https://tonyarnold.com/2008/12/05/move-your-application-windows-between-spaces.html) - Historical CGSMoveWorkspaceWindowList usage
- [Getting started with making macOS utility app using private APIs](https://speakerdeck.com/niw/getting-started-with-making-macos-utility-app-using-private-apis) - Overview of private API development
- [Exploring macOS private frameworks](https://www.jviotti.com/2023/11/20/exploring-macos-private-frameworks.html) - Deep dive into macOS private frameworks

### Terminal Emulator Research
- [Ghostty GitHub Repository](https://github.com/ghostty-org/ghostty) - Modern native Swift/Metal terminal emulator
- [Alacritty Drop-down setup on macOS](https://world.hey.com/jonash/alacritty-drop-down-guake-quake-style-terminal-setup-on-macos-6eef7d73) - Using Hammerspoon for Space management
- [Ghostty Discussion #3512](https://github.com/ghostty-org/ghostty/discussions/3512) - Quick Terminal and Aerospace workspace integration
- [Hammerspoon Issue #3698](https://github.com/Hammerspoon/hammerspoon/issues/3698) - macOS 15.0 Sequoia compatibility fixes

### Apple Documentation
- [NSWindow.CollectionBehavior](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct) - Public API for window behaviors
- [moveToActiveSpace](https://developer.apple.com/documentation/appkit/nswindow/collectionbehavior-swift.struct/movetoactivespace) - Public Space-related behavior

### Rust FFI Resources
- [FFI - The Rustonomicon](https://doc.rust-lang.org/nomicon/ffi.html) - Official Rust FFI documentation
- [objc2 GitHub Repository](https://github.com/madsmtm/objc2) - Rust bindings to Apple frameworks
- [objc2-core-graphics crate](https://crates.io/crates/objc2-core-graphics) - CoreGraphics bindings (no SLS APIs)

## Conclusion

macOS Space management via private SLS APIs is **viable but risky**:
- ✅ **Functional**: APIs work on modern macOS (Sonoma, Sequoia)
- ✅ **Well-documented**: Extensive prior art from Yabai, Hammerspoon, and community projects
- ⚠️ **Version-dependent**: Requires version-specific handling (14.5+ compatibility IDs)
- ❌ **App Store incompatible**: Automatic rejection for private API usage
- ⚠️ **Stability concerns**: No guarantees against future breakage

**Recommendation for par-term**:
- If distributing via Homebrew/direct download: Implement SLS APIs with version abstraction
- If targeting App Store: Use `NSWindow.collectionBehavior.moveToActiveSpace` only (limited functionality)
- Consider deferring Space management to external tools (Aerospace, Hammerspoon) and focus on core terminal features

**Rust Implementation Viability**: High—objc2 + manual FFI declarations provide a clean abstraction layer.
