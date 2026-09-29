//! VT byte sequence encoding for keyboard events.
//!
//! The encoder itself lives in `par-term-emu-core-rust`
//! ([`keyboard::encode_key_with`](par_term_emu_core_rust::keyboard::encode_key_with),
//! ENH-028): one implementation shared by every par-term frontend. This module
//! maps a winit key event onto the core's [`TermKeyEvent`] and hands it the
//! terminal modes the caller negotiated (modifyOtherKeys level, DECCKM) plus
//! the per-side Option-key modes in [`InputHandler::key_options`].
//!
//! # Mapping rules
//!
//! - **Codepoint.** The physical key's base character (`'1'` for the `Digit1`
//!   key) when Ctrl/Alt drive the encoding — the modifyOtherKeys 27-form and
//!   the Meta/Esc Option modes. The logical (OS-composed) character otherwise:
//!   plain typing, Normal Option mode, and Ctrl control codes (`Ctrl+Shift+2`
//!   is `'@'` → NUL, so Ctrl reads the layout-resolved character).
//! - **Ctrl over a character with no control form** (`Ctrl+ł`, `Ctrl+!`)
//!   drops CTRL from the event: par-term sends the character itself, with the
//!   Option mode applied when Alt is held, never a masked control byte.
//! - **Alt side.** `ALT_RIGHT` is set only when the right Alt alone is held;
//!   both held reports the left key (left wins the tie).
//! - **modifyOtherKeys** applies only when the physical key has a base
//!   character. Without one the core is told the mode is 0, so the key falls
//!   back to its ordinary encoding rather than reporting a guessed codepoint.
//!
//! # Deliberate changes from the pre-ENH-028 encoder
//!
//! - Home/End under DECCKM send SS3 `ESC O H` / `ESC O F`, the form the
//!   `xterm-256color` terminfo par-term advertises lists as `khome`/`kend`
//!   (paired with `smkx` = DECCKM), and what xterm and iTerm2 send.
//! - Alt+Space (and Ctrl+Alt+Space) run the Option-key transform like every
//!   other Alt+key, as iTerm2 does: `ESC SP` in Esc mode, `0xA0` in Meta mode,
//!   a plain space in Normal mode.

use par_term_emu_core_rust::keyboard::{
    TermKey, TermKeyEvent, encode_key_with, modifiers as term_mods, option_modes,
};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

use super::{InputHandler, KeyInput};

impl InputHandler {
    /// Convert a keyboard event to terminal input bytes
    ///
    /// If `modify_other_keys_mode` is > 0, keys with modifiers will be reported
    /// using the XTerm modifyOtherKeys format: CSI 27 ; modifier ; keycode ~
    pub fn handle_key_event(&mut self, event: KeyEvent) -> Option<Vec<u8>> {
        self.handle_key_event_with_mode(event, 0, false)
    }

    /// Convert a keyboard event to terminal input bytes with modifyOtherKeys support
    ///
    /// `modify_other_keys_mode`:
    /// - 0: Disabled (normal key handling)
    /// - 1: Report modifiers for special keys only
    /// - 2: Report modifiers for all keys
    ///
    /// `application_cursor`: When true (DECCKM mode enabled), unmodified arrow
    /// and Home/End keys send SS3 sequences (ESC O A) instead of CSI (ESC [ A).
    pub fn handle_key_event_with_mode(
        &mut self,
        event: KeyEvent,
        modify_other_keys_mode: u8,
        application_cursor: bool,
    ) -> Option<Vec<u8>> {
        self.handle_key_input_with_mode(
            &KeyInput::from(event),
            modify_other_keys_mode,
            application_cursor,
        )
    }

