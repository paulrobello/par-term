//! GPU renderer state-upload operations (phases 1–2 of the frame pipeline).
//!
//! `update_gpu_renderer_state` transfers per-frame terminal state to the GPU:
//! cells, cursor, progress bars, scrollbar, gutter indicators, animations,
//! inline graphics, visual bell, and scrollbar mark hit-testing.
//!
//! Extracted from `gpu_submit.rs` to keep each file under the 800-line limit.

use super::prettifier_cells;
use super::types::RendererSizing;
use crate::config::color_u8_to_f32_a;
use crate::progress_bar::ProgressBarSnapshot;
use crate::renderer::Renderer;
use crate::ui_constants::{SCROLLBAR_MARK_HIT_RADIUS_PX, VISUAL_BELL_FLASH_DURATION_MS};

/// Parameters for [`update_gpu_renderer_state`].
pub(super) struct GpuStateUpdateParams<'a> {
    pub(super) tab_manager: &'a mut crate::tab::TabManager,
    pub(super) config: &'a crate::config::Config,
    pub(super) cursor_anim: &'a crate::app::cursor_anim_state::CursorAnimState,
    pub(super) window: &'a Option<std::sync::Arc<winit::window::Window>>,
    pub(super) debug: &'a crate::app::debug_state::DebugState,
    pub(super) cells: &'a [crate::cell_renderer::Cell],
    pub(super) current_cursor_pos: Option<(usize, usize)>,
    pub(super) cursor_style: Option<par_term_emu_core_rust::cursor::CursorStyle>,
    pub(super) progress_snapshot: &'a Option<ProgressBarSnapshot>,
    pub(super) prettifier_graphics: &'a [prettifier_cells::PrettifierGraphic],
    pub(super) scroll_offset: usize,
    pub(super) visible_lines: usize,
    pub(super) scrollback_len: usize,
    pub(super) total_lines: usize,
    pub(super) is_alt_screen: bool,
    pub(super) scrollback_marks: &'a [crate::scrollback_metadata::ScrollbackMark],
    pub(super) status_bar_height: f32,
    pub(super) custom_status_bar_height: f32,
}

/// Values produced by the GPU-state-upload phase and consumed by the render phase.
pub(super) struct GpuUploadResult {
    pub(super) debug_update_cells_time: std::time::Duration,
    pub(super) debug_graphics_time: std::time::Duration,
    pub(super) debug_anim_time: std::time::Duration,
    pub(super) sizing: RendererSizing,
    pub(super) hovered_mark: Option<crate::scrollback_metadata::ScrollbackMark>,
}

