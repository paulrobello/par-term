# Assistant Prompt Library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an Assistant-specific Markdown-backed prompt library with per-prompt auto-submit behavior.

**Architecture:** Put prompt parsing/storage in `par-term-config` so both the main app and `par-term-settings-ui` can use it without a reverse dependency. Settings UI manages prompt files directly through this shared module, while the Assistant panel renders the in-memory prompt list and emits existing/new inspector actions.

**Tech Stack:** Rust 2024, egui, serde/serde_yaml_ng, existing `make test` / `make checkall` verification.

---

## File Structure

- Create `par-term-config/src/assistant_prompts.rs`
  - Defines `AssistantPrompt`, `AssistantPromptMetadata`, `AssistantPromptDraft`, parser/serializer, safe filename generation, list/save/delete helpers.
- Modify `par-term-config/src/lib.rs`
  - Export `assistant_prompts` module and key types.
- Modify `par-term-settings-ui/src/settings_ui/mod.rs`
  - Add prompt-library UI state fields.
- Modify `par-term-settings-ui/src/settings_ui/state.rs`
  - Initialize prompt-library UI state from disk.
- Create `par-term-settings-ui/src/ai_inspector_tab/prompt_library.rs`
  - Render Prompt Library settings section and call shared storage helpers.
- Modify `par-term-settings-ui/src/ai_inspector_tab/mod.rs`
  - Include the new section and keywords.
- Modify `src/ai_inspector/panel/mod.rs`
  - Store loaded prompts in `AIInspectorPanel`, load on creation, pass through render body.
- Modify `src/ai_inspector/panel/panel_body.rs`
  - Account for prompt-library button width if connected.
- Modify `src/ai_inspector/panel/chat_view.rs`
  - Add Prompt Library menu next to the chat input.
- Modify `src/ai_inspector/panel/types.rs`
  - Add `LoadPrompt(String)` action.
- Modify `src/app/window_state/action_handlers/inspector.rs`
  - Handle `LoadPrompt` by replacing chat input.

---

### Task 1: Shared prompt-library storage

**Files:**
- Create: `par-term-config/src/assistant_prompts.rs`
- Modify: `par-term-config/src/lib.rs`

- [ ] **Step 1: Write failing tests for parsing, serialization, filenames, and listing**

Add tests inside `par-term-config/src/assistant_prompts.rs` before implementation:

```rust
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
        assert_eq!(safe_prompt_filename(" Debug: build/fix! "), "debug-build-fix.md");
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
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test -p par-term-config assistant_prompts -- --nocapture
```

Expected: fails to compile because `assistant_prompts` and related types/functions do not exist.

- [ ] **Step 3: Implement shared storage module**

Create `par-term-config/src/assistant_prompts.rs` with:

```rust
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
    let body = body.strip_prefix('\n').unwrap_or(body);
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
    Ok(format!("---\n{}---\n\n{}\n", frontmatter, draft.prompt.trim_end()))
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
```

Modify `par-term-config/src/lib.rs`:

```rust
pub mod assistant_prompts;
```

and re-export:

```rust
pub use assistant_prompts::{
    AssistantPrompt, AssistantPromptDraft, assistant_prompts_dir, delete_prompt, list_prompts,
    parse_prompt_markdown, safe_prompt_filename, save_prompt, serialize_prompt_markdown,
};
```

- [ ] **Step 4: Run tests and verify GREEN**

Run:

```bash
cargo test -p par-term-config assistant_prompts -- --nocapture
```

Expected: all assistant prompt tests pass.

- [ ] **Step 5: Commit**

```bash
git add par-term-config/src/assistant_prompts.rs par-term-config/src/lib.rs
git commit -m "feat: add assistant prompt library storage"
```

---

### Task 2: Settings > Assistant prompt-library editor

**Files:**
- Modify: `par-term-settings-ui/src/settings_ui/mod.rs`
- Modify: `par-term-settings-ui/src/settings_ui/state.rs`
- Create: `par-term-settings-ui/src/ai_inspector_tab/prompt_library.rs`
- Modify: `par-term-settings-ui/src/ai_inspector_tab/mod.rs`

