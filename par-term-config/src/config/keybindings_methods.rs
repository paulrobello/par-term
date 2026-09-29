//! Keybinding management methods for `Config`.
//!
//! Covers:
//! - Merging default keybindings and status-bar widgets into user config
//! - Generating / synchronising snippet and action keybindings

use super::config_struct::Config;
use crate::types::KeyBinding;
use std::collections::{BTreeSet, HashSet};

/// Action ids renamed between releases, as `(previous id, current id)`.
///
/// Applied in place at load: the user's chord moves to the current id instead
/// of a default for the new id being added alongside it (UX.md §3.3 migration
/// rule 2). Empty until an action rename ships — D1 plans
/// `split_horizontal` → `split_down`; add the pair here in the same release
/// that renames the default.
const ACTION_RENAMES: &[(&str, &str)] = &[];

/// Canonical comparison form of a chord string, for duplicate-chord detection
/// at config-load time (UX.md §3.3 rule 1).
///
/// Mirrors the modifier vocabulary of `par_term_keybindings::parser`, folding
/// synonyms (`Ctrl`/`Control`, `Alt`/`Option`, `Super`/`Cmd`/`Command`/`Meta`/
/// `Win`) into one token and resolving `CmdOrCtrl` to the platform modifier it
/// stands for, so `CmdOrCtrl+Shift+P` and `Shift+CmdOrCtrl+P` compare equal on
/// every platform. The parsed registry remains the match-time authority; this
/// catches the spellings users and defaults actually write.
///
/// Returns `None` for chords this cannot canonicalize (no key, more than one
/// key). A `None` chord never claims anything — the registry still rejects it
/// at registration if it is invalid.
pub(crate) fn canonical_chord(key: &str) -> Option<String> {
    let parts: Vec<&str> = key.split('+').map(str::trim).collect();
    if parts.is_empty() {
        return None;
    }

    let mut modifiers: BTreeSet<&str> = BTreeSet::new();
    let mut key_part: Option<&str> = None;

    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        let part_lower = part.to_lowercase();

        let is_modifier = match part_lower.as_str() {
            "ctrl" | "control" => modifiers.insert("ctrl"),
            "alt" | "option" => modifiers.insert("alt"),
            "shift" => modifiers.insert("shift"),
            "super" | "cmd" | "command" | "meta" | "win" => modifiers.insert("super"),
            "cmdorctrl" => {
                if cfg!(target_os = "macos") {
                    modifiers.insert("super")
                } else {
                    modifiers.insert("ctrl")
                }
            }
            _ => false,
        };

        if !is_modifier {
            if key_part.is_some() {
                // Same shape the parser rejects: two non-modifier tokens.
                return None;
            }
            key_part = Some(part);
        } else if is_last {
            // Ends with a modifier and no key.
            return None;
        }
    }

    let key = key_part?;
    if key.is_empty() {
        return None;
    }

    let mut canonical = String::new();
    for modifier in &modifiers {
        canonical.push_str(modifier);
        canonical.push('+');
    }
    canonical.push_str(&key.to_lowercase());
    Some(canonical)
}

impl Config {
    /// Merge default keybindings into the user's config, applying the UX.md
    /// §3.3 migration rule: renamed actions migrate in place first, then a
    /// new or moved default is added only if its action id is unbound AND its
    /// chord is unclaimed by any other binding.
    pub(crate) fn merge_default_keybindings(&mut self) {
        self.migrate_renamed_keybinding_actions();
        self.merge_default_keybindings_from(&crate::defaults::keybindings());
    }

    /// Rewrite keybinding action ids through [`ACTION_RENAMES`], keeping each
    /// binding's chord (migration rule 2).
    pub(crate) fn migrate_renamed_keybinding_actions(&mut self) {
        self.migrate_renamed_keybinding_actions_from(ACTION_RENAMES);
    }

    fn migrate_renamed_keybinding_actions_from(&mut self, renames: &[(&str, &str)]) {
        for binding in &mut self.keybindings {
            if let Some((_, current)) = renames
                .iter()
                .find(|(previous, _)| *previous == binding.action)
            {
                log::info!(
                    "Migrating keybinding action '{}' -> '{}' (chord '{}' kept)",
                    binding.action,
                    current,
                    binding.key
                );
                binding.action = current.to_string();
            }
        }
    }