/// Upload GPU state for the current frame (phases 1–2).
///
/// Handles all GPU data transfers:
/// - Cell and cursor data upload
/// - Progress bar shader uniforms
/// - Scrollbar position and marks
/// - Gutter indicators for prettified blocks
/// - Animation frame updates
/// - Terminal graphics (Sixel/iTerm2/Kitty and prettifier diagrams)
/// - Visual bell flash intensity
///
/// Returns timing measurements and the computed renderer sizing for use in phase 4.
pub(super) fn update_gpu_renderer_state(
    renderer: &mut Renderer,
    p: GpuStateUpdateParams<'_>,
) -> GpuUploadResult {
    let GpuStateUpdateParams {
        tab_manager,
        config,
        cursor_anim,
        window,
        debug,
        cells,
        current_cursor_pos,
        cursor_style,
        progress_snapshot,
        prettifier_graphics,
        scroll_offset,
        visible_lines,
        scrollback_len,
        total_lines,
        is_alt_screen,
        scrollback_marks,
        status_bar_height,
        custom_status_bar_height,
    } = p;
    let mut debug_update_cells_time = std::time::Duration::ZERO;
    #[allow(unused_assignments)]
    let mut debug_graphics_time = std::time::Duration::ZERO;

    // Disable cursor shader when alt screen is active (TUI apps like vim, htop)
    let disable_cursor_shader = config.shader.cursor_shader_disable_in_alt_screen && is_alt_screen;
    renderer.set_cursor_shader_disabled_for_alt_screen(disable_cursor_shader);

    // Only update renderer with cells if they changed (cache MISS).
    // This avoids re-uploading the same cell data to GPU on every frame.
    if !debug.cache_hit {
        let t = std::time::Instant::now();
        renderer.update_cells(cells);
        debug_update_cells_time = t.elapsed();
    }

    // Update cursor position and style for geometric rendering
    if let (Some(pos), Some(opacity), Some(style)) = (
        current_cursor_pos,
        Some(cursor_anim.cursor_opacity),
        cursor_style,
    ) {
        renderer.update_cursor(pos, opacity, style);
        // Forward cursor state to custom shader for Ghostty-compatible cursor animations
        let cursor_color = color_u8_to_f32_a(config.cursor_color, 1.0);
        renderer.update_shader_cursor(pos.0, pos.1, opacity, cursor_color, style);
    } else {
        renderer.clear_cursor();
    }

    // Update progress bar state for shader uniforms
    if let Some(snap) = progress_snapshot {
        use par_term_emu_core_rust::terminal::ProgressState;
        let state_val = match snap.simple.state {
            ProgressState::Hidden => 0.0,
            ProgressState::Normal => 1.0,
            ProgressState::Error => 2.0,
            ProgressState::Indeterminate => 3.0,
            ProgressState::Warning => 4.0,
        };
        let active_count = (if snap.simple.is_active() { 1 } else { 0 })
            + snap.named.values().filter(|b| b.state.is_active()).count();
        renderer.update_shader_progress(
            state_val,
            snap.simple.progress as f32 / 100.0,
            if snap.has_active() { 1.0 } else { 0.0 },
            active_count as f32,
        );
    } else {
        renderer.update_shader_progress(0.0, 0.0, 0.0, 0.0);
    }

    // Update scrollbar
    renderer.update_scrollbar(scroll_offset, visible_lines, total_lines, scrollback_marks);

    // Compute and set command separator marks for single-pane rendering
    if config.command_separator_enabled {
        let separator_marks = crate::renderer::compute_visible_separator_marks(
            scrollback_marks,
            scrollback_len,
            scroll_offset,
            visible_lines,
        );
        renderer.set_separator_marks(separator_marks);
    } else {
        renderer.set_separator_marks(Vec::new());
    }

    // Compute and set gutter indicators for prettified blocks
    {
        let gutter_data = if let Some(tab) = tab_manager.active_tab() {
            if let Some(ref pipeline) = tab.prettifier {
                if pipeline.is_enabled() {
                    let indicators = tab.gutter_manager.indicators_for_viewport(
                        pipeline,
                        scroll_offset,
                        visible_lines,
                    );
                    let gutter_color = [0.3, 0.5, 0.8, 0.15];
                    indicators
                        .iter()
                        .flat_map(|ind| {
                            (ind.row..ind.row + ind.height).map(move |r| (r, gutter_color))
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        renderer.set_gutter_indicators(gutter_data);
    }

    // Update animations and request redraw if frames changed.
    // Use try_write() to avoid blocking the event loop when PTY reader holds the lock.
    let anim_start = std::time::Instant::now();
    if let Some(tab) = tab_manager.active_tab()
        && let Ok(terminal) = tab.terminal.try_write()
        && terminal.update_animations()
    {
        // Animation frame changed — request continuous redraws.
        // NOTE: Cannot use self.request_redraw() here because &mut renderer is held.
        if let Some(w) = window {
            w.request_redraw();
        }
    }
    let debug_anim_time = anim_start.elapsed();

    // Update graphics from terminal.
    // In split-pane mode each pane has its own PTY terminal; graphics are collected
    // per-pane inside the pane data gather loop and do not go through here.
    let graphics_start = std::time::Instant::now();
    let has_pane_manager_for_graphics = tab_manager
        .active_tab()
        .and_then(|t| t.pane_manager.as_ref())
        .map(|pm| pm.pane_count() > 0)
        .unwrap_or(false);
    if !has_pane_manager_for_graphics
        && let Some(tab) = tab_manager.active_tab()
        && let Ok(terminal) = tab.terminal.try_write()
    {
        let mut graphics = terminal.get_graphics_with_animations();
        let scrollback_len_for_gfx = terminal.scrollback_len();

        let scrollback_graphics = terminal.get_scrollback_graphics();
        let scrollback_count = scrollback_graphics.len();
        graphics.extend(scrollback_graphics);

        crate::debug_info!(
            "APP",
            "Got {} graphics ({} scrollback) from terminal (scroll_offset={}, scrollback_len={})",
            graphics.len(),
            scrollback_count,
            scroll_offset,
            scrollback_len_for_gfx
        );
        if let Err(e) = renderer.update_graphics(
            &graphics,
            scroll_offset,
            scrollback_len_for_gfx,
            visible_lines,
        ) {
            log::error!("Failed to update graphics: {}", e);
        }
    }
    debug_graphics_time = graphics_start.elapsed();

    // Upload prettifier diagram graphics (rendered Mermaid, etc.) to the GPU.
    if !prettifier_graphics.is_empty() {
        #[allow(clippy::type_complexity)]
        let refs: Vec<(u64, &[u8], u32, u32, isize, usize)> = prettifier_graphics
            .iter()
            .map(|(id, data, w, h, row, col)| (*id, data.as_slice(), *w, *h, *row, *col))
            .collect();
        if let Err(e) = renderer.update_prettifier_graphics(&refs) {
            crate::debug_error!("PRETTIFIER", "Failed to upload prettifier graphics: {}", e);
        }
    }

    // Calculate visual bell flash intensity (0.0 = no flash, 1.0 = full flash)
    let visual_bell_flash = tab_manager
        .active_tab()
        .and_then(|t| t.active_bell().visual_flash);
    let visual_bell_intensity = if let Some(flash_start) = visual_bell_flash {
        let elapsed = flash_start.elapsed().as_millis();
        if elapsed < VISUAL_BELL_FLASH_DURATION_MS {
            // Request continuous redraws while flash is active.
            // NOTE: Cannot use self.request_redraw() here because &mut renderer is held.
            if let Some(w) = window {
                w.request_redraw();
            }
            0.3 * (1.0 - (elapsed as f32 / VISUAL_BELL_FLASH_DURATION_MS as f32))
        } else {
            0.0
        }
    } else {
        0.0
    };
    renderer.set_visual_bell_intensity(visual_bell_intensity);

    // Compute hovered scrollbar mark for tooltip display
    let hovered_mark: Option<crate::scrollback_metadata::ScrollbackMark> =
        if config.scrollbar_mark_tooltips && config.scrollbar_command_marks {
            tab_manager
                .active_tab()
                .map(|tab| tab.active_mouse().position)
                .and_then(|(mx, my)| {
                    renderer.scrollbar_mark_at_position(
                        mx as f32,
                        my as f32,
                        SCROLLBAR_MARK_HIT_RADIUS_PX,
                    )
                })
                .cloned()
        } else {
            None
        };

    // Extract renderer sizing info for split-pane calculations
    let sizing = RendererSizing {
        size: renderer.size(),
        content_offset_y: renderer.content_offset_y(),
        content_offset_x: renderer.content_offset_x(),
        content_inset_bottom: renderer.content_inset_bottom(),
        content_inset_right: renderer.content_inset_right(),
        cell_width: renderer.cell_width(),
        cell_height: renderer.cell_height(),
        padding: renderer.window_padding(),
        status_bar_height: (status_bar_height + custom_status_bar_height) * renderer.scale_factor(),
        scale_factor: renderer.scale_factor(),
    };

    GpuUploadResult {
        debug_update_cells_time,
        debug_graphics_time,
        debug_anim_time,
        sizing,
        hovered_mark,
    }
}
