# Icon Picker Expansion + Scrollbar Fix — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add ~47 new Nerd Font icons (2 new categories + expanded existing ones) to the tab icon picker and fix the scrollbar-overlaps-rightmost-column bug.

**Architecture:** Two files only — `par-term-settings-ui/src/nerd_font.rs` (icon data) and `src/tab_bar_ui/context_menu.rs` (scrollbar padding UI fix). No new types, no new modules. Changes are purely additive for icons; the scrollbar fix wraps existing scroll content in a right-padded Frame.

**Tech Stack:** Rust, egui (UI layout), Nerd Font SymbolsNerdFontMono-Regular.ttf v3.4.0 (embedded font, codepoints from Devicons, Font Logos, Font Awesome, Codicons ranges)

---

## Codepoint Ranges Reference

| Range | Source | Examples already in file |
|-------|--------|--------------------------|
| `\u{e5fa}`–`\u{e6xx}` | Seti-UI / Custom | `\u{e65c}` GitLab, `\u{e5fb}` Git Folder |
| `\u{e700}`–`\u{e7xx}` | Devicons | `\u{e718}` Node.js, `\u{e7a8}` Rust |
| `\u{ea60}`–`\u{ebxx}` | Codicons | `\u{ea66}` Tag, `\u{eb05}` Heart |
| `\u{ee00}`–`\u{ee0x}` | Progress | `\u{ee0d}` Robot |
| `\u{f000}`–`\u{f2xx}` | Font Awesome 4 | `\u{f005}` Star, `\u{f185}` Sun |
| `\u{f300}`–`\u{f37x}` | Font Logos | `\u{f308}` Docker, `\u{f31a}` Tux |