    /// Add defaults whose action id is unbound and whose chord is unclaimed
    /// (migration rule 1). Split from [`Self::merge_default_keybindings`] so
    /// tests can drive the claim check with synthetic future defaults.
    fn merge_default_keybindings_from(&mut self, default_keybindings: &[KeyBinding]) {
        // Owned strings: the sets outlive the pushes into `self.keybindings`.
        let existing_actions: HashSet<String> = self
            .keybindings
            .iter()
            .map(|kb| kb.action.clone())
            .collect();
        let mut claimed_chords: HashSet<String> = self
            .keybindings
            .iter()
            .filter_map(|kb| canonical_chord(&kb.key))
            .collect();

        let mut added_count = 0;
        for default_kb in default_keybindings {
            if existing_actions.contains(&default_kb.action) {
                continue;
            }

            // A new or moved default never takes a chord the user's config
            // already binds to another action — otherwise an upgrading user
            // silently gains a duplicate chord (B20/B23). Inserting into the
            // claimed set returns false when the chord is taken.
            if let Some(canonical) = canonical_chord(&default_kb.key) {
                if !claimed_chords.insert(canonical) {
                    log::warn!(
                        "Not adding default keybinding {} -> {}: the chord is already bound by \
                         another action in this config",
                        default_kb.key,
                        default_kb.action
                    );
                    continue;
                }
            } else {
                // Uncanonicalizable chord: still add the row so behavior
                // matches previous releases; the registry warns if the chord
                // cannot actually parse.
            }

            log::info!(
                "Adding new default keybinding: {} -> {}",
                default_kb.key,
                default_kb.action
            );
            self.keybindings.push(KeyBinding {
                key: default_kb.key.clone(),
                action: default_kb.action.clone(),
            });
            added_count += 1;
        }

        if added_count > 0 {
            log::info!(
                "Merged {} new default keybinding(s) into user config",
                added_count
            );
        }
    }

    /// Merge default status bar widgets into the user's config.
    /// Only adds widgets whose `WidgetId` doesn't already exist in the user's widget list.
    /// This ensures new built-in widgets are available to existing users.
    pub(crate) fn merge_default_widgets(&mut self) {
        let default_widgets = crate::status_bar::default_widgets();

        let existing_ids: std::collections::HashSet<crate::status_bar::WidgetId> = self
            .status_bar
            .status_bar_widgets
            .iter()
            .map(|w| w.id.clone())
            .collect();

        let mut added_count = 0;
        for default_widget in default_widgets {
            if !existing_ids.contains(&default_widget.id) {
                log::info!(
                    "Adding new default status bar widget: {:?}",
                    default_widget.id
                );
                self.status_bar.status_bar_widgets.push(default_widget);
                added_count += 1;
            }
        }

        if added_count > 0 {
            log::info!(
                "Merged {} new default status bar widget(s) into user config",
                added_count
            );
        }
    }

