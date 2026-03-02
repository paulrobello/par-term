//! ANSI SGR (Select Graphic Rendition) parser for styled terminal output.
//!
//! Parses escape sequences in terminal output lines into [`StyledLine`] values
//! composed of [`StyledSegment`]s, each carrying RGB colour and text-attribute
//! information derived from the SGR codes.
//!
//! The colour data (`ANSI_COLORS`, `ANSI_BRIGHT`, `color_256_to_rgb`) lives in
//! [`crate::ansi_colors`]; this module only contains the state machine and
//! parser.

use crate::ansi_colors::{ANSI_BRIGHT, ANSI_COLORS, color_256_to_rgb};
use crate::types::{StyledLine, StyledSegment};

// ---------------------------------------------------------------------------
// SGR state
// ---------------------------------------------------------------------------

/// Current SGR (Select Graphic Rendition) attribute state during ANSI parsing.
#[derive(Clone, Default)]
pub(crate) struct SgrState {
    pub fg: Option<[u8; 3]>,
    pub bg: Option<[u8; 3]>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

/// Apply a sequence of SGR parameters to the current state.
fn apply_sgr(state: &mut SgrState, params: &[u16]) {
    let mut i = 0;
    while i < params.len() {
        match params[i] {
            0 => *state = SgrState::default(),
            1 => state.bold = true,
            3 => state.italic = true,
            4 => state.underline = true,
            9 => state.strikethrough = true,
            22 => state.bold = false,
            23 => state.italic = false,
            24 => state.underline = false,
            29 => state.strikethrough = false,
            // Standard foreground colors
            30..=37 => state.fg = Some(ANSI_COLORS[(params[i] - 30) as usize]),
            38 => {
                // Extended foreground: 38;5;N or 38;2;R;G;B
                if i + 1 < params.len() {
                    match params[i + 1] {
                        5 if i + 2 < params.len() => {
                            state.fg = Some(color_256_to_rgb(params[i + 2] as u8));
                            i += 2;
                        }
                        2 if i + 4 < params.len() => {
                            state.fg = Some([
                                params[i + 2] as u8,
                                params[i + 3] as u8,
                                params[i + 4] as u8,
                            ]);
                            i += 4;
                        }
                        _ => {}
                    }
                }
            }
            39 => state.fg = None,
            // Standard background colors
            40..=47 => state.bg = Some(ANSI_COLORS[(params[i] - 40) as usize]),
            48 => {
                // Extended background: 48;5;N or 48;2;R;G;B
                if i + 1 < params.len() {
                    match params[i + 1] {
                        5 if i + 2 < params.len() => {
                            state.bg = Some(color_256_to_rgb(params[i + 2] as u8));
                            i += 2;
                        }
                        2 if i + 4 < params.len() => {
                            state.bg = Some([
                                params[i + 2] as u8,
                                params[i + 3] as u8,
                                params[i + 4] as u8,
                            ]);
                            i += 4;
                        }
                        _ => {}
                    }
                }
            }
            49 => state.bg = None,
            // Bright foreground colors
            90..=97 => state.fg = Some(ANSI_BRIGHT[(params[i] - 90) as usize]),
            // Bright background colors
            100..=107 => state.bg = Some(ANSI_BRIGHT[(params[i] - 100) as usize]),
            _ => {} // Ignore unrecognized codes
        }
        i += 1;
    }
}

// ---------------------------------------------------------------------------
// Line parser
// ---------------------------------------------------------------------------

/// Parse a line that may contain ANSI escape codes into a `StyledLine`.
///
/// Handles CSI SGR sequences (`ESC[...m`) including:
/// - Reset (0), bold (1), italic (3), underline (4), strikethrough (9)
/// - Standard colors (30–37 fg, 40–47 bg)
/// - Bright colors (90–97 fg, 100–107 bg)
/// - 256-color mode (38;5;N / 48;5;N)
/// - RGB true-color (38;2;R;G;B / 48;2;R;G;B)
///
/// Non-SGR escape sequences (cursor movement, etc.) are silently skipped.
pub(crate) fn parse_ansi_line(line: &str) -> StyledLine {
    let mut segments: Vec<StyledSegment> = Vec::new();
    let mut state = SgrState::default();
    let mut text_buf = String::new();
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['

                // Collect the parameter string until a letter terminates the sequence.
                let mut param_str = String::new();
                let mut terminator = ' ';
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        terminator = next;
                        break;
                    }
                    param_str.push(next);
                }

