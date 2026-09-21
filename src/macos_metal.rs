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

        // SAFETY: `ns_view_ptr` is a non-null NSView pointer obtained from winit's AppKit
        // window handle. winit guarantees the pointer is valid and that this function is
        // called on the main thread (required by AppKit/Objective-C). Casting to
        // `*mut NSView` and creating a shared reference is valid because the type matches
        // and the pointer is aligned and properly initialized by the runtime.
        //
        // `msg_send![view, layer]` is a safe Objective-C message on a valid NSView;
        // it returns a retained `AnyObject` whose lifetime is managed by `Retained<T>`.
        // `msg_send![metal_layer_ptr, class]` returns the metaclass object — non-null
        // for any live Objective-C object. Dereferencing `class_obj` is safe because
        // every Objective-C object has an associated class.
        //
        // The `setOpaque:`, `setOpacity:`, `setDisplaySyncEnabled:`, and
        // `displaySyncEnabled` messages are only sent after verifying the layer is a
        // `CAMetalLayer` by name check, so the Metal-specific selectors are valid.
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
                // This is the KEY to bypass macOS VSync throttling
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

        // SAFETY: ns_view_ptr is a non-null NSView pointer obtained from winit's AppKit
        // window handle. winit guarantees it is valid for the lifetime of the window.
        // We are on the main thread (required by AppKit/winit), so Objective-C message
        // sends are safe. The layer obtained via `msg_send![view, layer]` is retained
        // by the view and remains valid for the duration of this block.
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

/// Sync the NSWindow background color with the per-pixel translucency setting.
///
/// macOS 27 paints the titlebar from the NSWindow background color when the
/// window is non-opaque. winit's `with_transparent(true)` (required for the
/// runtime opacity feature) sets that color to clear, which turned the
/// titlebar see-through. Setting a real background color restores the standard
/// titlebar, but `NSWindow.backgroundColor` is one alpha floor for the whole
/// window backing store — an opaque floor also greys out the CAMetalLayer's
/// translucent content (measured 2026-09-20, reverted c0f06688). So while
/// translucency is in effect the background stays clear and an opaque backing
/// bar (a borderless filled NSBox in the frame view, pinned under the
/// titlebar) paints the titlebar instead. The CAMetalLayer's per-pixel alpha
/// composites over the clear floor everywhere else, keeping the desktop
/// visible through the content area.
pub fn set_window_background_for_translucency(
    window: &winit::window::Window,
    translucent: bool,
) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        use objc2::rc::Retained;
        use objc2_app_kit::{NSColor, NSView, NSWindow};
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};

        let window_handle = window.window_handle()?;
        let ns_view_ptr = match window_handle.as_raw() {
            RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr(),
            _ => anyhow::bail!("Not a macOS AppKit window"),
        };

        if ns_view_ptr.is_null() {
            anyhow::bail!("NSView pointer is null");
        }

        // SAFETY: ns_view_ptr is a non-null NSView pointer obtained from winit's AppKit
        // window handle. winit guarantees it is valid for the lifetime of the window.
        // We are on the main thread (required by AppKit/winit), so the Objective-C
        // message send is safe. The `window` selector is not in the alloc/new/init
        // method family, so the optional retained return is fine; everything after
        // the extraction uses the safe generated objc2-app-kit API.
        unsafe {
            let ns_view = ns_view_ptr as *mut NSView;
            let ns_window: Option<Retained<NSWindow>> = objc2::msg_send![ns_view, window];
            let Some(ns_window) = ns_window else {
                anyhow::bail!("NSView is not installed in an NSWindow");
            };

            let color = if translucent {
                NSColor::clearColor()
            } else {
                NSColor::windowBackgroundColor()
            };
            ns_window.setBackgroundColor(Some(&color));

            if translucent {
                ensure_titlebar_backing_bar(&ns_window)?;
            } else {
                remove_titlebar_backing_bar(&ns_window);
            }

            log::debug!(
                "NSWindow background set to {} (translucent: {}); titlebar bar {}",
                if translucent {
                    "clear"
                } else {
                    "windowBackgroundColor"
                },
                translucent,
                if translucent { "installed" } else { "removed" }
            );
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, translucent);
    }

    Ok(())
}

// (Layer opacity remains fixed at 1.0; per-pixel transparency handled in renderer.)

