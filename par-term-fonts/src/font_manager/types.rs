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
    /// Swash font reference for glyph operations.
    ///
    /// Deliberately private. The `'static` lifetime is fabricated by the transmute
    /// in [`FontData::new_with_index`] and `FontRef` is `Copy`, so a public field
    /// would let any caller copy the reference out and keep dereferencing it after
    /// this `FontData` — and with it the `Arc` keeping the bytes alive — is dropped.
    /// [`FontData::font_ref`] hands it out re-bound to the borrow of `self`.
    font_ref: FontRef<'static>,
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

        // SAFETY: the bytes live in a heap buffer owned by `data_arc`, so the slice
        // address stays valid however this `FontData` is moved or cloned, and the
        // buffer is only freed when the last `FontData` sharing the `Arc` drops. The
        // fabricated `'static` is never observable outside this module: `font_ref` is
        // private and `FontData::font_ref` re-binds it to a borrow of `self`, so no
        // caller can hold the reference past the owner's drop.
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

    /// Borrow the swash font reference for glyph lookups and rasterization.
    ///
    /// The reference borrows `self`, so neither it nor a `FontRef` copied out of it
    /// can outlive the `FontData` that owns the underlying bytes.
    pub fn font_ref(&self) -> &FontRef<'_> {
        &self.font_ref
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

#[cfg(test)]
mod tests {
    /// SEC-017: `font_ref` is reachable only through the accessor, and the slice it
    /// exposes must point into the `Arc`-owned buffer rather than the caller's
    /// original `Vec` — otherwise the fabricated `'static` would be dangling the
    /// moment the argument to `new_with_index` is dropped.
    #[test]
    fn font_ref_borrows_the_arc_owned_bytes() {
        let font = super::super::loader::load_embedded_font().expect("embedded font parses");

        assert_eq!(
            font.data.as_ptr(),
            font.font_ref().data.as_ptr(),
            "font_ref must alias the Arc-owned buffer, not a temporary"
        );
    }

    /// Cloning shares the `Arc`, so a clone's `font_ref` stays valid after the
    /// original is dropped. This is the property the `'static` transmute relies on.
    #[test]
    fn clone_outlives_the_original() {
        let font = super::super::loader::load_embedded_font().expect("embedded font parses");
        let clone = font.clone();
        drop(font);

        assert_ne!(clone.font_ref().charmap().map('m'), 0);
    }
}
