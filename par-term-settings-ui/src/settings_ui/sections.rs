//! Settings section layout: sidebar navigation, page selector, page content,
//! deep links, keybinding check.
//!
//! Contains: show_settings_sections(), show_tab_content(), open_section(),
//! check_keybinding_conflict().

use par_term_config::snippets::normalize_action_prefix_char;

use super::SettingsUI;

impl SettingsUI {
    /// Show all settings sections using the sidebar + tab layout.
    pub(super) fn show_settings_sections(
        &mut self,
        ui: &mut egui::Ui,
        changes_this_frame: &mut bool,
    ) {
        self.take_pending_section(ui.ctx());
        self.publish_search_view(ui.ctx());

        let available_width = ui.available_width();
        // Reserve space for the footer (separator + button row)
        let footer_height = 45.0;
        let available_height = (ui.available_height() - footer_height).max(100.0);
        let sidebar_width = 150.0;
        let content_width = (available_width - sidebar_width - 15.0).max(300.0);

        let layout = egui::Layout::left_to_right(egui::Align::Min);
        ui.allocate_ui_with_layout(
            egui::vec2(available_width, available_height),
            layout,
            |ui| {
                // Sidebar with its own scroll area
                ui.allocate_ui_with_layout(
                    egui::vec2(sidebar_width, available_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("settings_sidebar")
                            .max_height(available_height)
                            .show(ui, |ui| {
                                crate::sidebar::show(ui, self);
                            });
                    },
                );

                ui.separator();

                // Content area: page selector, then the page in its own
                // scroll area
                ui.allocate_ui_with_layout(
                    egui::vec2(content_width, available_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        self.show_page_selector(ui);
                        let height = ui.available_height().max(100.0);
                        egui::ScrollArea::vertical()
                            .id_salt(("settings_tab_content", self.selected_tab.index()))
                            .max_height(height)
                            .show(ui, |ui| {
                                ui.set_min_width(content_width - 20.0);
                                self.show_tab_content(ui, changes_this_frame);
                            });
                    },
                );
            },
        );
    }

    /// The sub-page selector above the content (iTerm2-style segmented
    /// control). While a search is active every page with a match is shown,
    /// so the selector names that instead.
    fn show_page_selector(&mut self, ui: &mut egui::Ui) {
        let tab = self.selected_tab;
        if self.searching() {
            ui.label(
                egui::RichText::new(format!(
                    "{} \u{2014} matches from every page",
                    tab.display_name()
                ))
                .strong(),
            );
        } else {
            let selected = self.selected_page();
            ui.horizontal_wrapped(|ui| {
                for (i, page) in crate::layout::pages(tab).iter().enumerate() {
                    if ui.selectable_label(i == selected, page.title).clicked() && i != selected {
                        self.select_page(tab, i);
                    }
                }
            });
        }
        ui.separator();
    }

