//! Split-pane rendering helpers.
//!
//! Contains:
//! - `PaneRenderData`: per-pane snapshot used during `submit_gpu_frame`
//! - `render_split_panes_with_data`: drives the GPU split-pane render pass

use crate::cell_renderer::PaneViewport;
use crate::config::{Config, color_u8_to_f32};
use crate::renderer::{DividerRenderInfo, PaneDividerSettings, PaneRenderInfo, PaneTitleInfo, Renderer};
use crate::scrollback_metadata::ScrollbackMark;
use anyhow::Result;

/// Pane render data for split pane rendering
pub(super) struct PaneRenderData {
    /// Viewport bounds and state for this pane
    pub(super) viewport: PaneViewport,
    /// Cells to render (should match viewport grid size)
    pub(super) cells: Vec<crate::cell_renderer::Cell>,
    /// Grid dimensions (cols, rows)
    pub(super) grid_size: (usize, usize),
    /// Cursor position within this pane (col, row), or None if no cursor visible
    pub(super) cursor_pos: Option<(usize, usize)>,
    /// Cursor opacity (0.0 = hidden, 1.0 = fully visible)
    pub(super) cursor_opacity: f32,
    /// Scrollback marks for this pane
    pub(super) marks: Vec<ScrollbackMark>,
    /// Scrollback length for this pane (needed for separator mark mapping)
    pub(super) scrollback_len: usize,
    /// Current scroll offset for this pane (needed for separator mark mapping)
    pub(super) scroll_offset: usize,
    /// Per-pane background image override (None = use global background)
    pub(super) background: Option<crate::pane::PaneBackground>,
    /// Inline graphics (Sixel/iTerm2/Kitty) to render for this pane
    pub(super) graphics: Vec<par_term_emu_core_rust::graphics::TerminalGraphic>,
}

impl crate::app::window_state::WindowState {
    /// Render split panes when the active tab has multiple panes
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_split_panes_with_data(
        renderer: &mut Renderer,
        pane_data: Vec<PaneRenderData>,
        dividers: Vec<crate::pane::DividerRect>,
        pane_titles: Vec<PaneTitleInfo>,
        focused_viewport: Option<PaneViewport>,
        config: &Config,
        egui_data: Option<(egui::FullOutput, &egui::Context)>,
        hovered_divider_index: Option<usize>,
        show_scrollbar: bool,
    ) -> Result<bool> {
        // Two-phase construction: separate owned cell data from pane metadata
        // so PaneRenderInfo can borrow cell slices safely.  This replaces the
        // previous unsafe Box::into_raw / Box::from_raw pattern that leaked
        // memory if render_split_panes panicked.
        //
        // Phase 1: Extract cells into a Vec that outlives the render infos.
        // The remaining pane fields are collected into partial render infos.
        let mut owned_cells: Vec<Vec<crate::cell_renderer::Cell>> =
            Vec::with_capacity(pane_data.len());
        let mut partial_infos: Vec<PaneRenderInfo> = Vec::with_capacity(pane_data.len());

        for pane in pane_data {
            let focused = pane.viewport.focused;
            owned_cells.push(pane.cells);
            partial_infos.push(PaneRenderInfo {
                viewport: pane.viewport,
                // Placeholder — will be patched in Phase 2 once owned_cells
                // is finished growing and its elements have stable addresses.
                cells: &[],
                grid_size: pane.grid_size,
                cursor_pos: pane.cursor_pos,
                cursor_opacity: pane.cursor_opacity,
                show_scrollbar: show_scrollbar && focused,
                marks: pane.marks,
                scrollback_len: pane.scrollback_len,
                scroll_offset: pane.scroll_offset,
                background: pane.background,
                graphics: pane.graphics,
            });
        }

        // Phase 2: Patch cell references now that owned_cells won't reallocate.
        // owned_cells lives until scope exit (even on panic), so the borrows
        // are valid for the lifetime of partial_infos.
        for (info, cells) in partial_infos.iter_mut().zip(owned_cells.iter()) {
            info.cells = cells.as_slice();
        }
        let pane_render_infos = partial_infos;

        // Build divider render info
        let divider_render_infos: Vec<DividerRenderInfo> = dividers
            .iter()
            .enumerate()
            .map(|(i, d)| DividerRenderInfo::from_rect(d, hovered_divider_index == Some(i)))
            .collect();

        // Build divider settings from config
        let divider_settings = PaneDividerSettings {
            divider_color: color_u8_to_f32(config.pane_divider_color),
            hover_color: color_u8_to_f32(config.pane_divider_hover_color),
            show_focus_indicator: config.pane_focus_indicator,
            focus_color: color_u8_to_f32(config.pane_focus_color),
            focus_width: config.pane_focus_width * renderer.scale_factor(),
            divider_style: config.pane_divider_style,
        };

        // Call the split pane renderer.
        // owned_cells is dropped automatically at scope exit, even on panic.
        renderer.render_split_panes(
            &pane_render_infos,
            &divider_render_infos,
            &pane_titles,
            focused_viewport.as_ref(),
            &divider_settings,
            egui_data,
            false,
        )
    }
}
