//! Keybinding management methods for `Config`.
//!
//! Covers:
//! - Merging default keybindings and status-bar widgets into user config
//! - Generating / synchronising snippet and action keybindings

use super::config_struct::Config;

impl Config {
    /// Merge default keybindings into the user's config.
    /// Only adds keybindings for actions that don't already exist in the user's config.
    /// This ensures new features with default keybindings are available to existing users.
    pub(crate) fn merge_default_keybindings(&mut self) {
        let default_keybindings = crate::defaults::keybindings();

        // Get the set of actions already configured by the user (owned strings to avoid borrow issues)
        let existing_actions: std::collections::HashSet<String> = self
            .keybindings
            .iter()
            .map(|kb| kb.action.clone())
            .collect();

        // Add any default keybindings whose actions are not already configured
        let mut added_count = 0;
        for default_kb in default_keybindings {
            if !existing_actions.contains(&default_kb.action) {
                log::info!(
                    "Adding new default keybinding: {} -> {}",
                    default_kb.key,
                    default_kb.action
                );
                self.keybindings.push(default_kb);
                added_count += 1;
            }
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
    /// using the format "snippet:<id>" for snippets and "action:<id>" for actions.
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