- [ ] **Step 1: Write failing settings-state tests**

Add tests in `par-term-settings-ui/src/settings_ui/state.rs` tests module or create one if needed:

```rust
#[cfg(test)]
mod assistant_prompt_tests {
    use super::*;
    use par_term_config::Config;

    #[test]
    fn settings_ui_initializes_empty_prompt_library_state() {
        let settings = SettingsUI::new(Config::default());

        assert!(settings.assistant_prompts.is_empty());
        assert!(settings.assistant_prompt_error.is_none());
        assert_eq!(settings.editing_assistant_prompt_index, None);
        assert!(!settings.adding_new_assistant_prompt);
        assert!(!settings.temp_assistant_prompt_auto_submit);
    }
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test -p par-term-settings-ui assistant_prompt_tests -- --nocapture
```

Expected: fails to compile because settings fields do not exist.

- [ ] **Step 3: Add settings state fields and initialization**

In `par-term-settings-ui/src/settings_ui/mod.rs`, add fields to `SettingsUI`:

```rust
// Assistant prompt library state
pub assistant_prompts: Vec<par_term_config::AssistantPrompt>,
pub assistant_prompt_error: Option<String>,
pub editing_assistant_prompt_index: Option<usize>,
pub adding_new_assistant_prompt: bool,
pub temp_assistant_prompt_title: String,
pub temp_assistant_prompt_body: String,
pub temp_assistant_prompt_auto_submit: bool,
```

In `SettingsUI::new` in `state.rs`, initialize using:

```rust
let (assistant_prompts, assistant_prompt_error) = match par_term_config::list_prompts() {
    Ok(prompts) => (prompts, None),
    Err(e) => (Vec::new(), Some(e)),
};
```

and fields:

```rust
assistant_prompts,
assistant_prompt_error,
editing_assistant_prompt_index: None,
adding_new_assistant_prompt: false,
temp_assistant_prompt_title: String::new(),
temp_assistant_prompt_body: String::new(),
temp_assistant_prompt_auto_submit: false,
```

- [ ] **Step 4: Add Prompt Library settings section**

Create `par-term-settings-ui/src/ai_inspector_tab/prompt_library.rs` with a small inline editor. It must:

- Show `settings.assistant_prompt_error` if present.
- List `settings.assistant_prompts` with title, auto-submit label, Edit/Delete buttons.
- Use deferred mutations for edit/delete while iterating.
- Add `+ Add Prompt` button.
- Save with `par_term_config::save_prompt(existing_path, &AssistantPromptDraft { ... })`.
- Delete with `par_term_config::delete_prompt(&prompt.path)`.
- Refresh `settings.assistant_prompts` after save/delete via `par_term_config::list_prompts()`.
- Reject empty title/body by setting `assistant_prompt_error`.

- [ ] **Step 5: Wire section into Assistant tab**

Modify `par-term-settings-ui/src/ai_inspector_tab/mod.rs`:

```rust
mod prompt_library;
```

Call after agent section:

```rust
prompt_library::show_prompt_library_section(ui, settings, collapsed);
```

Add keywords: `prompt`, `library`, `saved prompt`, `auto submit`, `markdown`, `frontmatter`.

- [ ] **Step 6: Run tests and verify GREEN**

Run:

```bash
cargo test -p par-term-settings-ui assistant_prompt_tests -- --nocapture
```

Expected: tests pass.

- [ ] **Step 7: Commit**

```bash
git add par-term-settings-ui/src/settings_ui/mod.rs par-term-settings-ui/src/settings_ui/state.rs par-term-settings-ui/src/ai_inspector_tab/mod.rs par-term-settings-ui/src/ai_inspector_tab/prompt_library.rs
git commit -m "feat: add assistant prompt library settings"
```

---

### Task 3: Assistant panel prompt-library picker

