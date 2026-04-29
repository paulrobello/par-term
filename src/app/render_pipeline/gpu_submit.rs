//! GPU frame submission for the render pipeline.
//!
//! `submit_gpu_frame` drives the full egui + wgpu render pass for one frame:
//! - egui overlay rendering (FPS, toast, tab bar, all dialogs)
//! - Cell / cursor / progress / scrollbar / graphics upload to the GPU
//! - Split-pane or single-pane wgpu render call
//!
//! Returns a `PostRenderActions` bundle that the caller dispatches after the
//! renderer borrow is released (`update_post_render_state` in `post_render.rs`).
//!
//! GPU state upload (phases 1–2) lives in `renderer_ops.rs`.
//! Standalone egui overlay free functions live in `egui_overlays.rs`.
//! The `render_egui_frame` method and `RenderEguiParams` live in `egui_submit.rs`.

use super::egui_submit::{RenderEguiParams, scroll_offset_from_tab};
use super::pane_render;
use super::renderer_ops::{GpuStateUpdateParams, update_gpu_renderer_state};
use super::types::{FrameRenderData, PostRenderActions};
use crate::app::window_state::WindowState;
use crate::progress_bar::ProgressBarSnapshot;
use crate::ui_constants::VISUAL_BELL_FLASH_DURATION_MS;
use wgpu::SurfaceError;

impl WindowState {
    /// Run egui overlays and GPU render pass.
    /// Returns collected post-render actions to handle after the renderer borrow is released.
    pub(super) fn submit_gpu_frame(&mut self, frame_data: FrameRenderData) -> PostRenderActions {
        let FrameRenderData {
            cells,
            cursor_pos: current_cursor_pos,
            cursor_style,
            is_alt_screen,
            scrollback_len,
            show_scrollbar,
            visible_lines,
            scrollback_marks,
            total_lines,
            debug_url_detect_time,
        } = frame_data;

        let mut actions = PostRenderActions::default();

        let render_start = std::time::Instant::now();

        #[allow(unused_assignments)]
        let mut debug_actual_render_time = std::time::Duration::ZERO;
        let _ = &debug_actual_render_time;

        // Process agent messages and refresh AI Inspector snapshot
        self.process_agent_messages_tick();

        // Check tmux gateway state before renderer borrow to avoid borrow conflicts.
        // Note: pane_padding is in logical pixels (config); we defer DPI scaling to
        // where it's used with physical pixel coordinates (via sizing.scale_factor).
        let is_tmux_gateway = self.is_gateway_active();
        // Compute pane_count early (only needs tab_manager, not renderer) so we can
        // suppress padding when there is only one pane (no visible dividers).
        let active_pane_count = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager.as_ref())
            .map(|pm| pm.pane_count())
            .unwrap_or(0);
        // In split mode, add half the divider width as a mandatory base so content
        // doesn't render under the divider line, plus the user-configured extra padding.
        // Single-pane and tmux-gateway modes use zero padding (no dividers present).
        let effective_pane_padding = if is_tmux_gateway || active_pane_count <= 1 {
            0.0
        } else {
            self.config.pane_divider_width.unwrap_or(2.0) / 2.0 + self.config.pane_padding
        };

        // Calculate status bar heights before mutable renderer borrow.
        // Note: These are in logical pixels; they get scaled to physical in RendererSizing.
        let is_tmux_connected = self.is_tmux_connected();
        let status_bar_height =
            crate::tmux_status_bar_ui::TmuxStatusBarUI::height(&self.config, is_tmux_connected);
        let custom_status_bar_height = self.status_bar_ui.height(&self.config, self.is_fullscreen);

        // Capture window size before mutable borrow (for badge rendering in egui)
        let window_size_for_badge = self.renderer.as_ref().map(|r| r.size());

