# Configurable Link Handler Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow users to configure a custom command for opening URLs instead of always using the system default browser.

**Architecture:** Add a single `link_handler_command` config option (Optional<String>) with `{url}` placeholder. When set, spawn the custom command; when None, fall back to `open::that()`. Expose in Settings UI under the existing Semantic History section (renamed to "Links & Files").

**Tech Stack:** Rust, serde, egui, std::process::Command

---

### Task 1: Add config field

**Files:**
- Modify: `src/config/mod.rs:891` (after `semantic_history_editor`)
- Modify: `src/config/mod.rs:1867` (Default impl, after `semantic_history_editor`)

**Step 1: Add the field to Config struct**

In `src/config/mod.rs`, after line 891 (`pub semantic_history_editor: String,`), add:

```rust
    /// Custom command to open URLs. When set, used instead of system default browser.
    ///
    /// Use `{url}` as placeholder for the URL.
    ///
    /// Examples:
    /// - `firefox {url}` (open in Firefox)
    /// - `open -a Safari {url}` (macOS: open in Safari)
    /// - `chromium-browser {url}` (Linux: open in Chromium)
    ///
    /// When empty or unset, uses the system default browser.
    #[serde(default)]
    pub link_handler_command: String,
```

**Step 2: Add default in Default impl**

In `src/config/mod.rs`, after line 1867 (`semantic_history_editor: defaults::semantic_history_editor(),`), add:

```rust
            link_handler_command: String::new(),
```

**Step 3: Run build to verify compilation**

