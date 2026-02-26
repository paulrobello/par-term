//! Title parsing, sanitization, and rendering utilities for the tab bar.
//!
//! This module handles HTML-subset title parsing, emoji-to-monochrome sanitization,
//! and styled segment rendering used by the tab bar UI.

use std::borrow::Cow;
use unicode_segmentation::UnicodeSegmentation;

/// Styled text segment for rich tab titles
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StyledSegment {
    pub(super) text: String,
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) underline: bool,
    pub(super) color: Option<[u8; 3]>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TitleStyle {
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) underline: bool,
    pub(super) color: Option<[u8; 3]>,
}

pub(super) fn truncate_plain(title: &str, max_len: usize) -> String {
    if max_len == 0 {
        return "…".to_string();
    }
    let mut chars = title.chars();
    let mut taken = String::new();
    for _ in 0..max_len {
        if let Some(c) = chars.next() {
            taken.push(c);
        } else {
            return taken;
        }
    }
    if chars.next().is_some() {
        if max_len > 0 {
            taken.pop();
        }
        taken.push('…');
    }
    taken
}

/// Replace emoji-heavy graphemes with monochrome symbols/icons that render reliably in egui.
///
/// par-term configures Nerd Font Symbols as an egui fallback, but many full-color emoji glyphs
/// (especially SMP emoji / ZWJ sequences) still fail or render poorly. We keep the stored title
/// untouched and only sanitize at the egui title rendering boundary.
pub(super) fn sanitize_egui_title_text(input: &str) -> Cow<'_, str> {
    if !input.chars().any(is_egui_title_suspect_char) {
        return Cow::Borrowed(input);
    }

    let mut out = String::with_capacity(input.len());
    let mut changed = false;

    for grapheme in UnicodeSegmentation::graphemes(input, true) {
        if let Some(replacement) = map_title_grapheme_to_monochrome(grapheme) {
            out.push_str(replacement);
            changed = true;
            continue;
        }

        if grapheme.chars().all(is_regional_indicator) {
            for ch in grapheme.chars() {
                if let Some(letter) = regional_indicator_to_ascii(ch) {
                    out.push(letter);
                }
            }
            changed = true;
            continue;
        }

        let mut stripped = String::new();
        let mut stripped_any = false;
        for ch in grapheme.chars() {
            if is_egui_title_ignorable_char(ch) {
                stripped_any = true;
                changed = true;
                continue;
            }
            stripped.push(ch);
        }

        if stripped_any {
            if stripped.is_empty() {
                continue;
            }

            if let Some(replacement) = map_title_grapheme_to_monochrome(&stripped) {
                out.push_str(replacement);
                continue;
            }

            if stripped.chars().any(is_smp_pictograph) {
                // Mixed grapheme (often a ZWJ emoji sequence) still contains unsupported emoji.
                // Collapse to a single visible marker rather than tofu boxes.
                out.push('•');
                continue;
            }

            out.push_str(&stripped);
            continue;
        }

        if grapheme.chars().any(is_smp_pictograph) {
            out.push('•');
            changed = true;
            continue;
        }

        out.push_str(grapheme);
    }

    if changed {
        Cow::Owned(out)
    } else {
        Cow::Borrowed(input)
    }
}

pub(super) fn sanitize_styled_segments_for_egui(
    mut segments: Vec<StyledSegment>,
) -> Vec<StyledSegment> {
    for segment in &mut segments {
        let safe = sanitize_egui_title_text(&segment.text);
        if let Cow::Owned(text) = safe {
            segment.text = text;
        }
    }
    segments
}

pub(super) fn is_egui_title_suspect_char(ch: char) -> bool {
    is_egui_title_ignorable_char(ch) || is_regional_indicator(ch) || is_smp_pictograph(ch)
}

pub(super) fn is_egui_title_ignorable_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{FE0E}' // text presentation selector (safe to drop in tab titles)
            | '\u{FE0F}' // emoji presentation selector
            | '\u{200D}' // zero-width joiner
            | '\u{20E3}' // combining keycap
    ) || matches!(ch, '\u{1F3FB}'..='\u{1F3FF}') // skin tone modifiers
}

