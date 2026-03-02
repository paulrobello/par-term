//! Split-pane rendering helpers.
//!
//! Contains:
//! - `PaneRenderData`: per-pane snapshot used during `submit_gpu_frame`
//! - `gather_pane_render_data`: collects per-pane cells/graphics/metadata from the pane manager
//! - `render_split_panes_with_data`: drives the GPU split-pane render pass

use super::types::RendererSizing;
use crate::cell_renderer::PaneViewport;
use crate::config::{Config, PaneTitlePosition, color_u8_to_f32};
use crate::renderer::{
    DividerRenderInfo, PaneDividerSettings, PaneRenderInfo, PaneTitleInfo, Renderer,
};
use crate::scrollback_metadata::ScrollbackMark;
use crate::selection::SelectionMode;
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

/// Result of `gather_pane_render_data`.
pub(super) type PaneRenderDataResult = Option<(
    Vec<PaneRenderData>,
    Vec<crate::pane::DividerRect>,
    Vec<PaneTitleInfo>,
    Option<PaneViewport>,
    usize, // focused pane scrollback_len (for tab.cache update)
)>;

/// Gather per-pane render data from the active tab's pane manager.
///
/// This is a free function (not a `&mut self` method) so it can be called while
/// `self.renderer` is mutably borrowed.  The caller must already hold `&mut Tab`
/// from `tab_manager.active_tab_mut()`.
///
/// Returns `None` when no pane manager is present or the tab is absent.
pub(super) fn gather_pane_render_data(
    tab: &mut crate::tab::Tab,
    config: &Config,
    sizing: &RendererSizing,
    effective_pane_padding: f32,
    cursor_opacity: f32,
    pane_count: usize,
) -> PaneRenderDataResult {
    let effective_padding = if pane_count > 1 && config.hide_window_padding_on_split {
        0.0
    } else {
        sizing.padding
    };

    let content_width = sizing.size.width as f32
        - effective_padding * 2.0
        - sizing.content_offset_x
        - sizing.content_inset_right;
    let content_height = sizing.size.height as f32
        - sizing.content_offset_y
        - sizing.content_inset_bottom
        - effective_padding
        - sizing.status_bar_height;

    let tab_scroll_offset = tab.active_scroll_state().offset;

    let pm = tab.pane_manager.as_mut()?;

    // Update pane bounds
    let bounds = crate::pane::PaneBounds::new(
        effective_padding + sizing.content_offset_x,
        sizing.content_offset_y,
        content_width,
        content_height,
    );
    pm.set_bounds(bounds);

    // Scale title bar height from logical to physical pixels
    let title_height_offset = if config.show_pane_titles {
        config.pane_title_height * sizing.scale_factor
    } else {
        0.0
    };

    // Resize all pane terminals to match their new bounds
    pm.resize_all_terminals_with_padding(
        sizing.cell_width,
        sizing.cell_height,
        effective_pane_padding * sizing.scale_factor,
        title_height_offset,
    );

    let focused_pane_id = pm.focused_pane_id();
    let all_pane_ids: Vec<_> = pm.all_panes().iter().map(|p| p.id).collect();
    let dividers = pm.get_dividers();

    let pane_bg_opacity = config.pane_background_opacity;
    let inactive_opacity = if config.dim_inactive_panes {
        config.inactive_pane_opacity
    } else {
        1.0
    };

    // Title settings (all in physical pixels)
    let show_titles = config.show_pane_titles;
    let title_height = config.pane_title_height * sizing.scale_factor;
    let title_position = config.pane_title_position;
    let title_text_color = color_u8_to_f32(config.pane_title_color);
    let title_bg_color = color_u8_to_f32(config.pane_title_bg_color);
    let need_marks = config.scrollbar_command_marks || config.command_separator_enabled;

    let mut pane_data: Vec<PaneRenderData> = Vec::new();
    let mut pane_titles: Vec<PaneTitleInfo> = Vec::new();
    let mut focused_pane_scrollback_len: usize = 0;
    let mut focused_viewport: Option<PaneViewport> = None;

    for pane_id in &all_pane_ids {
        let Some(pane) = pm.get_pane(*pane_id) else {
            continue;
        };
        let is_focused = Some(*pane_id) == focused_pane_id;
        let bounds = pane.bounds;

        // Viewport y and height accounting for title bar position
        let (viewport_y, viewport_height) = if show_titles {
            match title_position {
                PaneTitlePosition::Top => (
                    bounds.y + title_height,
                    (bounds.height - title_height).max(0.0),
                ),
                PaneTitlePosition::Bottom => (bounds.y, (bounds.height - title_height).max(0.0)),
            }
        } else {
            (bounds.y, bounds.height)
        };

        let physical_pane_padding = effective_pane_padding * sizing.scale_factor;
        let viewport = PaneViewport::with_padding(
            bounds.x,
            viewport_y,
            bounds.width,
            viewport_height,
            is_focused,
            if is_focused {
                pane_bg_opacity
            } else {
                pane_bg_opacity * inactive_opacity
            },
            physical_pane_padding,
        );

        if is_focused {
            focused_viewport = Some(viewport);
        }

        // Build pane title info
        if show_titles {
            let title_y = match title_position {
                PaneTitlePosition::Top => bounds.y,
                PaneTitlePosition::Bottom => bounds.y + bounds.height - title_height,
            };
            pane_titles.push(PaneTitleInfo {
                x: bounds.x,
                y: title_y,
                width: bounds.width,
                height: title_height,
                title: pane.get_title(),
                focused: is_focused,
                text_color: title_text_color,
                bg_color: title_bg_color,
            });
        }

        // Gather cells
        let scroll_offset = if is_focused { tab_scroll_offset } else { 0 };
        let cells = if let Ok(term) = pane.terminal.try_write() {
            let selection = pane.mouse.selection.map(|sel| sel.normalized());
            let rectangular = pane
                .mouse
                .selection
                .map(|sel| sel.mode == SelectionMode::Rectangular)
                .unwrap_or(false);
            term.get_cells_with_scrollback(scroll_offset, selection, rectangular, None)
        } else {
            Vec::new()
        };

        // Gather marks and scrollback length
        let (marks, pane_scrollback_len) = if need_marks {
            if let Ok(mut term) = pane.terminal.try_write() {
                let sb_len = term.scrollback_len();
                term.update_scrollback_metadata(sb_len, 0);
                (term.scrollback_marks(), sb_len)
            } else {
                (Vec::new(), 0)
            }
        } else {
            // Still need scrollback_len for graphics position math
            let sb_len = if let Ok(term) = pane.terminal.try_write() {
                term.scrollback_len()
            } else {
                0
            };
            (Vec::new(), sb_len)
        };
        let pane_scroll_offset = if is_focused { tab_scroll_offset } else { 0 };

        // Cache focused pane scrollback_len for scroll operations
        if is_focused && pane_scrollback_len > 0 {
            focused_pane_scrollback_len = pane_scrollback_len;
        }

        // Per-pane backgrounds only apply when multiple panes exist
        let pane_background = if all_pane_ids.len() > 1 && pane.background().has_image() {
            Some(pane.background().clone())
        } else {
            None
        };

        // Cursor position
        let cursor_pos = if let Ok(term) = pane.terminal.try_write() {
            if term.is_cursor_visible() {
                Some(term.cursor_position())
            } else {
                None
            }
        } else {
            None
        };

        // Grid size must match the terminal's actual size
        let content_w = (bounds.width - physical_pane_padding * 2.0).max(sizing.cell_width);
        let content_h = (viewport_height - physical_pane_padding * 2.0).max(sizing.cell_height);
        let cols = ((content_w / sizing.cell_width).floor() as usize).max(1);
        let rows = ((content_h / sizing.cell_height).floor() as usize).max(1);

        // Collect inline graphics (Sixel/iTerm2/Kitty)
        let pane_graphics = if let Ok(term) = pane.terminal.try_write() {
            let mut g = term.get_graphics_with_animations();
            let sb = term.get_scrollback_graphics();
            crate::debug_log!(
                "GRAPHICS",
                "pane {:?}: active_graphics={}, scrollback_graphics={}, scrollback_len={}, scroll_offset={}, visible_rows={}, viewport=({},{},{}x{})",
                pane_id,
                g.len(),
                sb.len(),
                pane_scrollback_len,
                pane_scroll_offset,
                rows,
                viewport.x,
                viewport.y,
                viewport.width,
                viewport.height
            );
            for (i, gfx) in g.iter().chain(sb.iter()).enumerate() {
                crate::debug_log!(
                    "GRAPHICS",
                    "  graphic[{}]: id={}, pos=({},{}), scroll_offset_rows={}, scrollback_row={:?}, size={}x{}",
                    i,
                    gfx.id,
                    gfx.position.0,
                    gfx.position.1,
                    gfx.scroll_offset_rows,
                    gfx.scrollback_row,
                    gfx.width,
                    gfx.height
                );
            }
            g.extend(sb);
            g
        } else {
            crate::debug_log!(
                "GRAPHICS",
                "pane {:?}: try_lock() failed, no graphics",
                pane_id
            );
            Vec::new()
        };

        pane_data.push(PaneRenderData {
            viewport,
            cells,
            grid_size: (cols, rows),
            cursor_pos,
            cursor_opacity: if is_focused { cursor_opacity } else { 0.0 },
            marks,
            scrollback_len: pane_scrollback_len,
            scroll_offset: pane_scroll_offset,
            background: pane_background,
            graphics: pane_graphics,
        });
    }

    Some((
        pane_data,
        dividers,
        pane_titles,
        focused_viewport,
        focused_pane_scrollback_len,
    ))
}

