//! Semantic history and link-highlighting settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::{LinkUnderlineStyle, SemanticHistoryEditorMode};
use serde::{Deserialize, Serialize};

/// File-path/URL detection, the editor that opens them, and link highlighting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticHistoryConfig {
    /// Enable semantic history (file path detection and opening)
    /// When enabled, Cmd/Ctrl+Click on detected file paths opens them in the editor.
    #[serde(default = "crate::defaults::bool_true")]
    pub semantic_history_enabled: bool,

    /// Editor selection mode for semantic history
    ///
    /// - `custom` - Use the editor command specified in `semantic_history_editor`
    /// - `environment_variable` - Use `$EDITOR` or `$VISUAL` environment variable (default)
    /// - `system_default` - Use system default application for each file type
    #[serde(default)]
    pub semantic_history_editor_mode: SemanticHistoryEditorMode,

    /// Editor command for semantic history (when mode is `custom`).
    ///
    /// Placeholders: `{file}` = file path, `{line}` = line number (if available)
    ///
    /// Examples:
    /// - `code -g {file}:{line}` (VS Code with line number)
    /// - `subl {file}:{line}` (Sublime Text)
    /// - `vim +{line} {file}` (Vim)
    /// - `emacs +{line} {file}` (Emacs)
    #[serde(default = "crate::defaults::semantic_history_editor")]
    pub semantic_history_editor: String,

    /// Color for highlighted links (URLs and file paths) [R, G, B] (0-255)
    #[serde(default = "crate::defaults::link_highlight_color")]
    pub link_highlight_color: [u8; 3],

    /// Apply color to highlighted links (URLs and file paths)
    #[serde(default = "crate::defaults::bool_true")]
    pub link_highlight_color_enabled: bool,

    /// Underline highlighted links (URLs and file paths)
    #[serde(default = "crate::defaults::bool_true")]
    pub link_highlight_underline: bool,

    /// Style for link highlight underlines (solid or stipple)
    #[serde(default)]
    pub link_underline_style: LinkUnderlineStyle,

    /// Custom command to open URLs. When set, used instead of system default browser.
    ///
    /// Use `{url}` as placeholder for the URL.
    ///
    /// Examples:
    /// - `firefox {url}` (open in Firefox)
    /// - `open -a Safari {url}` (macOS: open in Safari)
    /// - `chromium-browser {url}` (Linux: open in Chromium)
    ///
    /// When empty or unset, uses the system default browser.
    #[serde(default)]
    pub link_handler_command: String,

    /// Allow Cmd+Click (macOS) / Ctrl+Click to open `file://` OSC 8 hyperlinks
    /// and file URLs via the OS default handler (browser for `.html`, Finder for
    /// directories, etc.).
    ///
    /// Disabled by default for security (SEC-009): a remote program can emit an
    /// OSC 8 hyperlink with a `file://` scheme to make the OS handler open an
    /// arbitrary local path. Enable only if you trust your terminal sessions.
    #[serde(default = "crate::defaults::bool_false")]
    pub allow_file_scheme_urls: bool,
}

impl Default for SemanticHistoryConfig {
    fn default() -> Self {
        Self {
            semantic_history_enabled: crate::defaults::bool_true(),
            semantic_history_editor_mode: SemanticHistoryEditorMode::default(),
            semantic_history_editor: crate::defaults::semantic_history_editor(),
            link_highlight_color: crate::defaults::link_highlight_color(),
            link_highlight_color_enabled: crate::defaults::bool_true(),
            link_highlight_underline: crate::defaults::bool_true(),
            link_underline_style: LinkUnderlineStyle::default(),
            link_handler_command: String::new(),
            allow_file_scheme_urls: crate::defaults::bool_false(),
        }
    }
}
