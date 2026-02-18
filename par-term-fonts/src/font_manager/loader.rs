//! Font loading utilities for system and embedded fonts.

use anyhow::Result;
use fontdb::{Database, Family, Query};

use super::types::FontData;

/// Embedded DejaVu Sans Mono font (TTF format).
const EMBEDDED_FONT: &[u8] = include_bytes!("../../fonts/DejaVuSansMono.ttf");

/// Load the embedded DejaVu Sans Mono font.
///
/// This is used as the ultimate fallback when no system fonts are available.
///
/// # Errors
/// Returns an error if the embedded font data is invalid.
pub fn load_embedded_font() -> Result<FontData> {
    let data = EMBEDDED_FONT.to_vec();
    FontData::new(data).ok_or_else(|| anyhow::anyhow!("Failed to load embedded font"))
}

/// Load a font from the system font database.
///
/// # Arguments
/// * `db` - The font database to query
/// * `family_name` - Name of the font family to load
///
/// # Returns
/// `Some(FontData)` if the font was found and loaded successfully.
pub fn load_font_from_db(db: &mut Database, family_name: &str) -> Option<FontData> {
    load_font_from_db_with_style(db, family_name, None, None)
}

/// Load a font from the system font database with specific style.
///
/// # Arguments
/// * `db` - The font database to query
/// * `family_name` - Name of the font family to load
/// * `weight` - Optional font weight (default: NORMAL)
/// * `style` - Optional font style (default: Normal)
///
/// # Returns
/// `Some(FontData)` if the font was found and loaded successfully.
pub fn load_font_from_db_with_style(
    db: &mut Database,
    family_name: &str,
    weight: Option<fontdb::Weight>,
    style: Option<fontdb::Style>,
) -> Option<FontData> {
    // Query for the font family with optional weight and style
    let query = Query {
        families: &[Family::Name(family_name)],
        weight: weight.unwrap_or(fontdb::Weight::NORMAL),
        style: style.unwrap_or(fontdb::Style::Normal),
        ..Query::default()
    };

    let id = db.query(&query)?;

    // Load font data from the database
    // SAFETY: make_shared_face_data is safe when called with a valid ID from query()
    let (data, face_index) = unsafe { db.make_shared_face_data(id)? };

    // Convert the shared data to Vec<u8>
    // Pass face_index for TTC (TrueType Collection) files where multiple fonts
    // share the same data but have different face indices.
    let bytes = data.as_ref().as_ref();
    FontData::new_with_index(bytes.to_vec(), face_index as usize)
}
