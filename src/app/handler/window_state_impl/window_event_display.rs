//! Display-geometry arms of `WindowState::handle_window_event` (free
//! functions per the ARC-001 impl-block budget).

use crate::app::window_state::WindowState;

/// A scale-factor change is a display change: reconfigure through the
/// present-mode-cycling path and resize every tab's terminal with pixel
/// dimensions (TIOCGWINSZ support).
pub(super) fn scale_factor_changed(ws: &mut WindowState, scale_factor: f64) {
    if let (Some(renderer), Some(window)) = (&mut ws.renderer, &ws.window) {
        log::info!(
            "Scale factor changed to {} (display change detected)",
            scale_factor
        );

        let size = window.inner_size();
        let (cols, rows) = renderer.handle_scale_factor_change(scale_factor, size);

        // A scale-factor change is a display change, and a
        // same-config reconfigure heals nothing (measured
        // 2026-09-21) — recover through the present-mode-cycling
        // path instead, which also covers an unchanged extent.
        renderer.reconfigure_after_display_change(size.width, size.height);
        // Calculate pixel dimensions
        let cell_width = renderer.cell_width();
        let cell_height = renderer.cell_height();
        let width_px = (cols as f32 * cell_width) as usize;
        let height_px = (rows as f32 * cell_height) as usize;

        // Resize all tabs' terminals with pixel dimensions for TIOCGWINSZ support
        for tab in ws.tab_manager.tabs_mut() {
            // try_lock: intentional — resize happens during ScaleFactorChanged
            // which fires in the sync event loop. On miss: this tab's terminal
            // keeps its old size until the next resize event. Low risk as scale
            // factor changes are rare (drag between displays).
            if let Ok(mut term) = tab.terminal.try_write() {
                if let Err(e) = term.resize_with_pixels(cols, rows, width_px, height_px) {
                    crate::debug_error!(
                        "TERMINAL",
                        "resize_with_pixels failed (scale_factor): {e}"
                    );
                }
            } else {
                crate::debug::record_try_lock_failure("scale_factor_resize");
            }
        }

        // Request redraw to apply changes
        window.request_redraw();
    }
}

/// Handle window moved to a different monitor: edge-triggered on an actual
/// monitor crossing (`display_recovery::WindowMoveGate`) — a same-config
/// `Surface::configure` heals nothing and one configure per move event is a
/// strobe candidate under wgpu 30.
pub(super) fn moved(ws: &mut WindowState) {
    let crossed_monitors = match ws.window.as_deref() {
        Some(window) => ws.window_move_gate.observe(window.current_monitor()),
        None => false,
    };
    if crossed_monitors && let (Some(renderer), Some(window)) = (&mut ws.renderer, &ws.window) {
        log::warn!("Window moved to a different monitor — healing reconfigure");

        let size = window.inner_size();
        renderer.reconfigure_after_display_change(size.width, size.height);

        // Request redraw to ensure proper rendering on new display
        window.request_redraw();
    }
}

