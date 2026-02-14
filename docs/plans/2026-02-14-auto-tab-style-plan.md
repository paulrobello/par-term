# Auto Tab Style Switching Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Auto-switch tab bar style (Dark/Light/Compact/Minimal/HighContrast) based on system light/dark appearance, with configurable light/dark style mapping.

**Architecture:** Add `Automatic` variant to `TabStyle` enum. Add `light_tab_style` and `dark_tab_style` config fields. When `tab_style == Automatic`, resolve to the appropriate sub-style and apply its colors. Hook into the same two detection points as the existing `auto_dark_mode` feature (startup + `ThemeChanged` event).

**Tech Stack:** Rust, serde/serde_yaml (config), egui (settings UI), winit (system theme detection)

---

### Task 1: Add `Automatic` variant to `TabStyle` enum

**Files:**
- Modify: `src/config/types.rs:199-241`

**Step 1: Add the variant**

In `src/config/types.rs`, add `Automatic` to the `TabStyle` enum (after `HighContrast`):

```rust
/// Tab visual style preset
///
/// Controls the cosmetic appearance of tabs (colors, sizes, spacing).
/// Each preset applies a set of color/size/spacing adjustments to the tab bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TabStyle {
    /// Default dark theme styling
    #[default]
    Dark,
    /// Light theme tab styling
    Light,
    /// Smaller tabs, more visible terminal content
    Compact,
    /// Clean, minimal tab appearance
    Minimal,
    /// Enhanced contrast for accessibility
    HighContrast,
    /// Automatically switch between light/dark styles based on system theme
    Automatic,
}
```

**Step 2: Update `display_name()` and `all()`; add `all_concrete()`**

```rust
impl TabStyle {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            TabStyle::Dark => "Dark",
            TabStyle::Light => "Light",
            TabStyle::Compact => "Compact",
            TabStyle::Minimal => "Minimal",
            TabStyle::HighContrast => "High Contrast",
            TabStyle::Automatic => "Automatic",
        }
    }

    /// All available styles for UI iteration (includes Automatic)
    pub fn all() -> &'static [TabStyle] {
        &[
            TabStyle::Dark,
            TabStyle::Light,
            TabStyle::Compact,
            TabStyle::Minimal,
            TabStyle::HighContrast,
            TabStyle::Automatic,
        ]
    }

    /// All concrete styles (excludes Automatic) — for sub-style dropdowns
    pub fn all_concrete() -> &'static [TabStyle] {
        &[
            TabStyle::Dark,
            TabStyle::Light,
            TabStyle::Compact,
            TabStyle::Minimal,
            TabStyle::HighContrast,
        ]
    }
}
```

**Step 3: Build to verify compilation**

Run: `cargo build 2>&1 | head -30`
Expected: Compiler errors about non-exhaustive match in `apply_tab_style()` — that's correct, we'll fix it in Task 2.

**Step 4: Commit**

```bash
git add src/config/types.rs
git commit -m "feat: add Automatic variant to TabStyle enum (#141)"
```

---

### Task 2: Add config fields and `apply_system_tab_style()` method

**Files:**
- Modify: `src/config/defaults.rs` (add default functions)
- Modify: `src/config/mod.rs:1050-1055` (add config fields)
- Modify: `src/config/mod.rs:2033-2100` (update `apply_tab_style()`)
- Modify: `src/config/mod.rs:2530-2547` (add new method after `apply_system_theme()`)
- Modify: `src/config/mod.rs:1880-1890` (update `Default` impl)

**Step 1: Add default functions in `src/config/defaults.rs`**

Add after the existing tab defaults (around line 338):

```rust
pub fn light_tab_style() -> TabStyle {
    TabStyle::Light
}

pub fn dark_tab_style() -> TabStyle {
    TabStyle::Dark
}
```

Note: This requires adding the import `use super::types::TabStyle;` at the top of `defaults.rs` if not already present.

**Step 2: Add config fields in `src/config/mod.rs`**

After the `tab_style` field at line 1055, add:

