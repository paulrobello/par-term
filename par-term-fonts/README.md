# par-term-fonts

Font management and text shaping for the par-term terminal emulator.

This crate provides font loading with system font discovery, Unicode range-specific fallback
chains, HarfBuzz-based text shaping via rustybuzz for ligatures and complex scripts, and
grapheme cluster detection for correct Unicode rendering.

## What This Crate Provides

- `FontManager` — orchestrates font loading and glyph lookup across a priority-ordered font chain
- `FontData` — loaded font data with face index and raw bytes
- `UnicodeRangeFont` — maps a Unicode codepoint range to a specific font family
- `FALLBACK_FAMILIES` — curated list of system fallback font families across platforms
- `TextShaper` — HarfBuzz-based text shaping with LRU result caching
- `ShapedGlyph` / `ShapedRun` — output types from the text shaper
- `ShapingOptions` — controls ligatures, kerning, and other OpenType features

## Font Chain Priority

The `FontManager` resolves glyphs through a priority-ordered chain:

1. Primary font (with bold, italic, and bold-italic variants)
2. Unicode range-specific fonts (user-configured for CJK, emoji, etc.)
3. System fallback fonts (platform-specific families)

## Architecture

```
FontManager
├── Primary font (regular / bold / italic / bold-italic)
├── Unicode range fonts (user-configured per codepoint range)
└── System fallbacks (FALLBACK_FAMILIES list)

TextShaper
└── LRU cache keyed by (text, font, options) → Vec<ShapedGlyph>
```

## Workspace Position

Layer 2 in the dependency graph. Depends on `par-term-config` for font configuration.
Used by `par-term-render` (Layer 3) for glyph rasterization.

## Installation

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
par-term-fonts = { version = "0.1.7" }
```

## Usage

```rust
use par_term_fonts::{FontManager, TextShaper, ShapingOptions};

// Create a font manager with the primary font
let mut font_manager = FontManager::new();
font_manager.set_primary_font("JetBrains Mono");

// Shape text for rendering
let shaper = TextShaper::new(font_manager);
let glyphs = shaper.shape("Hello, world!", &ShapingOptions::default());
```

## Related Documentation

- [Quick Start: Fonts](../docs/QUICK_START_FONTS.md) — font configuration guide
- [Config Reference](../docs/CONFIG_REFERENCE.md) — font configuration options
- [Architecture Overview](../docs/ARCHITECTURE.md) — workspace structure
- [Crate Structure](../docs/CRATE_STRUCTURE.md) — dependency layers