/// Identifier on the titlebar backing bar so repeated config changes can find
/// the existing bar instead of stacking duplicates. `setTag:` is not an option:
/// on modern macOS NSBox no longer inherits NSControl and does not respond to
/// it (measured: NSInvalidArgumentException at window creation), while
/// `setIdentifier:` is available on every NSView.
#[cfg(target_os = "macos")]
const TITLEBAR_BACKING_BAR_ID: &str = "par-term-titlebar-backing-bar";

/// Find the titlebar backing bar previously added to the window's frame view,
/// by identifier. NSView has no `viewWithIdentifier:`, so scan the subviews.
#[cfg(target_os = "macos")]
fn find_titlebar_backing_bar(
    frame_view: &objc2_app_kit::NSView,
) -> Option<objc2::rc::Retained<objc2_app_kit::NSView>> {
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSView;
    use objc2_foundation::NSString;

    let wanted = NSString::from_str(TITLEBAR_BACKING_BAR_ID);
    // SAFETY: subviews/identifier/isEqual are standard AppKit/Foundation
    // messages on live objects owned by the frame view, which the caller
    // keeps alive. objectAtIndex: returns a borrowed reference, so the match
    // is retained before being returned.
    unsafe {
        let subs: Retained<AnyObject> = objc2::msg_send![frame_view, subviews];
        let count: usize = objc2::msg_send![&*subs, count];
        for i in 0..count {
            let sub: *mut AnyObject = objc2::msg_send![&*subs, objectAtIndex: i];
            let ident: *mut AnyObject = objc2::msg_send![sub, identifier];
            if ident.is_null() {
                continue;
            }
            let eq: bool = objc2::msg_send![ident, isEqualToString: &*wanted];
            if eq {
                return Retained::retain(sub as *mut NSView);
            }
        }
    }
    None
}

/// The titlebar strip's rect in the frame view's bottom-left coordinates.
///
/// `contentLayoutRect` (window coords, same bottom-left origin) covers the
/// content area ONLY — for a titled window its `origin.y` is 0 and its height
/// is the frame height minus the titlebar. So the strip is derived from the
/// layout's HEIGHT (frame height minus it, pinned at its top), never from its
/// `origin.y`: the first backing-bar port computed `frame.height -
/// layout.origin.y` and `origin.y = layout.origin.y`, which with `origin.y ==
/// 0` drew a bar covering the ENTIRE window and greyed out all content
/// (measured in the isolated AppKit harness /tmp/titlebar-test3, 2026-09-20:
/// shipped formula bar y=0 h=203 vs corrected y=171 h=32 on a 203px frame).
/// Returns None when there is no strip (fullscreen / borderless).
#[cfg(target_os = "macos")]
fn titlebar_strip(
    layout_rect: &objc2_foundation::NSRect,
    frame_bounds: &objc2_foundation::NSRect,
) -> Option<objc2_foundation::NSRect> {
    use objc2_foundation::{NSPoint, NSSize};

    let height = frame_bounds.size.height - layout_rect.size.height;
    if height <= 1.0 {
        return None;
    }
    Some(objc2_foundation::NSRect {
        origin: NSPoint {
            x: 0.0,
            y: layout_rect.size.height,
        },
        size: NSSize {
            width: frame_bounds.size.width,
            height,
        },
    })
}