**IMPORTANT:** Wrong codepoints render as empty boxes (tofu). After each task, build and visually open the icon picker to confirm icons appear. A tofu box means the codepoint is wrong — find the correct one in the [NF v3.4.0 cheat sheet](https://www.nerdfonts.com/cheat-sheet).

---

## Task 1: Fix Scrollbar Padding in Icon Picker

**Files:**
- Modify: `src/tab_bar_ui/context_menu.rs:124–165`

The vertical `ScrollArea` inside the popup renders content flush to the edge. When the scrollbar appears (content height > `TAB_ICON_PICKER_MAX_HEIGHT`), it overlays the rightmost icons. Fix: wrap inner scroll content in `egui::Frame::NONE` with right `inner_margin`.

**Step 1: Locate the scroll area content**

Open `src/tab_bar_ui/context_menu.rs`. The scroll area is at lines 124–165:

```rust
egui::ScrollArea::vertical()
    .max_height(TAB_ICON_PICKER_MAX_HEIGHT)
    .show(ui, |ui| {
        for (category, icons) in
            crate::settings_ui::nerd_font::NERD_FONT_PRESETS
        { ... }
        ui.add_space(4.0);
        if ui.button("Clear icon").clicked() { ... }
    });
```

**Step 2: Wrap inner content in a right-padded Frame**

Replace the `.show(ui, |ui| {` body so ALL content is inside a Frame with right padding:

```rust
egui::ScrollArea::vertical()
    .max_height(TAB_ICON_PICKER_MAX_HEIGHT)
    .show(ui, |ui| {
        egui::Frame::NONE
            .inner_margin(egui::Margin {
                right: 10.0,
                ..Default::default()
            })
            .show(ui, |ui| {
                for (category, icons) in
                    crate::settings_ui::nerd_font::NERD_FONT_PRESETS
                {
                    ui.label(
                        egui::RichText::new(*category)
                            .small()
                            .strong(),
                    );
                    ui.horizontal_wrapped(|ui| {
                        for (icon, label) in *icons {
                            let btn = ui.add_sized(
                                [
                                    TAB_ICON_PICKER_GLYPH_SIZE
                                        + 12.0,
                                    TAB_ICON_PICKER_GLYPH_SIZE
                                        + 12.0,
                                ],
                                egui::Button::new(
                                    egui::RichText::new(*icon)
                                        .size(TAB_ICON_PICKER_GLYPH_SIZE),
                                )
                                .frame(false),
                            );
                            if btn.on_hover_text(*label).clicked() {
                                self.icon_buffer = icon.to_string();
                                egui::Popup::close_all(ui.ctx());
                            }
                        }
                    });
                    ui.add_space(2.0);
                }
                ui.add_space(4.0);
                if ui.button("Clear icon").clicked() {
                    self.icon_buffer.clear();
                    egui::Popup::close_all(ui.ctx());
                }
            });
    });
```

**Step 3: Build and verify**

```bash
make build
```

Expected: compiles without errors.

Open the icon picker (right-click a tab → Set Icon → click the picker button), scroll down, confirm the rightmost column is no longer clipped by the scrollbar.

**Step 4: Commit**

```bash
git add src/tab_bar_ui/context_menu.rs
git commit -m "fix(icon-picker): add right padding inside scroll area to prevent scrollbar overlap"
```

---

## Task 2: Expand "Dev & Tools" and "OS & Platforms" Categories

**Files:**
- Modify: `par-term-settings-ui/src/nerd_font.rs`

**Step 1: Add 11 icons to "Dev & Tools"**

Locate the `"Dev & Tools"` tuple (currently ends at `("\u{f0ad}", "Wrench")`). Append these entries before the closing `]`:

```rust
("\u{e74a}", "TypeScript"),   // Devicons
("\u{e724}", "Go"),           // Devicons
("\u{e61d}", "C"),            // Seti-UI
("\u{e646}", "C++"),          // Seti-UI
("\u{e753}", "Angular"),      // Devicons
("\u{e6a0}", "Vue.js"),       // Seti-UI
("\u{e697}", "Svelte"),       // Seti-UI
("\u{e736}", "HTML5"),        // Devicons
("\u{e7a6}", "CSS3"),         // Devicons
("\u{e739}", "Haskell"),      // Devicons
("\u{e737}", "Scala"),        // Devicons
```

**Step 2: Add 5 icons to "OS & Platforms"**

Locate `"OS & Platforms"` (currently ends at `("\u{e7fd}", "Homebrew")`). Append:

```rust
("\u{f31b}", "Ubuntu"),       // Font Logos
("\u{f303}", "Arch"),         // Font Logos
("\u{f315}", "Raspi"),        // Font Logos
("\u{f30a}", "Fedora"),       // Font Logos
("\u{f17b}", "Android"),      // Font Awesome
```

**Step 3: Build and visual check**

```bash
make build
```

Open icon picker → scroll to "Dev & Tools" and "OS & Platforms" sections. Verify new icons are visible (not tofu boxes). If any show as empty boxes, check the NF cheat sheet for the correct codepoint and update.

**Step 4: Commit**

```bash
git add par-term-settings-ui/src/nerd_font.rs
git commit -m "feat(icon-picker): add TypeScript/Go/C/C++/Angular/Vue/Svelte/HTML/CSS/Haskell/Scala and OS platform icons"
```

---

## Task 3: Expand "Git & VCS" and Add "Weather & Nature" Category

**Files:**
- Modify: `par-term-settings-ui/src/nerd_font.rs`

**Step 1: Add 3 icons to "Git & VCS"**

Locate `"Git & VCS"` (currently ends at `("\u{e5fb}", "Git Folder")`). Append:

```rust
("\u{ea64}", "Pull Request"),  // Codicons git-pull-request
("\u{e72a}", "Bitbucket"),     // Devicons
("\u{ea6d}", "Diff"),          // Codicons diff
```

**Step 2: Add new "Weather & Nature" category**

After the closing `)` of `"Git & VCS"`, add a new category tuple. Insert it before `"Containers & Infra"`:

```rust
(
    "Weather & Nature",
    &[
        ("\u{f185}", "Sun"),            // Font Awesome
        ("\u{f186}", "Moon"),           // Font Awesome
        ("\u{f2dc}", "Snowflake"),      // Font Awesome
        ("\u{f0e9}", "Umbrella"),       // Font Awesome (rain)
        ("\u{f0e7}", "Lightning"),      // Font Awesome bolt (reuse)
        ("\u{f06c}", "Leaf"),           // Font Awesome
        ("\u{f1bb}", "Tree"),           // Font Awesome
        ("\u{f2c9}", "Thermometer"),    // Font Awesome
        ("\u{f1b0}", "Paw"),            // Font Awesome
        ("\u{e30d}", "Day Sunny"),      // Weather Icons
        ("\u{e308}", "Rainy"),          // Weather Icons
        ("\u{e31a}", "Night Clear"),    // Weather Icons
    ],
),
```

**Step 3: Build and visual check**

```bash
make build
```

Open icon picker, scroll to "Git & VCS" (verify Pull Request, Bitbucket, Diff icons) and "Weather & Nature" (verify all 12 icons visible, no tofu). Fix any wrong codepoints.

**Step 4: Commit**

```bash
git add par-term-settings-ui/src/nerd_font.rs
git commit -m "feat(icon-picker): expand Git & VCS and add Weather & Nature category"
```

---

## Task 4: Add "Fun & Seasonal" Category

**Files:**
- Modify: `par-term-settings-ui/src/nerd_font.rs`

**Step 1: Add new "Fun & Seasonal" category at the end**

After the closing `)` of `"People & Misc"` (the last category), add:

```rust
(
    "Fun & Seasonal",
    &[
        ("\u{f091}", "Trophy"),        // Font Awesome
        ("\u{f521}", "Crown"),         // Font Awesome 5
        ("\u{f1fd}", "Birthday Cake"), // Font Awesome
        ("\u{f06b}", "Gift"),          // Font Awesome
        ("\u{f6e2}", "Ghost"),         // Font Awesome 5
        ("\u{f54c}", "Skull"),         // Font Awesome 5
        ("\u{f6be}", "Cat"),           // Font Awesome 5
        ("\u{f6d3}", "Dog"),           // Font Awesome 5
        ("\u{f6d5}", "Dragon"),        // Font Awesome 5
        ("\u{f0d0}", "Magic Wand"),    // Font Awesome
        ("\u{f1e2}", "Bomb"),          // Font Awesome
        ("\u{f0fc}", "Beer"),          // Font Awesome
        ("\u{f0f4}", "Coffee"),        // Font Awesome
        ("\u{f818}", "Pizza"),         // Font Awesome 5
        ("\u{f522}", "Dice"),          // Font Awesome 5
        ("\u{eb2b}", "Sparkle"),       // Codicons
    ],
),
```

**Step 2: Build and visual check**

```bash
make build
```

Open icon picker, scroll to the bottom to "Fun & Seasonal". Verify all 16 icons are visible (not tofu). Font Awesome 5 icons (`f521`, `f6xx`, `f818`, `f522`) are included in NF v3.4.0 — if any show tofu, cross-reference the [NF cheat sheet](https://www.nerdfonts.com/cheat-sheet) and update the codepoint.

**Step 3: Run full build and quick smoke test**

```bash
make build
```

Launch the app, right-click a tab → Set Icon → open picker. Scroll through all 14 categories end-to-end and confirm no tofu boxes remain.

**Step 4: Commit**

```bash
git add par-term-settings-ui/src/nerd_font.rs
git commit -m "feat(icon-picker): add Fun & Seasonal category with 16 icons"
```

---

## Final Count

| Category | Before | After |
|----------|--------|-------|
| Terminal | 11 | 11 |
| Dev & Tools | 16 | 27 |
| Files & Data | 12 | 12 |
| Network & Cloud | 12 | 12 |
| Security | 12 | 12 |
| Git & VCS | 8 | 11 |
| **Weather & Nature** | — | 12 |
| Containers & Infra | 12 | 12 |
| OS & Platforms | 10 | 15 |
| Status & Alerts | 16 | 16 |
| UI Actions | 16 | 16 |
| Navigation | 16 | 16 |
| People & Misc | 12 | 12 |
| **Fun & Seasonal** | — | 16 |
| **Total** | **192** | **240** |
