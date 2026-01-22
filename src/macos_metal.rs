//! macOS-specific CAMetalLayer configuration
//!
//! This module accesses the underlying CAMetalLayer created by wgpu/winit
//! to ensure per-pixel transparency is honored and to disable VSync throttling.

use anyhow::Result;

/// Configure CAMetalLayer for optimal transparency + performance on macOS
///
/// This function:
/// 1. Extracts the NSView from the winit window
/// 2. Gets the CAMetalLayer from the view
/// 3. Sets `opaque = false` so per-pixel alpha (content only) is respected
/// 4. Keeps layer `opacity = 1.0` so only rendered pixels control alpha (window chrome untouched)
/// 5. Sets `displaySyncEnabled = false` to disable VSync throttling
/// 6. This allows `surface.present()` to return immediately instead of blocking
pub fn configure_metal_layer_for_performance(window: &winit::window::Window) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        use objc2::rc::Retained;
        use objc2::runtime::AnyObject;
        use objc2_app_kit::NSView;
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};

        log::info!("🎯 Configuring CAMetalLayer for 60 FPS performance");

        // Get the raw window handle
        let window_handle = window.window_handle()?;

        // Extract NSView pointer from AppKit handle
        let ns_view_ptr = match window_handle.as_raw() {
            RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr(),
            _ => anyhow::bail!("Not a macOS AppKit window"),
        };

        // SAFETY: We're on the main thread (required by winit), NSView pointer is valid
        unsafe {
            // Cast to NSView
            let ns_view = ns_view_ptr as *mut NSView;
            let view = &*ns_view;

            // Get the layer (should be CAMetalLayer, created by wgpu)
            let layer: Retained<AnyObject> = objc2::msg_send![view, layer];

            // Get raw pointer to layer
            let metal_layer_ptr = Retained::as_ptr(&layer);

            // Check if this is actually a CAMetalLayer before calling Metal-specific methods
            // Get the class object
            let class_obj: *const objc2::runtime::AnyClass =
                objc2::msg_send![metal_layer_ptr, class];

            // Get the class name from the class object
            let class_name = objc2::runtime::AnyClass::name(&*class_obj);
            let class_name_str = class_name.to_str().unwrap_or("Unknown");
            log::info!("Layer class: {}", class_name_str);

            if class_name_str == "CAMetalLayer" {
                // Allow per-pixel transparency (content only)
                let _: () = objc2::msg_send![metal_layer_ptr, setOpaque: false];
                // Keep global layer opacity at 1.0; we rely on per-pixel alpha instead
                let _: () = objc2::msg_send![metal_layer_ptr, setOpacity: 1.0_f32];

                // Set displaySyncEnabled to false
                // This is the KEY to bypassing macOS VSync throttling
                let _: () = objc2::msg_send![metal_layer_ptr, setDisplaySyncEnabled: false];

                // Verify the setting was applied
                let display_sync_enabled: bool =
                    objc2::msg_send![metal_layer_ptr, displaySyncEnabled];

                log::info!(
                    "✅ CAMetalLayer configured: displaySyncEnabled = {}",
                    display_sync_enabled
                );
                if display_sync_enabled {
                    log::warn!(
                        "   ⚠️  displaySyncEnabled is still true! Setting may not have taken effect."
                    );
                } else {
                    log::info!("   Expected: present() will no longer block for VSync");
                    log::info!("   Target: 60+ FPS instead of ~20 FPS");
                }
            } else {
                log::warn!(
                    "❌ Layer is not CAMetalLayer (found: {}), skipping configuration",
                    class_name_str
                );
                log::warn!("   This is normal if called before wgpu creates the Metal surface");
                log::warn!("   Will retry after renderer initialization");
                anyhow::bail!("Layer is not yet a CAMetalLayer");
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = window; // Silence unused warning
    }

    Ok(())
}

/// Update CAMetalLayer opacity (affects rendered content only, not window chrome)
pub fn set_layer_opacity(window: &winit::window::Window, opacity: f32) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        use objc2::rc::Retained;
        use objc2::runtime::AnyObject;
        use objc2_app_kit::NSView;
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};

        let clamped = opacity.clamp(0.0, 1.0);
        let window_handle = window.window_handle()?;
        let ns_view_ptr = match window_handle.as_raw() {
            RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr(),
            _ => anyhow::bail!("Not a macOS AppKit window"),
        };

        if ns_view_ptr.is_null() {
            anyhow::bail!("NSView pointer is null");
        }

        unsafe {
            let ns_view = ns_view_ptr as *mut NSView;
            let layer: Retained<AnyObject> = objc2::msg_send![ns_view, layer];
            let metal_layer_ptr = Retained::as_ptr(&layer);

            // Ensure per-pixel alpha is allowed and set content opacity
            let _: () = objc2::msg_send![metal_layer_ptr, setOpaque: false];
            let _: () = objc2::msg_send![metal_layer_ptr, setOpacity: clamped];

            let current_opacity: f32 = objc2::msg_send![metal_layer_ptr, opacity];
            log::debug!(
                "CAMetalLayer content opacity set to {:.3} (reported {:.3})",
                clamped,
                current_opacity
            );
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, opacity);
    }

    Ok(())
}

// (Layer opacity remains fixed at 1.0; per-pixel transparency handled in renderer.)
