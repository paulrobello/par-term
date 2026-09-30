//! Snippet import/export via YAML file dialogs.

use super::SettingsUI;
use par_term_config::snippets::SnippetLibrary;

/// Export all snippets to a YAML file via a save dialog.
///
/// SEC-021: the export is staged and renamed so that overwriting an existing
/// export cannot truncate it. The destination is a path the user picked in a
/// save dialog and is outside par-term's control, so its mode is preserved
/// rather than forced to `0o600`; a file created fresh by the export gets
/// `0o600`, which is the safe direction for snippet bodies that may embed
/// credentials, and the user can loosen it.
pub(super) fn export_snippets(settings: &mut SettingsUI) {
    let path = rfd::FileDialog::new()
        .set_title("Export Snippets")
        .add_filter("YAML", &["yaml", "yml"])
        .set_file_name("snippets.yaml")
        .save_file();

    if let Some(path) = path {
        let library = SnippetLibrary {
            snippets: settings.config.snippets.clone(),
        };
        let result = serde_yaml_ng::to_string(&library)
            .map_err(|e| format!("could not serialize snippets: {e}"))
            .and_then(|yaml| {
                par_term_config::atomic_save::save_string_atomic_preserving_mode(&path, &yaml)
                    .map_err(|e| format!("{e:#}"))
            });
        match result {
            Ok(()) => settings.show_info_banner(format!(
                "Exported {} snippets to {}",
                library.snippets.len(),
                path.display()
            )),
            Err(e) => {
                log::error!("Failed to export snippets: {e}");
                settings.show_error_banner(format!(
                    "Could not export snippets to {}: {e}",
                    path.display()
                ));
            }
        }
    }
}

/// What importing a snippet library did (UX.md B55: nothing is dropped
/// silently).
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct SnippetImportSummary {
    /// Snippets added.
    pub imported: usize,
    /// Titles skipped because a snippet with the same ID exists.
    pub duplicate_titles: Vec<String>,
    /// Titles imported without their keybinding, which was already in use.
    pub cleared_keybinding_titles: Vec<String>,
}

impl SnippetImportSummary {
    fn message(&self, source: &std::path::Path) -> String {
        let mut text = format!(
            "Imported {} snippets from {}.",
            self.imported,
            source.display()
        );
        if !self.duplicate_titles.is_empty() {
            text.push_str(&format!(
                " Skipped {} already present (same ID): {}.",
                self.duplicate_titles.len(),
                self.duplicate_titles.join(", ")
            ));
        }
        if !self.cleared_keybinding_titles.is_empty() {
            text.push_str(&format!(
                " Keybinding removed because it was already in use: {}.",
                self.cleared_keybinding_titles.join(", ")
            ));
        }
        text
    }
}

/// Merge a parsed snippet library into the working config, skipping IDs that
/// already exist and clearing keybindings that conflict.
pub(crate) fn merge_snippet_library(
    settings: &mut SettingsUI,
    library: SnippetLibrary,
) -> SnippetImportSummary {
    let existing_ids: std::collections::HashSet<String> = settings
        .config
        .snippets
        .iter()
        .map(|s| s.id.clone())
        .collect();
    let mut summary = SnippetImportSummary::default();
    for mut snippet in library.snippets {
        if existing_ids.contains(&snippet.id) {
            summary.duplicate_titles.push(snippet.title);
            continue;
        }
        if let Some(ref kb) = snippet.keybinding
            && settings.check_keybinding_conflict(kb, None).is_some()
        {
            snippet.keybinding = None;
            summary
                .cleared_keybinding_titles
                .push(snippet.title.clone());
        }
        settings.config.snippets.push(snippet);
        summary.imported += 1;
    }
    summary
}

/// Import snippets from a YAML file via an open dialog.
///
/// Merges imported snippets with existing ones, skipping duplicates by ID.
pub(super) fn import_snippets(settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    let path = rfd::FileDialog::new()
        .set_title("Import Snippets")
        .add_filter("YAML", &["yaml", "yml"])
        .pick_file();

    let Some(path) = path else {
        return;
    };
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("could not read the file: {e}"));
    import_snippet_text(settings, changes_this_frame, &path, content);
}

/// Merge the text read from `path` (or the read error) into the working
/// config and report the result in the banner (UX.md SS7, B55).
pub(crate) fn import_snippet_text(
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    path: &std::path::Path,
    content: Result<String, String>,
) {
    let library = content.and_then(|content| {
        serde_yaml_ng::from_str::<SnippetLibrary>(&content)
            .map_err(|e| format!("not a snippet library: {e}"))
    });
    match library {
        Ok(library) => {
            let summary = merge_snippet_library(settings, library);
            if summary.imported > 0 {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            log::info!("{}", summary.message(path));
            settings.show_info_banner(summary.message(path));
        }
        Err(e) => {
            log::error!("Failed to import snippets: {e}");
            settings.show_error_banner(format!(
                "Could not import snippets from {}: {e}",
                path.display()
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use par_term_config::Config;
    use par_term_config::snippets::SnippetConfig;

    fn snippet(id: &str, title: &str, key: Option<&str>) -> SnippetConfig {
        let mut s = SnippetConfig::new(id.to_string(), title.to_string(), "echo".to_string());
        s.keybinding = key.map(str::to_string);
        s
    }

    #[test]
    fn import_reports_duplicates_and_cleared_keybindings() {
        let config = Config {
            snippets: vec![snippet("one", "One", Some("Ctrl+Alt+J"))],
            ..Config::default()
        };
        let mut settings = SettingsUI::new_for_tests(config);

        let summary = merge_snippet_library(
            &mut settings,
            SnippetLibrary {
                snippets: vec![
                    snippet("one", "One again", None),
                    snippet("two", "Two", Some("Ctrl+Alt+J")),
                    snippet("three", "Three", None),
                ],
            },
        );

        assert_eq!(summary.imported, 2);
        assert_eq!(summary.duplicate_titles, ["One again"]);
        assert_eq!(summary.cleared_keybinding_titles, ["Two"]);
        assert_eq!(settings.config.snippets.len(), 3);
        let message = summary.message(std::path::Path::new("lib.yaml"));
        assert!(message.contains("One again") && message.contains("Two"));
    }

    #[test]
    fn snippet_import_failures_show_an_error_banner() {
        use crate::settings_ui::BannerKind;
        let path = std::path::Path::new("lib.yaml");
        for content in [
            Err("could not read the file: denied".to_string()),
            Ok("snippets: [not, a, snippet".to_string()),
        ] {
            let mut settings = SettingsUI::new_for_tests(Config::default());
            let mut changed = false;
            import_snippet_text(&mut settings, &mut changed, path, content);
            assert_eq!(settings.banner().map(|b| b.kind), Some(BannerKind::Error));
            assert!(!changed);
        }
    }
}