```rust
    /// Tab style to use when system is in light mode (used when tab_style is Automatic)
    #[serde(default = "defaults::light_tab_style")]
    pub light_tab_style: TabStyle,

    /// Tab style to use when system is in dark mode (used when tab_style is Automatic)
    #[serde(default = "defaults::dark_tab_style")]
    pub dark_tab_style: TabStyle,
```

**Step 3: Update `Default` impl**

In the `Default` impl for `Config` (around line 1885), after `tab_style: TabStyle::default()`, add:

```rust
            light_tab_style: defaults::light_tab_style(),
            dark_tab_style: defaults::dark_tab_style(),
```

**Step 4: Update `apply_tab_style()` to handle `Automatic`**

In `apply_tab_style()` at line 2033, add the `Automatic` arm. When `Automatic`, this is a no-op because the actual style is applied by `apply_system_tab_style()`:

```rust
            TabStyle::Automatic => {
                // No-op here: actual style is resolved and applied by apply_system_tab_style()
            }
```

**Step 5: Add `apply_system_tab_style()` method**

After `apply_system_theme()` (around line 2547), add:

```rust
    /// Apply tab style based on system theme when tab_style is Automatic.
    /// Returns true if the style was applied.
    pub fn apply_system_tab_style(&mut self, is_dark: bool) -> bool {
        if self.tab_style != TabStyle::Automatic {
            return false;
        }
        let target = if is_dark {
            self.dark_tab_style
        } else {
            self.light_tab_style
        };
        // Temporarily set to concrete style, apply colors, then restore Automatic
        self.tab_style = target;
        self.apply_tab_style();
        self.tab_style = TabStyle::Automatic;
        true
    }
```

**Step 6: Build to verify compilation**

Run: `cargo build`
Expected: Success

**Step 7: Commit**

```bash
git add src/config/defaults.rs src/config/mod.rs
git commit -m "feat: add light/dark tab style config and apply_system_tab_style (#141)"
```

---

### Task 3: Write tests for the new functionality

**Files:**
- Modify: `tests/config_tests.rs` (add tests after the existing `auto_dark_mode` tests around line 1235)

**Step 1: Write tests**

Add the following tests to `tests/config_tests.rs` after the existing auto dark mode tests:

```rust
// =============================================================================
// Auto Tab Style Tests
// =============================================================================

#[test]
fn test_auto_tab_style_defaults() {
    let config = Config::default();
    assert_eq!(config.tab_style, TabStyle::Dark);
    assert_eq!(config.light_tab_style, TabStyle::Light);
    assert_eq!(config.dark_tab_style, TabStyle::Dark);
}

#[test]
fn test_apply_system_tab_style_disabled_when_not_automatic() {
    let mut config = Config::default();
    config.tab_style = TabStyle::Dark;
    assert!(!config.apply_system_tab_style(true));
    assert!(!config.apply_system_tab_style(false));
}

#[test]
fn test_apply_system_tab_style_dark() {
    let mut config = Config::default();
    config.tab_style = TabStyle::Automatic;
    config.dark_tab_style = TabStyle::HighContrast;

    assert!(config.apply_system_tab_style(true));
    // Should have applied HighContrast colors but kept Automatic as the tab_style
    assert_eq!(config.tab_style, TabStyle::Automatic);
    // HighContrast sets tab_bar_background to [0, 0, 0]
    assert_eq!(config.tab_bar_background, [0, 0, 0]);
}

#[test]
fn test_apply_system_tab_style_light() {
    let mut config = Config::default();
    config.tab_style = TabStyle::Automatic;
    config.light_tab_style = TabStyle::Light;

    assert!(config.apply_system_tab_style(false));
    assert_eq!(config.tab_style, TabStyle::Automatic);
    // Light sets tab_bar_background to [235, 235, 235]
    assert_eq!(config.tab_bar_background, [235, 235, 235]);
}

#[test]
fn test_apply_system_tab_style_preserves_automatic() {
    let mut config = Config::default();
    config.tab_style = TabStyle::Automatic;
    config.dark_tab_style = TabStyle::Compact;

    config.apply_system_tab_style(true);
    // tab_style must remain Automatic after applying
    assert_eq!(config.tab_style, TabStyle::Automatic);
}

#[test]
fn test_tab_style_all_concrete_excludes_automatic() {
    let concrete = TabStyle::all_concrete();
    assert!(!concrete.contains(&TabStyle::Automatic));
    assert_eq!(concrete.len(), 5);
}

#[test]
fn test_tab_style_all_includes_automatic() {
    let all = TabStyle::all();
    assert!(all.contains(&TabStyle::Automatic));
    assert_eq!(all.len(), 6);
}

#[test]
fn test_auto_tab_style_yaml_deserialization() {
    let yaml = r#"
tab_style: automatic
light_tab_style: compact
dark_tab_style: high_contrast
"#;
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.tab_style, TabStyle::Automatic);
    assert_eq!(config.light_tab_style, TabStyle::Compact);
    assert_eq!(config.dark_tab_style, TabStyle::HighContrast);
}

#[test]
fn test_auto_tab_style_yaml_defaults_when_absent() {
    let yaml = "cols: 120\n";
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.light_tab_style, TabStyle::Light);
    assert_eq!(config.dark_tab_style, TabStyle::Dark);
}
```