        // Capture progress bar snapshot before mutable borrow
        let progress_snapshot = if self.config.progress_bar_enabled {
            self.tab_manager.active_tab().and_then(|tab| {
                tab.terminal
                    .try_write()
                    .ok()
                    .map(|term| ProgressBarSnapshot {
                        simple: term.progress_bar(),
                        named: term.named_progress_bars(),
                    })
            })
        } else {
            None
        };

        // Sync AI Inspector panel width before scrollbar update so the scrollbar
        // position uses the current panel width on this frame (not the previous one).
        self.sync_ai_inspector_width();

        // Cache modal visibility before entering the renderer borrow scope.
        // Method calls borrow all of `self`, which conflicts with `&mut self.renderer`.
        let any_modal_visible = self.any_modal_ui_visible();

        // =====================================================================
        // Phase 1-2: GPU state upload
        //
        // Upload cell data, cursor, scrollbar, animations, and graphics to the GPU.
        // Produces `GpuUploadResult` (timing + sizing + hovered scrollbar mark).
        // Delegated to `renderer_ops::update_gpu_renderer_state`.
        // =====================================================================

        // Compute scroll offset before taking a mutable renderer borrow to avoid
        // simultaneous &mut self.tab_manager and &self.tab_manager in the same call.
        let scroll_offset = scroll_offset_from_tab(&self.tab_manager);

        let gpu_result = if let Some(renderer) = &mut self.renderer {
            Some(update_gpu_renderer_state(
                renderer,
                GpuStateUpdateParams {
                    tab_manager: &mut self.tab_manager,
                    config: &self.config,
                    cursor_anim: &self.cursor_anim,
                    window: &self.window,
                    debug: &self.debug,
                    cells: &cells,
                    current_cursor_pos,
                    cursor_style,
                    progress_snapshot: &progress_snapshot,
                    scroll_offset,
                    visible_lines,
                    scrollback_len,
                    total_lines,
                    is_alt_screen,
                    scrollback_marks: &scrollback_marks,
                    status_bar_height,
                    custom_status_bar_height,
                },
            ))
        } else {
            None
        };

