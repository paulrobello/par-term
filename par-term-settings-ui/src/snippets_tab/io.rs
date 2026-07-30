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
        match serde_yaml_ng::to_string(&library) {
            Ok(yaml) => {
                if let Err(e) =
                    par_term_config::atomic_save::save_string_atomic_preserving_mode(&path, &yaml)
                {
                    log::error!("Failed to write snippet library: {:#}", e);
                } else {
                    log::info!(
                        "Exported {} snippets to {}",
                        library.snippets.len(),
                        path.display()
                    );
                }
            }
            Err(e) => {
                log::error!("Failed to serialize snippet library: {}", e);
            }
        }
    }
}

/// Import snippets from a YAML file via an open dialog.
///
/// Merges imported snippets with existing ones, skipping duplicates by ID.
pub(super) fn import_snippets(settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    let path = rfd::FileDialog::new()
        .set_title("Import Snippets")
        .add_filter("YAML", &["yaml", "yml"])
        .pick_file();

    if let Some(path) = path {
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_yaml_ng::from_str::<SnippetLibrary>(&content) {
                Ok(library) => {
                    let existing_ids: std::collections::HashSet<String> = settings
                        .config
                        .snippets
                        .iter()
                        .map(|s| s.id.clone())
                        .collect();

                    let mut imported = 0usize;
                    let mut skipped = 0usize;

                    for mut snippet in library.snippets {
                        if existing_ids.contains(&snippet.id) {
                            skipped += 1;
                            continue;
                        }

                        // Clear keybinding if it conflicts with an existing one
                        if let Some(ref kb) = snippet.keybinding
                            && settings.check_keybinding_conflict(kb, None).is_some()
                        {
                            snippet.keybinding = None;
                        }

                        settings.config.snippets.push(snippet);
                        imported += 1;
                    }

                    if imported > 0 {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    log::info!(
                        "Imported {} snippets ({} skipped as duplicates) from {}",
                        imported,
                        skipped,
                        path.display()
                    );
                }
                Err(e) => {
                    log::error!("Failed to parse snippet library: {}", e);
                }
            },
            Err(e) => {
                log::error!("Failed to read snippet file: {}", e);
            }
        }
    }
}
