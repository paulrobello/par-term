//! macOS window blur using private CGS API
//!
//! Uses CGSSetWindowBackgroundBlurRadius to apply gaussian blur to window background.
//! This is a private API that iTerm2, Alacritty, and other terminals use.
//!
//! Note: This only works on macOS. On other platforms, the functions are no-ops.
//!
//! TODO(QA-011): The unsafe FFI blocks in this module lack automated test coverage.
//! Testing requires a macOS display server (CI runners may not have one).
//! Consider adding: (1) compile-time signature validation via `bindgen` for the
//! private CGS functions, (2) a manual test script that verifies blur is applied
//! at each supported macOS version, (3) dlsym-null-return defensive tests.

#[cfg(not(target_os = "macos"))]
use anyhow::Result;

#[cfg(target_os = "macos")]
mod inner {
    use anyhow::Result;
    use objc2_app_kit::NSView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::sync::OnceLock;

    type CGSConnectionID = u32;
    type CGError = i32;

    type CGSSetWindowBackgroundBlurRadiusFn =
        unsafe extern "C" fn(CGSConnectionID, u32, u32) -> CGError;
    type CGSDefaultConnectionForThreadFn = unsafe extern "C" fn() -> CGSConnectionID;

    static BLUR_FN: OnceLock<Option<CGSSetWindowBackgroundBlurRadiusFn>> = OnceLock::new();
    static CONN_FN: OnceLock<Option<CGSDefaultConnectionForThreadFn>> = OnceLock::new();

    // ========================================================================
    // macOS version detection for blur API
    // ========================================================================

    /// Minimum macOS major version on which the blur API has been validated.
    /// The private CGS API has been available since macOS 10.x and remains
    /// stable through macOS 15 (Sequoia). If the running version is below this
    /// threshold we skip loading the symbols to avoid undefined behavior from
    /// ABI changes in old versions.
    ///
    /// Tested on: macOS 13 (Ventura), 14 (Sonoma), 15 (Sequoia).
    const MIN_SUPPORTED_MACOS_MAJOR: u32 = 13;

    static MACOS_MAJOR_VERSION: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

    fn macos_major_version() -> u32 {
        *MACOS_MAJOR_VERSION.get_or_init(|| {
            match std::process::Command::new("sw_vers")
                .arg("-productVersion")
                .output()
            {
                Ok(output) => {
                    let s = String::from_utf8_lossy(&output.stdout);
                    s.trim()
                        .split('.')
                        .next()
                        .and_then(|v| v.parse::<u32>().ok())
                        .unwrap_or(0)
                }
                Err(_) => 0,
            }
        })
    }

    fn load_functions() {
        // Guard: skip loading private APIs on untested/unsupported macOS versions.
        // The CGS ABI may differ on very old macOS releases; transmuting a symbol
        // with a mismatched ABI produces undefined behavior.
        if macos_major_version() < MIN_SUPPORTED_MACOS_MAJOR {
            log::warn!(
                "macOS version {} is below minimum supported version {} for blur API — \
                 window blur disabled",
                macos_major_version(),
                MIN_SUPPORTED_MACOS_MAJOR,
            );
            BLUR_FN.get_or_init(|| None);
            CONN_FN.get_or_init(|| None);
            return;
        }

        // SEC-007: Load both symbols from a single dlopen call. Each dlsym result
        // is checked for null before transmuting to a function pointer. If any
        // symbol is missing, the corresponding OnceLock is set to None and the
        // function returns early so that set_window_blur() returns an error
        // instead of calling through a null pointer.
        //
        // SAFETY: `dlopen` opens a system framework using a well-known, null-terminated
        // C string literal. `dlsym` returns either a null pointer (checked immediately
        // below) or a valid symbol address. Transmutation from `*mut c_void` to a
        // function pointer type is valid because both are pointer-sized on all Apple
        // platforms. ABI signatures have been validated on macOS 13, 14, and 15.
        BLUR_FN.get_or_init(|| unsafe {
            let handle = libc::dlopen(
                c"/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices"
                    .as_ptr(),
                libc::RTLD_LAZY,
            );
            if handle.is_null() {
                log::warn!("Failed to open ApplicationServices framework for blur");
                return None;
            }

            let blur_sym = libc::dlsym(handle, c"CGSSetWindowBackgroundBlurRadius".as_ptr());
            if blur_sym.is_null() {
                log::warn!(
                    "CGSSetWindowBackgroundBlurRadius not found in ApplicationServices — \
                     blur API unavailable"
                );
                return None;
            }

            Some(std::mem::transmute::<
                *mut libc::c_void,
                CGSSetWindowBackgroundBlurRadiusFn,
            >(blur_sym))
        });

        CONN_FN.get_or_init(|| unsafe {
            // Re-open the framework (dlopen returns the same handle for an already-loaded
            // library, so this is cheap). Using a separate dlopen avoids coupling the two
            // OnceLock initialization order.
            let handle = libc::dlopen(
                c"/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices"
                    .as_ptr(),
                libc::RTLD_LAZY,
            );
            if handle.is_null() {
                log::warn!(
                    "Failed to open ApplicationServices framework for blur (connection fn)"
                );
                return None;
            }

            let conn_sym = libc::dlsym(handle, c"CGSDefaultConnectionForThread".as_ptr());
            if conn_sym.is_null() {
                log::warn!(
                    "CGSDefaultConnectionForThread not found in ApplicationServices — \
                     blur API unavailable"
                );
                return None;
            }

            Some(std::mem::transmute::<
                *mut libc::c_void,
                CGSDefaultConnectionForThreadFn,
            >(conn_sym))
        });
    }

