/// A single terminal cell with styled content for rendering.
///
/// This is the bridge between terminal emulation (core library cells with VT attributes)
/// and GPU rendering (colored rectangles and textured glyphs). The `TerminalManager`
/// converts core cells into these, applying theme colors and selection state.
#[derive(Clone, Debug, PartialEq)]
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
