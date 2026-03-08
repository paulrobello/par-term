//! Cursor overlay instance generation for pane rendering.
//!
//! Provides [`CellRenderer::emit_cursor_overlays`] which appends background instances
//! for cursor-related overlays (guide, shadow, beam/underline bar, hollow outline) to
//! `self.bg_instances` starting at `bg_index` and returns the updated index.

use super::super::instance_buffers::HOLLOW_CURSOR_BORDER_PX;
use super::super::{BackgroundInstance, CellRenderer};

/// Parameters for [`CellRenderer::emit_cursor_overlays`].
pub(super) struct CursorOverlayParams {
    /// Pixel X of the left edge of the cursor cell.
    pub cursor_x0: f32,
    /// Pixel X of the right edge of the cursor cell.
    pub cursor_x1: f32,
    /// Pixel Y of the top edge of the cursor cell.
    pub cursor_y0: f32,
    /// Pixel Y of the bottom edge of the cursor cell.
    pub cursor_y1: f32,
    /// Number of visible columns in the pane (for guide line width).
    pub cols: usize,
    /// Pixel X of the content area origin (left edge after padding).
    pub content_x: f32,
    /// Current blink opacity (0.0 = invisible, 1.0 = fully visible).
    pub cursor_opacity: f32,
}

impl CellRenderer {
    /// Append cursor overlay instances (guide, shadow, beam/underline bar, hollow outline)
    /// to `self.bg_instances` starting at `bg_index`.
    ///
    /// Returns the updated `bg_index` after all overlays have been appended.
    pub(super) fn emit_cursor_overlays(
        &mut self,
        p: CursorOverlayParams,
        mut bg_index: usize,
    ) -> usize {
        let CursorOverlayParams {
            cursor_x0,
            cursor_x1,
            cursor_y0,
            cursor_y1,
            cols,
            content_x,
            cursor_opacity,
        } = p;

        let w = self.config.width as f32;
        let h = self.config.height as f32;

        // Cursor guide (horizontal line spanning viewport width at cursor row)
        if cursor_opacity > 0.0
            && !self.cursor.hidden_for_shader
            && self.cursor.guide_enabled
            && bg_index < self.buffers.max_bg_instances
        {
            let guide_x0 = content_x;
            let guide_x1 = content_x + cols as f32 * self.grid.cell_width;
            self.bg_instances[bg_index] = BackgroundInstance {
                position: [guide_x0 / w * 2.0 - 1.0, 1.0 - (cursor_y0 / h * 2.0)],
                size: [
                    (guide_x1 - guide_x0) / w * 2.0,
                    (cursor_y1 - cursor_y0) / h * 2.0,
                ],
                color: self.cursor.guide_color,
            };
            bg_index += 1;
        }

        // Cursor shadow (offset rectangle behind cursor)
        if cursor_opacity > 0.0
            && !self.cursor.hidden_for_shader
            && self.cursor.shadow_enabled
            && bg_index < self.buffers.max_bg_instances
        {
            let shadow_x0 = cursor_x0 + self.cursor.shadow_offset[0];
            let shadow_y0 = cursor_y0 + self.cursor.shadow_offset[1];
            self.bg_instances[bg_index] = BackgroundInstance {
                position: [shadow_x0 / w * 2.0 - 1.0, 1.0 - (shadow_y0 / h * 2.0)],
                size: [
                    self.grid.cell_width / w * 2.0,
                    self.grid.cell_height / h * 2.0,
                ],
                color: self.cursor.shadow_color,
            };
            bg_index += 1;
        }

        // Beam or underline cursor bar (on top of text)
        if cursor_opacity > 0.0 && !self.cursor.hidden_for_shader {
            use par_term_emu_core_rust::cursor::CursorStyle;
            let cc = self.cursor.color;
            let overlay = match self.cursor.style {
                CursorStyle::SteadyBar | CursorStyle::BlinkingBar => Some(BackgroundInstance {
                    position: [cursor_x0 / w * 2.0 - 1.0, 1.0 - (cursor_y0 / h * 2.0)],
                    size: [2.0 / w * 2.0, (cursor_y1 - cursor_y0) / h * 2.0],
                    color: [cc[0], cc[1], cc[2], cursor_opacity],
                }),
                CursorStyle::SteadyUnderline | CursorStyle::BlinkingUnderline => {
                    Some(BackgroundInstance {
                        position: [
                            cursor_x0 / w * 2.0 - 1.0,
                            1.0 - ((cursor_y1 - 2.0) / h * 2.0),
                        ],
                        size: [(cursor_x1 - cursor_x0) / w * 2.0, 2.0 / h * 2.0],
                        color: [cc[0], cc[1], cc[2], cursor_opacity],
                    })
                }
                _ => None,
            };
            if let Some(inst) = overlay
                && bg_index < self.buffers.max_bg_instances
            {
                self.bg_instances[bg_index] = inst;
                bg_index += 1;
            }
        }

        // Hollow cursor outline (4 borders) — independent of blink opacity
        let render_hollow = !self.cursor.hidden_for_shader
            && !self.is_focused
            && self.cursor.unfocused_style == par_term_config::UnfocusedCursorStyle::Hollow;
        if render_hollow {
            let border_width = HOLLOW_CURSOR_BORDER_PX;
            let color = [
                self.cursor.color[0],
                self.cursor.color[1],
                self.cursor.color[2],
                1.0, // Always fully opaque regardless of blink phase
            ];
            let cell_w = (cursor_x1 - cursor_x0) / w * 2.0;
            let cell_h = (cursor_y1 - cursor_y0) / h * 2.0;
            let bw = border_width / w * 2.0;
            let bh = border_width / h * 2.0;
            let cx = cursor_x0 / w * 2.0 - 1.0;
            let cy = 1.0 - (cursor_y0 / h * 2.0);
            let borders = [
                // Top
                BackgroundInstance {
                    position: [cx, cy],
                    size: [cell_w, bh],
                    color,
                },
                // Bottom
                BackgroundInstance {
                    position: [cx, 1.0 - ((cursor_y1 - border_width) / h * 2.0)],
                    size: [cell_w, bh],
                    color,
                },
                // Left
                BackgroundInstance {
                    position: [cx, 1.0 - ((cursor_y0 + border_width) / h * 2.0)],
                    size: [bw, cell_h - bh * 2.0],
                    color,
                },
                // Right
                BackgroundInstance {
                    position: [
                        (cursor_x1 - border_width) / w * 2.0 - 1.0,
                        1.0 - ((cursor_y0 + border_width) / h * 2.0),
                    ],
                    size: [bw, cell_h - bh * 2.0],
                    color,
                },
            ];
            for border in borders {
                if bg_index < self.buffers.max_bg_instances {
                    self.bg_instances[bg_index] = border;
                    bg_index += 1;
                }
            }
        }

        bg_index
    }
}