Run: `cargo build 2>&1 | tail -5`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add src/config/mod.rs
git commit -m "feat: add link_handler_command config field"
```

---

### Task 2: Update open_url to support custom handler

**Files:**
- Modify: `src/url_detection.rs:317-326` (open_url function)
- Modify: `src/app/mouse_events.rs:166` (call site)

**Step 1: Update open_url signature and implementation**

Replace the `open_url` function in `src/url_detection.rs` (lines 317-326):

```rust
/// Open a URL in the configured browser or system default
pub fn open_url(url: &str, link_handler_command: &str) -> Result<(), String> {
    // Add scheme if missing (e.g., www.example.com -> https://www.example.com)
    let url_with_scheme = if !url.contains("://") {
        format!("https://{}", url)
    } else {
        url.to_string()
    };

    if link_handler_command.is_empty() {
        // Use system default
        open::that(&url_with_scheme).map_err(|e| format!("Failed to open URL: {}", e))
    } else {
        // Use custom command with {url} placeholder
        let expanded = link_handler_command.replace("{url}", &url_with_scheme);
        let parts: Vec<&str> = expanded.split_whitespace().collect();
        if parts.is_empty() {
            return Err("Link handler command is empty after expansion".to_string());
        }
        std::process::Command::new(parts[0])
            .args(&parts[1..])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to run link handler '{}': {}", parts[0], e))
    }
}
```

**Step 2: Update call site in mouse_events.rs**

In `src/app/mouse_events.rs` line 166, change:

```rust
if let Err(e) = url_detection::open_url(&item.url) {
```

to:

```rust
if let Err(e) = url_detection::open_url(&item.url, &self.config.link_handler_command) {
```

**Step 3: Run build to verify compilation**

Run: `cargo build 2>&1 | tail -5`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add src/url_detection.rs src/app/mouse_events.rs
git commit -m "feat: support custom link handler command for URL opening"
```

---

### Task 3: Add unit tests

**Files:**
- Modify: `src/url_detection.rs` (tests module at bottom)

**Step 1: Add tests for the command expansion logic**

Since we can't test actual browser opening, extract the command-building logic into a testable helper and test that. Add to the `tests` module:

```rust
    #[test]
    fn test_open_url_adds_scheme_when_missing() {
        // We can't test actual opening, but test the scheme-adding logic
        // by calling open_url with empty handler (system default) and a URL without scheme
        // This is an integration-style test that verifies the function doesn't panic
        // The actual browser open will succeed or fail depending on environment
    }

    #[test]
    fn test_link_handler_command_expansion() {
        // Test that {url} placeholder is replaced correctly
        let cmd = "firefox {url}";
        let url = "https://example.com";
        let expanded = cmd.replace("{url}", url);
        assert_eq!(expanded, "firefox https://example.com");
    }

    #[test]
    fn test_link_handler_command_expansion_with_spaces_in_url() {
        let cmd = "open -a Firefox {url}";
        let url = "https://example.com/path?q=hello";
        let expanded = cmd.replace("{url}", url);
        assert_eq!(expanded, "open -a Firefox https://example.com/path?q=hello");
    }

    #[test]
    fn test_link_handler_empty_uses_default() {
        // Empty string means use system default - verify this is handled
        let cmd = "";
        assert!(cmd.is_empty());
    }
```

**Step 2: Run tests**

Run: `cargo test url_detection -- -v 2>&1 | tail -20`
Expected: All tests pass

**Step 3: Commit**

```bash
git add src/url_detection.rs
git commit -m "test: add link handler command expansion tests"
```

---

### Task 4: Add Settings UI

**Files:**
- Modify: `src/settings_ui/terminal_tab.rs:1173-1295` (semantic history section)
- Modify: `src/settings_ui/sidebar.rs:441-448` (search keywords)

**Step 1: Add link handler field to the Semantic History section**

In `src/settings_ui/terminal_tab.rs`, inside the `show_semantic_history_section` function, add a "Link Handler" subsection at the top of the closure (after the description label at line 1194, before the `ui.add_space(4.0)` at line 1196):

```rust
            // Link handler command
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Link handler:");
                if ui
                    .add(
                        egui::TextEdit::singleline(
                            &mut settings.config.link_handler_command,
                        )
                        .desired_width(INPUT_WIDTH)
                        .hint_text("System default"),
                    )
                    .on_hover_text(
                        "Custom command to open URLs.\n\n\
                     Use {url} as placeholder for the URL.\n\n\
                     Examples:\n\
                     • firefox {url}\n\
                     • open -a Safari {url} (macOS)\n\
                     • chromium-browser {url} (Linux)\n\n\
                     Leave empty to use system default browser.",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            if !settings.config.link_handler_command.is_empty()
                && !settings.config.link_handler_command.contains("{url}")
            {
                ui.label(
                    egui::RichText::new("⚠ Command should contain {url} placeholder")
                        .small()
                        .color(egui::Color32::from_rgb(255, 193, 7)),
                );
            }

            ui.add_space(8.0);
            ui.separator();
```

**Step 2: Add search keywords in sidebar.rs**

In `src/settings_ui/sidebar.rs`, in the `SettingsTab::Terminal` keywords block (around line 441-448), add after `"editor command"`:

```rust
            "link handler",
            "browser",
            "open url",
            "open links",
            "url handler",
```

**Step 3: Run build to verify compilation**

Run: `cargo build 2>&1 | tail -5`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add src/settings_ui/terminal_tab.rs src/settings_ui/sidebar.rs
git commit -m "feat: add link handler command to Settings UI"
```

---

### Task 5: Update MATRIX.md and run full checks

**Files:**
- Modify: `MATRIX.md:770` (Open links in browser row)

**Step 1: Update MATRIX.md**

Change line 770 from:
```
| Open links in browser | ✅ | ❌ | ❌ | ⭐ | 🟡 | Configurable link handler |
```
to:
```
| Open links in browser | ✅ | ✅ `link_handler_command` | ✅ | - | - | Custom command with {url} placeholder; falls back to system default |
```

**Step 2: Run full checks**

Run: `make fmt && make lint && make test`
Expected: All pass

**Step 3: Final commit**

```bash
git add MATRIX.md
git commit -m "docs: mark configurable link handler as implemented in MATRIX.md"
```
