use super::TerminalManager;
use par_term_config::{Cell, Theme};

impl TerminalManager {
    /// Get terminal grid with scrollback offset as Cell array for CellRenderer
    pub fn get_cells_with_scrollback(
        &self,
        scroll_offset: usize,
        selection: Option<((usize, usize), (usize, usize))>,
        rectangular: bool,
        _cursor: Option<((usize, usize), f32)>,
    ) -> Vec<Cell> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        let grid = term.active_grid();

        let cursor_with_style = None;

        let rows = grid.rows();
        let cols = grid.cols();
        let scrollback_len = grid.scrollback_len();
        let clamped_offset = scroll_offset.min(scrollback_len);
        let total_lines = scrollback_len + rows;
        let end_line = total_lines.saturating_sub(clamped_offset);
        let start_line = end_line.saturating_sub(rows);

        let mut cells = Vec::with_capacity(rows * cols);

        for line_idx in start_line..end_line {
            let screen_row = line_idx - start_line;

            if line_idx < scrollback_len {
                if let Some(line) = grid.scrollback_line(line_idx) {
                    Self::push_line_from_slice(
                        line,
                        cols,
                        &mut cells,
                        screen_row,
                        selection,
                        rectangular,
                        cursor_with_style,
                        &self.theme,
                    );
                } else {
                    Self::push_empty_cells(cols, &mut cells);
                }
            } else {
                let grid_row = line_idx - scrollback_len;
                Self::push_grid_row(
                    grid,
                    grid_row,
                    cols,
                    &mut cells,
                    screen_row,
                    selection,
                    rectangular,
                    cursor_with_style,
                    &self.theme,
                );
            }
        }

        // Apply trigger highlights on top of cell colors
        let highlights = term.get_trigger_highlights();
        for highlight in &highlights {
            let abs_row = scrollback_len + highlight.row;
            if abs_row < start_line || abs_row >= end_line {
                continue;
            }
            let screen_row = abs_row - start_line;

            for col in highlight.col_start..highlight.col_end.min(cols) {
                let cell_idx = screen_row * cols + col;
                if cell_idx < cells.len() {
                    if let Some((r, g, b)) = highlight.fg {
                        cells[cell_idx].fg_color = [r, g, b, 255];
                    }
                    if let Some((r, g, b)) = highlight.bg {
                        cells[cell_idx].bg_color = [r, g, b, 255];
                    }
                }
            }
        }
        term.clear_expired_highlights();