    /// Convert a [`KeyInput`] to terminal input bytes with modifyOtherKeys support.
    ///
    /// This is the implementation behind the [`KeyEvent`] entry points above, which
    /// exist only to convert. Prefer this when you do not already hold a winit
    /// event: `KeyEvent` cannot be constructed outside winit without undefined
    /// behaviour, so callers that would otherwise forge one should use this.
    ///
    /// Encoding is delegated to the core's shared encoder; see the module docs
    /// for how the winit event is mapped onto it.
    pub fn handle_key_input_with_mode(
        &mut self,
        input: &KeyInput,
        modify_other_keys_mode: u8,
        application_cursor: bool,
    ) -> Option<Vec<u8>> {
        if input.state != ElementState::Pressed {
            return None;
        }

        let mods = self.term_modifiers();
        let base = base_character(input.physical_key);

        // modifyOtherKeys reports Ctrl/Alt chords as CSI 27;mods;base~ for any
        // logical key whose physical key has a base character (Shift-only is
        // exempt: the OS-shifted glyph cannot be recovered from the base).
        if modify_other_keys_mode > 0
            && mods & (term_mods::CTRL | term_mods::ALT) != 0
            && let Some(base) = base
        {
            return self.encode(
                &[TermKeyEvent::char_(base, mods)],
                modify_other_keys_mode,
                application_cursor,
            );
        }

        let events = match &input.logical_key {
            Key::Named(named) => vec![named_key_event(*named, mods)?],
            Key::Character(text) => self.character_events(text, base, mods)?,
            _ => return None,
        };
        self.encode(&events, 0, application_cursor)
    }

    /// Core events for a `Key::Character` press at modifyOtherKeys level 0.
    fn character_events(
        &self,
        text: &str,
        base: Option<char>,
        mods: u8,
    ) -> Option<Vec<TermKeyEvent>> {
        let logical = text.chars().next()?;

        if mods & term_mods::CTRL != 0 {
            if has_control_form(logical) {
                return Some(vec![TermKeyEvent::char_(logical, mods)]);
            }
            // No control form: Ctrl is ignored and the character goes out as
            // typed (Option transform still applies below).
            return self.character_events(text, base, mods & !term_mods::CTRL);
        }

        if mods & term_mods::ALT != 0 && self.active_option_mode(mods) != option_modes::NORMAL {
            // Meta/Esc transform the unmodified character, not the composed one.
            let mut events = vec![TermKeyEvent::char_(base.unwrap_or(logical), mods)];
            if base.is_none() && !logical.is_ascii() {
                // The non-ASCII fallback is an ESC prefix, so the rest of a
                // multi-scalar string follows it intact.
                events.extend(text.chars().skip(1).map(|c| TermKeyEvent::char_(c, 0)));
            }
            return Some(events);
        }

        // Plain text (or Normal Option mode): every scalar passes through, so
        // an IME string or ZWJ sequence reaches the PTY whole.
        Some(text.chars().map(|c| TermKeyEvent::char_(c, mods)).collect())
    }

    /// Run `events` through the core encoder under the given terminal modes.
    fn encode(
        &mut self,
        events: &[TermKeyEvent],
        modify_other_keys_mode: u8,
        application_cursor: bool,
    ) -> Option<Vec<u8>> {
        self.encoder_modes
            .sync(modify_other_keys_mode, application_cursor);
        let mut out = Vec::new();
        for ev in events {
            out.extend(encode_key_with(
                ev,
                self.encoder_modes.terminal(),
                &self.key_options,
            ));
        }
        (!out.is_empty()).then_some(out)
    }
}

/// True when Ctrl+`c` has a legacy control byte: ASCII letters and the
/// 0x40..=0x5F punctuation (`@ [ \ ] ^ _`). The ASCII guard keeps a non-ASCII
/// scalar whose low byte lands in that range (Ctrl+ŕ, U+0155) from being
/// masked into a control code.
fn has_control_form(c: char) -> bool {
    c.is_ascii_alphabetic() || (c.is_ascii() && (0x40..=0x5F).contains(&(c as u8)))
}