pub(super) fn is_regional_indicator(ch: char) -> bool {
    matches!(ch, '\u{1F1E6}'..='\u{1F1FF}')
}

pub(super) fn regional_indicator_to_ascii(ch: char) -> Option<char> {
    if !is_regional_indicator(ch) {
        return None;
    }
    let offset = (ch as u32) - 0x1F1E6;
    char::from_u32(u32::from(b'A') + offset)
}

/// SMP pictographs (most "emoji-only" symbols) commonly fail in egui's font stack.
pub(super) fn is_smp_pictograph(ch: char) -> bool {
    matches!(
        ch as u32,
        0x1F300..=0x1FAFF // Misc Symbols & Pictographs -> Symbols & Pictographs Extended-A
    )
}

pub(super) fn map_title_grapheme_to_monochrome(grapheme: &str) -> Option<&'static str> {
    // Prefer Nerd Font monochrome icons (egui fallback font is configured on window init).
    // Fall back to standard BMP symbols where a good equivalent exists.
    match grapheme {
        "👨‍💻" | "👩‍💻" | "🧑‍💻" => Some("\u{f121}"),      // nerd-font code
        "🤖" => Some("\u{ee0d}"),                    // nerd-font robot
        "🧠" => Some("\u{f2db}"), // nerd-font chip (closest dev-ish monochrome analog)
        "🚀" => Some("\u{f135}"), // nerd-font rocket
        "💡" => Some("\u{f0eb}"), // nerd-font lightbulb
        "🎯" => Some("\u{f140}"), // nerd-font crosshairs
        "🎛" | "🎛️" | "🎚" | "🎚️" => Some("\u{f1de}"), // nerd-font sliders
        "🛠" | "🛠️" | "🔧" | "🔨" | "🧰" => Some("\u{f0ad}"), // nerd-font wrench
        "🔒" => Some("\u{f023}"), // nerd-font lock
        "🔓" => Some("\u{eb74}"), // nerd-font unlock
        "🔔" => Some("\u{f0f3}"), // nerd-font bell
        "📁" | "🗂" | "🗂️" => Some("\u{ea83}"), // nerd-font folder
        "📂" => Some("\u{eaf7}"), // nerd-font open folder
        "📄" => Some("\u{ea7b}"), // nerd-font file
        "📦" => Some("\u{f487}"), // nerd-font package
        "📝" | "✍" | "✍️" => Some("\u{f040}"), // nerd-font pencil
        "🌐" => Some("\u{f0ac}"), // nerd-font globe
        "☁" | "☁️" => Some("\u{ebaa}"), // nerd-font cloud
        "⭐" | "🌟" => Some("★"),
        "✨" | "💫" => Some("✦"),
        "🔥" => Some("\u{f06d}"), // nerd-font fire
        "✅" => Some("✓"),
        "❌" => Some("✕"),
        "🔍" | "🔎" => Some("⌕"),
        "🔗" => Some("⛓"),
        "📌" | "📍" => Some("•"),
        "🧪" => Some("⚗"),
        "🟢" | "🟩" | "🔵" | "🟦" | "🟣" | "🟪" | "🟡" | "🟨" | "🟠" | "🟧" | "🔴" | "🟥" => {
            Some("●")
        }
        "⚪" | "⚫" | "⬜" | "⬛" => Some("●"),
        _ => None,
    }
}

pub(super) fn truncate_segments(segments: &[StyledSegment], max_len: usize) -> Vec<StyledSegment> {
    if max_len == 0 {
        return vec![StyledSegment {
            text: "…".to_string(),
            bold: false,
            italic: false,
            underline: false,
            color: None,
        }];
    }
    let mut remaining = max_len;
    let mut out: Vec<StyledSegment> = Vec::new();
    for seg in segments {
        if remaining == 0 {
            break;
        }
        let seg_len = seg.text.chars().count();
        if seg_len == 0 {
            continue;
        }
        if seg_len <= remaining {
            out.push(seg.clone());
            remaining -= seg_len;
        } else {
            let truncated_text: String =
                seg.text.chars().take(remaining.saturating_sub(1)).collect();
            let mut truncated = seg.clone();
            truncated.text = truncated_text;
            truncated.text.push('…');
            out.push(truncated);
            remaining = 0;
        }
    }
    out
}

