//! When the native menu must be rebuilt (UX.md MN3).
//!
//! The menu is built from a set of keybindings. Every path that changes the
//! bindings — Settings apply and save, `reload_config` (F5), the config-file
//! watcher picking up an outside edit — ends in the focused window's config,
//! which `WindowManager::sync_menus` compares against the bindings the menu
//! was built from on every event-loop tick. A difference rebuilds the menu,
//! so its accelerators always match the live registry and a chord the user
//! unbound is released.
//!
//! On macOS the menu is also rebuilt, without the accelerators a dialog or
//! text field must receive, while the keyboard belongs to one: a native key
//! equivalent fires before the window sees the key, and even a disabled item
//! swallows it (see [`super::state::release_captured_accelerators`]). Kept
//! separate from `MenuManager` so the decision is testable without a
//! main-thread muda menu.

use super::state::Capture;
use par_term_config::KeyBinding;

/// Why the menu needs a rebuild.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebuildReason {
    /// No menu was recorded as built yet.
    Initial,
    /// The live bindings differ from the ones the menu was built from.
    Bindings,
    /// The keyboard was captured or released (macOS accelerator release).
    Capture,
}

/// What the current menu was built from.
#[derive(Debug, Default)]
pub struct MenuSync {
    built: Option<(Vec<KeyBinding>, Capture)>,
}

impl MenuSync {
    /// A sync with no menu recorded.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a menu built from `live` with `capture` would differ from the
    /// current one.
    pub fn rebuild_reason(&self, live: &[KeyBinding], capture: &Capture) -> Option<RebuildReason> {
        match &self.built {
            None => Some(RebuildReason::Initial),
            Some((built, _)) if built.as_slice() != live => Some(RebuildReason::Bindings),
            Some((_, built_capture)) if built_capture != capture => Some(RebuildReason::Capture),
            Some(_) => None,
        }
    }

    /// Record that the menu was (re)built from `live` with `capture`.
    pub fn record(&mut self, live: &[KeyBinding], capture: Capture) {
        self.built = Some((live.to_vec(), capture));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn kb(key: &str, action: &str) -> KeyBinding {
        KeyBinding {
            key: key.to_string(),
            action: action.to_string(),
        }
    }

    #[test]
    fn nothing_recorded_means_build() {
        let sync = MenuSync::new();
        assert_eq!(
            sync.rebuild_reason(&[], &None),
            Some(RebuildReason::Initial)
        );
    }

    #[test]
    fn a_changed_binding_list_rebuilds_and_the_same_one_does_not() {
        let mut sync = MenuSync::new();
        let bindings = vec![kb("Cmd+T", "new_tab")];
        sync.record(&bindings, None);
        assert_eq!(sync.rebuild_reason(&bindings, &None), None);

        let rebound = vec![kb("Cmd+Alt+T", "new_tab")];
        assert_eq!(
            sync.rebuild_reason(&rebound, &None),
            Some(RebuildReason::Bindings)
        );
        let unbound: Vec<KeyBinding> = Vec::new();
        assert_eq!(
            sync.rebuild_reason(&unbound, &None),
            Some(RebuildReason::Bindings)
        );
    }

    #[test]
    fn capturing_or_releasing_the_keyboard_rebuilds() {
        let mut sync = MenuSync::new();
        let bindings = vec![kb("Cmd+T", "new_tab")];
        sync.record(&bindings, None);
        let captured: Capture = Some(BTreeSet::new());
        assert_eq!(
            sync.rebuild_reason(&bindings, &captured),
            Some(RebuildReason::Capture)
        );
        sync.record(&bindings, captured.clone());
        assert_eq!(sync.rebuild_reason(&bindings, &captured), None);

        // A different panel open while captured keeps a different toggle.
        let palette: Capture = Some(BTreeSet::from(["toggle_command_palette"]));
        assert_eq!(
            sync.rebuild_reason(&bindings, &palette),
            Some(RebuildReason::Capture)
        );
        assert_eq!(
            sync.rebuild_reason(&bindings, &None),
            Some(RebuildReason::Capture)
        );
    }
}