/// Parameters for [`WindowState::render_split_panes_with_data`].
pub(super) struct SplitPaneRenderParams<'a> {
    pub pane_data: Vec<PaneRenderData>,
    pub dividers: Vec<crate::pane::DividerRect>,
    pub pane_titles: Vec<PaneTitleInfo>,
    pub focused_viewport: Option<PaneViewport>,
    pub config: &'a Config,
    pub egui_data: Option<(egui::FullOutput, &'a egui::Context)>,
    pub hovered_divider_index: Option<usize>,
    pub show_scrollbar: bool,
}

impl crate::app::window_state::WindowState {
    /// Render split panes when the active tab has multiple panes
    pub(super) fn render_split_panes_with_data(
        renderer: &mut Renderer,
        p: SplitPaneRenderParams<'_>,
    ) -> Result<bool> {
        let SplitPaneRenderParams {
            pane_data,
            dividers,
            pane_titles,
            focused_viewport,
            config,
            egui_data,
            hovered_divider_index,
            show_scrollbar,
        } = p;
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
        renderer.render_split_panes(crate::renderer::SplitPanesRenderParams {
            panes: &pane_render_infos,
            dividers: &divider_render_infos,
            pane_titles: &pane_titles,
            focused_viewport: focused_viewport.as_ref(),
            divider_settings: &divider_settings,
            egui_data,
            force_egui_opaque: false,
        })
    }
}