pub(super) fn render_segments(
    ui: &mut egui::Ui,
    segments: &[StyledSegment],
    fallback_color: egui::Color32,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for segment in segments {
            let mut rich = egui::RichText::new(&segment.text);
            if segment.bold {
                rich = rich.strong();
            }
            if segment.italic {
                rich = rich.italics();
            }
            if segment.underline {
                rich = rich.underline();
            }
            if let Some(color) = segment.color {
                rich = rich.color(egui::Color32::from_rgb(color[0], color[1], color[2]));
            } else {
                rich = rich.color(fallback_color);
            }
            ui.label(rich);
        }
    });
}

pub(super) fn estimate_max_chars(
    _ui: &egui::Ui,
    font_id: &egui::FontId,
    available_width: f32,
) -> usize {
    let char_width = (font_id.size * 0.55).max(4.0); // heuristic: ~0.55em per character
    ((available_width / char_width).floor() as usize).max(4)
}

pub(super) fn parse_html_title(input: &str) -> Vec<StyledSegment> {
    let mut segments: Vec<StyledSegment> = Vec::new();
    let mut style_stack: Vec<TitleStyle> = vec![TitleStyle {
        bold: false,
        italic: false,
        underline: false,
        color: None,
    }];
    let mut buffer = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            // flush buffer
            if !buffer.is_empty() {
                let style = *style_stack.last().unwrap_or(&TitleStyle {
                    bold: false,
                    italic: false,
                    underline: false,
                    color: None,
                });
                segments.push(StyledSegment {
                    text: buffer.clone(),
                    bold: style.bold,
                    italic: style.italic,
                    underline: style.underline,
                    color: style.color,
                });
                buffer.clear();
            }

            // read tag
            let mut tag = String::new();
            while let Some(&c) = chars.peek() {
                chars.next();
                if c == '>' {
                    break;
                }
                tag.push(c);
            }

            let tag_trimmed = tag.trim().to_lowercase();
            match tag_trimmed.as_str() {
                "b" => {
                    let mut style = *style_stack
                        .last()
                        .expect("style stack always has at least one entry");
                    style.bold = true;
                    style_stack.push(style);
                }
                "/b" => {
                    pop_style(&mut style_stack, |s| s.bold);
                }
                "i" => {
                    let mut style = *style_stack
                        .last()
                        .expect("style stack always has at least one entry");
                    style.italic = true;
                    style_stack.push(style);
                }
                "/i" => {
                    pop_style(&mut style_stack, |s| s.italic);
                }
                "u" => {
                    let mut style = *style_stack
                        .last()
                        .expect("style stack always has at least one entry");
                    style.underline = true;
                    style_stack.push(style);
                }
                "/u" => {
                    pop_style(&mut style_stack, |s| s.underline);
                }
                t if t.starts_with("span") => {
                    if let Some(color) = parse_span_color(&tag_trimmed) {
                        let mut style = *style_stack
                            .last()
                            .expect("style stack always has at least one entry");
                        style.color = Some(color);
                        style_stack.push(style);
                    } else {
                        // unsupported span attributes: ignore tag
                    }
                }
                "/span" => {
                    pop_style(&mut style_stack, |s| s.color.is_some());
                }
                _ => {
                    // Unknown or unsupported tag, ignore
                }
            }
        } else {
            buffer.push(ch);
        }
    }

    if !buffer.is_empty() {
        let style = *style_stack.last().unwrap_or(&TitleStyle {
            bold: false,
            italic: false,
            underline: false,
            color: None,
        });
        segments.push(StyledSegment {
            text: buffer,
            bold: style.bold,
            italic: style.italic,
            underline: style.underline,
            color: style.color,
        });
    }

    segments
}

