//! Split-pane rendering helpers.
//!
//! Contains:
//! - `PaneRenderData`: per-pane snapshot used during `submit_gpu_frame`
//! - `gather_pane_render_data`: collects per-pane cells/graphics/metadata from the pane manager
//! - `with_pane_capture_params`: scoped construction of the borrowed pane render infos
//! - `render_split_panes_with_data`: drives the GPU split-pane render pass
//! - `capture_frame_image`: the same gather, composited offscreen for screenshots (QA-011)

use super::types::RendererSizing;
use crate::config::{Config, PaneTitlePosition, ScrollbackMark, color_u8_to_f32};
use crate::selection::SelectionMode;
use anyhow::Result;
use par_term_render::cell_renderer::PaneViewport;
use par_term_render::renderer::{
    DividerRenderInfo, PaneDividerSettings, PaneRenderInfo, PaneTitleInfo, Renderer,
};
use std::sync::Arc;

/// Pane render data for split pane rendering
pub(super) struct PaneRenderData {
    /// Viewport bounds and state for this pane
    pub(super) viewport: PaneViewport,
    /// Cells to render (should match viewport grid size)
    pub(super) cells: Arc<Vec<crate::config::Cell>>,
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
    /// Kitty virtual placements (U=1) used for Unicode placeholder rendering.
    pub(super) virtual_placements: Vec<par_term_emu_core_rust::graphics::TerminalGraphic>,
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
/// # Arguments
/// * `scrollbar_inset` - Physical pixels to subtract from each pane's content width
///   for the scrollbar.  In split-pane mode this is applied to ALL panes so the
///   column count never changes on focus switch (preventing layout reflow).
///   The scrollbar is shown per-pane based on each pane's scrollback state.
pub(super) fn gather_pane_render_data(
    tab: &mut crate::tab::Tab,
    config: &Config,
    sizing: &RendererSizing,
    effective_pane_padding: f32,
    cursor_opacity: f32,
    pane_count: usize,
    scrollbar_inset: f32,
) -> PaneRenderDataResult {
    let effective_padding = if pane_count > 1 && config.window.hide_window_padding_on_split {
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

    // Terminal resize is done per-pane in the loop below so each pane
    // subtracts `scrollbar_inset` from its column calculation.
    // This avoids two competing resize calls that would cause SIGWINCH storms.
    // Note: title_height_offset is not needed here because `viewport_height`
    // (computed per-pane below) already subtracts the title bar height.

    let focused_pane_id = pm.focused_pane_id();
    let all_pane_ids: Vec<_> = pm.all_panes().iter().map(|p| p.id).collect();
    let dividers = pm.get_dividers();

    let pane_bg_opacity = config.panes.pane_background_opacity;
    let inactive_opacity = if config.panes.dim_inactive_panes {
        config.panes.inactive_pane_opacity
    } else {
        1.0
    };

    // Title settings (all in physical pixels)
    let show_titles = config.panes.show_pane_titles;
    let title_height = config.panes.pane_title_height * sizing.scale_factor;
    let title_position = config.panes.pane_title_position;
    let title_text_color = color_u8_to_f32(config.panes.pane_title_color);
    let title_bg_color = color_u8_to_f32(config.panes.pane_title_bg_color);
    let need_marks = config.scrollbar.scrollbar_command_marks
        || config.command_separator.command_separator_enabled;

    let mut pane_data: Vec<PaneRenderData> = Vec::new();
    let mut pane_titles: Vec<PaneTitleInfo> = Vec::new();
    let mut focused_pane_scrollback_len: usize = 0;
    let mut focused_viewport: Option<PaneViewport> = None;

    for pane_id in &all_pane_ids {
        let Some(pane) = pm.get_pane_mut(*pane_id) else {
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

        // Compute grid size and resize the PTY BEFORE gathering cells so that
        // get_cells_with_scrollback returns cells at the correct dimensions.
        // All panes subtract the scrollbar inset so column counts remain stable
        // across focus changes (preventing layout reflow on pane click).
        let sb_inset = scrollbar_inset;
        let content_w =
            (bounds.width - physical_pane_padding * 2.0 - sb_inset).max(sizing.cell_width);
        let content_h = (viewport_height - physical_pane_padding * 2.0).max(sizing.cell_height);
        let cols = ((content_w / sizing.cell_width).floor() as usize).max(1);
        let rows = ((content_h / sizing.cell_height).floor() as usize).max(1);

        // Center the cell grid within the content area by distributing
        // remainder pixels evenly on both sides (like Alacritty/Kitty).
        // Floor to integer pixels so all cell boundaries land on exact pixel
        // positions — sub-pixel centering offsets cause hairline gaps between
        // adjacent differently-colored cells due to GPU FP rasterization.
        let actual_content_w = cols as f32 * sizing.cell_width;
        let actual_content_h = rows as f32 * sizing.cell_height;
        let center_offset_x = ((content_w - actual_content_w) / 2.0).floor();
        let center_offset_y = ((content_h - actual_content_h) / 2.0).floor();

        pane.resize_terminal_with_cell_dims(
            cols,
            rows,
            sizing.cell_width as u32,
            sizing.cell_height as u32,
        );

        let mut viewport = PaneViewport::with_padding(
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
        viewport.content_offset_x = center_offset_x;
        viewport.content_offset_y = center_offset_y;

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

        // Gather cells — fall back to cached cells on lock contention to prevent
        // empty-frame flashes (animated shaders trigger 60fps redraws, so lock
        // contention with the PTY reader is common during heavy output).
        //
        // For the focused pane, gather_render_data already called
        // get_cells_with_scrollback() in extract_tab_cells and stored the result
        // in pane.cache.pane_cells with the current generation.  Reuse those cells
        // to avoid a second blocking terminal.lock() call, which is the primary
        // cause of FPS drops when the PTY reader is busy (e.g. tmux with many panes).
        //
        // Unfocused panes MUST NOT use this fast path: nothing refreshes their
        // cache between frames, so cache_dims_match would return true forever
        // with stale content.  They always go through try_read() which compares
        // the terminal's current generation against the cached generation — this
        // is the cross-frame cache: it still takes a cheap `try_read()` on the
        // pane's TerminalManager every frame, but skips the expensive
        // `try_get_cells_with_scrollback()` call (which takes the CORE terminal's
        // write lock, contended by the PTY reader thread) whenever the generation,
        // scroll offset, selection, and grid dims are unchanged since the cache
        // was last populated.
        let grid_size = (cols, rows);
        let expected_cell_count = cols * rows;
        let scroll_offset = if is_focused { tab_scroll_offset } else { 0 };
        // Selection is baked directly into cell colors (see `is_cell_selected` in
        // rendering.rs), so it MUST be part of the cache key — a selection change
        // does not bump `update_generation()`.
        let current_selection = pane.mouse.selection;
        let cache_dims_match = is_focused
            && pane.cache.pane_cells_generation > 0
            && pane.cache.pane_cells_scroll_offset == scroll_offset
            && pane.cache.pane_cells_selection == current_selection
            && pane.cache.pane_cells_grid_dims == grid_size
            && pane
                .cache
                .pane_cells
                .as_ref()
                .is_some_and(|c| c.len() == expected_cell_count);
        let cells = if cache_dims_match {
            Arc::clone(
                pane.cache
                    .pane_cells
                    .as_ref()
                    .expect("pane_cells must exist when pane cache dimensions match"),
            )
        } else if let Ok(term) = pane.terminal.try_read() {
            let current_gen = term.update_generation();
            let selection =
                current_selection.map(|sel| sel.viewport_adjusted(scroll_offset).normalized());
            let rectangular = current_selection
                .map(|sel| sel.mode == SelectionMode::Rectangular)
                .unwrap_or(false);
            // Use try_get_cells_with_scrollback to avoid blocking on the internal
            // terminal mutex when the PTY reader is processing output.  Falls through
            // to the pane_cells cache on contention.
            if current_gen == pane.cache.pane_cells_generation
                && pane.cache.pane_cells_scroll_offset == scroll_offset
                && pane.cache.pane_cells_selection == current_selection
                && pane.cache.pane_cells_grid_dims == grid_size
                && let Some(ref cached) = pane.cache.pane_cells
                && cached.len() == expected_cell_count
            {
                Arc::clone(cached)
            } else if let Some(fresh) =
                term.try_get_cells_with_scrollback(scroll_offset, selection, rectangular)
            {
                let fresh = Arc::new(fresh);
                pane.cache.pane_cells = Some(Arc::clone(&fresh));
                pane.cache.pane_cells_generation = current_gen;
                pane.cache.pane_cells_scroll_offset = scroll_offset;
                pane.cache.pane_cells_selection = current_selection;
                pane.cache.pane_cells_grid_dims = grid_size;
                fresh
            } else if pane.cache.pane_cells_grid_dims == grid_size
                && let Some(ref cached) = pane.cache.pane_cells
            {
                Arc::clone(cached)
            } else {
                Arc::new(Vec::new())
            }
        } else if pane.cache.pane_cells_grid_dims == grid_size
            && let Some(ref cached) = pane.cache.pane_cells
        {
            // try_lock miss — use last successfully gathered cells to avoid
            // rendering an empty pane for this frame.
            Arc::clone(cached)
        } else {
            Arc::new(Vec::new())
        };

        // Gather marks and scrollback length — use cached scrollback_len on lock miss
        let (marks, pane_scrollback_len) = if need_marks {
            if let Ok(mut term) = pane.terminal.try_write() {
                let sb_len = term.scrollback_len();
                term.update_scrollback_metadata(sb_len, 0);
                pane.cache.pane_scrollback_len = sb_len;
                (term.scrollback_marks(), sb_len)
            } else {
                (Vec::new(), pane.cache.pane_scrollback_len)
            }
        } else {
            // Still need scrollback_len for graphics position math
            let sb_len = if let Ok(term) = pane.terminal.try_read() {
                pane.cache.pane_scrollback_len = term.scrollback_len();
                pane.cache.pane_scrollback_len
            } else {
                pane.cache.pane_scrollback_len
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

        // Cursor position — only show when viewport is not scrolled away from the live screen.
        // When scroll_offset > 0 the cursor is off-screen (in scrollback), so hide it.
        let cursor_pos = if scroll_offset == 0 {
            if let Ok(term) = pane.terminal.try_read() {
                if term.is_cursor_visible() {
                    Some(term.cursor_position())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Collect Kitty virtual placements (U=1) — these are stored separately
        // from active graphics and get rendered wherever the cell grid contains
        // the corresponding placeholder character runs.
        let pane_virtual_placements = if let Ok(term) = pane.terminal.try_read() {
            term.get_virtual_placements()
        } else {
            Vec::new()
        };

        // Collect inline graphics (Sixel/iTerm2/Kitty)
        let pane_graphics = if let Ok(term) = pane.terminal.try_read() {
            // Remove graphics whose rows have been overwritten by cell writes
            // (e.g. tmux control-mode redraw doesn't send ED 2).
            term.invalidate_overwritten_graphics();

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
            virtual_placements: pane_virtual_placements,
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

/// Gathered pane state, before it is turned into the borrowed render infos.
///
/// This is [`SplitPaneRenderParams`] minus `egui_data`, which is the part an
/// offscreen capture can also supply.
pub(super) struct PaneCaptureInput<'a> {
    pub pane_data: Vec<PaneRenderData>,
    pub dividers: Vec<crate::pane::DividerRect>,
    pub pane_titles: Vec<PaneTitleInfo>,
    pub focused_viewport: Option<PaneViewport>,
    pub config: &'a Config,
    pub hovered_divider_index: Option<usize>,
    pub show_scrollbar: bool,
}

/// Build the borrowed [`par_term_render::renderer::PaneCaptureParams`] and hand it to `f`.
///
/// A scoped callback rather than a returned value because the pane infos borrow
/// the owned cell `Arc`s, the divider infos and the divider settings, all of
/// which have to outlive the render call and none of which can escape this
/// frame. Both the live path and the QA-011 screenshot path go through here, so
/// a capture cannot drift out of step with what is drawn on screen.
fn with_pane_capture_params<R>(
    renderer: &mut Renderer,
    input: PaneCaptureInput<'_>,
    f: impl FnOnce(&mut Renderer, par_term_render::renderer::PaneCaptureParams<'_>) -> R,
) -> R {
    let PaneCaptureInput {
        pane_data,
        dividers,
        pane_titles,
        focused_viewport,
        config,
        hovered_divider_index,
        show_scrollbar,
    } = input;
    // Two-phase construction: separate owned cell data from pane metadata
    // so PaneRenderInfo can borrow cell slices safely.  This replaces the
    // previous unsafe Box::into_raw / Box::from_raw pattern that leaked
    // memory if render_split_panes panicked.
    //
    // Phase 1: Extract cells into a Vec that outlives the render infos.
    // The remaining pane fields are collected into partial render infos.
    let mut owned_cells: Vec<Arc<Vec<crate::config::Cell>>> = Vec::with_capacity(pane_data.len());
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
            // Focused pane: respect autohide via show_scrollbar flag.
            // Unfocused panes: always show scrollbar when they have scrollback
            // content, so the scrollbar doesn't disappear on focus loss.
            show_scrollbar: if focused {
                show_scrollbar && pane.scrollback_len > 0
            } else {
                pane.scrollback_len > 0
            },
            marks: pane.marks,
            scrollback_len: pane.scrollback_len,
            scroll_offset: pane.scroll_offset,
            background: pane.background,
            graphics: pane.graphics,
            virtual_placements: pane.virtual_placements,
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
        divider_color: color_u8_to_f32(config.panes.pane_divider_color),
        hover_color: color_u8_to_f32(config.panes.pane_divider_hover_color),
        show_focus_indicator: config.panes.pane_focus_indicator,
        focus_color: color_u8_to_f32(config.panes.pane_focus_color),
        focus_width: config.panes.pane_focus_width * renderer.scale_factor(),
        divider_style: config.panes.pane_divider_style,
    };

    renderer.update_shader_focused_pane(focused_viewport.as_ref());

    // owned_cells is dropped automatically at scope exit, even on panic.
    f(
        renderer,
        par_term_render::renderer::PaneCaptureParams {
            panes: &pane_render_infos,
            dividers: &divider_render_infos,
            pane_titles: &pane_titles,
            focused_viewport: focused_viewport.as_ref(),
            divider_settings: &divider_settings,
        },
    )
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
        with_pane_capture_params(
            renderer,
            PaneCaptureInput {
                pane_data,
                dividers,
                pane_titles,
                focused_viewport,
                config,
                hovered_divider_index,
                show_scrollbar,
            },
            move |renderer, cap| {
                renderer.render_split_panes(par_term_render::renderer::SplitPanesRenderParams {
                    panes: cap.panes,
                    dividers: cap.dividers,
                    pane_titles: cap.pane_titles,
                    focused_viewport: cap.focused_viewport,
                    divider_settings: cap.divider_settings,
                    egui_data,
                    force_egui_opaque: false,
                })
            },
        )
    }

    /// Capture the current frame as an image through the live pane render path.
    ///
    /// QA-011: screenshots used to re-render from the renderer's single-grid
    /// state, which does not match a split — the capture showed one grid's worth
    /// of the focused pane's cells re-wrapped at the full-window stride. This
    /// gathers exactly the pane data the next live frame would draw and
    /// composites it into an offscreen target, so the image is the screen.
    ///
    /// Not included: the egui overlay (tab bar, dialogs, menus). `render_egui`
    /// consumes an `egui::FullOutput` produced once per frame by the live egui
    /// pass, and a capture taken between frames has none. Unchanged from the
    /// previous behaviour, and worth knowing when using `--screenshot` to verify
    /// UI work.
    pub(crate) fn capture_frame_image(&mut self) -> Result<image::RgbaImage, String> {
        // Everything that needs `&self` is read up front, before the disjoint
        // `self.renderer` / `self.tab_manager` field borrows below.
        let config = self.config.load_full();
        let is_tmux_gateway = self.is_gateway_active();
        let is_tmux_connected = self.is_tmux_connected();
        let show_scrollbar = self.should_show_scrollbar();
        let cursor_opacity = self.cursor_anim.cursor_opacity;
        let status_bar_height =
            crate::tmux_status_bar_ui::TmuxStatusBarUI::height(&config, is_tmux_connected);
        let custom_status_bar_height = self.status_bar_ui.height(&config, self.is_fullscreen);
        let pane_count = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager.as_ref())
            .map(|pm| pm.pane_count())
            .unwrap_or(0);
        let hovered_divider_index = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.active_mouse().hovered_divider_index);
        // Mirrors `submit_gpu_frame`: no divider padding when no divider is drawn.
        let effective_pane_padding = if is_tmux_gateway || pane_count <= 1 {
            0.0
        } else {
            config.panes.pane_divider_width.unwrap_or(2.0) / 2.0 + config.panes.pane_padding
        };

        let Some(renderer) = self.renderer.as_mut() else {
            return Err("No renderer available for screenshot".to_string());
        };
        let sizing = RendererSizing {
            size: renderer.size(),
            content_offset_y: renderer.content_offset_y(),
            content_offset_x: renderer.content_offset_x(),
            content_inset_bottom: renderer.content_inset_bottom(),
            content_inset_right: renderer.content_inset_right(),
            cell_width: renderer.cell_width(),
            cell_height: renderer.cell_height(),
            padding: renderer.window_padding(),
            status_bar_height: (status_bar_height + custom_status_bar_height)
                * renderer.scale_factor(),
            scale_factor: renderer.scale_factor(),
            scrollbar_width: renderer.scrollbar_width(),
        };

        // Same call the live frame makes. `resize_terminal_with_cell_dims` inside
        // is a no-op when the dimensions already match, so a capture does not
        // resize the PTY or emit SIGWINCH.
        let Some((pane_data, dividers, pane_titles, focused_viewport, _)) =
            self.tab_manager.active_tab_mut().and_then(|tab| {
                gather_pane_render_data(
                    tab,
                    &config,
                    &sizing,
                    effective_pane_padding,
                    cursor_opacity,
                    pane_count,
                    sizing.scrollbar_width,
                )
            })
        else {
            return Err("No pane data available for screenshot".to_string());
        };

        with_pane_capture_params(
            renderer,
            PaneCaptureInput {
                pane_data,
                dividers,
                pane_titles,
                focused_viewport,
                config: &config,
                hovered_divider_index,
                show_scrollbar,
            },
            |renderer, cap| renderer.take_screenshot(cap),
        )
        .map_err(|e| format!("Renderer screenshot failed: {e}"))
    }
}
