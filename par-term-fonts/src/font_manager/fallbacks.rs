//! Font fallback chain configuration.
//!
//! Defines the priority order of fallback fonts for comprehensive Unicode coverage.

/// Fallback font families in priority order.
///
/// These fonts are searched in order when the primary font doesn't have a glyph.
/// The order is designed to provide:
/// 1. Nerd Font icon support (programming symbols, powerline)
/// 2. Standard monospace fonts for ASCII/Latin
/// 3. CJK support (Japanese, Simplified/Traditional Chinese, Korean)
/// 4. Emoji and symbol fonts (including flags)
/// 5. General Unicode coverage
pub const FALLBACK_FAMILIES: &[&str] = &[
    // Nerd Fonts (first priority for icon/symbol support)
    "JetBrainsMono Nerd Font",
    "JetBrainsMono NF",
    "FiraCode Nerd Font",
    "FiraCode NF",
    "Hack Nerd Font",
    "Hack NF",
    "MesloLGS NF",
    // Standard monospace fonts
    "JetBrains Mono",
    "Fira Code",
    "Consolas",
    "Monaco",
    "Menlo",
    "Courier New",
    // CJK fonts (critical for Asian language support)
    "Noto Sans CJK JP",
    "Noto Sans CJK SC",
    "Noto Sans CJK TC",
    "Noto Sans CJK KR",
    "Microsoft YaHei",
    "MS Gothic",
    "SimHei",
    "Malgun Gothic",
    // Monochrome symbol fonts (before emoji to prefer outline rendering for dingbats/symbols)
    "Symbols Nerd Font",
    "Apple Symbols",
    "Zapf Dingbats",
    "STIX Two Math",
    "Segoe UI Symbol",
    "Noto Sans Symbols",
    "Noto Sans Symbols 2",
    "DejaVu Sans",
    "Symbola",
    "Arial Unicode MS",
    // Color emoji fonts (flag support, pictographs)
    "Noto Color Emoji",
    "Apple Color Emoji",
    "Segoe UI Emoji",
    // General fallbacks
    "Arial",
    "Liberation Sans",
];