    /// Set the window background blur radius.
    ///
    /// # Arguments
    /// * `window` - The winit window to apply blur to
    /// * `radius` - Blur radius in points (0-64). Use 0 to disable blur.
    ///
    /// # Notes
    /// - This uses private macOS APIs that may change between OS versions
    /// - iTerm2 uses a minimum blur of 1 to avoid a macOS bug where setting to 0
    ///   permanently disables blur for the window
    pub fn set_window_blur(window: &winit::window::Window, radius: u32) -> Result<()> {
        load_functions();

        let blur_fn = BLUR_FN
            .get()
            .and_then(|f| *f)
            .ok_or_else(|| anyhow::anyhow!("CGSSetWindowBackgroundBlurRadius not available"))?;
        let conn_fn = CONN_FN
            .get()
            .and_then(|f| *f)
            .ok_or_else(|| anyhow::anyhow!("CGSDefaultConnectionForThread not available"))?;

        let window_handle = window.window_handle()?;
        let ns_view_ptr = match window_handle.as_raw() {
            RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr(),
            _ => anyhow::bail!("Not a macOS AppKit window"),
        };

        // SAFETY: ns_view_ptr is a non-null NSView pointer obtained from winit's AppKit
        // window handle. winit guarantees it is valid and that we are on the main thread
        // (required by AppKit). Casting to `*mut NSView` and dereferencing is valid
        // because the type matches and the pointer is aligned and initialized.
        // The `msg_send![view, window]` ObjC message is safe to call on a valid NSView
        // and returns either a valid NSWindow pointer or null (checked below).
        // `conn_fn()` and `blur_fn()` are valid C function pointers loaded via dlsym
        // whose signatures match their respective type aliases.
        unsafe {
            // Get the NSWindow from the NSView
            let ns_view = ns_view_ptr as *mut NSView;
            let view = &*ns_view;
            let ns_window: *const objc2::runtime::AnyObject = objc2::msg_send![view, window];

            if ns_window.is_null() {
                anyhow::bail!("Failed to get NSWindow from NSView");
            }

            // Get the CGS connection
            let cid = conn_fn();

            // Get the window number from NSWindow
            let window_number: i64 = objc2::msg_send![ns_window, windowNumber];

            // macOS bug workaround: clamp radius to valid range
            let actual_radius = radius.min(64);

            let result = blur_fn(cid, window_number as u32, actual_radius);
            if result != 0 {
                anyhow::bail!(
                    "CGSSetWindowBackgroundBlurRadius failed with error code: {}",
                    result
                );
            }
        }

        log::info!("Window blur set to radius {}", radius);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub use inner::set_window_blur;

/// Set the window background blur radius (no-op on non-macOS platforms).
#[cfg(not(target_os = "macos"))]
pub fn set_window_blur(_window: &winit::window::Window, _radius: u32) -> Result<()> {
    // No-op on non-macOS platforms
    Ok(())
}