**Files:**
- Modify: `src/ai_inspector/panel/mod.rs`
- Modify: `src/ai_inspector/panel/panel_body.rs`
- Modify: `src/ai_inspector/panel/chat_view.rs`
- Modify: `src/ai_inspector/panel/types.rs`
- Modify: `src/app/window_state/action_handlers/inspector.rs`

- [ ] **Step 1: Write failing panel/action tests**

Add tests in `src/ai_inspector/panel/mod.rs` tests:

```rust
#[test]
fn inspector_panel_loads_prompt_library_on_creation() {
    let config = Config::default();
    let panel = AIInspectorPanel::new(&config);
    assert!(panel.assistant_prompts_error.is_none());
}

#[test]
fn load_prompt_action_carries_prompt_body() {
    let action = InspectorAction::LoadPrompt("hello".to_string());
    assert!(matches!(action, InspectorAction::LoadPrompt(text) if text == "hello"));
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test inspector_panel_loads_prompt_library_on_creation load_prompt_action_carries_prompt_body -- --nocapture
```

Expected: fails to compile because panel fields/action do not exist.

- [ ] **Step 3: Add panel state and action**

In `src/ai_inspector/panel/types.rs`, add:

```rust
LoadPrompt(String),
```

In `AIInspectorPanel` add:

```rust
pub assistant_prompts: Vec<par_term_config::AssistantPrompt>,
pub assistant_prompts_error: Option<String>,
```

Initialize in `AIInspectorPanel::new` by calling `par_term_config::list_prompts()`.

- [ ] **Step 4: Render prompt library button in chat input**

In `render_chat_input`, reserve room for two small buttons plus the prompt button. Add a `Prompt Library`/`Prompts` button using `ui.menu_button(...)` before Send/Clear. For each prompt:

```rust
if ui.button(&prompt.title).clicked() {
    if prompt.auto_submit {
        action = InspectorAction::SendPrompt(prompt.prompt.clone());
    } else {
        action = InspectorAction::LoadPrompt(prompt.prompt.clone());
    }
    ui.close();
}
```

If the list is empty, show disabled text `No prompts saved`.

- [ ] **Step 5: Handle LoadPrompt action**

In `src/app/window_state/action_handlers/inspector.rs`, add match arm:

```rust
InspectorAction::LoadPrompt(text) => {
    self.overlay_ui.ai_inspector.chat.input = text;
    self.focus_state.needs_redraw = true;
}
```

- [ ] **Step 6: Run tests and verify GREEN**

Run:

```bash
cargo test inspector_panel_loads_prompt_library_on_creation load_prompt_action_carries_prompt_body -- --nocapture
```

Expected: tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/ai_inspector/panel/mod.rs src/ai_inspector/panel/panel_body.rs src/ai_inspector/panel/chat_view.rs src/ai_inspector/panel/types.rs src/app/window_state/action_handlers/inspector.rs
git commit -m "feat: add assistant prompt picker"
```

---

### Task 4: Documentation and verification

**Files:**
- Modify: `docs/ASSISTANT_PANEL.md`

- [ ] **Step 1: Update docs**

Add a short `Prompt Library` subsection under `ACP Agent Chat` explaining:

- Prompts live in `~/.config/par-term/assistant-prompts/`.
- Files use YAML frontmatter plus Markdown body.
- `auto_submit: false` loads into input.
- `auto_submit: true` sends immediately.
- Prompts can be managed in Settings > Assistant > Prompt Library.

- [ ] **Step 2: Run focused tests**

Run:

```bash
cargo test -p par-term-config assistant_prompts -- --nocapture
cargo test -p par-term-settings-ui assistant_prompt_tests -- --nocapture
cargo test inspector_panel_loads_prompt_library_on_creation load_prompt_action_carries_prompt_body -- --nocapture
```

Expected: all pass.

- [ ] **Step 3: Run repository verification**

Run:

```bash
make checkall
```

Expected: format, lint, typecheck, and tests pass.

- [ ] **Step 4: Commit**

```bash
git add docs/ASSISTANT_PANEL.md
git commit -m "docs: document assistant prompt library"
```
