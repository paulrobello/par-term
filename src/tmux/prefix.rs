//! Tmux prefix key handling for control mode
//!
//! In normal tmux, the prefix key (default Ctrl+B) puts tmux into command mode.
//! In control mode, we need to intercept the prefix key and translate the
//! following command key into actual tmux commands.
//!
//! This module provides:
//! - Prefix key parsing from config strings like "C-b", "C-Space"
//! - State tracking for prefix mode
//! - Translation of prefix + key to tmux commands

use winit::keyboard::{Key, ModifiersState, NamedKey};

/// Parsed prefix key configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixKey {
    /// Whether Ctrl modifier is required
    pub ctrl: bool,
    /// Whether Alt/Option modifier is required
    pub alt: bool,
    /// Whether Shift modifier is required
    pub shift: bool,
    /// The base key (lowercase letter or special key name)
    pub key: PrefixKeyType,
}

/// The type of key in the prefix
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefixKeyType {
    /// A character key (a-z)
    Char(char),
    /// Space key
    Space,
    /// Other named key
    Named(String),
}

impl PrefixKey {
    /// Parse a prefix key string like "C-b", "C-Space", "M-a", "C-M-x"
    ///
    /// Format:
    /// - `C-` = Ctrl modifier
    /// - `M-` = Alt/Meta modifier
    /// - `S-` = Shift modifier
    /// - Followed by key name: single letter, or "Space", "Tab", etc.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        let mut remaining = s;

        // Parse modifiers
        loop {
            if remaining.starts_with("C-") {
                ctrl = true;
                remaining = &remaining[2..];
            } else if remaining.starts_with("M-") || remaining.starts_with("A-") {
                alt = true;
                remaining = &remaining[2..];
            } else if remaining.starts_with("S-") {
                shift = true;
                remaining = &remaining[2..];
            } else {
                break;
            }
        }

        // Parse the key
        let key = if remaining.eq_ignore_ascii_case("space") {
            PrefixKeyType::Space
        } else if remaining.len() == 1 {
            PrefixKeyType::Char(remaining.chars().next()?.to_ascii_lowercase())
        } else {
            PrefixKeyType::Named(remaining.to_string())
        };

        Some(Self {
            ctrl,
            alt,
            shift,
            key,
        })
    }

    /// Check if a key event matches this prefix key
    pub fn matches(&self, key: &Key, modifiers: ModifiersState) -> bool {
        // Check modifiers
        if self.ctrl != modifiers.control_key() {
            return false;
        }
        if self.alt != modifiers.alt_key() {
            return false;
        }
        if self.shift != modifiers.shift_key() {
            return false;
        }

        // Check key
        match (&self.key, key) {
            (PrefixKeyType::Space, Key::Named(NamedKey::Space)) => true,
            (PrefixKeyType::Char(c), Key::Character(s)) => {
                s.chars().next().map(|k| k.to_ascii_lowercase()) == Some(*c)
            }
            _ => false,
        }
    }
}

/// State for tracking prefix key mode
#[derive(Debug, Clone, Default)]
pub struct PrefixState {
    /// Whether we're currently in prefix mode (waiting for command key)
    in_prefix_mode: bool,
}

impl PrefixState {
    /// Create a new prefix state
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if we're in prefix mode
    pub fn is_active(&self) -> bool {
        self.in_prefix_mode
    }

    /// Enter prefix mode
    pub fn enter(&mut self) {
        self.in_prefix_mode = true;
    }

    /// Exit prefix mode
    pub fn exit(&mut self) {
        self.in_prefix_mode = false;
    }
}

