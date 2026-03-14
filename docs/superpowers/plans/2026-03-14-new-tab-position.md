# New Tab Position Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `new_tab_position` config option (`End` / `AfterActive`) so users can choose whether new tabs open at the end of the tab bar or immediately to the right of the active tab.

**Architecture:** Add a `NewTabPosition` enum to the config crate, thread it through the re-export chain, capture the active tab index before creation in `WindowState::new_tab()` and `WindowState::open_profile()`, then call the existing `move_tab_to_index()` inside the `Ok` arm if `AfterActive` is configured. All callers (keybinding, "+" button, snippet custom actions, profile picker) get the behavior for free because they route through those two functions.

**Tech Stack:** Rust 2024 edition, egui (settings UI), serde (`snake_case` enum serialization), serde_yaml_ng (config tests).

**Spec:** `docs/superpowers/specs/2026-03-14-new-tab-position-design.md`

---

## Chunk 1: Config — `NewTabPosition` enum

### Task 1: Add `NewTabPosition` enum to `par-term-config`

**Files:**
- Modify: `par-term-config/src/types/tab_bar.rs` — add enum after `StatusBarPosition`, before `#[cfg(test)]`
- Modify: `par-term-config/src/types/mod.rs` — add to `pub use tab_bar::{...}` re-export (line 52)
- Modify: `par-term-config/src/lib.rs` — add to the `pub use types::{...}` block that includes `TabBarMode, TabBarPosition` (around line 57–69)

- [ ] **Step 1: Write the failing tests**

  In `par-term-config/src/types/tab_bar.rs`, add a **new** `#[cfg(test)]` module after the existing `mod remote_format_tests { ... }` block (do not nest inside it):

  ```rust
  #[cfg(test)]
  mod new_tab_position_tests {
      use super::*;

      #[test]
      fn default_is_end() {
          assert_eq!(NewTabPosition::default(), NewTabPosition::End);
      }

      #[test]
      fn all_has_two_variants() {
          assert_eq!(NewTabPosition::all().len(), 2);
      }

      #[test]
      fn display_name_non_empty() {
          for v in NewTabPosition::all() {
              assert!(!v.display_name().is_empty());
          }
      }

      #[test]
      fn serde_round_trip() {
          let end: NewTabPosition = serde_json::from_str("\"end\"").unwrap();
          assert_eq!(end, NewTabPosition::End);
          let after: NewTabPosition = serde_json::from_str("\"after_active\"").unwrap();
          assert_eq!(after, NewTabPosition::AfterActive);
      }
  }
  ```

- [ ] **Step 2: Run tests to confirm they fail**

  ```bash
  cd par-term-config && cargo test new_tab_position 2>&1 | head -20
  ```
  Expected: compile error — `NewTabPosition` not found.

- [ ] **Step 3: Add the enum**

  In `par-term-config/src/types/tab_bar.rs`, add before the first `#[cfg(test)]` block (after `StatusBarPosition`):

  ```rust
  /// Controls where newly created tabs are inserted in the tab bar.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
  #[serde(rename_all = "snake_case")]
  pub enum NewTabPosition {
      /// Append to the end of the tab bar (default — existing behavior).
      #[default]
      End,
      /// Insert immediately to the right of the currently active tab.
      AfterActive,
  }

  impl NewTabPosition {
      /// Human-readable label for the settings UI combo box.
      pub fn display_name(&self) -> &'static str {
          match self {
              Self::End => "End of tab bar",
              Self::AfterActive => "After active tab",
          }
      }

      /// All variants in display order.
      pub fn all() -> &'static [Self] {
          &[Self::End, Self::AfterActive]
      }
  }
  ```

  Note: `display_name` takes `&self` (not `self`) to match the convention of every other enum in this file (`TabBarPosition::display_name`, `TabStyle::display_name`, etc.).

- [ ] **Step 4: Add to `types/mod.rs` re-export**

  In `par-term-config/src/types/mod.rs`, find the line:
  ```rust
  pub use tab_bar::{
      RemoteTabTitleFormat, StatusBarPosition, TabBarMode, TabBarPosition, TabStyle, TabTitleMode,
  ```
  Add `NewTabPosition` to this list (alphabetical order is not required but conventional):
  ```rust
  pub use tab_bar::{
      NewTabPosition, RemoteTabTitleFormat, StatusBarPosition, TabBarMode, TabBarPosition, TabStyle, TabTitleMode,
  ```