**Step 2: Run tests to verify they pass**

Run: `cargo test test_auto_tab_style -- --nocapture 2>&1`
Expected: All 9 new tests pass.

Also run the full test suite to check for regressions:

Run: `cargo test 2>&1 | tail -20`
Expected: All tests pass (no regressions).

**Step 3: Commit**

```bash
git add tests/config_tests.rs
git commit -m "test: add tests for automatic tab style switching (#141)"
```

---

### Task 4: Hook into runtime detection points

**Files:**
- Modify: `src/app/window_state.rs:614-626` (startup detection)
- Modify: `src/app/handler.rs:677-700` (ThemeChanged event)

**Step 1: Add startup detection in `src/app/window_state.rs`**

After the existing `apply_system_theme()` block (line 614-626), add tab style detection. The new code should apply regardless of `auto_dark_mode` — it depends on `tab_style == Automatic`:

```rust
        // Detect system theme at startup and apply tab style if tab_style is Automatic
        {
            let is_dark = window
                .theme()
                .is_none_or(|t| t == winit::window::Theme::Dark);
            if self.config.apply_system_tab_style(is_dark) {
                log::info!(
                    "Auto tab style: detected {} system theme, applying {} tab style",
                    if is_dark { "dark" } else { "light" },
                    if is_dark {
                        self.config.dark_tab_style.display_name()
                    } else {
                        self.config.light_tab_style.display_name()
                    }
                );
            }
        }
```

**Step 2: Add ThemeChanged handler in `src/app/handler.rs`**

Inside the `WindowEvent::ThemeChanged` arm (line 677-700), after the existing `apply_system_theme` block, add:

```rust
                // Also switch tab style if set to Automatic
                if self.config.apply_system_tab_style(is_dark) {
                    log::info!(
                        "Auto tab style: switching to {} tab style",
                        if is_dark {
                            self.config.dark_tab_style.display_name()
                        } else {
                            self.config.light_tab_style.display_name()
                        }
                    );
                    self.needs_redraw = true;
                }
```

Note: Place this INSIDE the `ThemeChanged` arm but OUTSIDE the existing `if self.config.apply_system_theme(is_dark)` block — the tab style should switch even if the terminal theme didn't change. Also, `self.request_redraw()` and config save already happen in the existing flow, but we should ensure redraw happens even if only tab style changed (not theme). Restructure the ThemeChanged handler so that config save and request_redraw happen if EITHER theme or tab style changed.

The full handler should be:

```rust
            WindowEvent::ThemeChanged(system_theme) => {
                let is_dark = system_theme == winit::window::Theme::Dark;
                let theme_changed = self.config.apply_system_theme(is_dark);
                let tab_style_changed = self.config.apply_system_tab_style(is_dark);

                if theme_changed {
                    log::info!(
                        "System theme changed to {}, switching to theme: {}",
                        if is_dark { "dark" } else { "light" },
                        self.config.theme
                    );
                    let theme = self.config.load_theme();
                    for tab in self.tab_manager.tabs_mut() {
                        if let Ok(mut term) = tab.terminal.try_lock() {
                            term.set_theme(theme.clone());
                        }
                        tab.cache.cells = None;
                    }
                }

                if tab_style_changed {
                    log::info!(
                        "Auto tab style: switching to {} tab style",
                        if is_dark {
                            self.config.dark_tab_style.display_name()
                        } else {
                            self.config.light_tab_style.display_name()
                        }
                    );
                }

                if theme_changed || tab_style_changed {
                    if let Err(e) = self.config.save() {
                        log::error!("Failed to save config after theme change: {}", e);
                    }
                    self.needs_redraw = true;
                    self.request_redraw();
                }
            }
```

