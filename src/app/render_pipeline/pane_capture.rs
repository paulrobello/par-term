//! Offscreen frame capture through the live pane render path (QA-011).

use super::pane_render::{
    PaneCaptureInput, PaneLayoutOptions, gather_pane_render_data, with_pane_capture_params,
};
use super::types::RendererSizing;

impl crate::app::window_state::WindowState {
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
        let mux_attached = self.tmux_state.transport.is_some();
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
                    PaneLayoutOptions {
                        scrollbar_inset: sizing.scrollbar_width,
                        mux_attached,
                    },
                    &self.copy_mode,
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