    /// Show the content for the currently selected tab: its selected page,
    /// or every page with a match while a search is active.
    pub(super) fn show_tab_content(&mut self, ui: &mut egui::Ui, changes_this_frame: &mut bool) {
        let mut collapsed = std::mem::take(&mut self.collapsed_sections);
        let tab = self.selected_tab;
        if self.searching() {
            let view = crate::search::live_view(ui.ctx());
            for (i, page) in crate::layout::pages(tab).iter().enumerate() {
                let shown = page
                    .sections
                    .iter()
                    .any(|s| !view.as_ref().is_some_and(|v| v.hides(s.id)));
                if shown {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(page.title).heading());
                }
                crate::layout::show_page(ui, self, tab, i, changes_this_frame, &mut collapsed);
            }
        } else {
            let page = self.selected_page();
            crate::layout::show_page(ui, self, tab, page, changes_this_frame, &mut collapsed);
        }
        self.collapsed_sections = collapsed;
    }

    fn searching(&self) -> bool {
        !self.search_query.trim().is_empty()
    }

    /// Open Settings at a section (UX.md B51): select its tab and page,
    /// clear any search that would hide it, and on the next frame expand,
    /// scroll to, and flash it. Unknown ids are ignored.
    pub fn open_section(&mut self, section_id: &str) {
        let Some((tab, page)) = crate::layout::locate(section_id) else {
            log::warn!("Settings deep link to unknown section {section_id:?}");
            return;
        };
        self.select_page(tab, page);
        self.search_query.clear();
        self.pending_section = Some(section_id.to_string());
    }

    /// Start the jump a deep link queued (needs the frame's context).
    fn take_pending_section(&mut self, ctx: &egui::Context) {
        if let Some(id) = self.pending_section.take() {
            crate::search::start_jump(ctx, crate::search::Jump::new(vec![id], None));
        }
    }

    /// Check if a keybinding conflicts with existing keybindings.
    ///
    /// `exclude_id` is the snippet or action ID being edited — its own keybinding
    /// entries (both in `config.keybindings` and in `config.snippets`) are skipped
    /// so that editing an existing item never reports a false self-conflict.
    ///
    /// Comparison is normalized (K25): two spellings of the same chord
    /// (`Ctrl+D` / `Control+D`, `Cmd+D` / `CmdOrCtrl+D` on macOS) conflict.
    pub fn check_keybinding_conflict(&self, key: &str, exclude_id: Option<&str>) -> Option<String> {
        for binding in &self.config.keybindings {
            if !chords_equal(&binding.key, key) {
                continue;
            }
            // Skip the excluded item's own entry (stored as "action:<id>" or "snippet:<id>")
            if let Some(id) = exclude_id {
                let action_entry = format!("action:{}", id);
                let snippet_entry = format!("snippet:{}", id);
                if binding.action == action_entry || binding.action == snippet_entry {
                    continue;
                }
            }
            return Some(format!("Already bound to: {}", binding.action));
        }

        for snippet in &self.config.snippets {
            if let Some(snippet_key) = &snippet.keybinding
                && chords_equal(snippet_key, key)
            {
                if exclude_id == Some(&snippet.id) {
                    continue;
                }
                return Some(format!("Already bound to snippet: {}", snippet.title));
            }
        }

        None
    }

    /// Conflict check for a chord recorded in the Keybindings tab for `action`
    /// (UX.md K25): user rows, snippet chords, and the shipped default
    /// bindings — the same defaults the menu advertises — with normalized
    /// comparison. The recorded action's own entries never self-conflict.
    pub fn check_recorded_chord_conflict(&self, key: &str, action: &str) -> Option<String> {
        // The leader chord is not a registry action: recording any other
        // binding onto it is a conflict (UX.md K25).
        let leader = self.config.input.leader_key.trim();
        if action != "leader_key" && !leader.is_empty() && chords_equal(leader, key) {
            return Some("Already bound to the leader chord".to_string());
        }

        for binding in &self.config.keybindings {
            if binding.action == action || !chords_equal(&binding.key, key) {
                continue;
            }
            return Some(format!("Already bound to: {}", binding.action));
        }

        for snippet in &self.config.snippets {
            if let Some(snippet_key) = &snippet.keybinding
                && chords_equal(snippet_key, key)
            {
                return Some(format!("Already bound to snippet: {}", snippet.title));
            }
        }

        self.check_default_chord_conflict(key, action)
    }

    /// Check a chord against the shipped default bindings (menu accelerators
    /// and layer chords). A default bound to a *different* action is flagged:
    /// the §3.3 merge rule means a user row claiming the chord silently
    /// shadows that default.
    pub fn check_default_chord_conflict(&self, key: &str, exclude_action: &str) -> Option<String> {
        for default in par_term_config::defaults::keybindings() {
            if default.action == exclude_action || !chords_equal(&default.key, key) {
                continue;
            }
            return Some(format!(
                "Conflicts with default binding for: {}",
                default.action
            ));
        }
        None
    }

    /// Check whether a custom action prefix character conflicts with another action.
    pub fn check_action_prefix_char_conflict(
        &self,
        prefix_char: char,
        exclude_id: Option<&str>,
    ) -> Option<String> {
        let normalized_prefix_char = normalize_action_prefix_char(prefix_char);

        for action in &self.config.actions {
            if exclude_id == Some(action.id()) {
                continue;
            }

            if action.prefix_follow_up_char() == Some(normalized_prefix_char) {
                return Some(format!("Already used by action: {}", action.title()));
            }
        }

        None
    }
}

