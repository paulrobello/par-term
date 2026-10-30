//! Exhaustive key-matrix tests for copy-mode dispatch.
//!
//! Every key in [`all_keys`] is driven through
//! `WindowState::handle_copy_mode_logical_key` from several starting states,
//! with and without Ctrl, and the resulting copy-mode state is recorded. The
//! transcript must match the frozen reference in `handler_golden.txt`.
//! Regenerate only for an intended behavior change:
//! `PAR_TERM_BLESS_COPY_MODE=1 cargo test copy_mode_key_matrix`.

use crate::app::window_state::WindowState;
use crate::copy_mode::{PendingOperator, VisualMode};
use std::fmt::Write as _;
use winit::keyboard::{Key, ModifiersState, NamedKey};

const GOLDEN: &str = include_str!("handler_golden.txt");

fn window() -> WindowState {
    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime"),
    );
    WindowState::new(crate::config::Config::default(), runtime)
}

/// Every printable ASCII character plus the named keys copy mode could
/// plausibly see. Unmapped keys must leave the state untouched.
fn all_keys() -> Vec<Key> {
    let mut keys: Vec<Key> = (0x20u8..0x7f)
        .map(|b| Key::Character((b as char).to_string().into()))
        .collect();
    keys.extend(
        [
            NamedKey::Escape,
            NamedKey::Enter,
            NamedKey::Tab,
            NamedKey::Backspace,
            NamedKey::Delete,
            NamedKey::Insert,
            NamedKey::ArrowLeft,
            NamedKey::ArrowRight,
            NamedKey::ArrowUp,
            NamedKey::ArrowDown,
            NamedKey::Home,
            NamedKey::End,
            NamedKey::PageUp,
            NamedKey::PageDown,
            NamedKey::F1,
            NamedKey::Space,
        ]
        .map(Key::Named),
    );
    keys
}

/// Starting states: fresh, mid-buffer with a count, visual line with a
/// pending yank, and visual char with a pending yank and a count.
fn prime(ws: &mut WindowState, start: usize) {
    ws.copy_mode.enter(10, 5, 80, 24, 100);
    match start {
        0 => {}
        1 => {
            ws.copy_mode.cursor_absolute_line = 60;
            ws.copy_mode.cursor_col = 40;
            ws.copy_mode.count = Some(3);
        }
        2 => {
            ws.copy_mode.toggle_visual_line();
            ws.copy_mode.pending_operator = Some(PendingOperator::Yank);
        }
        3 => {
            ws.copy_mode.toggle_visual_char();
            ws.copy_mode.pending_operator = Some(PendingOperator::Yank);
            ws.copy_mode.count = Some(12);
        }
        _ => unreachable!(),
    }
}

fn project(ws: &WindowState) -> String {
    let c = &ws.copy_mode;
    let mut marks: Vec<_> = c
        .marks
        .iter()
        .map(|(k, m)| (*k, m.col, m.absolute_line))
        .collect();
    marks.sort();
    format!(
        "active={} cur=({},{}) visual={:?} anchor={:?} count={:?} op={:?} \
         searching={} dir={:?} query={:?} g={} mset={} mgoto={} marks={:?}",
        c.active,
        c.cursor_absolute_line,
        c.cursor_col,
        c.visual_mode,
        c.selection_anchor,
        c.count,
        c.pending_operator,
        c.is_searching,
        c.search_direction,
        c.search_query,
        c.pending_g,
        c.pending_mark_set,
        c.pending_mark_goto,
        marks,
    )
}

fn transcript() -> String {
    let mut ws = window();
    let mut out = String::new();
    for ctrl in [false, true] {
        ws.input_handler.modifiers = if ctrl {
            ModifiersState::CONTROL.into()
        } else {
            ModifiersState::empty().into()
        };
        for start in 0..4 {
            for key in all_keys() {
                prime(&mut ws, start);
                ws.handle_copy_mode_logical_key(&key);
                let _ = writeln!(
                    out,
                    "ctrl={ctrl} start={start} key={key:?} -> {}",
                    project(&ws)
                );
            }
        }
    }
    // Two-key sequences that route through the pending prefixes.
    ws.input_handler.modifiers = ModifiersState::empty().into();
    for (first, second) in [
        ("g", "g"),
        ("g", "x"),
        ("m", "a"),
        ("m", "A"),
        ("'", "a"),
        ("'", "z"),
    ] {
        prime(&mut ws, 1);
        ws.handle_copy_mode_logical_key(&Key::Character(first.into()));
        ws.handle_copy_mode_logical_key(&Key::Character(second.into()));
        let _ = writeln!(out, "seq {first}{second} -> {}", project(&ws));
    }
    out
}

