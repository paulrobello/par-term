//! macOS window blur using private CGS API
//!
//! Uses CGSSetWindowBackgroundBlurRadius to apply gaussian blur to window background.
//! This is a private API that iTerm2, Alacritty, and other terminals use.
//!
//! Note: This only works on macOS. On other platforms, the functions are no-ops.

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

    fn load_functions() {
        // SAFETY: `dlopen` is an FFI call to open a system framework; the path is a
        // well-known, null-terminated C string literal. `dlsym` returns either a null
        // pointer (checked below) or the address of the named symbol. The resulting
        // pointer is transmuted into the correct function pointer type
        // `CGSSetWindowBackgroundBlurRadiusFn`, which matches the actual C ABI of
        // `CGSSetWindowBackgroundBlurRadius(CGSConnectionID, uint32_t, uint32_t)`.
        // The transmutation is valid because both sides are pointer-sized function
        // pointers and we verify the symbol exists before transmuting.
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
            let sym = libc::dlsym(handle, c"CGSSetWindowBackgroundBlurRadius".as_ptr());
            if sym.is_null() {
                log::warn!("CGSSetWindowBackgroundBlurRadius not found");
                None
            } else {
                Some(std::mem::transmute::<
                    *mut libc::c_void,
                    CGSSetWindowBackgroundBlurRadiusFn,
                >(sym))
            }
        });
        // SAFETY: Same dlopen/dlsym pattern as BLUR_FN above. The symbol pointer is
        // transmuted to `CGSDefaultConnectionForThreadFn` which matches the C ABI of
        // `CGSDefaultConnectionForThread() -> CGSConnectionID` (returns a u32).
        // The transmutation is valid: both sides are pointer-sized function pointers
        // and the null check ensures we only transmute a valid symbol address.
        CONN_FN.get_or_init(|| unsafe {
            let handle = libc::dlopen(
                c"/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices"
                    .as_ptr(),
                libc::RTLD_LAZY,
            );
            if handle.is_null() {
                return None;
            }
            let sym = libc::dlsym(handle, c"CGSDefaultConnectionForThread".as_ptr());
            if sym.is_null() {
                log::warn!("CGSDefaultConnectionForThread not found");
                None
            } else {
                Some(std::mem::transmute::<
                    *mut libc::c_void,
                    CGSDefaultConnectionForThreadFn,
                >(sym))
            }
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