/// Normalized chord equality (UX.md K25): parse both spellings and compare
/// platform-normalized combos, so `Ctrl+D`, `Control+D`, and `CmdOrCtrl+D`
/// (on macOS) are one chord. Spellings that do not parse fall back to exact
/// string comparison.
fn chords_equal(a: &str, b: &str) -> bool {
    match (
        par_term_keybindings::parser::parse_key_combo(a),
        par_term_keybindings::parser::parse_key_combo(b),
    ) {
        (Ok(a), Ok(b)) => a.platform_normalized() == b.platform_normalized(),
        _ => a == b,
    }
}

/// Whether a par-term chord (`Ctrl+B`) is the same key as a tmux prefix in
/// tmux's own notation (`C-b`, `M-a`, `C-S-x`) (UX.md B49: comparing the two
/// strings directly could never match).
pub(crate) fn chord_matches_tmux_prefix(chord: &str, tmux_prefix: &str) -> bool {
    let mut rest = tmux_prefix.trim();
    let mut parts: Vec<String> = Vec::new();
    loop {
        let modifier = match rest.get(..2) {
            Some("C-") => "Ctrl",
            Some("M-") | Some("A-") => "Alt",
            Some("S-") => "Shift",
            _ => break,
        };
        parts.push(modifier.to_string());
        rest = &rest[2..];
    }
    if rest.is_empty() {
        return false;
    }
    parts.push(if rest.chars().count() == 1 {
        rest.to_uppercase()
    } else {
        rest.to_string()
    });
    chords_equal(chord, &parts.join("+"))
}

#[cfg(test)]
mod tmux_prefix_tests {
    use super::chord_matches_tmux_prefix;