/// Resize terminals (scrollback reflow), scrollbar marks, the resize
/// overlay, then snap the window to grid-cell boundaries.
pub(super) fn resized(ws: &mut WindowState, physical_size: winit::dpi::PhysicalSize<u32>) {
    // Fullscreen transitions (keyboard, menu, green button) all emit
    // Resized — sync the macOS titlebar backing bar's visibility here
    // so no transition path is missed. No-op when translucency is off.
    if let Some(window) = &ws.window {
        let _ = crate::macos_metal::sync_titlebar_backing_bar_visibility(window);
    }

    if let Some(renderer) = &mut ws.renderer {
        let (cols, rows) = renderer.resize(physical_size);

        // Calculate text area pixel dimensions
        let cell_width = renderer.cell_width();
        let cell_height = renderer.cell_height();
        let width_px = (cols as f32 * cell_width) as usize;
        let height_px = (rows as f32 * cell_height) as usize;

        // Resize all tabs' terminals with pixel dimensions for TIOCGWINSZ support
        // This allows applications like kitty icat to query pixel dimensions
        // Note: The core library (v0.11.0+) implements scrollback reflow when
        // width changes - wrapped lines are unwrapped/re-wrapped as needed.
        for tab in ws.tab_manager.tabs_mut() {
            // try_lock: intentional — Resized fires in the sync event loop.
            // On miss: this tab's terminal keeps its old dimensions; the cell
            // cache is still invalidated below so rendering uses the correct
            // grid size. The terminal size will be fixed on the next resize event.
            let new_scrollback_len = if let Ok(mut term) = tab.terminal.try_write() {
                if let Err(e) = term.resize_with_pixels(cols, rows, width_px, height_px) {
                    crate::debug_error!("TERMINAL", "resize_with_pixels failed (Resized): {e}");
                }
                Some(term.scrollback_len())
            } else {
                crate::debug::record_try_lock_failure("resize");
                None
            };
            if let Some(sl) = new_scrollback_len {
                tab.active_cache_mut().scrollback_len = sl;
            }
            // Invalidate cell cache to force regeneration
            tab.active_cache_mut().cells = None;
        }

        // Update scrollbar for active tab
        if let Some(tab) = ws.tab_manager.active_tab() {
            let total_lines = rows + tab.active_cache().scrollback_len;
            // try_lock: intentional — scrollbar mark update during Resized event.
            // On miss: scrollbar renders without marks this frame. Cosmetic only.
            let marks = tab
                .try_with_terminal(|term| term.scrollback_marks())
                .unwrap_or_default();
            renderer.update_scrollbar(tab.active_scroll_state().offset, rows, total_lines, &marks);
        }

        // Update resize overlay state
        ws.overlay_state.resize_dimensions =
            Some((physical_size.width, physical_size.height, cols, rows));
        ws.overlay_state.resize_overlay_visible = true;
        // Hide overlay 1 second after resize stops
        ws.overlay_state.resize_overlay_hide_time =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(1));

        // Notify tmux of the new size if gateway mode is active
        ws.notify_tmux_of_resize();

        // --- Snap window to grid cell boundaries ---
        //
        // Goal: eliminate the partial-cell gap between the terminal grid
        // and the window edge that appears after a user drag.
        //
        // Anti-loop guard: if this Resized event IS the response to our
        // own request_inner_size call, skip — we're already at the snapped size.
        // We allow a ±1 px tolerance because the OS may round to even physical
        // pixels (e.g. Retina 2× requires integer logical pixels).
        if let Some(pending) = ws.pending_snap_size.take() {
            let dw = (pending.width as i32 - physical_size.width as i32).unsigned_abs();
            let dh = (pending.height as i32 - physical_size.height as i32).unsigned_abs();
            if dw > 1 || dh > 1 {
                // Not our snap response (concurrent user drag or OS constraint).
                // Clear pending — the snap logic below will re-evaluate.
                crate::debug_info!(
                    "RESIZE",
                    "snap guard: pending {}x{} != physical {}x{} (dw={} dh={}), clearing",
                    pending.width,
                    pending.height,
                    physical_size.width,
                    physical_size.height,
                    dw,
                    dh
                );
            }
            // else: this resize was triggered by our own snap request — done.
        }

        if ws.pending_snap_size.is_none() && ws.config.load().window.snap_window_to_grid {
            // Only snap in single-pane mode (split pane handled separately).
            let is_split = ws
                .tab_manager
                .active_tab()
                .map(|t| t.pane_count() > 1)
                .unwrap_or(false);

            if !is_split && let Some(renderer) = &ws.renderer {
                let (chrome_x, chrome_y) = renderer.chrome_overhead();
                let cell_w = renderer.cell_width();
                let cell_h = renderer.cell_height();
                let snapped_w = (chrome_x + cols as f32 * cell_w).round() as u32;
                let snapped_h = (chrome_y + rows as f32 * cell_h).round() as u32;

                crate::debug_info!(
                    "RESIZE",
                    "snap: physical={}x{} snapped={}x{} chrome=({:.1},{:.1}) cell=({:.1},{:.1}) grid={}x{}",
                    physical_size.width,
                    physical_size.height,
                    snapped_w,
                    snapped_h,
                    chrome_x,
                    chrome_y,
                    cell_w,
                    cell_h,
                    cols,
                    rows
                );

                if snapped_w != physical_size.width || snapped_h != physical_size.height {
                    let snapped = winit::dpi::PhysicalSize::new(snapped_w, snapped_h);
                    ws.pending_snap_size = Some(snapped);
                    ws.with_window(|w| {
                        let _ = w.request_inner_size(snapped);
                    });
                }
            }
        }
    }
}
