//! Handlers for the UX.md P3 pane actions, referenced from
//! [`super::keybinding_actions::ACTION_HANDLERS`] as plain fn items so the
//! table stays one line per action.

use crate::app::window_state::WindowState;

pub(super) fn toggle_pane_zoom(s: &mut WindowState) -> bool {
    s.toggle_pane_zoom();
    true
}

pub(super) fn next_pane(s: &mut WindowState) -> bool {
    s.focus_pane_cycle(true);
    true
}

pub(super) fn prev_pane(s: &mut WindowState) -> bool {
    s.focus_pane_cycle(false);
    true
}

pub(super) fn last_pane(s: &mut WindowState) -> bool {
    s.focus_last_pane();
    true
}

pub(super) fn restart_pane(s: &mut WindowState) -> bool {
    s.restart_focused_pane();
    true
}

pub(super) fn toggle_broadcast_input(s: &mut WindowState) -> bool {
    s.toggle_broadcast_input();
    true
}

pub(super) fn toggle_pane_broadcast(s: &mut WindowState) -> bool {
    s.toggle_broadcast_for_focused_pane();
    true
}

pub(super) fn equalize_panes(s: &mut WindowState) -> bool {
    s.equalize_panes();
    true
}

pub(super) fn cycle_layout(s: &mut WindowState) -> bool {
    s.cycle_layout_preset();
    true
}

pub(super) fn split_left(s: &mut WindowState) -> bool {
    s.split_pane_before(crate::pane::SplitDirection::Vertical);
    true
}

pub(super) fn split_up(s: &mut WindowState) -> bool {
    s.split_pane_before(crate::pane::SplitDirection::Horizontal);
    true
}

/// `layout:<name>` (UX.md A4): apply one preset by name. `None` when the
/// action is not a `layout:` action; `Some(false)` for an unknown name.
pub(super) fn layout_by_name(s: &mut WindowState, action: &str) -> Option<bool> {
    let name = action.strip_prefix("layout:")?;
    match crate::pane::LayoutPreset::from_name(name) {
        Some(preset) => {
            s.apply_layout_preset(preset);
            Some(true)
        }
        None => {
            log::warn!("Unknown layout preset '{name}'");
            Some(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use par_term_keybindings::KeybindingRegistry;
    use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};

    fn lookup(key: Key, physical: KeyCode, state: ModifiersState) -> Option<String> {
        let registry = KeybindingRegistry::from_config(&par_term_config::defaults::keybindings());
        registry
            .lookup_with_key_fields(
                &key,
                PhysicalKey::Code(physical),
                &winit::event::Modifiers::from(state),
                &Default::default(),
                false,
            )
            .map(str::to_string)
    }

    #[cfg(target_os = "macos")]
    const PANE_MOD: ModifiersState = ModifiersState::SUPER;
    #[cfg(not(target_os = "macos"))]
    const PANE_MOD: ModifiersState = ModifiersState::CONTROL.union(ModifiersState::ALT);
    #[cfg(target_os = "macos")]
    const ZOOM_MOD: ModifiersState = ModifiersState::SUPER.union(ModifiersState::SHIFT);
    #[cfg(not(target_os = "macos"))]
    const ZOOM_MOD: ModifiersState = ModifiersState::CONTROL.union(ModifiersState::SHIFT);

    /// UX.md RT1: the registry resolves the zoom chord, and `handle_key_event`
    /// consults the registry before the hardcoded Shift+Enter branch, so that
    /// branch never sees the chord.
    #[test]
    fn the_zoom_chord_resolves_to_toggle_pane_zoom() {
        assert_eq!(
            lookup(Key::Named(NamedKey::Enter), KeyCode::Enter, ZOOM_MOD).as_deref(),
            Some("toggle_pane_zoom")
        );
        assert_eq!(
            lookup(
                Key::Named(NamedKey::Enter),
                KeyCode::Enter,
                ModifiersState::SHIFT
            ),
            None,
            "plain Shift+Enter stays with the terminal"
        );
    }

    #[test]
    fn the_bracket_chords_cycle_panes() {
        assert_eq!(
            lookup(Key::Character("]".into()), KeyCode::BracketRight, PANE_MOD).as_deref(),
            Some("next_pane")
        );
        assert_eq!(
            lookup(Key::Character("[".into()), KeyCode::BracketLeft, PANE_MOD).as_deref(),
            Some("prev_pane")
        );
    }
}