    #[test]
    fn par_term_chord_matches_tmux_notation() {
        assert!(chord_matches_tmux_prefix("Ctrl+B", "C-b"));
        assert!(chord_matches_tmux_prefix("Control+b", "C-b"));
        assert!(chord_matches_tmux_prefix("Ctrl+Alt+X", "C-M-x"));
        assert!(chord_matches_tmux_prefix("Ctrl+Space", "C-Space"));
        assert!(!chord_matches_tmux_prefix("Ctrl+A", "C-b"));
        assert!(!chord_matches_tmux_prefix("Ctrl+B", ""));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use par_term_config::Config;
    use par_term_config::snippets::CustomActionConfig;
    use std::collections::HashMap;

    fn insert_text(
        id: &str,
        title: &str,
        keybinding: Option<&str>,
        prefix_char: Option<char>,
    ) -> CustomActionConfig {
        CustomActionConfig::InsertText {
            id: id.to_string(),
            title: title.to_string(),
            text: "x".to_string(),
            variables: HashMap::new(),
            keybinding: keybinding.map(str::to_string),
            prefix_char,
            keybinding_enabled: true,
            description: None,
        }
    }

    #[test]
    fn prefix_char_conflicts_with_single_char_keybinding_on_another_action() {
        let mut config = Config::default();
        config
            .actions
            .push(insert_text("lenny1", "lenny1", Some("1"), None));
        let settings = SettingsUI::new_for_tests(config);

        let conflict = settings.check_action_prefix_char_conflict('1', None);
        assert_eq!(conflict.as_deref(), Some("Already used by action: lenny1"));
    }

    #[test]
    fn prefix_char_does_not_conflict_with_chord_keybinding() {
        let mut config = Config::default();
        config
            .actions
            .push(insert_text("run", "Run", Some("Ctrl+1"), None));
        let settings = SettingsUI::new_for_tests(config);

        assert_eq!(settings.check_action_prefix_char_conflict('1', None), None);
    }

    #[test]
    fn chord_conflict_uses_normalized_comparison() {
        // K25: "Control+D" and "Ctrl+D" (and any case variant) are the same
        // chord; an exact string compare misses the collision.
        let mut config = Config::default();
        config.keybindings.push(par_term_config::KeyBinding {
            key: "Control+D".to_string(),
            action: "my_action".to_string(),
        });
        let settings = SettingsUI::new_for_tests(config);

        let conflict = settings.check_keybinding_conflict("ctrl+d", None);
        assert!(
            conflict.is_some(),
            "spelling variants of one chord must conflict"
        );
        assert!(conflict.unwrap().contains("my_action"));
    }

    #[test]
    fn chord_conflict_unparseable_falls_back_to_exact_match() {
        let mut config = Config::default();
        config.keybindings.push(par_term_config::KeyBinding {
            key: "NotAKey".to_string(),
            action: "my_action".to_string(),
        });
        let settings = SettingsUI::new_for_tests(config);

        assert!(
            settings
                .check_keybinding_conflict("NotAKey", None)
                .is_some()
        );
        assert!(
            settings
                .check_keybinding_conflict("AlsoNotAKey", None)
                .is_none()
        );
    }

    #[test]
    fn recorded_chord_flags_default_menu_binding() {
        // Criterion 4 / K25: recording CmdOrCtrl+Shift+BracketRight (what the
        // recorder emits for Cmd/Ctrl+Shift+]) duplicates the shipped
        // next_tab default (CmdOrCtrl+Shift+] on macOS, Ctrl+Shift+]
        // elsewhere) after normalization — same chord on both platforms.
        let settings = SettingsUI::new_for_tests(Config::default());

        let conflict =
            settings.check_recorded_chord_conflict("CmdOrCtrl+Shift+BracketRight", "my_action");
        assert!(
            conflict.as_deref().is_some_and(|c| c.contains("next_tab")),
            "recorded chord must flag the next_tab default, got {conflict:?}"
        );

        // Re-binding an action to its own default chord is not a conflict.
        assert_eq!(
            settings.check_recorded_chord_conflict("CmdOrCtrl+Shift+BracketRight", "next_tab"),
            None
        );
    }

    #[test]
    fn recorded_chord_flags_other_user_row_but_not_own() {
        let mut config = Config::default();
        config.keybindings.push(par_term_config::KeyBinding {
            key: "Ctrl+D".to_string(),
            action: "other_action".to_string(),
        });
        let settings = SettingsUI::new_for_tests(config);

        let conflict = settings.check_recorded_chord_conflict("Control+D", "my_action");
        assert!(
            conflict
                .as_deref()
                .is_some_and(|c| c.contains("other_action"))
        );

        // The action's own row (the one just recorded) never self-conflicts.
        let mut config = Config::default();
        config.keybindings.push(par_term_config::KeyBinding {
            key: "Alt+K".to_string(),
            action: "my_action".to_string(),
        });
        let settings = SettingsUI::new_for_tests(config);
        assert_eq!(
            settings.check_recorded_chord_conflict("Alt+K", "my_action"),
            None
        );
    }

    #[test]
    fn recorded_chord_flags_leader_chord_but_not_leader_itself() {
        let settings = SettingsUI::new_for_tests(Config::default());
        let leader = settings.config.input.leader_key.clone();
        let conflict = settings.check_recorded_chord_conflict(&leader, "my_action");
        assert!(
            conflict
                .as_deref()
                .is_some_and(|c| c.contains("leader chord")),
            "recording onto the leader chord must conflict, got {conflict:?}"
        );
        // The leader recorder itself does not self-conflict with its old value.
        let own = settings.check_recorded_chord_conflict(&leader, "leader_key");
        assert!(own.as_deref().is_none_or(|c| !c.contains("leader chord")));

        // A disabled leader (empty chord) never conflicts.
        let mut config = Config::default();
        config.input.leader_key.clear();
        let settings = SettingsUI::new_for_tests(config);
        assert!(
            settings
                .check_recorded_chord_conflict("Ctrl+B", "my_action")
                .is_none_or(|c| !c.contains("leader chord"))
        );
    }

    #[test]
    fn leader_and_keybinding_conflicts_are_separate_fields() {
        let mut settings = SettingsUI::new_for_tests(Config::default());
        settings.leader_conflict = Some("x".to_string());
        assert!(settings.keybinding_conflict.is_none());
        settings.leader_conflict = None;
        settings.keybinding_conflict = Some("y".to_string());
        assert!(settings.leader_conflict.is_none());
    }
}
