//! Command separator line instance generation for pane rendering.
//!
//! Provides [`CellRenderer::emit_separator_instances`] which injects horizontal separator
//! lines into `self.bg_instances` at row boundaries recorded by the PTY parser.

use super::super::{BackgroundInstance, CellRenderer};
use par_term_config::SeparatorMark;

impl CellRenderer {
    /// Inject command separator line instances for the given separator marks.
    ///
    /// Each separator mark represents a row boundary between commands.  A thin horizontal
    /// rectangle is emitted at the top of that row using either the exit-code colour or
    /// the user-supplied custom colour.
    ///
    /// Returns the updated `bg_index` after all separator instances have been appended.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn emit_separator_instances(
        &mut self,
        separator_marks: &[SeparatorMark],
        cols: usize,
        rows: usize,
        content_x: f32,
        content_y: f32,
        opacity_multiplier: f32,
        mut bg_index: usize,
    ) -> usize {
        if !self.separator.enabled || separator_marks.is_empty() {
            return bg_index;
        }

        let width_f = self.config.width as f32;
        let height_f = self.config.height as f32;

        for &(screen_row, exit_code, custom_color) in separator_marks {
            if screen_row < rows && bg_index < self.buffers.max_bg_instances {
                let x0 = content_x;
                let x1 = content_x + cols as f32 * self.grid.cell_width;
                let y0 = content_y + screen_row as f32 * self.grid.cell_height;
                let color = self.separator_color(exit_code, custom_color, opacity_multiplier);
                self.bg_instances[bg_index] = BackgroundInstance {
                    position: [x0 / width_f * 2.0 - 1.0, 1.0 - (y0 / height_f * 2.0)],
                    size: [
                        (x1 - x0) / width_f * 2.0,
                        self.separator.thickness / height_f * 2.0,
                    ],
                    color,
                };
                bg_index += 1;
            }
        }

        bg_index
    }
}
