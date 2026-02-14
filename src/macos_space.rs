//! macOS window Space (virtual desktop) targeting using private SLS APIs
//!
//! Uses SkyLight Server (SLS) private APIs to move windows to specific macOS Spaces.
//! macOS 14.5+ requires a compatibility ID workaround; older versions use the legacy API.
//!
//! This follows the same dlopen/dlsym pattern as `macos_blur.rs` for private API access.
//! CoreFoundation public functions are linked directly since they are always available.
//!
//! Note: This only works on macOS. On other platforms, the function is a no-op.

#[cfg(not(target_os = "macos"))]
use anyhow::Result;

#[cfg(target_os = "macos")]
mod inner {
    use anyhow::Result;
    use objc2_app_kit::NSView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::ffi::c_void;
    use std::sync::OnceLock;

    // ========================================================================
    // CoreFoundation public API declarations (always available on macOS)
    // ========================================================================

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;
    const K_CF_NUMBER_SINT64_TYPE: isize = 4;
    const K_CF_NUMBER_SINT32_TYPE: isize = 3;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFArrayGetCount(theArray: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(theArray: *const c_void, idx: isize) -> *const c_void;
        fn CFDictionaryGetValue(theDict: *const c_void, key: *const c_void) -> *const c_void;
        fn CFNumberGetValue(number: *const c_void, theType: isize, valuePtr: *mut c_void) -> bool;
        fn CFRelease(cf: *const c_void);
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            cStr: *const i8,
            encoding: u32,
        ) -> *const c_void;
    }

    // ========================================================================
    // SLS private API function types (loaded via dlsym)
    // ========================================================================

    type SLSMainConnectionIDFn = unsafe extern "C" fn() -> i32;
    type SLSCopyManagedDisplaySpacesFn = unsafe extern "C" fn(cid: i32) -> *const c_void;
    type SLSMoveWindowsToManagedSpaceFn =
        unsafe extern "C" fn(cid: i32, window_list: *const c_void, sid: u64);
    type SLSSpaceSetCompatIDFn = unsafe extern "C" fn(cid: i32, sid: u64, workspace: i32) -> i32;
    type SLSSetWindowListWorkspaceFn =
        unsafe extern "C" fn(cid: i32, window_list: *mut u32, count: i32, workspace: i32) -> i32;

    // ========================================================================
    // Cached function pointers
    // ========================================================================

    struct SlsFunctions {
        main_connection_id: SLSMainConnectionIDFn,
        copy_managed_display_spaces: SLSCopyManagedDisplaySpacesFn,
        move_windows_to_managed_space: SLSMoveWindowsToManagedSpaceFn,
        space_set_compat_id: SLSSpaceSetCompatIDFn,
        set_window_list_workspace: SLSSetWindowListWorkspaceFn,
    }

    static SLS_FNS: OnceLock<Option<SlsFunctions>> = OnceLock::new();

    fn load_sls_functions() -> Option<&'static SlsFunctions> {
        SLS_FNS
            .get_or_init(|| unsafe {
                let handle = libc::dlopen(
                    c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight".as_ptr(),
                    libc::RTLD_LAZY,
                );
                if handle.is_null() {
                    log::warn!("Failed to open SkyLight framework for Space targeting");
                    return None;
                }

                macro_rules! load_sym {
                    ($name:expr, $ty:ty) => {{
                        let sym = libc::dlsym(handle, $name.as_ptr());
                        if sym.is_null() {
                            log::warn!(
                                "SLS function {} not found",
                                String::from_utf8_lossy($name.to_bytes())
                            );
                            return None;
                        }
                        std::mem::transmute::<*mut c_void, $ty>(sym)
                    }};
                }

                Some(SlsFunctions {
                    main_connection_id: load_sym!(c"SLSMainConnectionID", SLSMainConnectionIDFn),
                    copy_managed_display_spaces: load_sym!(
                        c"SLSCopyManagedDisplaySpaces",
                        SLSCopyManagedDisplaySpacesFn
                    ),
                    move_windows_to_managed_space: load_sym!(
                        c"SLSMoveWindowsToManagedSpace",
                        SLSMoveWindowsToManagedSpaceFn
                    ),
                    space_set_compat_id: load_sym!(c"SLSSpaceSetCompatID", SLSSpaceSetCompatIDFn),
                    set_window_list_workspace: load_sym!(
                        c"SLSSetWindowListWorkspace",
                        SLSSetWindowListWorkspaceFn
                    ),
                })
            })
            .as_ref()
    }

    // ========================================================================
    // macOS version detection
    // ========================================================================

    static IS_14_5_OR_NEWER: OnceLock<bool> = OnceLock::new();

    fn is_macos_14_5_or_newer() -> bool {
        *IS_14_5_OR_NEWER.get_or_init(|| {
            // Use sw_vers to detect macOS version
            match std::process::Command::new("sw_vers")
                .arg("-productVersion")
                .output()
            {
                Ok(output) => {
                    let version_str = String::from_utf8_lossy(&output.stdout);
                    let parts: Vec<&str> = version_str.trim().split('.').collect();
                    match (
                        parts.first().and_then(|s| s.parse::<u32>().ok()),
                        parts.get(1).and_then(|s| s.parse::<u32>().ok()),
                    ) {
                        (Some(major), Some(minor)) => {
                            log::info!(
                                "macOS version {} — using {} Space API",
                                version_str.trim(),
                                if major > 14 || (major == 14 && minor >= 5) {
                                    "modern (compat ID)"
                                } else {
                                    "legacy"
                                }
                            );
                            major > 14 || (major == 14 && minor >= 5)
                        }
                        _ => {
                            log::warn!("Could not parse macOS version, defaulting to modern API");
                            true
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to run sw_vers: {}, defaulting to modern API", e);
                    true
                }
            }
        })
    }

    // ========================================================================
    // Space enumeration
    // ========================================================================

    /// Enumerate user Space IDs on the primary display, in Mission Control order.
    /// Returns Space IDs for type-0 (user) Spaces only (excludes fullscreen Spaces).
    fn enumerate_user_spaces(sls: &SlsFunctions) -> Vec<u64> {
        unsafe {
            let cid = (sls.main_connection_id)();
            let display_spaces = (sls.copy_managed_display_spaces)(cid);
            if display_spaces.is_null() {
                log::warn!("SLSCopyManagedDisplaySpaces returned null");
                return Vec::new();
            }

            let mut user_spaces = Vec::new();
            let display_count = CFArrayGetCount(display_spaces);

            // Iterate over displays (usually just one for primary)
            for i in 0..display_count {
                let display_dict = CFArrayGetValueAtIndex(display_spaces, i);
                if display_dict.is_null() {
                    continue;
                }

                // Get the "Spaces" array from the display dictionary
                let spaces_key = CFStringCreateWithCString(
                    std::ptr::null(),
                    c"Spaces".as_ptr(),
                    K_CF_STRING_ENCODING_UTF8,
                );
                if spaces_key.is_null() {
                    continue;
                }

                let spaces_array = CFDictionaryGetValue(display_dict, spaces_key);
                CFRelease(spaces_key);
                if spaces_array.is_null() {
                    continue;
                }

                let space_count = CFArrayGetCount(spaces_array);
                let id64_key = CFStringCreateWithCString(
                    std::ptr::null(),
                    c"id64".as_ptr(),
                    K_CF_STRING_ENCODING_UTF8,
                );
                let type_key = CFStringCreateWithCString(
                    std::ptr::null(),
                    c"type".as_ptr(),
                    K_CF_STRING_ENCODING_UTF8,
                );
                if id64_key.is_null() || type_key.is_null() {
                    if !id64_key.is_null() {
                        CFRelease(id64_key);
                    }
                    if !type_key.is_null() {
                        CFRelease(type_key);
                    }
                    continue;
                }

                for j in 0..space_count {
                    let space_dict = CFArrayGetValueAtIndex(spaces_array, j);
                    if space_dict.is_null() {
                        continue;
                    }

                    // Get Space type — 0 = user Space, 4 = fullscreen
                    let type_num = CFDictionaryGetValue(space_dict, type_key);
                    if !type_num.is_null() {
                        let mut space_type: i32 = 0;
                        CFNumberGetValue(
                            type_num,
                            K_CF_NUMBER_SINT32_TYPE,
                            &mut space_type as *mut i32 as *mut c_void,
                        );
                        if space_type != 0 {
                            continue; // Skip fullscreen Spaces
                        }
                    }

                    // Get Space ID (id64)
                    let id_num = CFDictionaryGetValue(space_dict, id64_key);
                    if !id_num.is_null() {
                        let mut space_id: u64 = 0;
                        if CFNumberGetValue(
                            id_num,
                            K_CF_NUMBER_SINT64_TYPE,
                            &mut space_id as *mut u64 as *mut c_void,
                        ) {
                            user_spaces.push(space_id);
                        }
                    }
                }

                CFRelease(id64_key);
                CFRelease(type_key);

                // Only use the first display's Spaces (primary display)
                break;
            }

            CFRelease(display_spaces);
            user_spaces
        }
    }

    // ========================================================================
    // Window movement
    // ========================================================================

    /// Move a window to a specific Space using the modern API (macOS 14.5+).
    fn move_window_modern(
        sls: &SlsFunctions,
        cid: i32,
        window_number: u32,
        space_id: u64,
    ) -> Result<()> {
        unsafe {
            const COMPAT_ID: i32 = 0x79616265; // 'yabe' — standard marker used by yabai

            // Step 1: Set temporary compatibility ID on the target Space
            let err = (sls.space_set_compat_id)(cid, space_id, COMPAT_ID);
            if err != 0 {
                anyhow::bail!("SLSSpaceSetCompatID failed with error code: {}", err);
            }

            // Step 2: Move window to the workspace with that compat ID
            let mut wid = window_number;
            let err = (sls.set_window_list_workspace)(cid, &mut wid, 1, COMPAT_ID);
            if err != 0 {
                // Clean up the compat ID before returning error
                let _ = (sls.space_set_compat_id)(cid, space_id, 0);
                anyhow::bail!("SLSSetWindowListWorkspace failed with error code: {}", err);
            }

            // Step 3: Clear the compatibility ID
            let _ = (sls.space_set_compat_id)(cid, space_id, 0);

            Ok(())
        }
    }

    /// Move a window to a specific Space using the legacy API (macOS < 14.5).
    fn move_window_legacy(sls: &SlsFunctions, cid: i32, window_number: u32, space_id: u64) {
        unsafe {
            // Create a CFArray containing just the window number
            let num = objc2_foundation::NSNumber::new_i64(window_number as i64);
            let array = objc2_foundation::NSArray::from_slice(&[&*num]);
            // NSArray is toll-free bridged with CFArrayRef
            let cf_array = &*array as *const objc2_foundation::NSArray<objc2_foundation::NSNumber>
                as *const c_void;
            (sls.move_windows_to_managed_space)(cid, cf_array, space_id);
        }
    }

    /// Move the window to a specific macOS Space (virtual desktop).
    ///
    /// # Arguments
    /// * `window` - The winit window to move
    /// * `space_number` - 1-based Space ordinal (1 = first Space in Mission Control)
    ///
    /// # Notes
    /// - Uses private macOS SLS APIs that may change between OS versions
    /// - Gracefully fails if APIs are unavailable or Space doesn't exist
    pub fn move_window_to_space(window: &winit::window::Window, space_number: u32) -> Result<()> {
        let sls = load_sls_functions().ok_or_else(|| {
            anyhow::anyhow!("SLS Space functions not available on this macOS version")
        })?;

        // Get window number from NSView → NSWindow
        let window_handle = window.window_handle()?;
        let ns_view_ptr = match window_handle.as_raw() {
            RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr(),
            _ => anyhow::bail!("Not a macOS AppKit window"),
        };

        let window_number: u32 = unsafe {
            let ns_view = ns_view_ptr as *mut NSView;
            let view = &*ns_view;
            let ns_window: *const objc2::runtime::AnyObject = objc2::msg_send![view, window];
            if ns_window.is_null() {
                anyhow::bail!("Failed to get NSWindow from NSView");
            }
            let num: i64 = objc2::msg_send![ns_window, windowNumber];
            num as u32
        };

        // Enumerate user Spaces to map ordinal → Space ID
        let user_spaces = enumerate_user_spaces(sls);
        if user_spaces.is_empty() {
            anyhow::bail!("No user Spaces found");
        }

        let index = (space_number as usize).saturating_sub(1); // 1-based to 0-based
        let space_id = user_spaces.get(index).ok_or_else(|| {
            anyhow::anyhow!(
                "Space {} does not exist (found {} Spaces)",
                space_number,
                user_spaces.len()
            )
        })?;

        let cid = unsafe { (sls.main_connection_id)() };

        log::info!(
            "Moving window {} to Space {} (internal ID: {}, {} API)",
            window_number,
            space_number,
            space_id,
            if is_macos_14_5_or_newer() {
                "modern"
            } else {
                "legacy"
            }
        );

        if is_macos_14_5_or_newer() {
            move_window_modern(sls, cid, window_number, *space_id)?;
        } else {
            move_window_legacy(sls, cid, window_number, *space_id);
        }

        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub use inner::move_window_to_space;

/// Move the window to a specific macOS Space (no-op on non-macOS platforms).
#[cfg(not(target_os = "macos"))]
pub fn move_window_to_space(_window: &winit::window::Window, _space_number: u32) -> Result<()> {
    Ok(())
}
