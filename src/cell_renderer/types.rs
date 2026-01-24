/// Vertex for cell rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vertex {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2],
}

/// Instance data for background rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BackgroundInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}

/// Instance data for text rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct TextInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub tex_offset: [f32; 2],
    pub tex_size: [f32; 2],
    pub color: [f32; 4],
    pub is_colored: u32, // 1 for emoji/colored glyphs, 0 for regular text
}

/// A single terminal cell
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

/// Glyph info for atlas
#[derive(Clone, Debug)]
pub(crate) struct GlyphInfo {
    #[allow(dead_code)]
    pub key: u64,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    #[allow(dead_code)]
    pub bearing_x: f32,
    #[allow(dead_code)]
    pub bearing_y: f32,
    pub is_colored: bool,
    pub prev: Option<u64>,
    pub next: Option<u64>,
}

/// Row cache entry
pub(crate) struct RowCacheEntry {}
