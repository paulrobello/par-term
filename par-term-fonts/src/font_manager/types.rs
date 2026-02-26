//! Font data types and structures for font management.

use std::sync::Arc;
use swash::FontRef;

/// Stores font data with lifetime management.
///
/// This struct owns the font data bytes and provides a `FontRef` that can be used
/// for glyph lookups and rasterization. The `FontRef` is guaranteed to be valid
/// for the lifetime of this struct.
#[derive(Clone)]
pub struct FontData {
    /// Raw font data bytes (TTF/OTF)
    pub data: Arc<Vec<u8>>,
    /// Swash font reference for glyph operations
    pub font_ref: FontRef<'static>,
}

impl std::fmt::Debug for FontData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontData")
            .field("data_len", &self.data.len())
            .finish()
    }
}

impl FontData {
    /// Create a new FontData from bytes using face index 0.
    ///
    /// # Arguments
    /// * `data` - Raw font data bytes (TTF/OTF format)
    ///
    /// # Returns
    /// `Some(FontData)` if the font data is valid, `None` otherwise.
    pub fn new(data: Vec<u8>) -> Option<Self> {
        Self::new_with_index(data, 0)
    }

    /// Create a new FontData from bytes with a specific face index.
    ///
    /// This is needed for TrueType Collection (.ttc) files where multiple
    /// font faces share the same data but have different face indices.
    ///
    /// # Arguments
    /// * `data` - Raw font data bytes (TTF/OTF/TTC format)
    /// * `face_index` - Face index within the font data (0 for single-face fonts)
    ///
    /// # Returns
    /// `Some(FontData)` if the font data is valid, `None` otherwise.
    pub fn new_with_index(data: Vec<u8>, face_index: usize) -> Option<Self> {
        let data_arc = Arc::new(data);

        // SAFETY: We ensure the data outlives the FontRef by storing it in an Arc.
        // The FontRef will never outlive the FontData struct because they are stored
        // together and dropped together.
        let font_ref = unsafe {
            let bytes = data_arc.as_slice();
            let static_bytes: &'static [u8] = std::mem::transmute(bytes);
            FontRef::from_index(static_bytes, face_index)?
        };

        Some(FontData {
            data: data_arc,
            font_ref,
        })
    }
}

/// Font mapping for a specific Unicode range.
///
/// This allows configuring specific fonts for certain character ranges,
/// such as CJK characters, emoji, or special symbols.
#[derive(Debug, Clone)]
pub struct UnicodeRangeFont {
    /// Start of Unicode range (inclusive)
    pub start: u32,
    /// End of Unicode range (inclusive)
    pub end: u32,
    /// Font data for this range
    pub font: FontData,
    /// Font index in the overall font list (used for caching)
    pub font_index: usize,
}
