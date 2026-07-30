/// A single terminal cell with styled content for rendering.
///
/// This is the bridge between terminal emulation (core library cells with VT attributes)
/// and GPU rendering (colored rectangles and textured glyphs). The `TerminalManager`
/// converts core cells into these, applying theme colors and selection state.
#[derive(Debug, PartialEq)]
pub struct Cell {
    /// The grapheme cluster displayed in this cell (typically one character or a composed sequence).
    pub grapheme: String,
    /// Foreground color as RGBA (0–255 per channel).
    pub fg_color: [u8; 4],
    /// Background color as RGBA (0–255 per channel). Alpha 0 means transparent (default background).
    pub bg_color: [u8; 4],
    /// Whether to render the cell's font in bold weight.
    pub bold: bool,
    /// Whether to render the cell's font in italic style.
    pub italic: bool,
    /// Whether to draw an underline below the cell's glyph.
    pub underline: bool,
    /// Whether to draw a strikethrough line through the cell's glyph.
    pub strikethrough: bool,
    /// Optional OSC 8 hyperlink ID. Non-None cells are clickable and open a URL.
    pub hyperlink_id: Option<u32>,
    /// True if this cell holds the left half of a wide (double-width) character.
    pub wide_char: bool,
    /// True if this cell is the right-half spacer of a wide character. Has no renderable content.
    pub wide_char_spacer: bool,
}

/// Hand-written so `clone_from` reuses the destination's `grapheme` allocation.
///
/// `#[derive(Clone)]` does not emit a `clone_from` override, so the default
/// trait method (`*self = source.clone()`) runs and every cell allocates a new
/// `String`. The render path refreshes a persistent full-grid scratch buffer
/// once per frame via `Vec::clone_from`, which dispatches to this method per
/// cell — with the derive that was one heap allocation per cell per frame.
impl Clone for Cell {
    fn clone(&self) -> Self {
        Self {
            grapheme: self.grapheme.clone(),
            fg_color: self.fg_color,
            bg_color: self.bg_color,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strikethrough: self.strikethrough,
            hyperlink_id: self.hyperlink_id,
            wide_char: self.wide_char,
            wide_char_spacer: self.wide_char_spacer,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.grapheme.clone_from(&source.grapheme);
        self.fg_color = source.fg_color;
        self.bg_color = source.bg_color;
        self.bold = source.bold;
        self.italic = source.italic;
        self.underline = source.underline;
        self.strikethrough = source.strikethrough;
        self.hyperlink_id = source.hyperlink_id;
        self.wide_char = source.wide_char;
        self.wide_char_spacer = source.wide_char_spacer;
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            grapheme: " ".to_string(),
            fg_color: [255, 255, 255, 255],
            bg_color: [0, 0, 0, 0],
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            hyperlink_id: None,
            wide_char: false,
            wide_char_spacer: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Cell;

    fn cell(grapheme: &str) -> Cell {
        Cell {
            grapheme: grapheme.to_string(),
            ..Cell::default()
        }
    }

    /// Guards the reason `Clone` is hand-written instead of derived: the render
    /// path's per-frame scratch refresh relies on `clone_from` reusing the
    /// destination's `String` buffer. Reverting to `#[derive(Clone)]` silently
    /// reintroduces one heap allocation per cell per frame, and this fails.
    #[test]
    fn clone_from_reuses_the_destination_grapheme_allocation() {
        // Destination capacity must already cover the source, or `String::clone_from`
        // legitimately reallocates.
        let mut dst = cell("placeholder");
        let before = dst.grapheme.as_ptr();

        dst.clone_from(&cell("x"));

        assert_eq!(dst.grapheme, "x");
        assert_eq!(
            dst.grapheme.as_ptr(),
            before,
            "clone_from must reuse the existing grapheme buffer"
        );
    }

    #[test]
    fn clone_from_copies_every_field() {
        let source = Cell {
            grapheme: "@".to_string(),
            fg_color: [1, 2, 3, 4],
            bg_color: [5, 6, 7, 8],
            bold: true,
            italic: true,
            underline: true,
            strikethrough: true,
            hyperlink_id: Some(9),
            wide_char: true,
            wide_char_spacer: true,
        };
        let mut dst = Cell::default();

        dst.clone_from(&source);

        assert_eq!(dst, source);
        assert_eq!(dst, source.clone());
    }
}
