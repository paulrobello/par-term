//! KeySequence action handler and prefix-action character extraction.

use crate::app::window_state::WindowState;
use winit::event::KeyEvent;
use winit::keyboard::{Key, KeyCode, PhysicalKey};

impl WindowState {
    /// Execute a KeySequence custom action.
    ///
    /// Parses the key sequence string, converts to byte sequences, and writes them
    /// to the active terminal.
    pub(crate) fn execute_key_sequence_action(&mut self, keys: String, title: String) -> bool {
        use crate::keybindings::parse_key_sequence;

        let byte_sequences = match parse_key_sequence(&keys) {
            Ok(seqs) => seqs,
            Err(e) => {
                log::error!("Invalid key sequence '{}': {}", keys, e);
                self.show_toast(format!("Invalid key sequence: {}", e));
                return false;
            }
        };

        // Write all key sequences to the terminal
        let write_error = if let Some(tab) = self.tab_manager.active_tab_mut() {
            // try_lock: intentional -- send_keys action in sync event loop.
            // On miss: the key sequences are not written. User can retry.
            if let Ok(terminal) = tab.terminal.try_write() {
                let mut err: Option<String> = None;
                for bytes in &byte_sequences {
                    if let Err(e) = terminal.write(bytes) {
                        err = Some(format!("{}", e));
                        break;
                    }
                }
                err
            } else {
                log::error!("Failed to lock terminal for key sequence execution");
                return false;
            }
        } else {
            return false;
        };

        if let Some(e) = write_error {
            log::error!("Failed to write key sequence: {}", e);
            self.show_toast(format!("Key sequence error: {}", e));
            return false;
        }

        log::info!(
            "Executed key sequence action '{}' ({} keys)",
            title,
            byte_sequences.len()
        );
        true
    }
}

/// Extract a character suitable for prefix-action matching from a key event.
///
/// Prefers `event.text`, falls back to `logical_key`, then to `physical_key`
/// mapping. Filters out whitespace-only results.
pub(crate) fn extract_prefix_action_char(event: &KeyEvent) -> Option<char> {
    event
        .text
        .as_ref()
        .and_then(|text| text.chars().next())
        .filter(|ch| !ch.is_whitespace())
        .or_else(|| match &event.logical_key {
            Key::Character(text) => text.chars().next().filter(|ch| !ch.is_whitespace()),
            _ => None,
        })
        .or(match event.physical_key {
            PhysicalKey::Code(code) => match code {
                KeyCode::KeyA => Some('a'),
                KeyCode::KeyB => Some('b'),
                KeyCode::KeyC => Some('c'),
                KeyCode::KeyD => Some('d'),
                KeyCode::KeyE => Some('e'),
                KeyCode::KeyF => Some('f'),
                KeyCode::KeyG => Some('g'),
                KeyCode::KeyH => Some('h'),
                KeyCode::KeyI => Some('i'),
                KeyCode::KeyJ => Some('j'),
                KeyCode::KeyK => Some('k'),
                KeyCode::KeyL => Some('l'),
                KeyCode::KeyM => Some('m'),
                KeyCode::KeyN => Some('n'),
                KeyCode::KeyO => Some('o'),
                KeyCode::KeyP => Some('p'),
                KeyCode::KeyQ => Some('q'),
                KeyCode::KeyR => Some('r'),
                KeyCode::KeyS => Some('s'),
                KeyCode::KeyT => Some('t'),
                KeyCode::KeyU => Some('u'),
                KeyCode::KeyV => Some('v'),
                KeyCode::KeyW => Some('w'),
                KeyCode::KeyX => Some('x'),
                KeyCode::KeyY => Some('y'),
                KeyCode::KeyZ => Some('z'),
                KeyCode::Digit0 => Some('0'),
                KeyCode::Digit1 => Some('1'),
                KeyCode::Digit2 => Some('2'),
                KeyCode::Digit3 => Some('3'),
                KeyCode::Digit4 => Some('4'),
                KeyCode::Digit5 => Some('5'),
                KeyCode::Digit6 => Some('6'),
                KeyCode::Digit7 => Some('7'),
                KeyCode::Digit8 => Some('8'),
                KeyCode::Digit9 => Some('9'),
                KeyCode::Minus => Some('-'),
                KeyCode::Equal => Some('='),
                KeyCode::BracketLeft => Some('['),
                KeyCode::BracketRight => Some(']'),
                KeyCode::Backslash => Some('\\'),
                KeyCode::Semicolon => Some(';'),
                KeyCode::Quote => Some('\''),
                KeyCode::Backquote => Some('`'),
                KeyCode::Comma => Some(','),
                KeyCode::Period => Some('.'),
                KeyCode::Slash => Some('/'),
                KeyCode::Space => Some(' '),
                _ => None,
            },
            _ => None,
        })
        .filter(|ch| !ch.is_whitespace())
}