pub(super) fn pop_style<F>(stack: &mut Vec<TitleStyle>, predicate: F)
where
    F: Fn(&TitleStyle) -> bool,
{
    if stack.len() <= 1 {
        return;
    }
    for idx in (1..stack.len()).rev() {
        let style = stack[idx];
        if predicate(&style) {
            stack.remove(idx);
            return;
        }
    }
}

pub(super) fn parse_span_color(tag: &str) -> Option<[u8; 3]> {
    // expect like: span style="color:#rrggbb" or color:rgb(r,g,b)
    let style_attr = tag.split("style=").nth(1)?;
    let style_val = style_attr
        .trim_start_matches(['\"', '\''])
        .trim_end_matches(['\"', '\'']);
    let mut color_part = None;
    for decl in style_val.split(';') {
        let mut kv = decl.splitn(2, ':');
        let key = kv.next()?.trim();
        let val = kv.next()?.trim();
        if key == "color" {
            color_part = Some(val);
            break;
        }
    }
    let color_str = color_part?;
    if let Some(hex) = color_str.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some([r, g, b]);
        }
    } else if let Some(rgb) = color_str
        .strip_prefix("rgb(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let parts: Vec<&str> = rgb.split(',').map(|p| p.trim()).collect();
        if parts.len() == 3 {
            let r = parts[0].parse::<u8>().ok()?;
            let g = parts[1].parse::<u8>().ok()?;
            let b = parts[2].parse::<u8>().ok()?;
            return Some([r, g, b]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_html_title_basic_tags() {
        let segments = parse_html_title("<b>Hello</b> <i>world</i>");
        assert_eq!(
            segments,
            vec![
                StyledSegment {
                    text: "Hello".to_string(),
                    bold: true,
                    italic: false,
                    underline: false,
                    color: None
                },
                StyledSegment {
                    text: " ".to_string(),
                    bold: false,
                    italic: false,
                    underline: false,
                    color: None
                },
                StyledSegment {
                    text: "world".to_string(),
                    bold: false,
                    italic: true,
                    underline: false,
                    color: None
                }
            ]
        );
    }

    #[test]
    fn parse_html_title_span_color() {
        let segments = parse_html_title("<span style=\"color:#ff0000\">Red</span> text");
        assert_eq!(segments.len(), 2);
        assert_eq!(
            segments[0],
            StyledSegment {
                text: "Red".to_string(),
                bold: false,
                italic: false,
                underline: false,
                color: Some([255, 0, 0])
            }
        );
    }

    #[test]
    fn truncate_segments_adds_ellipsis() {
        let segs = vec![StyledSegment {
            text: "HelloWorld".to_string(),
            bold: false,
            italic: false,
            underline: false,
            color: None,
        }];
        let truncated = truncate_segments(&segs, 6);
        assert_eq!(truncated[0].text, "Hello…");
    }

    #[test]
    fn truncate_plain_handles_short_text() {
        assert_eq!(truncate_plain("abc", 5), "abc");
        assert_eq!(truncate_plain("abcdef", 5), "abcd…");
    }

    #[test]
    fn sanitize_egui_title_text_strips_variation_sequences() {
        let input = "Build ⚙️ 1️⃣ ready";
        assert_eq!(sanitize_egui_title_text(input), "Build ⚙ 1 ready");
    }

    #[test]
    fn sanitize_egui_title_text_maps_flags_to_letters() {
        assert_eq!(sanitize_egui_title_text("🇺🇸 deploy"), "US deploy");
    }

    #[test]
    fn sanitize_egui_title_text_maps_common_dev_emoji() {
        let mapped = sanitize_egui_title_text("🤖 tune 🎛️");
        assert_eq!(mapped, "\u{ee0d} tune \u{f1de}");
    }

    #[test]
    fn sanitize_egui_title_text_falls_back_for_unknown_smp_emoji() {
        assert_eq!(sanitize_egui_title_text("face 😀 ok"), "face • ok");
    }
}