        cells
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn push_line_from_slice(
        line: &[par_term_emu_core_rust::cell::Cell],
        cols: usize,
        dest: &mut Vec<Cell>,
        screen_row: usize,
        selection: Option<((usize, usize), (usize, usize))>,
        rectangular: bool,
        cursor: Option<(
            (usize, usize),
            f32,
            par_term_emu_core_rust::cursor::CursorStyle,
        )>,
        theme: &Theme,
    ) {
        let copy_len = cols.min(line.len());
        for (col, cell) in line[..copy_len].iter().enumerate() {
            let is_selected = Self::is_cell_selected(col, screen_row, selection, rectangular);
            let cursor_info = cursor.and_then(|((cx, cy), opacity, style)| {
                if cx == col && cy == screen_row {
                    Some((opacity, style))
                } else {
                    None
                }
            });
            dest.push(Self::convert_term_cell_with_theme(
                cell,
                is_selected,
                cursor_info,
                theme,
            ));
        }

        if copy_len < cols {
            Self::push_empty_cells(cols - copy_len, dest);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn push_grid_row(
        grid: &par_term_emu_core_rust::grid::Grid,
        row: usize,
        cols: usize,
        dest: &mut Vec<Cell>,
        screen_row: usize,
        selection: Option<((usize, usize), (usize, usize))>,
        rectangular: bool,
        cursor: Option<(
            (usize, usize),
            f32,
            par_term_emu_core_rust::cursor::CursorStyle,
        )>,
        theme: &Theme,
    ) {
        for col in 0..cols {
            let is_selected = Self::is_cell_selected(col, screen_row, selection, rectangular);
            let cursor_info = cursor.and_then(|((cx, cy), opacity, style)| {
                if cx == col && cy == screen_row {
                    Some((opacity, style))
                } else {
                    None
                }
            });
            if let Some(cell) = grid.get(col, row) {
                dest.push(Self::convert_term_cell_with_theme(
                    cell,
                    is_selected,
                    cursor_info,
                    theme,
                ));
            } else {
                dest.push(Cell::default());
            }
        }
    }

    pub(crate) fn push_empty_cells(count: usize, dest: &mut Vec<Cell>) {
        for _ in 0..count {
            dest.push(Cell::default());
        }
    }

    /// Check if a cell at (col, row) is within the selection range
    pub(crate) fn is_cell_selected(
        col: usize,
        row: usize,
        selection: Option<((usize, usize), (usize, usize))>,
        rectangular: bool,
    ) -> bool {
        if let Some(((start_col, start_row), (end_col, end_row))) = selection {
            if rectangular {
                let min_col = start_col.min(end_col);
                let max_col = start_col.max(end_col);
                let min_row = start_row.min(end_row);
                let max_row = start_row.max(end_row);

                return col >= min_col && col <= max_col && row >= min_row && row <= max_row;
            }

            if start_row == end_row {
                return row == start_row && col >= start_col && col <= end_col;
            }

            if row == start_row {
                return col >= start_col;
            } else if row == end_row {
                return col <= end_col;
            } else if row > start_row && row < end_row {
                return true;
            }
        }
        false
    }

    pub(crate) fn convert_term_cell_with_theme(
        term_cell: &par_term_emu_core_rust::cell::Cell,
        is_selected: bool,
        cursor_info: Option<(f32, par_term_emu_core_rust::cursor::CursorStyle)>,
        theme: &Theme,
    ) -> Cell {
        use par_term_emu_core_rust::color::{Color as TermColor, NamedColor};
        use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;

        let bg_rgb = term_cell.bg.to_rgb();
        let fg_rgb = term_cell.fg.to_rgb();
        let has_colored_bg = bg_rgb != (0, 0, 0);
        let has_reverse = term_cell.flags.reverse();

        if has_colored_bg || has_reverse {
            log::debug!(
                "Cell with colored BG or REVERSE: '{}' (U+{:04X}): fg={:?} (RGB:{},{},{}), bg={:?} (RGB:{},{},{}), reverse={}, flags={:?}",
                if term_cell.c.is_control() {
                    '?'
                } else {
                    term_cell.c
                },
                term_cell.c as u32,
                term_cell.fg,
                fg_rgb.0,
                fg_rgb.1,
                fg_rgb.2,
                term_cell.bg,
                bg_rgb.0,
                bg_rgb.1,
                bg_rgb.2,
                has_reverse,
                term_cell.flags
            );
        }

        // Apply theme colors for ANSI colors (Named colors)
        let fg = match &term_cell.fg {
            TermColor::Named(named) => {
                #[allow(unreachable_patterns)]
                let theme_color = match named {
                    NamedColor::Black => theme.black,
                    NamedColor::Red => theme.red,
                    NamedColor::Green => theme.green,
                    NamedColor::Yellow => theme.yellow,
                    NamedColor::Blue => theme.blue,
                    NamedColor::Magenta => theme.magenta,
                    NamedColor::Cyan => theme.cyan,
                    NamedColor::White => theme.white,
                    NamedColor::BrightBlack => theme.bright_black,
                    NamedColor::BrightRed => theme.bright_red,
                    NamedColor::BrightGreen => theme.bright_green,
                    NamedColor::BrightYellow => theme.bright_yellow,
                    NamedColor::BrightBlue => theme.bright_blue,
                    NamedColor::BrightMagenta => theme.bright_magenta,
                    NamedColor::BrightCyan => theme.bright_cyan,
                    NamedColor::BrightWhite => theme.bright_white,
                    _ => theme.foreground,
                };
                (theme_color.r, theme_color.g, theme_color.b)
            }
            _ => term_cell.fg.to_rgb(),
        };

        let bg = match &term_cell.bg {
            TermColor::Named(named) => {
                #[allow(unreachable_patterns)]
                let theme_color = match named {
                    NamedColor::Black => theme.black,
                    NamedColor::Red => theme.red,
                    NamedColor::Green => theme.green,
                    NamedColor::Yellow => theme.yellow,
                    NamedColor::Blue => theme.blue,
                    NamedColor::Magenta => theme.magenta,
                    NamedColor::Cyan => theme.cyan,
                    NamedColor::White => theme.white,
                    NamedColor::BrightBlack => theme.bright_black,
                    NamedColor::BrightRed => theme.bright_red,
                    NamedColor::BrightGreen => theme.bright_green,
                    NamedColor::BrightYellow => theme.bright_yellow,
                    NamedColor::BrightBlue => theme.bright_blue,
                    NamedColor::BrightMagenta => theme.bright_magenta,
                    NamedColor::BrightCyan => theme.bright_cyan,
                    NamedColor::BrightWhite => theme.bright_white,
                    _ => theme.background,
                };
                (theme_color.r, theme_color.g, theme_color.b)
            }
            _ => term_cell.bg.to_rgb(),
        };

        let is_reverse = term_cell.flags.reverse();

        let (fg_color, bg_color) = if let Some((opacity, style)) = cursor_info {
            let blend = |normal: u8, inverted: u8, opacity: f32| -> u8 {
                (normal as f32 * (1.0 - opacity) + inverted as f32 * opacity) as u8
            };

            match style {
                TermCursorStyle::SteadyBlock | TermCursorStyle::BlinkingBlock => (
                    [
                        blend(fg.0, bg.0, opacity),
                        blend(fg.1, bg.1, opacity),
                        blend(fg.2, bg.2, opacity),
                        255,
                    ],
                    [
                        blend(bg.0, fg.0, opacity),
                        blend(bg.1, fg.1, opacity),
                        blend(bg.2, fg.2, opacity),
                        255,
                    ],
                ),
                TermCursorStyle::SteadyBar
                | TermCursorStyle::BlinkingBar
                | TermCursorStyle::SteadyUnderline
                | TermCursorStyle::BlinkingUnderline => (
                    [
                        blend(fg.0, bg.0, opacity),
                        blend(fg.1, bg.1, opacity),
                        blend(fg.2, bg.2, opacity),
                        255,
                    ],
                    [
                        blend(bg.0, fg.0, opacity),
                        blend(bg.1, fg.1, opacity),
                        blend(bg.2, fg.2, opacity),
                        255,
                    ],
                ),
            }
        } else if is_selected || is_reverse {
            ([bg.0, bg.1, bg.2, 255], [fg.0, fg.1, fg.2, 255])
        } else {
            ([fg.0, fg.1, fg.2, 255], [bg.0, bg.1, bg.2, 255])
        };

        let grapheme = if term_cell.has_combining_chars() {
            term_cell.get_grapheme()
        } else {
            term_cell.base_char().to_string()
        };

        Cell {
            grapheme,
            fg_color,
            bg_color,
            bold: term_cell.flags.bold(),
            italic: term_cell.flags.italic(),
            underline: term_cell.flags.underline(),
            strikethrough: term_cell.flags.strikethrough(),
            hyperlink_id: term_cell.flags.hyperlink_id,
            wide_char: term_cell.flags.wide_char(),
            wide_char_spacer: term_cell.flags.wide_char_spacer(),
        }
    }
}
