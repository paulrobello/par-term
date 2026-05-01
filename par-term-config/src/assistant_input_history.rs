//! Assistant input-history persistence.
//!
//! Stores submitted Assistant prompts as YAML in the par-term config directory.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::Config;

const INPUT_HISTORY_FILE_NAME: &str = "assistant_input_history.yaml";
pub const MAX_ASSISTANT_INPUT_HISTORY_ENTRIES: usize = 200;

/// Internal YAML file structure for assistant input history persistence.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AssistantInputHistoryFile {
    /// Ordered list of previously submitted prompt strings.
    #[serde(default)]
    entries: Vec<String>,
}

pub fn assistant_input_history_path() -> PathBuf {
    Config::config_dir().join(INPUT_HISTORY_FILE_NAME)
}

pub fn load_assistant_input_history() -> Result<Vec<String>, String> {
    load_assistant_input_history_from_path(&assistant_input_history_path())
}

pub fn save_assistant_input_history(entries: &[String]) -> Result<(), String> {
    save_assistant_input_history_to_path(&assistant_input_history_path(), entries)
}

pub fn normalize_assistant_input_history(entries: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut normalized = Vec::new();

    for entry in entries {
        let trimmed = entry.trim();
        if trimmed.is_empty() || normalized.iter().any(|existing| existing == trimmed) {
            continue;
        }

        normalized.push(trimmed.to_string());
        if normalized.len() >= MAX_ASSISTANT_INPUT_HISTORY_ENTRIES {
            break;
        }
    }

    normalized
}

pub fn merge_assistant_input_history(
    current_entries: &[String],
    persisted_entries: &[String],
) -> Vec<String> {
    normalize_assistant_input_history(
        current_entries
            .iter()
            .chain(persisted_entries.iter())
            .cloned(),
    )
}

fn load_assistant_input_history_from_path(path: &Path) -> Result<Vec<String>, String> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "read assistant input history {}: {error}",
                path.display()
            ));
        }
    };

    if contents.trim().is_empty() {
        return Ok(Vec::new());
    }

    let file: AssistantInputHistoryFile = serde_yaml_ng::from_str(&contents)
        .map_err(|error| format!("parse assistant input history {}: {error}", path.display()))?;
    Ok(normalize_assistant_input_history(file.entries))
}

fn save_assistant_input_history_to_path(path: &Path, entries: &[String]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "create assistant input history directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let file = AssistantInputHistoryFile {
        entries: normalize_assistant_input_history(entries.iter().cloned()),
    };
    let yaml = serde_yaml_ng::to_string(&file).map_err(|error| {
        format!(
            "serialize assistant input history {}: {error}",
            path.display()
        )
    })?;
    fs::write(path, yaml)
        .map_err(|error| format!("write assistant input history {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{
        load_assistant_input_history_from_path, normalize_assistant_input_history,
        save_assistant_input_history_to_path,
    };

    #[test]
    fn assistant_input_history_normalize_preserves_first_newest_entry() {
        let entries = normalize_assistant_input_history(vec![
            " newest ".to_string(),
            String::new(),
            "older".to_string(),
            "newest".to_string(),
        ]);

        assert_eq!(entries, vec!["newest".to_string(), "older".to_string()]);
    }

    #[test]
    fn assistant_input_history_load_missing_file_is_empty() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("assistant_input_history.yaml");

        let entries = load_assistant_input_history_from_path(&path).expect("load history");

        assert!(entries.is_empty());
    }

    #[test]
    fn assistant_input_history_save_and_load_round_trips() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("assistant_input_history.yaml");

        save_assistant_input_history_to_path(
            &path,
            &[
                " newest ".to_string(),
                String::new(),
                "older".to_string(),
                "newest".to_string(),
            ],
        )
        .expect("save history");

        let entries = load_assistant_input_history_from_path(&path).expect("load history");

        assert_eq!(entries, vec!["newest".to_string(), "older".to_string()]);
    }

    #[test]
    fn assistant_input_history_merge_keeps_current_entries_before_persisted_entries() {
        let current = vec!["window prompt".to_string(), "shared".to_string()];
        let persisted = vec!["other window".to_string(), "shared".to_string()];

        let entries = super::merge_assistant_input_history(&current, &persisted);

        assert_eq!(
            entries,
            vec![
                "window prompt".to_string(),
                "shared".to_string(),
                "other window".to_string(),
            ]
        );
    }
}
