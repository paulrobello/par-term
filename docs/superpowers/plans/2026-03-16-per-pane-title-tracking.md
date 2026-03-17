# Per-Pane Title Tracking Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Each pane stores its own last-known title so the tab bar always shows the focused pane's title, with instant display on focus switch.

**Architecture:** Add `title` and `has_default_title` fields to `Pane`. Restructure `Tab::update_title()` to iterate all panes (updating each from its own terminal's OSC state) then derive `tab.title` from the focused pane. Route all existing direct `tab.title` writes through `set_title()` which now syncs the focused pane.

**Tech Stack:** Rust 2024, tokio async, `try_write()` non-blocking lock pattern.

**Spec:** `docs/superpowers/specs/2026-03-16-per-pane-title-tracking-design.md`

---

## Chunk 1: Pane data model + `update_title()` + `set_title()` + `set_default_title()`

### Task 1: Add `title` and `has_default_title` fields to `Pane`

**Files:**
- Modify: `src/pane/types/pane.rs`

- [ ] **Step 1: Add fields to the `Pane` struct**

  In `src/pane/types/pane.rs`, after the `background` field (line 74), add:

  ```rust
  /// Last-known title from OSC sequences or CWD fallback (empty if never set)
  pub title: String,
  /// True when pane still has its default/fallback title
  pub has_default_title: bool,
  ```

- [ ] **Step 2: Initialize fields in `Pane::new()` constructor (line ~125)**

  Add to the `Ok(Self { ... })` block in `Pane::new()`:

  ```rust
  title: String::new(),
  has_default_title: true,
  ```

- [ ] **Step 3: Initialize fields in `Pane::new_with_command()` (line ~190)**

  Same addition in the `Ok(Self { ... })` block:

  ```rust
  title: String::new(),
  has_default_title: true,
  ```

- [ ] **Step 4: Initialize fields in `Pane::new_wrapping_terminal()` (line ~236)**

  Same addition in the `Self { ... }` block:

  ```rust
  title: String::new(),
  has_default_title: true,
  ```

- [ ] **Step 5: Initialize fields in `Pane::new_for_tmux()` (line ~286)**

  Same addition in the `Ok(Self { ... })` block:

  ```rust
  title: String::new(),
  has_default_title: true,
  ```

- [ ] **Step 6: Verify it compiles**

  ```bash
  cargo check -p par-term 2>&1 | head -30
  ```

  Expected: No errors about missing fields. (There will be unused-field warnings for `title`/`has_default_title` until the next task — that is fine.)

---

### Task 2: Restructure `Tab::update_title()`

**Files:**
- Modify: `src/tab/profile_tracking.rs` (lines 32–103)

The current `update_title()` reads from `self.terminal` only. Replace the entire body
(after the `user_named` early-exit) with the two-phase all-pane loop.

- [ ] **Step 1: Replace `update_title()` body**

  Replace lines 38–103 of `src/tab/profile_tracking.rs` (everything after `pub fn update_title(...)` signature) with:

  ```rust
      // User-named tabs are static — never auto-update
      if self.user_named {
          return;
      }

      // Step 2 — Snapshot focused pane ID before the mutable borrow.
      // This avoids a Rust borrow-checker conflict: all_panes_mut() takes &mut pane_manager,
      // and we must re-borrow it immutably in Step 4 after the loop ends.
      let focused_id = self.pane_manager.as_ref().and_then(|pm| pm.focused_pane_id());

      // Step 3 — Iterate all panes and update each one's title from its own terminal.
      // try_write: intentional — called every frame; blocking would stall rendering.
      // On contention: skip that pane this frame, no data loss.
      if let Some(pm) = self.pane_manager.as_mut() {
          for pane in pm.all_panes_mut() {
              if let Ok(term) = pane.terminal.try_write() {
                  let osc_title = term.get_title();
                  let hostname = term.shell_integration_hostname();
                  let username = term.shell_integration_username();
                  let cwd = term.shell_integration_cwd();
                  drop(term);

                  let is_remote = if let Some(reported_host) = &hostname {
                      hostname::get()
                          .ok()
                          .and_then(|h| h.into_string().ok())
                          .map(|local| !reported_host.eq_ignore_ascii_case(&local))
                          .unwrap_or(false)
                  } else {
                      false
                  };

                  if is_remote {
                      if remote_osc_priority && !osc_title.is_empty() {
                          pane.title = osc_title;
                          pane.has_default_title = false;
                      } else {
                          pane.title =
                              format_remote_title(hostname, username, cwd, remote_format);
                          pane.has_default_title = false;
                      }
                  } else if !osc_title.is_empty() {
                      pane.title = osc_title;
                      pane.has_default_title = false;
                  } else if title_mode == par_term_config::TabTitleMode::Auto
                      && let Some(cwd) = cwd
                  {
                      let abbreviated = if let Some(home) = dirs::home_dir() {
                          cwd.replace(&home.to_string_lossy().to_string(), "~")
                      } else {
                          cwd
                      };
                      if let Some(last) = abbreviated.rsplit('/').next() {
                          if !last.is_empty() {
                              pane.title = last.to_string();
                          } else {
                              pane.title = abbreviated;
                          }
                      } else {
                          pane.title = abbreviated;
                      }
                      pane.has_default_title = false;
                  }
                  // else: keep existing pane.title unchanged this frame
              }
          }
      }
      // mutable borrow of pane_manager ends here

      // Step 4 — Derive tab.title from the focused pane (immutable re-borrow is now safe).
      if let Some((focused_id, pm)) = focused_id.zip(self.pane_manager.as_ref()) {
          if let Some(pane) = pm.get_pane(focused_id) {
              self.title = pane.title.clone();
              self.has_default_title = pane.has_default_title;
          }
      }
  ```

  Also update the tab-level hostname/CWD tracking (for profile auto-switching). This
  reads from `self.terminal` (primary pane) — leave it as-is by adding it AFTER the
  pane loop. Check if the current implementation calls `check_hostname_change()` /
  `check_cwd_change()` here or in `frame_setup.rs` — if they are called elsewhere,
  no action needed in this method.

- [ ] **Step 2: Compile check**

  ```bash
  cargo check -p par-term 2>&1 | head -40
  ```

  Expected: Compiles without errors. (Warnings about `self.terminal` no longer being
  used in `update_title()` are expected if the hostname tracking was removed — but
  check `frame_setup.rs` first to confirm those calls are already elsewhere.)

---

### Task 3: Update `set_title()` to sync the focused pane

**Files:**
- Modify: `src/tab/profile_tracking.rs` (lines 112–118)

- [ ] **Step 1: Replace `set_title()` body**

  Replace the current `set_title()` (lines 115–118):

  ```rust
  pub fn set_title(&mut self, title: &str) {
      self.title = title.to_string();
      self.has_default_title = false;
  }
  ```

  With:

  ```rust
  pub fn set_title(&mut self, title: &str) {
      self.title = title.to_string();
      self.has_default_title = false;
      // Sync focused pane so update_title() doesn't overwrite on the next frame.
      if let Some(pane) = self.pane_manager.as_mut().and_then(|pm| pm.focused_pane_mut()) {
          pane.title = title.to_string();
          pane.has_default_title = false;
      }
  }
  ```

- [ ] **Step 2: Fix `clear_auto_profile()` in the same file (lines 232–235)**

  Find the restore in `clear_auto_profile()`:

  ```rust
  if let Some(original) = self.profile.pre_profile_title.take() {
      self.title = original;
  }
  ```

  Replace with:

  ```rust
  if let Some(original) = self.profile.pre_profile_title.take() {
      self.set_title(&original);
  }
  ```

- [ ] **Step 3: Compile check**

  ```bash
  cargo check -p par-term 2>&1 | head -30
  ```

---

### Task 4: Fix `set_default_title()` to also write pane titles

**Files:**
- Modify: `src/tab/manager.rs` (the `set_default_title()` method, around line 106 in `profile_tracking.rs`)

  Actually `set_default_title()` is at `src/tab/profile_tracking.rs:106`. Confirm with:

  ```bash
  grep -n "fn set_default_title" src/tab/profile_tracking.rs
  ```

- [ ] **Step 1: Replace `set_default_title()` body**

  Current (lines 106–110 of `profile_tracking.rs`):

  ```rust
  pub fn set_default_title(&mut self, tab_number: usize) {
      if self.has_default_title {
          self.title = format!("Tab {}", tab_number);
      }
  }
  ```

  Replace with:

  ```rust
  pub fn set_default_title(&mut self, tab_number: usize) {
      if self.has_default_title {
          let title = format!("Tab {}", tab_number);
          self.title = title.clone();
          // Also write pane.title for every pane that still has a default title so
          // update_title()'s Step 4 derivation from pane.title returns "Tab N" correctly
          // (a brand-new pane has pane.title == "" which would otherwise overwrite).
          if let Some(pm) = self.pane_manager.as_mut() {
              for pane in pm.all_panes_mut() {
                  if pane.has_default_title {
                      pane.title = title.clone();
                  }
              }
          }
      }
  }
  ```

- [ ] **Step 2: Write a unit test for `set_default_title()` pane sync**

  Add to the `#[cfg(test)]` block at the bottom of `src/tab/profile_tracking.rs`:

  ```rust
  #[cfg(test)]
  mod default_title_tests {
      use crate::tab::Tab;

      /// set_default_title() must write pane.title for default-titled panes
      /// so that update_title()'s Step 4 derivation doesn't produce an empty string.
      #[test]
      fn set_default_title_syncs_pane_title() {
          // new_stub(id, tab_number) — creates a single-pane tab in test mode.
          // pane.title starts as "" (new field default), tab.title starts as "Tab 1".
          let mut tab = Tab::new_stub(1, 1);
          // Fresh pane: has default title, pane.title == ""
          {
              let pm = tab.pane_manager.as_ref().unwrap();
              let pane = pm.focused_pane().unwrap();
              assert!(pane.has_default_title);
              assert_eq!(pane.title, "");
          }
          tab.set_default_title(3);
          assert_eq!(tab.title, "Tab 3");
          // Pane must also be updated so derivation survives the next frame
          let pm = tab.pane_manager.as_ref().unwrap();
          let pane = pm.focused_pane().unwrap();
          assert_eq!(pane.title, "Tab 3");
          assert!(pane.has_default_title);
      }

      /// set_default_title() must NOT overwrite panes that already have a real title.
      #[test]
      fn set_default_title_skips_non_default_panes() {
          let mut tab = Tab::new_stub(1, 1);
          // Simulate pane having received a real title
          {
              let pm = tab.pane_manager.as_mut().unwrap();
              let pane = pm.focused_pane_mut().unwrap();
              pane.title = "vim".to_string();
              pane.has_default_title = false;
          }
          // tab.has_default_title stays true (simulates multi-pane where focused has real title
          // but tab-level tracking is slightly stale)
          tab.has_default_title = true;
          tab.set_default_title(2);
          // Pane with a real title must be untouched
          let pm = tab.pane_manager.as_ref().unwrap();
          let pane = pm.focused_pane().unwrap();
          assert_eq!(pane.title, "vim");
          assert!(!pane.has_default_title);
      }
  }
  ```

- [ ] **Step 3: Run the new tests**

  ```bash
  cargo test -p par-term default_title_tests 2>&1 | tail -20
  ```

  Expected: 2 tests pass.

- [ ] **Step 4: Write a test for `set_title()` pane sync**

  Add to the same test module in `profile_tracking.rs`:

  ```rust
  #[test]
  fn set_title_syncs_focused_pane() {
      let mut tab = Tab::new_stub(1, 1);
      tab.set_title("my-session");
      assert_eq!(tab.title, "my-session");
      assert!(!tab.has_default_title);
      let pm = tab.pane_manager.as_ref().unwrap();
      let pane = pm.focused_pane().unwrap();
      assert_eq!(pane.title, "my-session");
      assert!(!pane.has_default_title);
  }
  ```

- [ ] **Step 5: Run the new test**

  ```bash
  cargo test -p par-term set_title_syncs_focused_pane 2>&1 | tail -10
  ```

  Expected: PASS.

- [ ] **Step 6: Commit Chunk 1**

  ```bash
  cargo test -p par-term 2>&1 | tail -5
  make fmt
  git add src/pane/types/pane.rs src/tab/profile_tracking.rs
  git commit -m "feat(pane): add per-pane title tracking with update_title rewrite"
  ```

---

## Chunk 2: Call-site cleanup — route direct `tab.title` writes through `set_title()`

### Task 5: Fix `profile_auto_switch.rs` (5 direct writes)

**Files:**
- Modify: `src/app/tab_ops/profile_auto_switch.rs`

- [ ] **Step 1: Fix hostname restore (line 60)**

  Current:
  ```rust
  if let Some(original) = tab.profile.pre_profile_title.take() {
      tab.title = original;
  }
  ```

  Replace with:
  ```rust
  if let Some(original) = tab.profile.pre_profile_title.take() {
      tab.set_title(&original);
  }
  ```

- [ ] **Step 2: Fix hostname profile apply (line 124)**

  Current:
  ```rust
  tab.title = profile_tab_name.unwrap_or_else(|| profile_name.clone());
  ```

  Replace with:
  ```rust
  tab.set_title(&profile_tab_name.unwrap_or_else(|| profile_name.clone()));
  ```

- [ ] **Step 3: Fix SSH restore (line 236)**

  Same pattern as Step 1 — find:
  ```rust
  if let Some(original) = tab.profile.pre_profile_title.take() {
      tab.title = original;
  }
  ```
  Replace with:
  ```rust
  if let Some(original) = tab.profile.pre_profile_title.take() {
      tab.set_title(&original);
  }
  ```

- [ ] **Step 4: Fix directory profile apply (line 296)**

  Same pattern as Step 2 — find:
  ```rust
  tab.title = profile_tab_name.unwrap_or_else(|| profile_name.clone());
  ```
  Replace with:
  ```rust
  tab.set_title(&profile_tab_name.unwrap_or_else(|| profile_name.clone()));
  ```

- [ ] **Step 5: Fix directory restore (line 355)**

  Same pattern as Steps 1 & 3:
  ```rust
  if let Some(original) = tab.profile.pre_profile_title.take() {
      tab.set_title(&original);
  }
  ```

- [ ] **Step 6: Compile check**

  ```bash
  cargo check -p par-term 2>&1 | head -20
  ```

---

### Task 6: Fix `gateway_profile.rs`, `arrangements.rs`, `window_session.rs`, `tab_reopen.rs`

**Files:**
- Modify: `src/app/tmux_handler/gateway_profile.rs` (line 97)
- Modify: `src/app/window_manager/arrangements.rs` (line 159)
- Modify: `src/app/window_manager/window_session.rs` (line 123)
- Modify: `src/app/tab_ops/tab_reopen.rs` (line 113)

- [ ] **Step 1: Fix `gateway_profile.rs` line 97**

  Current:
  ```rust
  tab.title = tab_name.unwrap_or_else(|| profile_name.to_string());
  ```

  Replace with:
  ```rust
  tab.set_title(&tab_name.unwrap_or_else(|| profile_name.to_string()));
  ```

- [ ] **Step 2: Fix `arrangements.rs` lines 158–161**

  Current:
  ```rust
  if let Some(ref user_title) = snapshot.user_title {
      tab.title = user_title.clone();
      tab.user_named = true;
      tab.has_default_title = false;
  }
  ```

  Replace with:
  ```rust
  if let Some(ref user_title) = snapshot.user_title {
      tab.set_title(user_title);
      tab.user_named = true;
      // has_default_title = false is already set by set_title()
  }
  ```

- [ ] **Step 3: Fix `window_session.rs` lines 122–125**

  Same pattern as Step 2:
  ```rust
  if let Some(ref user_title) = session_tab.snapshot.user_title {
      tab.set_title(user_title);
      tab.user_named = true;
  }
  ```

- [ ] **Step 4: Fix `tab_reopen.rs` lines 112–115**

  Current:
  ```rust
  if !info.has_default_title {
      tab.title = info.title;
      tab.has_default_title = false;
  }
  ```

  Replace with:
  ```rust
  if !info.has_default_title {
      tab.set_title(&info.title);
  }
  ```

- [ ] **Step 5: Compile check**

  ```bash
  cargo check -p par-term 2>&1 | head -20
  ```

---

### Task 7: Fix `tab_bar.rs` — `RenameTab` handler

**Files:**
- Modify: `src/app/window_state/action_handlers/tab_bar.rs` (lines 68–86)

The `RenameTab` handler has two branches: empty name (clear → auto mode) and non-empty name (set user title).

- [ ] **Step 1: Fix the non-empty name branch (line 81)**

  Current:
  ```rust
  } else {
      tab.title = name;
      tab.user_named = true;
      tab.has_default_title = false;
  }
  ```

  Replace with:
  ```rust
  } else {
      tab.set_title(&name);
      tab.user_named = true;
      // has_default_title = false already set by set_title()
  }
  ```

- [ ] **Step 2: Fix the empty name (clear) branch (lines 70–79)**

  The clear branch sets `tab.user_named = false` and `tab.has_default_title = true`, then
  calls `update_title()` to immediately compute a fresh title. Under the new design we
  also need to reset the focused pane's `has_default_title` so the pane loop treats it
  as a default-titled pane. Add the pane reset before the `update_title()` call:

  Current:
  ```rust
  if name.is_empty() {
      // Blank name: revert to auto title mode
      tab.user_named = false;
      tab.has_default_title = true;
      // Trigger immediate title update
      tab.update_title(
          self.config.tab_title_mode,
          self.config.remote_tab_title_format,
          self.config.remote_tab_title_osc_priority,
      );
  ```

  Replace with:
  ```rust
  if name.is_empty() {
      // Blank name: revert to auto title mode
      tab.user_named = false;
      tab.has_default_title = true;
      // Reset focused pane so the per-pane loop re-derives its title from scratch
      if let Some(pane) = tab.pane_manager.as_mut().and_then(|pm| pm.focused_pane_mut()) {
          pane.title = String::new();
          pane.has_default_title = true;
      }
      // Trigger immediate title update
      tab.update_title(
          self.config.tab_title_mode,
          self.config.remote_tab_title_format,
          self.config.remote_tab_title_osc_priority,
      );
  ```

- [ ] **Step 3: Compile check**

  ```bash
  cargo check -p par-term 2>&1 | head -20
  ```

- [ ] **Step 4: Run all tests**

  ```bash
  cargo test -p par-term 2>&1 | tail -20
  ```

  Expected: All tests pass.

- [ ] **Step 5: Commit Chunk 2**

  ```bash
  make fmt
  git add src/app/tab_ops/profile_auto_switch.rs \
          src/app/tmux_handler/gateway_profile.rs \
          src/app/window_manager/arrangements.rs \
          src/app/window_manager/window_session.rs \
          src/app/tab_ops/tab_reopen.rs \
          src/app/window_state/action_handlers/tab_bar.rs
  git commit -m "feat(title): route all tab.title writes through set_title() for pane sync"
  ```

---

## Chunk 3: Verification

### Task 8: Full build, test, and smoke test

- [ ] **Step 1: Run all checks**

  ```bash
  make pre-commit
  ```

  Expected: fmt, lint, and tests all pass.

- [ ] **Step 2: Build dev-release**

  ```bash
  make build
  ```

  Expected: Clean build, no errors.

- [ ] **Step 3: Manual smoke test — single pane**

  Launch par-term normally. Open a shell. Run a command that sets the terminal title
  (e.g., `echo -ne "\033]0;my-title\007"`). Verify the tab bar shows `my-title`.
  Run `echo -ne "\033]0;\007"` (clear title). Verify tab reverts to CWD or default.

- [ ] **Step 4: Manual smoke test — split pane**

  1. Open par-term, split the tab vertically (check keyboard shortcut in docs).
  2. In pane A: run `echo -ne "\033]0;pane-A\007"`. Tab bar should show `pane-A`.
  3. Click pane B. Tab bar should immediately show pane B's last-known title (CWD or default).
  4. In pane B: run `echo -ne "\033]0;pane-B\007"`. Tab bar should show `pane-B`.
  5. Click pane A. Tab bar should immediately show `pane-A`.

- [ ] **Step 5: Manual smoke test — user-named tab**

  1. Right-click a tab, rename it to "my-tab". Verify it shows `my-tab` in tab bar.
  2. Split the pane. Switch focus. Tab bar should still show `my-tab` (frozen).
  3. In each pane send an OSC title sequence. Tab bar should still show `my-tab`.

- [ ] **Step 6: Manual smoke test — tmux mode (if available)**

  1. Open par-term in tmux mode.
  2. Split into multiple tmux panes.
  3. In each pane send an OSC title. Verify focus-switch shows correct per-pane title.

- [ ] **Step 7: Final commit**

  ```bash
  make fmt
  git add -A
  git commit -m "chore: per-pane title tracking — verified and complete"
  ```

---

## Quick Reference: Key Methods

| Method | File | Purpose |
|---|---|---|
| `Tab::update_title()` | `src/tab/profile_tracking.rs:32` | Iterates all panes, derives tab.title from focused |
| `Tab::set_title()` | `src/tab/profile_tracking.rs:115` | Writes both tab.title and focused pane.title |
| `Tab::set_default_title()` | `src/tab/profile_tracking.rs:106` | Writes "Tab N" to tab and all default-titled panes |
| `Tab::clear_auto_profile()` | `src/tab/profile_tracking.rs:229` | Restores pre_profile_title via set_title() |
| `PaneManager::all_panes_mut()` | `src/pane/manager/mod.rs:137` | Returns `Vec<&mut Pane>` for the iteration loop |
| `PaneManager::focused_pane_id()` | `src/pane/manager/focus.rs:120` | Snapshot before the mutable loop |
| `PaneManager::get_pane()` | `src/pane/manager/mod.rs:119` | Immutable lookup after the loop for Step 4 |
| `Tab::new_stub()` | `src/tab/constructors.rs` | Test constructor (inside `#[cfg(test)]`) |