- [ ] **Step 5: Add to `lib.rs` re-export**

  In `par-term-config/src/lib.rs`, find the `pub use types::{...}` block that includes `StatusBarPosition, TabBarMode, TabBarPosition` (around line 57–69). Add `NewTabPosition` to that list:
  ```rust
  NewTabPosition, StatusBarPosition, TabBarMode, TabBarPosition, ...
  ```

- [ ] **Step 6: Run tests to confirm they pass**

  ```bash
  cd par-term-config && cargo test new_tab_position 2>&1
  ```
  Expected: 4 tests pass.

- [ ] **Step 7: Commit**

  ```bash
  git add par-term-config/src/types/tab_bar.rs \
          par-term-config/src/types/mod.rs \
          par-term-config/src/lib.rs
  git commit -m "feat(config): add NewTabPosition enum to tab_bar types"
  ```

---

### Task 2: Add `new_tab_position` field to `Config`

**Files:**
- Modify: `par-term-config/src/config/config_struct/mod.rs` — add field after `new_tab_shortcut_shows_profiles` (around line 1001)
- Modify: `src/config/mod.rs` — add `NewTabPosition` to the `pub use par_term_config::{...}` block that includes `TabBarMode, TabBarPosition` (around line 57)

- [ ] **Step 1: Write the failing tests**

  Find the `#[cfg(test)]` block in `par-term-config/src/config/config_struct/mod.rs`. If it exists, add these tests inside it. If not, check `par-term-config/tests/` for integration test files and add there.

  ```rust
  #[test]
  fn new_tab_position_defaults_to_end() {
      let config = Config::default();
      assert_eq!(config.new_tab_position, NewTabPosition::End);
  }

  #[test]
  fn new_tab_position_deserializes_from_yaml() {
      let yaml = "new_tab_position: after_active";
      let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
      assert_eq!(config.new_tab_position, NewTabPosition::AfterActive);
  }

  #[test]
  fn config_without_new_tab_position_deserializes_to_default() {
      // Existing configs that don't have this field must deserialize cleanly — zero migration.
      let yaml = "tab_inherit_cwd: true";
      let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
      assert_eq!(config.new_tab_position, NewTabPosition::End);
  }
  ```

  > **Note:** The config crate uses `serde_yaml_ng`, not `serde_yaml`. Use `serde_yaml_ng::from_str`. Use plain `NewTabPosition::End` (not `par_term_config::NewTabPosition::End`) — you're inside the crate; use `use crate::types::tab_bar::NewTabPosition;` or rely on the existing `use super::*` import if tests are in the same module.

- [ ] **Step 2: Run tests to confirm they fail**

  ```bash
  cd par-term-config && cargo test new_tab_position 2>&1 | head -20
  ```
  Expected: compile error — `new_tab_position` field not found on `Config`.

- [ ] **Step 3: Add the field**

  In `par-term-config/src/config/config_struct/mod.rs`, after `new_tab_shortcut_shows_profiles` (around line 1001):

  ```rust
      /// Where to insert new tabs in the tab bar.
      /// `end` appends to the end (default); `after_active` inserts right of the active tab.
      #[serde(default)]
      pub new_tab_position: NewTabPosition,
  ```

  Check the imports at the top of `config_struct/mod.rs` — look for where `TabBarMode` or `TabBarPosition` is imported. Add `NewTabPosition` to that same `use` statement.

- [ ] **Step 4: Add `NewTabPosition` to `src/config/mod.rs`**

  In `src/config/mod.rs`, find the `pub use par_term_config::{...}` block around line 57 that includes `TabBarMode, TabBarPosition`. Add `NewTabPosition` to it:
  ```rust
  pub use par_term_config::{
      AlertEvent, ..., NewTabPosition, ..., TabBarMode, TabBarPosition, ...
  };
  ```
  This makes `crate::config::NewTabPosition` available to the rest of the main crate.

- [ ] **Step 5: Run tests to confirm they pass**

  ```bash
  cd par-term-config && cargo test new_tab_position 2>&1
  ```
  Expected: 7 tests pass (4 from Task 1 + 3 from Task 2).

- [ ] **Step 6: Check the whole workspace still compiles**

  ```bash
  cargo check --workspace 2>&1 | head -30
  ```
  Expected: no errors.

- [ ] **Step 7: Commit**

  ```bash
  git add par-term-config/src/config/config_struct/mod.rs \
          src/config/mod.rs
  git commit -m "feat(config): add new_tab_position field to Config"
  ```

---

## Chunk 2: Logic — positioning in `WindowState`

### Task 3: Apply positioning in `WindowState::new_tab()`

**Files:**
- Modify: `src/app/tab_ops/lifecycle.rs` — `new_tab()` function

