//! Assistant prompt-library storage.
//!
//! Prompts are Markdown files stored under the par-term config directory.
//! YAML frontmatter contains metadata; the Markdown body is the prompt text.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::Config;

const PROMPT_DIR_NAME: &str = "assistant-prompts";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistantPrompt {
    pub path: PathBuf,
    pub title: String,
    pub auto_submit: bool,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssistantPromptDraft {
    pub title: String,
    pub auto_submit: bool,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssistantPromptMetadata {
    title: String,
    auto_submit: bool,
}

pub fn assistant_prompts_dir() -> PathBuf {
    Config::config_dir().join(PROMPT_DIR_NAME)
}

pub fn list_prompts() -> Result<Vec<AssistantPrompt>, String> {
    list_prompts_in_dir(&assistant_prompts_dir())
}

pub fn list_prompts_in_dir(dir: &Path) -> Result<Vec<AssistantPrompt>, String> {
    fs::create_dir_all(dir).map_err(|e| format!("create prompt directory: {e}"))?;
    let mut prompts = Vec::new();

    for entry in fs::read_dir(dir).map_err(|e| format!("read prompt directory: {e}"))? {
        let entry = entry.map_err(|e| format!("read prompt entry: {e}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("read prompt file {}: {e}", path.display()))?;
        match parse_prompt_markdown(&content) {
            Ok(draft) => prompts.push(AssistantPrompt {
                path,
                title: draft.title,
                auto_submit: draft.auto_submit,
                prompt: draft.prompt,
            }),
            Err(e) => log::warn!("Skipping invalid assistant prompt {}: {e}", path.display()),
        }
    }

    prompts.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(prompts)
}

pub fn save_prompt(
    existing_path: Option<&Path>,
    draft: &AssistantPromptDraft,
) -> Result<AssistantPrompt, String> {
    validate_draft(draft)?;
    let dir = assistant_prompts_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create prompt directory: {e}"))?;

    let target_path = existing_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| unique_prompt_path(&dir, &draft.title));
    let markdown = serialize_prompt_markdown(draft)?;
    fs::write(&target_path, markdown)
        .map_err(|e| format!("write prompt file {}: {e}", target_path.display()))?;

    Ok(AssistantPrompt {
        path: target_path,
        title: draft.title.clone(),
        auto_submit: draft.auto_submit,
        prompt: draft.prompt.clone(),
    })
}

pub fn delete_prompt(path: &Path) -> Result<(), String> {
    fs::remove_file(path).map_err(|e| format!("delete prompt file {}: {e}", path.display()))
}

pub fn parse_prompt_markdown(input: &str) -> Result<AssistantPromptDraft, String> {
    let Some(rest) = input.strip_prefix("---\n") else {
        return Err("missing YAML frontmatter".to_string());
    };
    let Some((frontmatter, body)) = rest.split_once("\n---") else {
        return Err("missing closing YAML frontmatter delimiter".to_string());
    };
    let body = body.trim_start_matches('\n');
    let metadata: AssistantPromptMetadata = serde_yaml_ng::from_str(frontmatter)
        .map_err(|e| format!("parse prompt frontmatter: {e}"))?;
    let draft = AssistantPromptDraft {
        title: metadata.title,
        auto_submit: metadata.auto_submit,
        prompt: body.trim_end_matches('\n').to_string(),
    };
    validate_draft(&draft)?;
    Ok(draft)
}

pub fn serialize_prompt_markdown(draft: &AssistantPromptDraft) -> Result<String, String> {
    validate_draft(draft)?;
    let metadata = AssistantPromptMetadata {
        title: draft.title.clone(),
        auto_submit: draft.auto_submit,
    };
    let frontmatter = serde_yaml_ng::to_string(&metadata)
        .map_err(|e| format!("serialize prompt frontmatter: {e}"))?;
    Ok(format!(
        "---\n{}---\n\n{}\n",
        frontmatter,
        draft.prompt.trim_end()
    ))
}

pub fn safe_prompt_filename(title: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in title.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        slug.push_str("prompt");
    }
    format!("{slug}.md")
}

fn unique_prompt_path(dir: &Path, title: &str) -> PathBuf {
    let filename = safe_prompt_filename(title);
    let stem = filename.trim_end_matches(".md");
    let mut path = dir.join(&filename);
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stem}-{n}.md"));
        n += 1;
    }
    path
}

fn validate_draft(draft: &AssistantPromptDraft) -> Result<(), String> {
    if draft.title.trim().is_empty() {
        return Err("prompt title is required".to_string());
    }
    if draft.prompt.trim().is_empty() {
        return Err("prompt body is required".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_prompt_with_frontmatter() {
        let input = "---\ntitle: Debug build\nauto_submit: true\n---\n\nFix the build.";
        let parsed = parse_prompt_markdown(input).expect("parse prompt");
        assert_eq!(parsed.title, "Debug build");
        assert!(parsed.auto_submit);
        assert_eq!(parsed.prompt, "Fix the build.");
    }

    #[test]
    fn rejects_missing_frontmatter() {
        let err = parse_prompt_markdown("Fix the build.").expect_err("missing frontmatter fails");
        assert!(err.contains("frontmatter"));
    }

    #[test]
    fn serializes_prompt_with_frontmatter() {
        let draft = AssistantPromptDraft {
            title: "Debug build".to_string(),
            auto_submit: false,
            prompt: "Fix the build.".to_string(),
        };
        let output = serialize_prompt_markdown(&draft).expect("serialize prompt");
        assert!(output.starts_with("---\n"));
        assert!(output.contains("title: Debug build\n"));
        assert!(output.contains("auto_submit: false\n"));
        assert!(output.ends_with("Fix the build.\n"));
    }

    #[test]
    fn safe_filename_is_slugified() {
        assert_eq!(
            safe_prompt_filename(" Debug: build/fix! "),
            "debug-build-fix.md"
        );
        assert_eq!(safe_prompt_filename("!!!"), "prompt.md");
    }

    #[test]
    fn lists_only_markdown_prompts_sorted_by_title() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(
            temp.path().join("z.md"),
            "---\ntitle: Zed\nauto_submit: false\n---\n\nZ prompt",
        )
        .expect("write z");
        fs::write(
            temp.path().join("a.md"),
            "---\ntitle: Alpha\nauto_submit: true\n---\n\nA prompt",
        )
        .expect("write a");
        fs::write(temp.path().join("ignored.txt"), "nope").expect("write txt");

        let prompts = list_prompts_in_dir(temp.path()).expect("list prompts");

        assert_eq!(prompts.len(), 2);
        assert_eq!(prompts[0].title, "Alpha");
        assert_eq!(prompts[1].title, "Zed");
    }
}
