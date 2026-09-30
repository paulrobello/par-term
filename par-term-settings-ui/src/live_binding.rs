//! Live keybinding text for labels and tooltips (UX.md SC6, B53).
//!
//! Settings text used to name chords directly ("Cmd+Shift+S", "Cmd/Ctrl+F"),
//! which is wrong off macOS and after the user rebinds. Text that mentions a
//! shortcut now reads the action's current binding from the working config.

use par_term_config::Config;

/// The chord bound to `action` in `config`, with `CmdOrCtrl` spelled as
/// this platform's key (`Cmd+Shift+S` on macOS, `Ctrl+Shift+S` elsewhere),
/// or `None` when unbound.
pub fn binding_for(config: &Config, action: &str) -> Option<String> {
    config
        .keybindings
        .iter()
        .find(|binding| binding.action == action)
        .map(|binding| display_chord(&binding.key))
}

/// "`label` (`chord`)" when `action` is bound, else just `label`.
pub fn with_binding(config: &Config, label: &str, action: &str) -> String {
    match binding_for(config, action) {
        Some(chord) => format!("{label} ({chord})"),
        None => label.to_string(),
    }
}

fn display_chord(key: &str) -> String {
    key.split('+')
        .map(|part| match part {
            "CmdOrCtrl" => primary_modifier_name(),
            other => other,
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// The modifier the platform uses for Cmd-or-Ctrl chords, for text about
/// mouse gestures that have no keybinding action (Cmd+Click, Ctrl+Click).
pub fn primary_modifier_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "Cmd"
    } else {
        "Ctrl"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use par_term_config::KeyBinding;

    #[test]
    fn text_follows_a_rebinding() {
        let config = Config {
            keybindings: vec![KeyBinding {
                key: "Ctrl+Alt+Q".to_string(),
                action: "ssh_quick_connect".to_string(),
            }],
            ..Config::default()
        };
        let text = with_binding(&config, "SSH Quick Connect", "ssh_quick_connect");
        assert!(text.starts_with("SSH Quick Connect ("), "{text}");
        assert!(text.contains('Q'), "{text}");
    }

    #[test]
    fn unbound_action_shows_no_chord() {
        let config = Config {
            keybindings: Vec::new(),
            ..Config::default()
        };
        assert_eq!(with_binding(&config, "Search", "toggle_search"), "Search");
    }
}