The function has this shape:
```rust
pub fn new_tab(&mut self) {
    // max-tabs guard (early return) ...
    let old_tab_count = self.tab_manager.tab_count();
    let grid_size = ...;

    match self.tab_manager.new_tab(
        &self.config,
        Arc::clone(&self.runtime),
        self.config.tab_inherit_cwd,
        grid_size,
    ) {
        Ok(tab_id) => {
            // ... tab bar resize logic ...
            // ... refresh task start ...
        }
        Err(e) => { ... }
    }
}
```

- [ ] **Step 1: Capture prior active index**

  Before the `match self.tab_manager.new_tab(...)` call, add:
  ```rust
  // Capture BEFORE creation — tab_manager switches active to the new tab inside new_tab()
  let prior_active_idx = self.tab_manager.active_tab_index();
  ```

- [ ] **Step 2: Apply the move in the `Ok` arm**

  At the very top of the `Ok(tab_id)` arm (before the tab bar resize logic), add:
  ```rust
  // Reposition new tab if configured
  if self.config.new_tab_position == crate::config::NewTabPosition::AfterActive {
      if let Some(idx) = prior_active_idx {
          self.tab_manager.move_tab_to_index(tab_id, idx + 1);
      }
  }
  ```

  > **Why `crate::config::NewTabPosition`?** Other config types in this file are referenced as `crate::config::AlertEvent::NewTab` (line 148). Follow the same pattern — no new `use` import needed.
  >
  > **Why capture before the call?** `tab_manager.new_tab()` internally calls `set_active_tab(Some(new_id))`, so `active_tab_index()` after the call returns the new tab's index, not the previously-active tab's index.

- [ ] **Step 3: Build to verify no compile errors**

  ```bash
  cargo check --workspace 2>&1 | head -30
  ```
  Expected: no errors.

- [ ] **Step 4: Smoke-test manually**

  ```bash
  make run
  ```
  - Default config (`new_tab_position: end` or absent): open several tabs → new tabs appear at end. ✓
  - Add `new_tab_position: after_active` to `~/.config/par-term/config.yaml`, restart → switch to tab 2, Cmd+T → new tab appears at position 3. ✓

- [ ] **Step 5: Commit**

  ```bash
  git add src/app/tab_ops/lifecycle.rs
  git commit -m "feat(tabs): apply new_tab_position in WindowState::new_tab()"
  ```

---

### Task 4: Apply positioning in `WindowState::open_profile()`

**Files:**
- Modify: `src/app/tab_ops/profile_ops.rs` — `open_profile()` function (line 14)

Same pattern as Task 3. `new_tab_from_profile()` returns `Result<TabId>` (same type as `new_tab()`), so the `Ok(tab_id)` arm gives a `TabId` suitable for `move_tab_to_index`.

- [ ] **Step 1: Capture prior active index**

  Before the `match self.tab_manager.new_tab_from_profile(...)` call, add:
  ```rust
  let prior_active_idx = self.tab_manager.active_tab_index();
  ```

- [ ] **Step 2: Apply the move in the `Ok` arm**

  At the top of the `Ok(tab_id)` arm:
  ```rust
  if self.config.new_tab_position == crate::config::NewTabPosition::AfterActive {
      if let Some(idx) = prior_active_idx {
          self.tab_manager.move_tab_to_index(tab_id, idx + 1);
      }
  }
  ```

- [ ] **Step 3: Build to verify no compile errors**

  ```bash
  cargo check --workspace 2>&1 | head -30
  ```
  Expected: no errors.

- [ ] **Step 4: Smoke-test manually**

  With `new_tab_position: after_active`: open a named profile from the profile picker → new tab appears right of the active tab. ✓

- [ ] **Step 5: Commit**

  ```bash
  git add src/app/tab_ops/profile_ops.rs
  git commit -m "feat(tabs): apply new_tab_position in WindowState::open_profile()"
  ```

---

## Chunk 3: Settings UI

### Task 5: Add combo box to tab bar behavior settings

**Files:**
- Modify: `par-term-settings-ui/src/window_tab/tab_bar_behavior.rs` — add combo box after `tab_inherit_cwd` checkbox (around line 309)
- Modify: `par-term-settings-ui/src/window_tab/mod.rs` — add keywords to `keywords()` function

The pattern for enums with `display_name()` + `all()` is already used for `TabBarPosition` (lines 197–213 of `tab_bar_behavior.rs`):

