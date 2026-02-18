/// A single terminal cell with styled content for rendering
///
/// This is the bridge between terminal emulation (core library cells with VT attributes)
/// and GPU rendering (colored rectangles and textured glyphs). The `TerminalManager`
/// converts core cells into these, applying theme colors and selection state.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub grapheme: String,
    pub fg_color: [u8; 4],
    pub bg_color: [u8; 4],
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub hyperlink_id: Option<u32>,
    pub wide_char: bool,
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