/// Core event for a named key, or `None` for keys with no terminal encoding.
fn named_key_event(named: NamedKey, mods: u8) -> Option<TermKeyEvent> {
    let key = match named {
        NamedKey::Space => return Some(TermKeyEvent::char_(' ', mods)),
        NamedKey::ArrowUp => TermKey::Up,
        NamedKey::ArrowDown => TermKey::Down,
        NamedKey::ArrowRight => TermKey::Right,
        NamedKey::ArrowLeft => TermKey::Left,
        NamedKey::Home => TermKey::Home,
        NamedKey::End => TermKey::End,
        NamedKey::Insert => TermKey::Insert,
        NamedKey::Delete => TermKey::Delete,
        NamedKey::PageUp => TermKey::PageUp,
        NamedKey::PageDown => TermKey::PageDown,
        NamedKey::F1 => TermKey::F1,
        NamedKey::F2 => TermKey::F2,
        NamedKey::F3 => TermKey::F3,
        NamedKey::F4 => TermKey::F4,
        NamedKey::F5 => TermKey::F5,
        NamedKey::F6 => TermKey::F6,
        NamedKey::F7 => TermKey::F7,
        NamedKey::F8 => TermKey::F8,
        NamedKey::F9 => TermKey::F9,
        NamedKey::F10 => TermKey::F10,
        NamedKey::F11 => TermKey::F11,
        NamedKey::F12 => TermKey::F12,
        NamedKey::Enter => TermKey::Enter,
        NamedKey::Tab => TermKey::Tab,
        NamedKey::Backspace => TermKey::Backspace,
        NamedKey::Escape => TermKey::Escape,
        _ => return None,
    };
    Some(TermKeyEvent::functional(key, mods))
}

/// The unmodified character a physical key produces on a US layout.
///
/// This is needed because on macOS, Option+key produces a different logical
/// character, and modifyOtherKeys must report the base codepoint.
fn base_character(physical_key: PhysicalKey) -> Option<char> {
    let PhysicalKey::Code(code) = physical_key else {
        return None;
    };
    Some(match code {
        KeyCode::KeyA => 'a',
        KeyCode::KeyB => 'b',
        KeyCode::KeyC => 'c',
        KeyCode::KeyD => 'd',
        KeyCode::KeyE => 'e',
        KeyCode::KeyF => 'f',
        KeyCode::KeyG => 'g',
        KeyCode::KeyH => 'h',
        KeyCode::KeyI => 'i',
        KeyCode::KeyJ => 'j',
        KeyCode::KeyK => 'k',
        KeyCode::KeyL => 'l',
        KeyCode::KeyM => 'm',
        KeyCode::KeyN => 'n',
        KeyCode::KeyO => 'o',
        KeyCode::KeyP => 'p',
        KeyCode::KeyQ => 'q',
        KeyCode::KeyR => 'r',
        KeyCode::KeyS => 's',
        KeyCode::KeyT => 't',
        KeyCode::KeyU => 'u',
        KeyCode::KeyV => 'v',
        KeyCode::KeyW => 'w',
        KeyCode::KeyX => 'x',
        KeyCode::KeyY => 'y',
        KeyCode::KeyZ => 'z',
        KeyCode::Digit0 => '0',
        KeyCode::Digit1 => '1',
        KeyCode::Digit2 => '2',
        KeyCode::Digit3 => '3',
        KeyCode::Digit4 => '4',
        KeyCode::Digit5 => '5',
        KeyCode::Digit6 => '6',
        KeyCode::Digit7 => '7',
        KeyCode::Digit8 => '8',
        KeyCode::Digit9 => '9',
        KeyCode::Minus => '-',
        KeyCode::Equal => '=',
        KeyCode::BracketLeft => '[',
        KeyCode::BracketRight => ']',
        KeyCode::Backslash => '\\',
        KeyCode::Semicolon => ';',
        KeyCode::Quote => '\'',
        KeyCode::Backquote => '`',
        KeyCode::Comma => ',',
        KeyCode::Period => '.',
        KeyCode::Slash => '/',
        KeyCode::Space => ' ',
        _ => return None,
    })
}
