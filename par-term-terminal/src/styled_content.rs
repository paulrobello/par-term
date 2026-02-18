use par_term_emu_core_rust::grid::Grid;

/// A segment of text with consistent styling
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct StyledSegment {
    pub text: String,
    pub fg_color: (u8, u8, u8),
    pub bg_color: (u8, u8, u8),
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub line: usize,
    pub start_col: usize,
}

/// Extract styled segments from a terminal grid
#[allow(dead_code)]
pub fn extract_styled_segments(grid: &Grid) -> Vec<StyledSegment> {
    let mut segments = Vec::new();
    let rows = grid.rows();
    let cols = grid.cols();

    for row in 0..rows {
        let mut current_segment: Option<StyledSegment> = None;

        for col in 0..cols {
            if let Some(cell) = grid.get(col, row) {
                let fg = cell.fg.to_rgb();
                let bg = cell.bg.to_rgb();
                let bold = cell.flags.bold();
                let italic = cell.flags.italic();
                let underline = cell.flags.underline();

                // Check if this cell can be added to the current segment
                if let Some(ref mut segment) = current_segment {
                    let same_style = segment.fg_color == fg
                        && segment.bg_color == bg
                        && segment.bold == bold
                        && segment.italic == italic
                        && segment.underline == underline;

                    if same_style {
                        // Add to current segment
                        // Optimization: Avoid String allocation for cells without combining chars
                        if cell.has_combining_chars() {
                            segment.text.push_str(&cell.get_grapheme());
                        } else {
                            segment.text.push(cell.base_char());
                        }
                    } else {
                        // Different style, save current segment and start new one
                        segments.push(segment.clone());
                        // Optimization: Avoid String allocation for cells without combining chars
                        let text = if cell.has_combining_chars() {
                            cell.get_grapheme()
                        } else {
                            cell.base_char().to_string()
                        };
                        current_segment = Some(StyledSegment {
                            text,
                            fg_color: fg,
                            bg_color: bg,
                            bold,
                            italic,
                            underline,
                            line: row,
                            start_col: col,
                        });
                    }
                } else {
                    // Start new segment
                    // Optimization: Avoid String allocation for cells without combining chars
                    let text = if cell.has_combining_chars() {
                        cell.get_grapheme()
                    } else {
                        cell.base_char().to_string()
                    };
                    current_segment = Some(StyledSegment {
                        text,
                        fg_color: fg,
                        bg_color: bg,
                        bold,
                        italic,
                        underline,
                        line: row,
                        start_col: col,
                    });
                }
            }
        }

        // Save last segment of the line
        if let Some(segment) = current_segment {
            segments.push(segment);
        }
    }

    segments
}

/// Convert styled segments to plain text (for simple rendering)
#[allow(dead_code)]
pub fn segments_to_plain_text(segments: &[StyledSegment]) -> String {
    let mut result = String::new();
    let mut current_line = 0;

    for segment in segments {
        // Add newlines for line changes
        while current_line < segment.line {
            result.push('\n');
            current_line += 1;
        }

        result.push_str(&segment.text);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use par_term_emu_core_rust::cell::Cell;
    use par_term_emu_core_rust::color::{Color, NamedColor};
    use par_term_emu_core_rust::grid::Grid;

    #[test]
    fn test_extract_single_segment() {
        let mut grid = Grid::new(10, 1, 0);

        // Set some cells with same style
        for col in 0..5 {
            let mut cell = Cell::new('A');
            cell.fg = Color::Named(NamedColor::White);
            cell.bg = Color::Named(NamedColor::Black);
            grid.set(col, 0, cell);
        }

        let segments = extract_styled_segments(&grid);
        // Grid has 10 columns, so we get one segment for all 10
        // (5 'A's followed by 5 default space characters)
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text.trim_end(), "AAAAA");
    }

    #[test]
    fn test_extract_multiple_segments() {
        let mut grid = Grid::new(10, 1, 0);

        // First segment: white text
        for col in 0..3 {
            let mut cell = Cell::new('A');
            cell.fg = Color::Named(NamedColor::White);
            grid.set(col, 0, cell);
        }

        // Second segment: red text
        for col in 3..6 {
            let mut cell = Cell::new('B');
            cell.fg = Color::Named(NamedColor::Red);
            grid.set(col, 0, cell);
        }

        let segments = extract_styled_segments(&grid);
        // We should have at least 2 segments (white and red)
        assert!(segments.len() >= 2);
        assert_eq!(segments[0].text, "AAA");
        assert_eq!(segments[1].text.trim_start(), "BBB");
    }
}