```rust
ui.horizontal(|ui| {
    ui.label("Position:");
    let current_position = settings.config.tab_bar_position;
    egui::ComboBox::from_id_salt("window_tab_bar_position")
        .selected_text(current_position.display_name())
        .show_ui(ui, |ui| {
            for &pos in TabBarPosition::all() {
                if ui
                    .selectable_value(
                        &mut settings.config.tab_bar_position,
                        pos,
                        pos.display_name(),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            }
        });
});
```

- [ ] **Step 1: Add the import**

  At the top of `par-term-settings-ui/src/window_tab/tab_bar_behavior.rs`, find where `TabBarPosition` is imported from `par_term_config`. Add `NewTabPosition` next to it:
  ```rust
  use par_term_config::{..., NewTabPosition, ..., TabBarPosition, ...};
  ```

- [ ] **Step 2: Add the combo box**

  After the `tab_inherit_cwd` checkbox block (after its closing `}` around line 309) and before the `show_profile_drawer_button` checkbox, add:

  ```rust
  ui.add_space(4.0);
  ui.horizontal(|ui| {
      ui.label("New tab position:");
      egui::ComboBox::from_id_salt("window_new_tab_position")
          .selected_text(settings.config.new_tab_position.display_name())
          .show_ui(ui, |ui| {
              for &pos in NewTabPosition::all() {
                  if ui
                      .selectable_value(
                          &mut settings.config.new_tab_position,
                          pos,
                          pos.display_name(),
                      )
                      .on_hover_text(match pos {
                          NewTabPosition::End => {
                              "New tabs are added to the end of the tab bar"
                          }
                          NewTabPosition::AfterActive => {
                              "New tabs open immediately to the right of the active tab"
                          }
                      })
                      .changed()
                  {
                      settings.has_changes = true;
                      *changes_this_frame = true;
                  }
              }
          });
  });
  ```

- [ ] **Step 3: Add search keywords**

  In `par-term-settings-ui/src/window_tab/mod.rs`, find the `keywords()` function. Add to the returned slice:
  ```rust
  "new tab position",
  "after active",
  "tab order",
  "insert tab",
  ```

- [ ] **Step 4: Build the settings UI crate**

  ```bash
  cargo check -p par-term-settings-ui 2>&1 | head -30
  ```
  Expected: no errors.

- [ ] **Step 5: Full workspace build**

  ```bash
  make build 2>&1 | tail -10
  ```
  Expected: clean build.

- [ ] **Step 6: Smoke-test the settings UI**

  ```bash
  make run
  ```
  Open Settings → Window tab:
  - Scroll to "New tab position" combo box. ✓
  - Default selection shows "End of tab bar". ✓
  - Switch to "After active tab", close settings, reopen → selection persists. ✓
  - Search for "tab position" in settings search → control surfaces. ✓

- [ ] **Step 7: Commit**

  ```bash
  git add par-term-settings-ui/src/window_tab/tab_bar_behavior.rs \
          par-term-settings-ui/src/window_tab/mod.rs
  git commit -m "feat(settings): add new tab position combo box to window settings"
  ```

---

## Chunk 4: Final verification

### Task 6: Full check and integration test

- [ ] **Step 1: Run all tests**

  ```bash
  make test 2>&1 | tail -20
  ```
  Expected: all tests pass.

- [ ] **Step 2: Run full CI checks**

  ```bash
  make ci 2>&1 | tail -20
  ```
  Expected: format, lint, tests all pass.

- [ ] **Step 3: Manual end-to-end verification**

  ```bash
  make run
  ```

  Test matrix:

  | Config | Action | Expected |
  |--------|--------|----------|
  | `new_tab_position: end` (default) | Open 3 tabs, switch to tab 1, Cmd+T | New tab at position 4 (end) |
  | `new_tab_position: after_active` | Open 3 tabs, switch to tab 2, Cmd+T | New tab at position 3 |
  | `new_tab_position: after_active` | Open 3 tabs, switch to tab 2, open profile | New tab at position 3 |
  | `new_tab_position: after_active` | Close a tab, reopen (session undo) | Tab restores to original index |
  | `new_tab_position: after_active` | Duplicate tab | Duplicate opens right of source (unchanged) |
  | `new_tab_position: after_active` | First tab in window (no active tab) | `prior_active_idx` is None, tab opens normally |

- [ ] **Step 4: Verify config round-trip**

  Add `new_tab_position: after_active` to `~/.config/par-term/config.yaml`. Restart par-term. Open Settings → Window → confirm "After active tab" is selected. Change to "End of tab bar", save, restart — confirm `end` is in the config file.

- [ ] **Step 5: Final commit (if any stray changes)**

  ```bash
  git status
  # If clean: done.
  ```

- [ ] **Step 6: Update task tracking**

  Mark all tasks completed.