        // Clear visual bell if its duration has elapsed.
        // This is separate from the GPU upload to avoid borrow conflicts.
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            let flash = tab.active_bell().visual_flash;
            if let Some(flash_start) = flash
                && flash_start.elapsed().as_millis() >= VISUAL_BELL_FLASH_DURATION_MS
            {
                tab.active_bell_mut().visual_flash = None;
            }
        }

        // =====================================================================
        // Phase 3: egui overlay rendering
        //
        // Run egui dialogs, overlays, and UI panels. Returns egui output for the
        // render phase and updates `actions` with deferred UI responses.
        // =====================================================================
        let egui_data = if let Some(ref gpu) = gpu_result {
            self.render_egui_frame(RenderEguiParams {
                actions: &mut actions,
                hovered_mark: &gpu.hovered_mark,
                window_size_for_badge: window_size_for_badge.as_ref(),
                progress_snapshot: &progress_snapshot,
                visible_lines,
                scrollback_len,
                any_modal_visible,
                show_scrollbar,
            })
        } else {
            None
        };

        // =====================================================================
        // Phase 4-5: Frame submission and timing
        // =====================================================================
        if let (Some(renderer), Some(gpu)) = (&mut self.renderer, gpu_result) {
            let super::renderer_ops::GpuUploadResult {
                debug_update_cells_time,
                debug_graphics_time,
                debug_anim_time,
                sizing,
                ..
            } = gpu;

            let debug_egui_time = self.debug.last_egui_time;

            // Calculate FPS and timing stats
            let avg_frame_time = if !self.debug.frame_times.is_empty() {
                self.debug.frame_times.iter().sum::<std::time::Duration>()
                    / self.debug.frame_times.len() as u32
            } else {
                std::time::Duration::ZERO
            };
            let fps = if avg_frame_time.as_secs_f64() > 0.0 {
                1.0 / avg_frame_time.as_secs_f64()
            } else {
                0.0
            };

            // Update FPS value for overlay display
            self.debug.fps_value = fps;

            // Log debug info every 60 frames (about once per second at 60 FPS)
            if self.debug.frame_times.len() >= 60 {
                let (cache_gen, cache_has_cells) = self
                    .tab_manager
                    .active_tab()
                    .map(|t| {
                        (
                            t.active_cache().generation,
                            t.active_cache().cells.is_some(),
                        )
                    })
                    .unwrap_or((0, false));
                log::info!(
                    "PERF: FPS={:.1} Frame={:.2}ms CellGen={:.2}ms({}) URLDetect={:.2}ms Anim={:.2}ms Graphics={:.2}ms egui={:.2}ms UpdateCells={:.2}ms ActualRender={:.2}ms Total={:.2}ms Cells={} Gen={} Cache={}",
                    fps,
                    avg_frame_time.as_secs_f64() * 1000.0,
                    self.debug.cell_gen_time.as_secs_f64() * 1000.0,
                    if self.debug.cache_hit { "HIT" } else { "MISS" },
                    debug_url_detect_time.as_secs_f64() * 1000.0,
                    debug_anim_time.as_secs_f64() * 1000.0,
                    debug_graphics_time.as_secs_f64() * 1000.0,
                    debug_egui_time.as_secs_f64() * 1000.0,
                    debug_update_cells_time.as_secs_f64() * 1000.0,
                    debug_actual_render_time.as_secs_f64() * 1000.0,
                    self.debug.render_time.as_secs_f64() * 1000.0,
                    cells.len(),
                    cache_gen,
                    if cache_has_cells { "YES" } else { "NO" }
                );
            }

            // pane_count was already computed before the renderer borrow to inform
            // effective_pane_padding; reuse it here.
            let pane_count = active_pane_count;

            crate::debug_trace!("RENDER", "pane_count={}", pane_count);

            // render_egui_frame returns Option<(FullOutput, Context)> with an owned Context
            // (a cheap Arc clone). The downstream render functions expect
            // Option<(FullOutput, &Context)>. We split the tuple so the Context lives in
            // a separate binding that outlives its borrow in the render call.
            let (egui_output, egui_ctx_store) = match egui_data {
                Some((output, ctx)) => (Some(output), Some(ctx)),
                None => (None, None),
            };

            let actual_render_start = std::time::Instant::now();
            let render_result = if pane_count > 0 {
                // Gather all per-pane render data.
                let pane_render_data = self.tab_manager.active_tab_mut().and_then(|tab| {
                    pane_render::gather_pane_render_data(
                        tab,
                        &self.config,
                        &sizing,
                        effective_pane_padding,
                        self.cursor_anim.cursor_opacity,
                        pane_count,
                        // Always pass scrollbar width in split-pane mode so all
                        // panes have a stable column count regardless of focus or
                        // scrollback state — prevents layout reflow on pane click.
                        sizing.scrollbar_width,
                    )
                });

                if let Some((
                    mut pane_data,
                    dividers,
                    pane_titles,
                    focused_viewport,
                    focused_pane_scrollback_len,
                )) = pane_render_data
                {
                    // Update tab cache with the focused pane's scrollback_len so that scroll
                    // operations see the correct limit. Always write (even when 0) so
                    // that a newly-split pane with no scrollback clears any stale value.
                    if let Some(tab) = self.tab_manager.active_tab_mut() {
                        tab.active_cache_mut().scrollback_len = focused_pane_scrollback_len;
                    }

                    // Apply transient text overlays.
                    // Search highlights and URL underlines must be final-layer cell
                    // mutations.
                    {
                        let search_matches = self.overlay_ui.search_ui.matches();
                        let has_search_matches = !search_matches.is_empty();
                        let current_match_idx = self.overlay_ui.search_ui.current_match_index();
                        let highlight_color = self.config.search.search_highlight_color;
                        let current_highlight_color =
                            self.config.search.search_current_highlight_color;

                        let url_overlay = self.tab_manager.active_tab().and_then(|tab| {
                            let detected_urls = &tab.active_mouse().detected_urls;
                            if detected_urls.is_empty() {
                                return None;
                            }
                            let c = self.config.link_highlight_color;
                            Some((
                                detected_urls,
                                tab.active_mouse().url_detect_scroll_offset,
                                tab.active_mouse().hovered_url_bounds,
                                [c[0], c[1], c[2], 255],
                                self.config.link_highlight_color_enabled,
                                self.config.link_highlight_underline,
                            ))
                        });

                        if has_search_matches || url_overlay.is_some() {
                            for pane in &mut pane_data {
                                if pane.viewport.focused {
                                    let cells = std::sync::Arc::make_mut(&mut pane.cells);
                                    if has_search_matches {
                                        crate::app::window_state::search_highlight::apply_search_highlights_to_cells(
                                            crate::app::window_state::search_highlight::SearchHighlightParams {
                                                cells,
                                                cols: pane.grid_size.0,
                                                scroll_offset: pane.scroll_offset,
                                                scrollback_len: pane.scrollback_len,
                                                visible_lines: pane.grid_size.1,
                                                matches: search_matches,
                                                current_match_idx,
                                                highlight_color,
                                                current_highlight_color,
                                            },
                                        );
                                    }

                                    if let Some((
                                        detected_urls,
                                        url_scroll_offset,
                                        hovered_bounds,
                                        url_color,
                                        do_color,
                                        do_underline,
                                    )) = url_overlay
                                    {
                                        super::overlay_cells::apply_url_overlays_to_cells(
                                            super::overlay_cells::UrlOverlayParams {
                                                cells,
                                                cols: pane.grid_size.0,
                                                detected_urls,
                                                url_scroll_offset,
                                                hovered_bounds,
                                                url_color,
                                                do_color,
                                                do_underline,
                                            },
                                        );
                                    }
                                    break; // Only one focused pane
                                }
                            }
                        }
                    }

                    // Get hovered divider index for hover color rendering
                    let hovered_divider_index = self
                        .tab_manager
                        .active_tab()
                        .and_then(|t| t.active_mouse().hovered_divider_index);

                    // Recombine output and a reference to the context.
                    let split_egui: Option<(egui::FullOutput, &egui::Context)> =
                        egui_output.zip(egui_ctx_store.as_ref());

                    // Render split panes
                    Self::render_split_panes_with_data(
                        renderer,
                        pane_render::SplitPaneRenderParams {
                            pane_data,
                            dividers,
                            pane_titles,
                            focused_viewport,
                            config: &self.config,
                            egui_data: split_egui,
                            hovered_divider_index,
                            show_scrollbar,
                        },
                    )
                } else {
                    // No active tab during render — skip this frame.
                    crate::debug_error!(
                        "RENDER",
                        "gather_pane_render_data returned None with pane_count={}",
                        pane_count
                    );
                    Ok(false)
                }
            } else {
                // No active tab — nothing to render.
                Ok(false)
            };

            match render_result {
                Ok(rendered) => {
                    if !rendered {
                        log::trace!("Skipped rendering - no changes");
                    }
                }
                Err(e) => {
                    if let Some(surface_error) = e.downcast_ref::<SurfaceError>() {
                        match surface_error {
                            SurfaceError::Outdated | SurfaceError::Lost => {
                                log::warn!(
                                    "Surface error detected ({:?}), reconfiguring...",
                                    surface_error
                                );
                                self.force_surface_reconfigure();
                            }
                            SurfaceError::Timeout => {
                                log::warn!("Surface timeout, will retry next frame");
                                self.request_redraw();
                            }
                            SurfaceError::OutOfMemory => {
                                log::error!("Surface out of memory: {:?}", surface_error);
                            }
                            _ => {
                                log::error!("Surface error: {:?}", surface_error);
                            }
                        }
                    } else {
                        log::error!("Render error: {}", e);
                    }
                }
            }
            debug_actual_render_time = actual_render_start.elapsed();
            let _ = debug_actual_render_time;

            self.debug.render_time = render_start.elapsed();
        }

        actions
    }
}