**Step 3: Build to verify compilation**

Run: `cargo build`
Expected: Success

**Step 4: Commit**

```bash
git add src/app/window_state.rs src/app/handler.rs
git commit -m "feat: hook auto tab style into startup and ThemeChanged event (#141)"
```

---

### Task 5: Update Settings UI

**Files:**
- Modify: `src/settings_ui/window_tab.rs:842-871` (add sub-style dropdowns)

**Step 1: Update the tab bar section**

Replace the tab style dropdown section in `show_tab_bar_section()` to show sub-style dropdowns when `Automatic` is selected:

```rust
        // Tab style preset dropdown
        ui.horizontal(|ui| {
            ui.label("Tab style:");
            let current_style = settings.config.tab_style;
            egui::ComboBox::from_id_salt("window_tab_style")
                .selected_text(current_style.display_name())
                .show_ui(ui, |ui| {
                    for style in TabStyle::all() {
                        if ui
                            .selectable_value(
                                &mut settings.config.tab_style,
                                *style,
                                style.display_name(),
                            )
                            .changed()
                        {
                            settings.config.apply_tab_style();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        // Show light/dark sub-style dropdowns when Automatic is selected
        if settings.config.tab_style == TabStyle::Automatic {
            ui.indent("auto_tab_style_indent", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Light tab style:");
                    let current = settings.config.light_tab_style;
                    egui::ComboBox::from_id_salt("window_light_tab_style")
                        .selected_text(current.display_name())
                        .show_ui(ui, |ui| {
                            for style in TabStyle::all_concrete() {
                                if ui
                                    .selectable_value(
                                        &mut settings.config.light_tab_style,
                                        *style,
                                        style.display_name(),
                                    )
                                    .changed()
                                {
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Dark tab style:");
                    let current = settings.config.dark_tab_style;
                    egui::ComboBox::from_id_salt("window_dark_tab_style")
                        .selected_text(current.display_name())
                        .show_ui(ui, |ui| {
                            for style in TabStyle::all_concrete() {
                                if ui
                                    .selectable_value(
                                        &mut settings.config.dark_tab_style,
                                        *style,
                                        style.display_name(),
                                    )
                                    .changed()
                                {
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                            }
                        });
                });
            });
        }
```

**Step 2: Build to verify**

Run: `cargo build`
Expected: Success

**Step 3: Commit**

```bash
git add src/settings_ui/window_tab.rs
git commit -m "feat: add auto tab style sub-dropdowns in Settings UI (#141)"
```

---

### Task 6: Update search keywords

**Files:**
- Modify: `src/settings_ui/sidebar.rs:283-311` (add keywords to Window tab)

**Step 1: Add keywords**

In the Window tab keywords section, after `"close button color"` (around line 311), add in the "Tab bar appearance" comment group:

```rust
            "tab style",
            "auto tab style",
            "automatic tab",
            "system tab style",
```

**Step 2: Build and run full checks**

Run: `cargo build && cargo test 2>&1 | tail -20`
Expected: All tests pass.

Run: `cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -20`
Expected: No warnings.

**Step 3: Commit**

```bash
git add src/settings_ui/sidebar.rs
git commit -m "feat: add auto tab style search keywords (#141)"
```

---

### Task 7: Final verification

**Step 1: Run `make pre-commit`**

Run: `make pre-commit`
Expected: Format check, lint, and tests all pass.

**Step 2: Run full CI checks**

Run: `make ci`
Expected: All checks pass.

**Step 3: Final commit if any formatting changes were needed**

If `make pre-commit` auto-formatted anything:
```bash
git add -A
git commit -m "style: format auto tab style code (#141)"
```