/// Install or re-show the opaque titlebar backing bar.
///
/// The bar is a borderless, flat-filled NSBox added to the window's frame
/// view (the content view's superview), covering exactly the titlebar strip.
/// On macOS 27 the titlebar's own paint comes from the clear window
/// background, so this bar is what the user sees behind the traffic lights
/// and title text (both of which AppKit keeps above frame-view subviews).
/// A plain NSView without a mouseDown override answers
/// `mouseDownCanMoveWindow` YES, so dragging/double-clicking the titlebar
/// keeps working through it.
#[cfg(target_os = "macos")]
fn ensure_titlebar_backing_bar(ns_window: &objc2_app_kit::NSWindow) -> Result<()> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSAutoresizingMaskOptions, NSBox, NSBoxType, NSTitlePosition};
    use objc2_foundation::NSString;

    let Some(frame_view) = ns_window
        .contentView()
        .and_then(|cv| unsafe { cv.superview() })
    else {
        anyhow::bail!("window has no frame view (not a titled window?)");
    };

    if let Some(bar) = find_titlebar_backing_bar(&frame_view) {
        // Re-derive the strip from current geometry so the bar self-corrects
        // after fullscreen exits or a titlebar-thickness change, then refresh
        // the fill so light/dark appearance switches are picked up.
        match titlebar_strip(&ns_window.contentLayoutRect(), &frame_view.bounds()) {
            Some(strip) => {
                bar.setFrame(strip);
                bar.setHidden(false);
                if let Some(b) = bar.downcast_ref::<NSBox>() {
                    b.setFillColor(&objc2_app_kit::NSColor::windowBackgroundColor());
                }
            }
            // Fullscreen / borderless: no strip to back. The Resized-driven
            // sync re-shows it with a fresh frame once a strip exists again.
            None => bar.setHidden(true),
        }
        return Ok(());
    }

    let bar_frame = titlebar_strip(&ns_window.contentLayoutRect(), &frame_view.bounds())
        .ok_or_else(|| anyhow::anyhow!("window has no titlebar strip to back"))?;

    let mtm = MainThreadMarker::new()
        .ok_or_else(|| anyhow::anyhow!("AppKit calls require the main thread"))?;
    let bar = NSBox::initWithFrame(mtm.alloc(), bar_frame);
    // SAFETY: setIdentifier: is a plain NSView/protocol setter on a live
    // NSBox we just created; the argument is an NSString we own.
    unsafe {
        let ident = NSString::from_str(TITLEBAR_BACKING_BAR_ID);
        let _: () = objc2::msg_send![&*bar, setIdentifier: &*ident];
    }
    bar.setBoxType(NSBoxType::Custom);
    bar.setBorderWidth(0.0);
    bar.setTitlePosition(NSTitlePosition::NoTitle);
    bar.setFillColor(&objc2_app_kit::NSColor::windowBackgroundColor());
    // Follow the frame view's width, stay pinned to its top edge.
    bar.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMinYMargin,
    );
    frame_view.addSubview(&bar);

    log::debug!("Installed opaque titlebar backing bar ({TITLEBAR_BACKING_BAR_ID})");
    Ok(())
}

/// Drop the titlebar backing bar (translucency no longer active; the window
/// background color itself paints an opaque titlebar again).
#[cfg(target_os = "macos")]
fn remove_titlebar_backing_bar(ns_window: &objc2_app_kit::NSWindow) {
    if let Some(frame_view) = ns_window
        .contentView()
        .and_then(|cv| unsafe { cv.superview() })
        && let Some(bar) = find_titlebar_backing_bar(&frame_view)
    {
        bar.removeFromSuperview();
        log::debug!("Removed titlebar backing bar");
    }
}

/// Hide the titlebar backing bar while the window is fullscreen (there is no
/// titlebar there, and a top-pinned bar would paint a strip over content).
/// Call from the Resized event so every transition path — keyboard, menu, the
/// green traffic-light button — is covered without hooking each one.
#[allow(unused_variables)]
pub fn sync_titlebar_backing_bar_visibility(window: &winit::window::Window) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        use objc2::rc::Retained;
        use objc2_app_kit::{NSView, NSWindow, NSWindowStyleMask};
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};

        let window_handle = window.window_handle()?;
        let ns_view_ptr = match window_handle.as_raw() {
            RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr(),
            _ => anyhow::bail!("Not a macOS AppKit window"),
        };
        if ns_view_ptr.is_null() {
            anyhow::bail!("NSView pointer is null");
        }

        // SAFETY: Same extraction pattern as set_window_background_for_translucency;
        // the rest is the safe generated objc2-app-kit API.
        unsafe {
            let ns_view = ns_view_ptr as *mut NSView;
            let ns_window: Option<Retained<NSWindow>> = objc2::msg_send![ns_view, window];
            let Some(ns_window) = ns_window else {
                anyhow::bail!("NSView is not installed in an NSWindow");
            };

            let fullscreen = ns_window
                .styleMask()
                .contains(NSWindowStyleMask::FullScreen);
            if let Some(frame_view) = ns_window.contentView().and_then(|cv| cv.superview()) {
                if fullscreen {
                    if let Some(bar) = find_titlebar_backing_bar(&frame_view) {
                        bar.setHidden(true);
                    }
                } else if find_titlebar_backing_bar(&frame_view).is_some() {
                    // Re-derive the strip as well as showing it: a fullscreen
                    // transition can leave the bar with a stale frame. Only
                    // an EXISTING bar is refreshed — this runs on every
                    // Resized, and creating one here would reinstall the bar
                    // at 100% opacity, where the config path removes it.
                    // (If AppKit moved the content view to a fresh frame
                    // view and orphaned the old bar, the next translucency
                    // config change reinstalls a correctly-framed one.)
                    ensure_titlebar_backing_bar(&ns_window)?;
                }
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
    }

    Ok(())
}
