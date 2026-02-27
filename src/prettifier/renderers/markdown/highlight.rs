//! Keyword-based syntax highlighting for fenced code blocks.
//!
//! Provides [`LanguageDef`], [`get_language_def`], and [`highlight_code_line`].
//! The highlighter is intentionally simple — it does not build a full parse
//! tree but uses fast tokenisation with string matching against fixed keyword
//! and builtin lists for common languages.

use crate::prettifier::traits::ThemeColors;
use crate::prettifier::types::{StyledLine, StyledSegment};

// ---------------------------------------------------------------------------
// Language definition
// ---------------------------------------------------------------------------

/// Language definition for keyword-based syntax highlighting.
pub(super) struct LanguageDef {
    pub(super) keywords: &'static [&'static str],
    pub(super) comment_prefix: &'static str,
    pub(super) builtins: &'static [&'static str],
}

/// Return the [`LanguageDef`] for a given language tag, or `None` if the
/// language is not recognised.
pub(super) fn get_language_def(language: &str) -> Option<LanguageDef> {
    match language.to_lowercase().as_str() {
        "rust" | "rs" => Some(LanguageDef {
            keywords: &[
                "fn", "let", "mut", "const", "static", "if", "else", "match", "for", "while",
                "loop", "return", "break", "continue", "struct", "enum", "impl", "trait", "pub",
                "use", "mod", "crate", "self", "super", "where", "async", "await", "move",
                "unsafe", "type", "as", "in", "ref", "true", "false",
            ],
            comment_prefix: "//",
            builtins: &[
                "Self", "Option", "Result", "Vec", "String", "Box", "Rc", "Arc", "Some", "None",
                "Ok", "Err",
            ],
        }),
        "python" | "py" => Some(LanguageDef {
            keywords: &[
                "def", "class", "if", "elif", "else", "for", "while", "return", "import", "from",
                "as", "try", "except", "finally", "with", "yield", "lambda", "pass", "break",
                "continue", "raise", "and", "or", "not", "in", "is", "True", "False", "None",
                "async", "await",
            ],
            comment_prefix: "#",
            builtins: &[
                "print",
                "len",
                "range",
                "int",
                "str",
                "float",
                "list",
                "dict",
                "set",
                "tuple",
                "bool",
                "type",
                "isinstance",
                "self",
            ],
        }),
        "javascript" | "js" | "typescript" | "ts" => Some(LanguageDef {
            keywords: &[
                "function",
                "const",
                "let",
                "var",
                "if",
                "else",
                "for",
                "while",
                "return",
                "class",
                "new",
                "this",
                "import",
                "export",
                "from",
                "default",
                "try",
                "catch",
                "finally",
                "throw",
                "async",
                "await",
                "yield",
                "switch",
                "case",
                "break",
                "continue",
                "typeof",
                "instanceof",
                "true",
                "false",
                "null",
                "undefined",
            ],
            comment_prefix: "//",
            builtins: &[
                "console", "Promise", "Array", "Object", "Map", "Set", "JSON", "Math", "String",
                "Number", "Boolean", "Error",
            ],
        }),
        "json" => Some(LanguageDef {
            keywords: &["true", "false", "null"],
            comment_prefix: "",
            builtins: &[],
        }),
        "yaml" | "yml" => Some(LanguageDef {
            keywords: &["true", "false", "null", "yes", "no"],
            comment_prefix: "#",
            builtins: &[],
        }),
        "shell" | "sh" | "bash" | "zsh" => Some(LanguageDef {
            keywords: &[
                "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
                "function", "return", "exit", "export", "local", "readonly", "in", "select",
                "until", "true", "false",
            ],
            comment_prefix: "#",
            builtins: &[
                "echo", "cd", "ls", "cat", "grep", "sed", "awk", "find", "sort", "uniq", "wc",
                "head", "tail", "mkdir", "rm", "cp", "mv", "chmod", "chown", "curl", "wget",
            ],
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Line highlighting
// ---------------------------------------------------------------------------

/// Highlight a single code line using simple keyword matching.
///
/// Returns styled segments with colors for keywords, strings, comments,
/// numbers, and builtins.
pub(super) fn highlight_code_line(
    line: &str,
    lang_def: Option<&LanguageDef>,
    theme: &ThemeColors,
    show_bg: bool,
) -> StyledLine {
    let code_bg = if show_bg {
        Some(subtle_bg(theme))
    } else {
        None
    };

    let Some(def) = lang_def else {
        // No language definition — plain text with optional background.
        return StyledLine::new(vec![StyledSegment {
            text: line.to_string(),
            bg: code_bg,
            ..Default::default()
        }]);
    };

    // Check for full-line comment.
    if !def.comment_prefix.is_empty() && line.trim_start().starts_with(def.comment_prefix) {
        return StyledLine::new(vec![StyledSegment {
            text: line.to_string(),
            fg: Some(theme.palette[8]), // dim grey
            bg: code_bg,
            italic: true,
            ..Default::default()
        }]);
    }

    // Tokenize the line into segments.
    let mut segments = Vec::new();
    let mut chars = line.char_indices().peekable();

    while let Some(&(byte_pos, ch)) = chars.peek() {
        // String literal.
        if ch == '"' || ch == '\'' {
            let quote = ch;
            let start = byte_pos;
            chars.next(); // consume opening quote
            let mut escaped = false;
            while let Some(&(_, c)) = chars.peek() {
                chars.next();
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == quote {
                    break;
                }
            }
            let end = chars.peek().map(|&(i, _)| i).unwrap_or(line.len());
            segments.push(StyledSegment {
                text: line[start..end].to_string(),
                fg: Some(theme.palette[10]), // bright green
                bg: code_bg,
                ..Default::default()
            });
            continue;
        }

        // Inline comment.
        if !def.comment_prefix.is_empty() && line[byte_pos..].starts_with(def.comment_prefix) {
            segments.push(StyledSegment {
                text: line[byte_pos..].to_string(),
                fg: Some(theme.palette[8]),
                bg: code_bg,
                italic: true,
                ..Default::default()
            });
            break;
        }

        // Word (identifier or keyword).
        if ch.is_alphanumeric() || ch == '_' {
            let start = byte_pos;
            while let Some(&(_, c)) = chars.peek() {
                if c.is_alphanumeric() || c == '_' {
                    chars.next();
                } else {
                    break;
                }
            }
            let end = chars.peek().map(|&(i, _)| i).unwrap_or(line.len());
            let word = &line[start..end];

            let fg = if def.keywords.contains(&word) {
                Some(theme.palette[13]) // bright magenta
            } else if def.builtins.contains(&word) {
                Some(theme.palette[14]) // bright cyan
            } else if word
                .chars()
                .all(|c| c.is_ascii_digit() || c == '_' || c == '.')
            {
                Some(theme.palette[11]) // bright yellow
            } else {
                None
            };

            segments.push(StyledSegment {
                text: word.to_string(),
                fg,
                bg: code_bg,
                ..Default::default()
            });
            continue;
        }

        // Number (starting with digit).
        if ch.is_ascii_digit() {
            let start = byte_pos;
            while let Some(&(_, c)) = chars.peek() {
                if c.is_ascii_digit() || c == '.' || c == 'x' || c == 'o' || c == 'b' || c == '_' {
                    chars.next();
                } else {
                    break;
                }
            }
            let end = chars.peek().map(|&(i, _)| i).unwrap_or(line.len());
            segments.push(StyledSegment {
                text: line[start..end].to_string(),
                fg: Some(theme.palette[11]), // bright yellow
                bg: code_bg,
                ..Default::default()
            });
            continue;
        }

        // Other character (punctuation, whitespace, etc.).
        let start = byte_pos;
        chars.next();
        let end = chars.peek().map(|&(i, _)| i).unwrap_or(line.len());
        segments.push(StyledSegment {
            text: line[start..end].to_string(),
            bg: code_bg,
            ..Default::default()
        });
    }

    if segments.is_empty() {
        // Empty line within code block.
        segments.push(StyledSegment {
            text: String::new(),
            bg: code_bg,
            ..Default::default()
        });
    }

    StyledLine::new(segments)
}

// ---------------------------------------------------------------------------
// Background helper (shared with mod.rs via re-export)
// ---------------------------------------------------------------------------

/// Compute a subtle background highlight for inline code / code blocks.
pub(super) fn subtle_bg(theme: &ThemeColors) -> [u8; 3] {
    [
        theme.bg[0].saturating_add(25),
        theme.bg[1].saturating_add(25),
        theme.bg[2].saturating_add(25),
    ]
}