    /// Generate keybindings for snippets and actions that have keybindings configured.
    ///
    /// This method adds or updates keybindings for snippets and actions in the keybindings list,
    /// using the format `snippet:<id>` for snippets and `action:<id>` for actions.
    /// If a keybinding for a snippet/action already exists, it will be updated with the new key.
    pub fn generate_snippet_action_keybindings(&mut self) {
        use crate::config::KeyBinding;

        // Track actions we've seen to remove stale keybindings later
        let mut seen_actions = std::collections::HashSet::new();
        let mut added_count = 0;
        let mut updated_count = 0;

        // Generate keybindings for snippets
        for snippet in &self.snippets {
            if let Some(key) = &snippet.keybinding {
                let action = format!("snippet:{}", snippet.id);
                seen_actions.insert(action.clone());

                if !key.is_empty() && snippet.enabled && snippet.keybinding_enabled {
                    // Check if this action already has a keybinding
                    if let Some(existing) =
                        self.keybindings.iter_mut().find(|kb| kb.action == action)
                    {
                        // Update existing keybinding if the key changed
                        if existing.key != *key {
                            log::info!(
                                "Updating keybinding for snippet '{}': {} -> {} (was: {})",
                                snippet.title,
                                key,
                                action,
                                existing.key
                            );
                            existing.key = key.clone();
                            updated_count += 1;
                        }
                    } else {
                        // Add new keybinding
                        log::info!(
                            "Adding keybinding for snippet '{}': {} -> {} (enabled={}, keybinding_enabled={})",
                            snippet.title,
                            key,
                            action,
                            snippet.enabled,
                            snippet.keybinding_enabled
                        );
                        self.keybindings.push(KeyBinding {
                            key: key.clone(),
                            action,
                        });
                        added_count += 1;
                    }
                } else if !key.is_empty() {
                    log::info!(
                        "Skipping keybinding for snippet '{}': {} (enabled={}, keybinding_enabled={})",
                        snippet.title,
                        key,
                        snippet.enabled,
                        snippet.keybinding_enabled
                    );
                }
            }
        }

        // Generate keybindings for actions
        for action_config in &self.actions {
            if let Some(key) = action_config.keybinding() {
                let action = format!("action:{}", action_config.id());
                let prefix_only = !self.custom_action_prefix_key.trim().is_empty()
                    && action_config.prefix_char().is_none()
                    && action_config.single_char_keybinding_follow_up().is_some();

                if prefix_only {
                    log::info!(
                        "Skipping global keybinding for action '{}': {} is a prefix follow-up",
                        action_config.title(),
                        key
                    );
                    continue;
                }
                seen_actions.insert(action.clone());

                if !key.is_empty() && action_config.keybinding_enabled() {
                    // Check if this action already has a keybinding
                    if let Some(existing) =
                        self.keybindings.iter_mut().find(|kb| kb.action == action)
                    {
                        // Update existing keybinding if the key changed
                        if existing.key != key {
                            log::info!(
                                "Updating keybinding for action '{}': {} -> {} (was: {})",
                                action_config.title(),
                                key,
                                action,
                                existing.key
                            );
                            existing.key = key.to_string();
                            updated_count += 1;
                        }
                    } else {
                        // Add new keybinding
                        log::info!(
                            "Adding keybinding for action '{}': {} -> {} (keybinding_enabled={})",
                            action_config.title(),
                            key,
                            action,
                            action_config.keybinding_enabled()
                        );
                        self.keybindings.push(KeyBinding {
                            key: key.to_string(),
                            action,
                        });
                        added_count += 1;
                    }
                } else if !key.is_empty() {
                    log::info!(
                        "Skipping keybinding for action '{}': {} (keybinding_enabled={})",
                        action_config.title(),
                        key,
                        action_config.keybinding_enabled()
                    );
                }
            }
        }

        // Remove stale keybindings for snippets that no longer have keybindings or are disabled
        let original_len = self.keybindings.len();
        self.keybindings.retain(|kb| {
            // Keep if it's not a snippet/action keybinding
            if !kb.action.starts_with("snippet:") && !kb.action.starts_with("action:") {
                return true;
            }
            // Keep if we saw it during our scan
            seen_actions.contains(&kb.action)
        });
        let removed_count = original_len - self.keybindings.len();

        if added_count > 0 || updated_count > 0 || removed_count > 0 {
            log::info!(
                "Snippet/Action keybindings: {} added, {} updated, {} removed",
                added_count,
                updated_count,
                removed_count
            );
        }
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    fn config_with(keybindings: &[(&str, &str)]) -> Config {
        Config {
            keybindings: keybindings
                .iter()
                .map(|(key, action)| KeyBinding {
                    key: key.to_string(),
                    action: action.to_string(),
                })
                .collect(),
            ..Config::default()
        }
    }

    fn bound_chord(config: &Config, action: &str) -> Option<String> {
        config
            .keybindings
            .iter()
            .find(|kb| kb.action == action)
            .map(|kb| kb.key.clone())
    }

    /// Every canonical chord appears at most once across the binding list.
    fn no_duplicate_chords(config: &Config) -> bool {
        let mut seen = HashSet::new();
        config
            .keybindings
            .iter()
            .filter_map(|kb| canonical_chord(&kb.key))
            .all(|chord| seen.insert(chord))
    }

    #[test]
    fn canonical_chord_folds_synonyms_and_order() {
        assert_eq!(
            canonical_chord("Ctrl+Shift+T"),
            canonical_chord("shift+control+t")
        );
        assert_eq!(
            canonical_chord("CmdOrCtrl+Shift+P"),
            canonical_chord("Shift+CmdOrCtrl+P")
        );
        assert_eq!(canonical_chord("Alt+F5"), canonical_chord("Option+F5"));
        assert_eq!(canonical_chord("Cmd+O"), canonical_chord("command+O"));
        assert_eq!(
            canonical_chord("CmdOrCtrl+O"),
            if cfg!(target_os = "macos") {
                canonical_chord("Cmd+O")
            } else {
                canonical_chord("Ctrl+O")
            }
        );
    }

    #[test]
    fn canonical_chord_rejects_keyless_and_multi_key() {
        assert_eq!(canonical_chord("Ctrl+Shift+"), None);
        assert_eq!(canonical_chord("Ctrl"), None);
        assert_eq!(canonical_chord("Ctrl+D+T"), None);
        assert!(canonical_chord("Ctrl+D").is_some());
    }

    #[test]
    fn moved_default_is_not_added_when_chord_claimed() {
        // UX.md K10's planned move: command palette claims CmdOrCtrl+Shift+P,
        // which this previous-release config already binds to the drawer.
        let mut config = config_with(&[("CmdOrCtrl+Shift+P", "toggle_profile_drawer")]);

        let future_defaults = vec![KeyBinding {
            key: "Shift+CmdOrCtrl+P".to_string(), // different spelling, same chord
            action: "command_palette".to_string(),
        }];
        config.merge_default_keybindings_from(&future_defaults);

        assert!(
            bound_chord(&config, "command_palette").is_none(),
            "a default must not claim an already-bound chord"
        );
        assert_eq!(
            bound_chord(&config, "toggle_profile_drawer"),
            Some("CmdOrCtrl+Shift+P".to_string())
        );
    }

    #[test]
    fn renamed_action_migrates_in_place() {
        // D1's planned rename: split_horizontal -> split_down. The user's
        // chord moves to the new id; the new default is not added alongside.
        let mut config = config_with(&[("CmdOrCtrl+D", "split_horizontal")]);

        config.migrate_renamed_keybinding_actions_from(&[("split_horizontal", "split_down")]);
        assert_eq!(
            bound_chord(&config, "split_down"),
            Some("CmdOrCtrl+D".to_string())
        );
        assert!(bound_chord(&config, "split_horizontal").is_none());

        let future_defaults = vec![KeyBinding {
            key: "CmdOrCtrl+D".to_string(),
            action: "split_down".to_string(),
        }];
        config.merge_default_keybindings_from(&future_defaults);
        assert!(
            no_duplicate_chords(&config),
            "the renamed binding must not gain a duplicate chord"
        );
        assert_eq!(
            config
                .keybindings
                .iter()
                .filter(|kb| kb.action == "split_down")
                .count(),
            1
        );
    }

    /// UX.md P2 acceptance: loading a config saved by the previous release
    /// preserves every existing binding and produces no duplicate chords.
    #[test]
    fn previous_release_config_preserves_bindings_without_duplicates() {
        let previous_release_yaml = r#"
keybindings:
  - key: CmdOrCtrl+D
    action: split_horizontal
  - key: CmdOrCtrl+Shift+P
    action: toggle_profile_drawer
  - key: CmdOrCtrl+Shift+F9
    action: my_custom_action
"#;
        let mut config: Config =
            serde_yaml_ng::from_str(previous_release_yaml).expect("fixture YAML parses");

        config.merge_default_keybindings();

        // Every previous-release binding survives on its own chord.
        assert_eq!(
            bound_chord(&config, "split_horizontal"),
            Some("CmdOrCtrl+D".to_string())
        );
        assert_eq!(
            bound_chord(&config, "toggle_profile_drawer"),
            Some("CmdOrCtrl+Shift+P".to_string())
        );
        assert_eq!(
            bound_chord(&config, "my_custom_action"),
            Some("CmdOrCtrl+Shift+F9".to_string())
        );

        // And the merged result has no chord claimed twice.
        assert!(
            no_duplicate_chords(&config),
            "merged config must not contain duplicate chords"
        );

        // Missing defaults are still filled in (an action with no binding at
        // all gains the default, as before).
        assert!(bound_chord(&config, "toggle_background_shader").is_some());
    }
}