/// Translate a command key (pressed after prefix) to a tmux command
///
/// Takes an optional focused pane ID to target pane-specific commands.
/// Returns the tmux command string to send to the gateway, or None if unknown.
pub fn translate_command_key(
    key: &Key,
    _modifiers: ModifiersState,
    focused_pane: Option<u64>,
) -> Option<String> {
    // Helper to format target option
    // In control mode, pane IDs use the %N format for targeting
    let target = |cmd: &str| -> String {
        match focused_pane {
            Some(pane_id) => format!("{} -t %{}\n", cmd, pane_id),
            None => format!("{}\n", cmd),
        }
    };

    match key {
        Key::Character(s) => {
            let c = s.chars().next()?;
            match c {
                // Window commands (no pane target needed)
                'c' => Some("new-window\n".to_string()),
                'n' => Some("next-window\n".to_string()),
                'p' => Some("previous-window\n".to_string()),
                '0'..='9' => Some(format!("select-window -t :{}\n", c)),

                // Pane commands (need pane target)
                '%' => Some(target("split-window -h")),
                '"' => Some(target("split-window -v")),
                'o' => Some("select-pane -t :.+\n".to_string()),
                ';' => Some("last-pane\n".to_string()),
                'x' => Some(target("kill-pane")),
                'z' => Some(target("resize-pane -Z")),
                '{' => Some(target("swap-pane -U")),
                '}' => Some(target("swap-pane -D")),

                // Session commands
                'd' => Some("detach-client\n".to_string()),
                's' => Some("choose-tree -s\n".to_string()),
                '$' => Some("command-prompt -I \"#S\" \"rename-session '%%'\"\n".to_string()),
                '(' => Some("switch-client -p\n".to_string()),
                ')' => Some("switch-client -n\n".to_string()),
                'L' => Some("switch-client -l\n".to_string()),

                // Copy mode (need pane target)
                '[' => Some(target("copy-mode")),
                ']' => Some(target("paste-buffer")),

                // Other
                ':' => Some("command-prompt\n".to_string()),
                '?' => Some("list-keys\n".to_string()),
                't' => Some(target("clock-mode")),
                '!' => Some(target("break-pane")),
                '&' => Some("kill-window\n".to_string()),
                ',' => Some("command-prompt -I \"#W\" \"rename-window '%%'\"\n".to_string()),
                '.' => Some("command-prompt \"move-window -t '%%'\"\n".to_string()),
                'w' => Some("choose-tree -w\n".to_string()),
                'l' => Some("last-window\n".to_string()),
                'q' => Some("display-panes\n".to_string()),
                'i' => Some("display-message\n".to_string()),
                'f' => Some("command-prompt \"find-window '%%'\"\n".to_string()),

                // Arrow key navigation via hjkl (need pane target)
                'h' => Some(target("select-pane -L")),
                'j' => Some(target("select-pane -D")),
                'k' => Some(target("select-pane -U")),

                // Space cycles layouts
                ' ' => Some("next-layout\n".to_string()),

                _ => None,
            }
        }
        Key::Named(NamedKey::Space) => Some("next-layout\n".to_string()),
        Key::Named(NamedKey::ArrowUp) => Some(target("select-pane -U")),
        Key::Named(NamedKey::ArrowDown) => Some(target("select-pane -D")),
        Key::Named(NamedKey::ArrowLeft) => Some(target("select-pane -L")),
        Key::Named(NamedKey::ArrowRight) => Some(target("select-pane -R")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ctrl_b() {
        let pk = PrefixKey::parse("C-b").unwrap();
        assert!(pk.ctrl);
        assert!(!pk.alt);
        assert!(!pk.shift);
        assert_eq!(pk.key, PrefixKeyType::Char('b'));
    }

    #[test]
    fn test_parse_ctrl_space() {
        let pk = PrefixKey::parse("C-Space").unwrap();
        assert!(pk.ctrl);
        assert!(!pk.alt);
        assert!(!pk.shift);
        assert_eq!(pk.key, PrefixKeyType::Space);
    }

    #[test]
    fn test_parse_ctrl_a() {
        let pk = PrefixKey::parse("C-a").unwrap();
        assert!(pk.ctrl);
        assert_eq!(pk.key, PrefixKeyType::Char('a'));
    }

    #[test]
    fn test_parse_meta_a() {
        let pk = PrefixKey::parse("M-a").unwrap();
        assert!(!pk.ctrl);
        assert!(pk.alt);
        assert_eq!(pk.key, PrefixKeyType::Char('a'));
    }

    #[test]
    fn test_parse_ctrl_meta_x() {
        let pk = PrefixKey::parse("C-M-x").unwrap();
        assert!(pk.ctrl);
        assert!(pk.alt);
        assert_eq!(pk.key, PrefixKeyType::Char('x'));
    }

    #[test]
    fn test_translate_new_window() {
        let key = Key::Character("c".into());
        let cmd = translate_command_key(&key, ModifiersState::empty(), None);
        assert_eq!(cmd, Some("new-window\n".to_string()));
    }

    #[test]
    fn test_translate_split_horizontal_no_target() {
        // % creates horizontal split (side-by-side panes) using -h flag
        let key = Key::Character("%".into());
        let cmd = translate_command_key(&key, ModifiersState::empty(), None);
        assert_eq!(cmd, Some("split-window -h\n".to_string()));
    }

    #[test]
    fn test_translate_split_horizontal_with_target() {
        // % creates horizontal split (side-by-side panes) using -h flag
        let key = Key::Character("%".into());
        let cmd = translate_command_key(&key, ModifiersState::empty(), Some(42));
        assert_eq!(cmd, Some("split-window -h -t %42\n".to_string()));
    }

    #[test]
    fn test_translate_split_vertical_with_target() {
        // " creates vertical split (stacked panes) using -v flag
        let key = Key::Character("\"".into());
        let cmd = translate_command_key(&key, ModifiersState::empty(), Some(11));
        assert_eq!(cmd, Some("split-window -v -t %11\n".to_string()));
    }

    #[test]
    fn test_translate_detach() {
        let key = Key::Character("d".into());
        let cmd = translate_command_key(&key, ModifiersState::empty(), None);
        assert_eq!(cmd, Some("detach-client\n".to_string()));
    }
}
