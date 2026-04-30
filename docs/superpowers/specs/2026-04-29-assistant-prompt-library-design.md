# Assistant Prompt Library Design

## Goal

Add an Assistant-specific prompt library so users can save reusable prompts, choose them from the Assistant chat input, and decide per prompt whether it should auto-submit or only load into the input editor.

## Requirements

- Prompts are Assistant-panel specific and do not reuse terminal Snippets & Actions.
- Prompts are stored as Markdown files under the par-term user config directory:
  - `Config::config_dir()/assistant-prompts/`
- Each prompt file uses YAML frontmatter for metadata and Markdown body for the prompt:

```markdown
---
title: Debug build failure
auto_submit: false
---

Investigate the current build failure and propose a minimal fix.
```

- Prompt metadata contains only:
  - `title: String`
  - `auto_submit: bool`
- Prompt body is the Markdown content after frontmatter.
- Selecting a prompt with `auto_submit: false` replaces the entire current Assistant input.
- Selecting a prompt with `auto_submit: true` sends the prompt immediately.
- The Settings > Assistant panel provides create, edit, and delete controls for the prompt library.
- The Assistant chat input area provides a Prompt Library button that opens the saved prompt list.

## Architecture

### Storage module

Create an Assistant prompt library module in the main crate, for example `src/ai_inspector/prompt_library.rs`. It owns file I/O and parsing so UI code stays simple.

Responsibilities:

- Ensure the prompt directory exists when listing or saving prompts.
- List `.md` files from `Config::config_dir()/assistant-prompts/`.
- Parse YAML frontmatter into prompt metadata.
- Treat the remaining Markdown content as the prompt body.
- Save prompts back to `.md` files with generated safe filenames.
- Delete prompt files by path.

Prompt records should include the resolved file path so edit/delete operations can update the correct file even if titles are duplicated.

### Settings UI

Add a Prompt Library section to `par-term-settings-ui/src/ai_inspector_tab/`.

The settings UI crate should not directly depend on main-crate internals. Use trait/callback-style boundaries already present in the settings UI where needed, or keep pure UI state in `SettingsUI` and return settings-window actions for the main app to perform file operations.

The section should show:

- A list of prompt titles.
- Each row shows the auto-submit state and Edit/Delete buttons.
- An Add Prompt button.
- An inline edit form with:
  - Title single-line field.
  - Prompt multi-line field.
  - Auto-submit checkbox.
  - Save and Cancel buttons.

### Assistant panel UI

Add a Prompt Library button next to the Assistant chat input controls in `src/ai_inspector/panel/chat_view.rs`.

The button opens an egui dropdown/menu containing saved prompt titles. On selection, the panel returns an `InspectorAction` describing the selected prompt behavior:

- `LoadPrompt(String)` for non-auto-submit prompts.
- `SendPrompt(String)` for auto-submit prompts.

The action handler for `LoadPrompt` sets `self.overlay_ui.ai_inspector.chat.input` to the selected prompt body, replacing any existing input.

### Data flow

1. Prompt files are read from disk into an in-memory list owned by window/UI state.
2. Settings edits create/update/delete Markdown files in the prompt directory.
3. The Assistant panel receives the current prompt list during rendering.
4. User selects a prompt from the chat input prompt-library menu.
5. The panel emits an action.
6. The window action handler either replaces chat input or sends the prompt.

## Error handling

- Invalid or unreadable prompt files should not crash the app.
- The list operation should skip invalid files and expose a concise error message for the settings section or debug log.
- Missing frontmatter should be treated as invalid because title and auto-submit are required metadata.
- Empty titles should be rejected in the settings editor.
- Empty prompt bodies should be rejected in the settings editor.
- Filename generation should sanitize title characters and avoid overwriting another prompt unless editing that prompt.

## Testing

Use TDD for implementation.

Tests should cover:

- Parsing a Markdown prompt with valid frontmatter.
- Rejecting missing/invalid frontmatter.
- Serializing a prompt to frontmatter + Markdown body.
- Safe filename generation from a title.
- Listing only `.md` prompt files.
- Non-auto-submit selection replaces chat input.
- Auto-submit selection routes through the existing send-prompt path.

## Out of Scope

- Sharing prompts with terminal snippets.
- Prompt categories, descriptions, tags, search, or keybindings.
- Import/export beyond direct Markdown files in the prompt folder.
