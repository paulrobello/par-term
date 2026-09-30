//! Key routing by overlay stack (UX.md OV2), as a pure function.
//!
//! [`route_key`] decides where one key press goes from the stack and a
//! description of the key alone. `handle_window_event` and the `--ui-test`
//! chord injector both call it (through `WindowState::route_overlay_key`),
//! so a ui-test run proves the real routing path rather than a mirror.
//!
//! The contract:
//!
//! - The **top key owner** receives keys first. Modal, Popup, and Mode
//!   overlays always are key owners; a Panel only while an egui widget
//!   holds keyboard focus.
//! - **Escape** goes to the top key owner only and never reaches the shell
//!   while any key owner is open (see [`escape_behavior`]).
//! - The top overlay's **own toggle chord closes it**.
//! - Another overlay's chord **replaces** a top Popup, or opens above a
//!   focused Panel. A Modal is never replaced: a dialog must be answered,
//!   so F1 cannot silently dismiss the quit dialog.
//! - Modal and Popup overlays **consume every key they do not use** — it
//!   never reaches the PTY. Modes consume the keys they define (their
//!   handlers own that decision).

use super::{OverlayId, OverlayKind, OverlayStack};

/// What one key press is, as routing needs to know it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct KeyFacts {
    /// The key is Escape.
    pub(crate) is_escape: bool,
    /// The registry action the key's chord is bound to, if any.
    pub(crate) bound_action: Option<String>,
    /// The chord cannot produce text: it carries Cmd, Ctrl, or Alt, or it
    /// is a function key. Only such chords act while an egui text field
    /// holds focus — a bare-letter binding must not steal typing.
    pub(crate) is_command_chord: bool,
}

/// Where a key press goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KeyRoute {
    /// No overlay owns the key: normal terminal dispatch.
    Terminal,
    /// Close the top overlay (its own toggle chord, or Escape).
    CloseTop(OverlayId),
    /// Close the top Popup and run the bound action, which opens another
    /// overlay (OV2 "other overlay chords replace it").
    Replace { close: OverlayId, action: String },
    /// Run the bound action without closing anything: another overlay's
    /// chord pressed inside a focused Panel opens above the panel.
    OpenAbove { action: String },
    /// The overlay's own handler owns this key (a mode's key handler, or a
    /// panel's egui widgets); it never reaches the PTY unless that handler
    /// sends it there.
    ToOverlay(OverlayId),
    /// Swallow the key for the terminal: an overlay is open that does not
    /// use it. egui still sees it (typing into a focused filter field).
    Consume,
}

impl KeyRoute {
    /// Whether egui should see this key press. Presses the stack resolves
    /// itself (close, replace) are withheld: an Escape that closed the top
    /// overlay must not also reach the overlay beneath it through egui.
    pub(crate) fn feeds_egui(&self) -> bool {
        !matches!(
            self,
            KeyRoute::CloseTop(_) | KeyRoute::Replace { .. } | KeyRoute::OpenAbove { .. }
        )
    }

    /// Whether the press continues into terminal key dispatch
    /// (`handle_key_event`): only when no overlay owns it, or a mode does
    /// (mode handlers live there).
    pub(crate) fn continues_to_key_dispatch(&self) -> bool {
        match self {
            KeyRoute::Terminal => true,
            KeyRoute::ToOverlay(id) => id.kind() == OverlayKind::Mode,
            _ => false,
        }
    }
}

/// What Escape does when an overlay is the top key owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EscapeBehavior {
    /// Escape closes the overlay (Cancel, for a dialog).
    Close,
    /// The overlay's own handler decides (modes; the assistant panel,
    /// where Escape inside the chat field is not a close; the in-app menu,
    /// whose egui popup closes itself; the tab context menu, whose first
    /// Escape leaves inline rename and whose second closes the menu).
    Delegate,
    /// Escape is swallowed and closes nothing: the dialog carries an
    /// answer a stray Escape must not give. A trigger or agent-command
    /// prompt closing on Escape would silently deny a pending action; an
    /// update install in flight must not be abandoned.
    Inert,
}

pub(crate) const fn escape_behavior(id: OverlayId) -> EscapeBehavior {
    match id {
        OverlayId::TriggerConfirm | OverlayId::AgentCommandConfirm | OverlayId::UpdateDialog => {
            EscapeBehavior::Inert
        }
        OverlayId::AiInspector | OverlayId::AppMenu | OverlayId::TabContextMenu => {
            EscapeBehavior::Delegate
        }
        // A mode that guards the terminal keeps its key handler behind the
        // guard, so the stack must resolve its Escape (the demote pick).
        _ => match id.kind() {
            OverlayKind::Mode if !id.guards_terminal() => EscapeBehavior::Delegate,
            _ => EscapeBehavior::Close,
        },
    }
}