                if terminator == 'm' {
                    // SGR sequence — flush current text and apply new attributes.
                    if !text_buf.is_empty() {
                        segments.push(StyledSegment {
                            text: std::mem::take(&mut text_buf),
                            fg: state.fg,
                            bg: state.bg,
                            bold: state.bold,
                            italic: state.italic,
                            underline: state.underline,
                            strikethrough: state.strikethrough,
                            link_url: None,
                        });
                    }

                    let params: Vec<u16> = if param_str.is_empty() {
                        vec![0] // bare ESC[m means reset
                    } else {
                        param_str
                            .split(';')
                            .filter_map(|p| p.parse().ok())
                            .collect()
                    };
                    apply_sgr(&mut state, &params);
                }
                // Non-SGR sequences (cursor movement, etc.) are silently dropped.
            }
            // Bare ESC without '[' — skip it.
        } else {
            text_buf.push(c);
        }
    }

    // Flush remaining text.
    if !text_buf.is_empty() {
        segments.push(StyledSegment {
            text: text_buf,
            fg: state.fg,
            bg: state.bg,
            bold: state.bold,
            italic: state.italic,
            underline: state.underline,
            strikethrough: state.strikethrough,
            link_url: None,
        });
    }

    if segments.is_empty() {
        StyledLine::plain("")
    } else {
        StyledLine::new(segments)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ansi_colors::color_256_to_rgb;

    #[test]
    fn test_parse_ansi_plain_text() {
        let line = parse_ansi_line("hello");
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].text, "hello");
        assert!(line.segments[0].fg.is_none());
    }

    #[test]
    fn test_parse_ansi_fg_color() {
        let line = parse_ansi_line("\x1b[31mred\x1b[0m normal");
        assert_eq!(line.segments.len(), 2);
        assert_eq!(line.segments[0].text, "red");
        assert_eq!(line.segments[0].fg, Some([170, 0, 0])); // ANSI red
        assert_eq!(line.segments[1].text, " normal");
        assert!(line.segments[1].fg.is_none());
    }

    #[test]
    fn test_parse_ansi_bold_and_color() {
        let line = parse_ansi_line("\x1b[1;32mbold green\x1b[0m");
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].text, "bold green");
        assert!(line.segments[0].bold);
        assert_eq!(line.segments[0].fg, Some([0, 170, 0])); // ANSI green
    }

    #[test]
    fn test_parse_ansi_bright_colors() {
        let line = parse_ansi_line("\x1b[91mbright red\x1b[0m");
        assert_eq!(line.segments[0].fg, Some([255, 85, 85]));
    }

    #[test]
    fn test_parse_ansi_256_color() {
        let line = parse_ansi_line("\x1b[38;5;196mcolor\x1b[0m");
        assert_eq!(line.segments[0].text, "color");
        assert!(line.segments[0].fg.is_some());
    }

    #[test]
    fn test_parse_ansi_rgb_color() {
        let line = parse_ansi_line("\x1b[38;2;100;200;50mrgb\x1b[0m");
        assert_eq!(line.segments[0].text, "rgb");
        assert_eq!(line.segments[0].fg, Some([100, 200, 50]));
    }

    #[test]
    fn test_parse_ansi_bg_color() {
        let line = parse_ansi_line("\x1b[44mblue bg\x1b[0m");
        assert_eq!(line.segments[0].bg, Some([0, 0, 170])); // ANSI blue
    }

    #[test]
    fn test_parse_ansi_italic_underline_strikethrough() {
        let line =
            parse_ansi_line("\x1b[3mitalic\x1b[0m \x1b[4munderline\x1b[0m \x1b[9mstrike\x1b[0m");
        // Resets between styled words produce separate segments for the spaces
        let italic_seg = line.segments.iter().find(|s| s.text == "italic").unwrap();
        let underline_seg = line
            .segments
            .iter()
            .find(|s| s.text == "underline")
            .unwrap();
        let strike_seg = line.segments.iter().find(|s| s.text == "strike").unwrap();
        assert!(italic_seg.italic);
        assert!(underline_seg.underline);
        assert!(strike_seg.strikethrough);
    }

    #[test]
    fn test_parse_ansi_reset_bare() {
        // ESC[m (no params) means reset
        let line = parse_ansi_line("\x1b[31mred\x1b[mnormal");
        assert_eq!(line.segments.len(), 2);
        assert!(line.segments[0].fg.is_some());
        assert!(line.segments[1].fg.is_none());
    }

    #[test]
    fn test_parse_ansi_empty_line() {
        let line = parse_ansi_line("");
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].text, "");
    }

    #[test]
    fn test_color_256_to_rgb_grayscale() {
        let c = color_256_to_rgb(232); // first grayscale
        assert_eq!(c, [8, 8, 8]);
        let c = color_256_to_rgb(255); // last grayscale
        assert_eq!(c, [238, 238, 238]);
    }
}
