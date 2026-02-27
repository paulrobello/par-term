//! Configuration types for the Markdown renderer.
//!
//! Defines [`HeaderStyle`], [`LinkStyle`], [`HorizontalRuleStyle`], and
//! [`MarkdownRendererConfig`].

use super::super::table::TableStyle;

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// How to style header elements.
#[derive(Clone, Debug, Default)]
pub enum HeaderStyle {
    /// Each level gets a distinct color from the theme palette.
    #[default]
    Colored,
    /// All headers bold, with decreasing brightness per level.
    Bold,
    /// H1/H2 underlined, rest bold.
    Underlined,
}

/// How to render links.
#[derive(Clone, Debug, Default)]
pub enum LinkStyle {
    /// Underline + link color with OSC 8 hyperlink.
    #[default]
    UnderlineColor,
    /// Show `text (url)` inline.
    InlineUrl,
    /// Show `text[1]` with footnotes collected at end (not yet implemented).
    Footnote,
}

/// How to render horizontal rules.
#[derive(Clone, Debug, Default)]
pub enum HorizontalRuleStyle {
    /// `─` repeated.
    #[default]
    Thin,
    /// `━` repeated.
    Thick,
    /// `╌` repeated.
    Dashed,
}

/// Configuration for the `MarkdownRenderer`.
#[derive(Clone, Debug)]
pub struct MarkdownRendererConfig {
    pub header_style: HeaderStyle,
    pub link_style: LinkStyle,
    pub horizontal_rule_style: HorizontalRuleStyle,
    /// Show background shading on fenced code blocks.
    pub code_block_background: bool,
    /// Table border style.
    pub table_style: TableStyle,
    /// Table border color as [r, g, b]. Use dim grey by default.
    pub table_border_color: [u8; 3],
}

impl Default for MarkdownRendererConfig {
    fn default() -> Self {
        Self {
            header_style: HeaderStyle::Colored,
            link_style: LinkStyle::UnderlineColor,
            horizontal_rule_style: HorizontalRuleStyle::Thin,
            code_block_background: true,
            table_style: TableStyle::Unicode,
            table_border_color: [108, 112, 134],
        }
    }
}
