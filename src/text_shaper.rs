//! Text shaping module using HarfBuzz via rustybuzz.
//!
//! This module re-exports types from the par-term-fonts crate for backward compatibility.
//! All text shaping types and logic are defined in par-term-fonts.

pub use par_term_fonts::text_shaper::{ShapedGlyph, ShapedRun, ShapingOptions, TextShaper};