/// Decide where a key press goes (see the module docs for the contract).
///
/// `egui_focused` — an egui widget holds keyboard focus. It makes an open
/// Panel a key owner, and it turns bare-letter chords into typing.
pub(crate) fn route_key(stack: &OverlayStack, key: &KeyFacts, egui_focused: bool) -> KeyRoute {
    let Some(top) = stack.top_key_owner(egui_focused) else {
        return KeyRoute::Terminal;
    };

    if key.is_escape {
        return match escape_behavior(top) {
            EscapeBehavior::Close => KeyRoute::CloseTop(top),
            EscapeBehavior::Delegate => KeyRoute::ToOverlay(top),
            EscapeBehavior::Inert => KeyRoute::Consume,
        };
    }

    let chord_may_act = key.is_command_chord || !egui_focused;
    if let Some(action) = key.bound_action.as_deref()
        && chord_may_act
    {
        if top.toggle_action() == Some(action) {
            return KeyRoute::CloseTop(top);
        }
        if let Some(target) = OverlayId::opened_by_action(action)
            && target != top
        {
            match top.kind() {
                // Target already open beneath: closing the top reveals it
                // (running its toggle would close it instead).
                OverlayKind::Popup if stack.entries().contains(&target) => {
                    return KeyRoute::CloseTop(top);
                }
                OverlayKind::Popup => {
                    return KeyRoute::Replace {
                        close: top,
                        action: action.to_string(),
                    };
                }
                OverlayKind::Panel => {
                    return KeyRoute::OpenAbove {
                        action: action.to_string(),
                    };
                }
                OverlayKind::Modal | OverlayKind::Mode => {}
            }
        }
    }

    match top.kind() {
        OverlayKind::Mode | OverlayKind::Panel => KeyRoute::ToOverlay(top),
        OverlayKind::Modal | OverlayKind::Popup => KeyRoute::Consume,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(action: Option<&str>, command: bool) -> KeyFacts {
        KeyFacts {
            is_escape: false,
            bound_action: action.map(str::to_string),
            is_command_chord: command,
        }
    }

    fn escape() -> KeyFacts {
        KeyFacts {
            is_escape: true,
            ..KeyFacts::default()
        }
    }

    fn stack(ids: &[OverlayId]) -> OverlayStack {
        OverlayStack::from_open(ids.iter().copied())
    }

    #[test]
    fn no_overlay_routes_to_the_terminal() {
        assert_eq!(
            route_key(&stack(&[]), &key(None, false), false),
            KeyRoute::Terminal
        );
        assert_eq!(route_key(&stack(&[]), &escape(), false), KeyRoute::Terminal);
    }

    #[test]
    fn the_top_overlay_receives_keys_first() {
        // Palette over the tree picker: Escape closes the palette only.
        let s = stack(&[OverlayId::TreePicker, OverlayId::CommandPalette]);
        assert_eq!(
            route_key(&s, &escape(), true),
            KeyRoute::CloseTop(OverlayId::CommandPalette)
        );
        // A quit dialog above the palette owns the key instead.
        let s = stack(&[OverlayId::CommandPalette, OverlayId::QuitConfirm]);
        assert_eq!(
            route_key(&s, &escape(), false),
            KeyRoute::CloseTop(OverlayId::QuitConfirm)
        );
        // The close is withheld from egui, so the Escape cannot also close
        // the overlay beneath through egui's own handler.
        assert!(!KeyRoute::CloseTop(OverlayId::CommandPalette).feeds_egui());
    }

    #[test]
    fn a_modal_consumes_every_unused_key() {
        let s = stack(&[OverlayId::QuitConfirm]);
        for k in [
            key(None, false),
            key(None, true),
            key(Some("new_tab"), true),
            key(Some("split_right"), true),
        ] {
            let route = route_key(&s, &k, false);
            assert_eq!(route, KeyRoute::Consume, "{k:?}");
            assert!(!route.continues_to_key_dispatch(), "never reaches the PTY");
        }
    }

    #[test]
    fn a_modal_is_never_replaced_by_another_overlays_chord() {
        // F1 (toggle_help) must not dismiss the quit dialog.
        let s = stack(&[OverlayId::QuitConfirm]);
        assert_eq!(
            route_key(&s, &key(Some("toggle_help"), true), false),
            KeyRoute::Consume
        );
        let s = stack(&[OverlayId::MuxLastTab]);
        assert_eq!(
            route_key(&s, &key(Some("toggle_command_palette"), true), false),
            KeyRoute::Consume
        );
    }

    #[test]
    fn a_popup_consumes_every_unused_key() {
        let s = stack(&[OverlayId::CommandPalette]);
        assert_eq!(route_key(&s, &key(None, false), true), KeyRoute::Consume);
        assert_eq!(
            route_key(&s, &key(Some("new_tab"), true), true),
            KeyRoute::Consume
        );
        // Consumed keys still feed egui: typing reaches the filter field.
        assert!(KeyRoute::Consume.feeds_egui());
    }

    #[test]
    fn escape_goes_to_the_top_overlay_only() {
        let s = stack(&[
            OverlayId::AiInspector,
            OverlayId::Search,
            OverlayId::CommandHistory,
        ]);
        assert_eq!(
            route_key(&s, &escape(), true),
            KeyRoute::CloseTop(OverlayId::CommandHistory)
        );
    }

    #[test]
    fn escape_on_an_answer_bearing_dialog_is_consumed_not_closing() {
        for id in [
            OverlayId::TriggerConfirm,
            OverlayId::AgentCommandConfirm,
            OverlayId::UpdateDialog,
        ] {
            assert_eq!(
                route_key(&stack(&[id]), &escape(), false),
                KeyRoute::Consume,
                "{id:?}"
            );
        }
    }

    #[test]
    fn the_toggle_chord_closes_its_own_overlay() {
        for id in [
            OverlayId::CommandPalette,
            OverlayId::CommandHistory,
            OverlayId::ClipboardHistory,
            OverlayId::Search,
            OverlayId::TreePicker,
            OverlayId::SessionPicker,
            OverlayId::Help,
            OverlayId::AgentUsage,
        ] {
            let action = id.toggle_action().expect("popups with a chord");
            assert_eq!(
                route_key(&stack(&[id]), &key(Some(action), true), true),
                KeyRoute::CloseTop(id),
                "{id:?}"
            );
        }
    }

    #[test]
    fn another_overlays_chord_replaces_a_popup() {
        let s = stack(&[OverlayId::CommandHistory]);
        assert_eq!(
            route_key(&s, &key(Some("toggle_command_palette"), true), true),
            KeyRoute::Replace {
                close: OverlayId::CommandHistory,
                action: "toggle_command_palette".to_string(),
            }
        );
    }

    #[test]
    fn a_chord_for_an_overlay_already_open_beneath_reveals_it() {
        // Palette over search, Cmd+F: running toggle_search would close the
        // search bar; closing the palette reveals it instead.
        let s = stack(&[OverlayId::Search, OverlayId::CommandPalette]);
        assert_eq!(
            route_key(&s, &key(Some("toggle_search"), true), true),
            KeyRoute::CloseTop(OverlayId::CommandPalette)
        );
    }

    #[test]
    fn a_bare_letter_chord_is_typing_while_a_text_field_is_focused() {
        // A user binding `p` to the palette must not steal the letter from
        // the palette's own filter field.
        let s = stack(&[OverlayId::CommandPalette]);
        assert_eq!(
            route_key(&s, &key(Some("toggle_command_palette"), false), true),
            KeyRoute::Consume
        );
        // Without a focused field the same chord is a command.
        assert_eq!(
            route_key(&s, &key(Some("toggle_command_palette"), false), false),
            KeyRoute::CloseTop(OverlayId::CommandPalette)
        );
    }

    #[test]
    fn an_unfocused_panel_leaves_keys_to_the_terminal() {
        // The assistant panel open but unfocused must not eat vim's Escape.
        let s = stack(&[OverlayId::AiInspector]);
        assert_eq!(route_key(&s, &escape(), false), KeyRoute::Terminal);
        assert_eq!(route_key(&s, &key(None, false), false), KeyRoute::Terminal);
        // Focused, it owns its keys, its toggle chord closes it, and
        // another overlay's chord opens above it without closing it.
        assert_eq!(
            route_key(&s, &key(None, false), true),
            KeyRoute::ToOverlay(OverlayId::AiInspector)
        );
        assert_eq!(
            route_key(&s, &key(Some("toggle_ai_inspector"), true), true),
            KeyRoute::CloseTop(OverlayId::AiInspector)
        );
        assert_eq!(
            route_key(&s, &key(Some("toggle_command_palette"), true), true),
            KeyRoute::OpenAbove {
                action: "toggle_command_palette".to_string()
            }
        );
        // Escape inside the chat field is the panel's own call.
        assert_eq!(
            route_key(&s, &escape(), true),
            KeyRoute::ToOverlay(OverlayId::AiInspector)
        );
    }

    #[test]
    fn the_profile_drawer_closes_on_escape_while_focused() {
        let s = stack(&[OverlayId::ProfileDrawer]);
        assert_eq!(
            route_key(&s, &escape(), true),
            KeyRoute::CloseTop(OverlayId::ProfileDrawer)
        );
        assert_eq!(route_key(&s, &escape(), false), KeyRoute::Terminal);
    }

    #[test]
    fn a_mode_owns_its_keys_and_its_escape() {
        let s = stack(&[OverlayId::PaneHints]);
        let route = route_key(&s, &key(None, false), false);
        assert_eq!(route, KeyRoute::ToOverlay(OverlayId::PaneHints));
        assert!(route.continues_to_key_dispatch(), "mode handlers run there");
        assert_eq!(
            route_key(&s, &escape(), false),
            KeyRoute::ToOverlay(OverlayId::PaneHints)
        );
        // A mode is not replaced by another overlay's chord.
        assert_eq!(
            route_key(&s, &key(Some("toggle_command_palette"), true), false),
            KeyRoute::ToOverlay(OverlayId::PaneHints)
        );
        // Copy mode's toggle chord closes it.
        let s = stack(&[OverlayId::CopyMode]);
        assert_eq!(
            route_key(&s, &key(Some("toggle_copy_mode"), true), false),
            KeyRoute::CloseTop(OverlayId::CopyMode)
        );
    }
}