#[test]
fn copy_mode_key_matrix() {
    let actual = transcript();
    if std::env::var_os("PAR_TERM_BLESS_COPY_MODE").is_some() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/app/copy_mode/handler_golden.txt"
        );
        std::fs::write(path, &actual).expect("write golden");
        return;
    }
    for (i, (want, got)) in GOLDEN.lines().zip(actual.lines()).enumerate() {
        assert_eq!(got, want, "copy-mode transcript diverges at line {}", i + 1);
    }
    assert_eq!(
        actual.lines().count(),
        GOLDEN.lines().count(),
        "transcript length changed"
    );
}

#[test]
fn copy_mode_unmapped_keys_are_inert() {
    let mut ws = window();
    for key in [
        Key::Character("x".into()),
        Key::Character("z".into()),
        Key::Named(NamedKey::Enter),
        Key::Named(NamedKey::Tab),
        Key::Named(NamedKey::F1),
    ] {
        prime(&mut ws, 1);
        let before = project(&ws);
        ws.handle_copy_mode_logical_key(&key);
        assert_eq!(project(&ws), before, "{key:?} must be ignored");
    }
}

/// Text motions are no-ops in the matrix (no terminal line to read), so pin
/// which motion each key binds by running it over a known line.
#[test]
fn copy_mode_text_motion_bindings() {
    use super::handler::{CopyModeCommand, copy_mode_key_command};
    use crate::copy_mode::CopyModeState;

    let line = "  foo.bar baz-qux end";
    let run = |key: &str| -> usize {
        let mut state = CopyModeState::new();
        state.enter(10, 0, 80, 24, 0);
        match copy_mode_key_command(&Key::Character(key.into()), false) {
            Some(CopyModeCommand::LineMotion(f)) => f(&mut state, line),
            Some(CopyModeCommand::WordMotion(f)) => f(&mut state, line, ""),
            _ => panic!("{key:?} must be a text motion"),
        }
        state.cursor_col
    };
    let expect = |f: fn(&mut CopyModeState, &str)| -> usize {
        let mut state = CopyModeState::new();
        state.enter(10, 0, 80, 24, 0);
        f(&mut state, line);
        state.cursor_col
    };
    let expect_word = |f: fn(&mut CopyModeState, &str, &str)| -> usize {
        let mut state = CopyModeState::new();
        state.enter(10, 0, 80, 24, 0);
        f(&mut state, line, "");
        state.cursor_col
    };
    assert_eq!(run("^"), expect(CopyModeState::move_to_first_non_blank));
    assert_eq!(run("W"), expect(CopyModeState::move_big_word_forward));
    assert_eq!(run("B"), expect(CopyModeState::move_big_word_backward));
    assert_eq!(run("E"), expect(CopyModeState::move_big_word_end));
    assert_eq!(run("w"), expect_word(CopyModeState::move_word_forward));
    assert_eq!(run("b"), expect_word(CopyModeState::move_word_backward));
    assert_eq!(run("e"), expect_word(CopyModeState::move_word_end));
    // At least six of the seven motions land on distinct columns from
    // col 10, so a swapped binding cannot pass.
    let cols: std::collections::HashSet<usize> = ["^", "W", "B", "E", "w", "b", "e"]
        .map(run)
        .into_iter()
        .collect();
    assert!(
        cols.len() >= 6,
        "motions should be distinguishable: {cols:?}"
    );
}

#[test]
fn copy_mode_visual_mode_keys() {
    let mut ws = window();
    prime(&mut ws, 0);
    ws.handle_copy_mode_logical_key(&Key::Character("V".into()));
    assert_eq!(ws.copy_mode.visual_mode, VisualMode::Line);
    ws.handle_copy_mode_logical_key(&Key::Named(NamedKey::Escape));
    assert_eq!(ws.copy_mode.visual_mode, VisualMode::None);
    assert!(ws.copy_mode.active, "first Escape only leaves visual mode");
    ws.handle_copy_mode_logical_key(&Key::Named(NamedKey::Escape));
    assert!(!ws.copy_mode.active, "second Escape exits copy mode");
}
