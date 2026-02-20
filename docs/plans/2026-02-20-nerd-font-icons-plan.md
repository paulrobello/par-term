# Nerd Font Icons Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Embed Nerd Font Symbols into egui and replace emoji presets in the profile icon picker with curated Nerd Font icons that render reliably.

**Architecture:** Embed `SymbolsNerdFontMono-Regular.ttf` (v3.4.0, ~2.5MB) as a compile-time asset, register it as a Proportional fallback font in egui's `FontDefinitions`, and replace the `EMOJI_PRESETS` constant with `NERD_FONT_PRESETS` using verified codepoints.

**Tech Stack:** Rust, egui (`FontDefinitions`, `FontData`), Nerd Fonts v3.4.0

---

### Task 1: Add Nerd Font file to assets

**Files:**
- Create: `assets/fonts/SymbolsNerdFontMono-Regular.ttf`

**Step 1: Copy font file into assets**

```bash
mkdir -p assets/fonts
cp /tmp/SymbolsNerdFontMono-Regular.ttf assets/fonts/
```

**Step 2: Commit**

```bash
git add assets/fonts/SymbolsNerdFontMono-Regular.ttf
git commit -m "chore: add Nerd Font Symbols Mono (v3.4.0) for egui icon rendering"
```

---

### Task 2: Create nerd_font module in par-term-settings-ui

**Files:**
- Create: `par-term-settings-ui/src/nerd_font.rs`
- Modify: `par-term-settings-ui/src/lib.rs:31-33`

**Step 1: Create the nerd_font module**

Create `par-term-settings-ui/src/nerd_font.rs` with:

```rust
//! Nerd Font integration for egui.
//!
//! Provides font configuration and curated icon presets for the profile icon picker.
//! Uses SymbolsNerdFontMono-Regular.ttf (Nerd Fonts v3.4.0).

/// Embedded Nerd Font Symbols (Mono variant, ~2.5MB).
const NERD_FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/SymbolsNerdFontMono-Regular.ttf");

/// Configure egui to use Nerd Font Symbols as a fallback font.
///
/// Call this once after creating each `egui::Context` (main window and settings window).
/// Adds the Nerd Font as the last fallback in the Proportional family so that
/// standard Latin text still uses egui's default font, but Nerd Font codepoints render.
pub fn configure_nerd_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "nerd_font_symbols".to_owned(),
        egui::FontData::from_static(NERD_FONT_BYTES),
    );
    // Add as last fallback for Proportional family
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("nerd_font_symbols".to_owned());
    // Also add as fallback for Monospace family (for tab bar, badges, etc.)
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("nerd_font_symbols".to_owned());
    ctx.set_fonts(fonts);
}

/// Curated Nerd Font icon presets organized by category for the profile icon picker.
///
/// Each entry is (category_name, &[(icon_char, icon_label)]).
/// All codepoints verified against SymbolsNerdFontMono-Regular.ttf v3.4.0.
pub const NERD_FONT_PRESETS: &[(&str, &[(&str, &str)])] = &[
    (
        "Terminal",
        &[
            ("\u{e795}", "Terminal"),        // dev-terminal
            ("\u{ebca}", "Bash"),            // cod-terminal_bash
            ("\u{ebc7}", "PowerShell"),      // cod-terminal_powershell
            ("\u{ebc8}", "tmux"),            // cod-terminal_tmux
            ("\u{ea85}", "Console"),         // cod-terminal
            ("\u{ebc6}", "Linux Term"),      // cod-terminal_linux
            ("\u{ebc5}", "Debian Term"),     // cod-terminal_debian
            ("\u{ebc4}", "Cmd"),             // cod-terminal_cmd
            ("\u{f120}", "Prompt"),          // fa-terminal
            ("\u{e84f}", "Oh My Zsh"),       // dev-ohmyzsh
            ("\u{e691}", "Shell"),           // seti-shell
            ("\u{f489}", "Octicons Term"),   // oct-terminal
        ],
    ),
    (
        "Dev & Tools",
        &[
            ("\u{f121}", "Code"),            // fa-code
            ("\u{f09b}", "GitHub"),          // fa-github
            ("\u{e7ba}", "React"),           // dev-react
            ("\u{e73c}", "Python"),          // dev-python
            ("\u{e7a8}", "Rust"),            // dev-rust
            ("\u{e718}", "Node.js"),         // dev-nodejs_small
            ("\u{e738}", "Java"),            // dev-java
            ("\u{e755}", "Swift"),           // dev-swift
            ("\u{e81b}", "Kotlin"),          // dev-kotlin
            ("\u{e826}", "Lua"),             // dev-lua
            ("\u{e73d}", "PHP"),             // dev-php
            ("\u{e605}", "Ruby"),            // seti-ruby
            ("\u{e62b}", "Vim"),             // custom-vim
            ("\u{e6ae}", "Neovim"),          // custom-neovim
            ("\u{f188}", "Bug"),             // fa-bug
            ("\u{f0ad}", "Wrench"),          // fa-wrench
        ],
    ),
    (
        "Files & Data",
        &[
            ("\u{ea7b}", "File"),            // cod-file
            ("\u{eae9}", "File Code"),       // cod-file_code
            ("\u{ea83}", "Folder"),          // cod-folder
            ("\u{eaf7}", "Folder Open"),     // cod-folder_opened
            ("\u{f1c0}", "Database"),        // fa-database
            ("\u{eb4b}", "Save"),            // cod-save
            ("\u{f02d}", "Book"),            // fa-book
            ("\u{ea66}", "Tag"),             // cod-tag
            ("\u{f1b2}", "Cube"),            // fa-cube
            ("\u{f487}", "Package"),         // oct-package
            ("\u{f019}", "Download"),        // fa-download
            ("\u{f093}", "Upload"),          // fa-upload
        ],
    ),
    (
        "Network & Cloud",
        &[
            ("\u{f0ac}", "Globe"),           // fa-globe
            ("\u{f1eb}", "WiFi"),            // fa-wifi
            ("\u{ebaa}", "Cloud"),           // cod-cloud
            ("\u{f233}", "Server"),          // fa-server
            ("\u{ef09}", "Network"),         // fa-network_wired
            ("\u{f0e8}", "Sitemap"),         // fa-sitemap
            ("\u{eb2d}", "Plug"),            // cod-plug
            ("\u{e8b1}", "SSH"),             // dev-ssh
            ("\u{e7ad}", "AWS"),             // dev-aws
            ("\u{eac2}", "Cloud DL"),        // cod-cloud_download
            ("\u{eac3}", "Cloud UL"),        // cod-cloud_upload
            ("\u{f27a}", "Message"),         // fa-message
        ],
    ),
    (
        "Security",
        &[
            ("\u{f023}", "Lock"),            // fa-lock
            ("\u{eb74}", "Unlock"),          // cod-unlock
            ("\u{f132}", "Shield"),          // fa-shield
            ("\u{ed25}", "Shield Check"),    // fa-shield_halved
            ("\u{eb11}", "Key"),             // cod-key
            ("\u{f49c}", "Oct Shield"),      // oct-shield
            ("\u{ea70}", "Eye"),             // cod-eye
            ("\u{eae7}", "Eye Closed"),      // cod-eye_closed
            ("\u{f06a}", "Warning"),         // fa-exclamation_circle
            ("\u{f05a}", "Info"),            // fa-info_circle
            ("\u{edcf}", "User Shield"),     // fa-user_shield
            ("\u{f12e}", "Puzzle"),          // fa-puzzle_piece
        ],
    ),
    (
        "Git & VCS",
        &[
            ("\u{e725}", "Branch"),          // dev-git_branch
            ("\u{e727}", "Merge"),           // dev-git_merge
            ("\u{e729}", "Commit"),          // dev-git_commit
            ("\u{f09b}", "GitHub"),          // fa-github
            ("\u{e65c}", "GitLab"),          // seti-gitlab
            ("\u{e702}", "Git"),             // dev-git
            ("\u{e65d}", "Gitignore"),       // seti-git_ignore
            ("\u{e5fb}", "Git Folder"),      // custom-folder_git_branch
        ],
    ),
    (
        "Containers & Infra",
        &[
            ("\u{f308}", "Docker"),          // linux-docker
            ("\u{e81d}", "Kubernetes"),       // dev-kubernetes
            ("\u{f1b3}", "Cubes"),           // fa-cubes
            ("\u{f4b7}", "Container"),       // oct-container
            ("\u{f4bc}", "CPU"),             // oct-cpu
            ("\u{f2db}", "Chip"),            // fa-microchip
            ("\u{efc5}", "Memory"),          // fa-memory
            ("\u{f013}", "Gear"),            // fa-gear
            ("\u{f085}", "Gears"),           // fa-gears
            ("\u{f1de}", "Sliders"),         // fa-sliders
            ("\u{eb06}", "Home"),            // cod-home
            ("\u{f0e8}", "Sitemap"),         // fa-sitemap
        ],
    ),
    (
        "OS & Platforms",
        &[
            ("\u{f179}", "Apple"),           // fa-apple
            ("\u{f17a}", "Windows"),         // fa-windows
            ("\u{f17c}", "Linux"),           // fa-linux
            ("\u{f31a}", "Tux"),             // linux-tux
            ("\u{e712}", "Linux Dev"),       // dev-linux
            ("\u{e70f}", "Windows Dev"),     // dev-windows
            ("\u{e7ad}", "AWS"),             // dev-aws
            ("\u{e7e9}", "GitHub Actions"),  // dev-githubactions
            ("\u{e71e}", "npm"),             // dev-npm
            ("\u{e7fd}", "Homebrew"),        // dev-homebrew
        ],
    ),
    (
        "Status & Alerts",
        &[
            ("\u{f05d}", "Check"),           // fa-circle_check
            ("\u{f057}", "Times"),           // fa-times_circle
            ("\u{f06a}", "Exclamation"),     // fa-exclamation_circle
            ("\u{f0e7}", "Bolt"),            // fa-flash
            ("\u{f0eb}", "Lightbulb"),       // fa-lightbulb_o
            ("\u{f135}", "Rocket"),          // fa-rocket
            ("\u{f140}", "Crosshairs"),      // fa-bullseye
            ("\u{f06d}", "Fire"),            // fa-fire
            ("\u{f0f3}", "Bell"),            // fa-bell
            ("\u{f005}", "Star"),            // fa-star
            ("\u{eb05}", "Heart"),           // cod-heart
            ("\u{ea74}", "Info"),            // cod-info
        ],
    ),
    (
        "People & Misc",
        &[
            ("\u{f007}", "User"),            // fa-user
            ("\u{f0c0}", "Users"),           // fa-users
            ("\u{ea67}", "Person"),          // cod-person
            ("\u{ee0d}", "Robot"),           // fa-robot
            ("\u{f11b}", "Gamepad"),         // fa-gamepad
            ("\u{f001}", "Music"),           // fa-music
            ("\u{f030}", "Camera"),          // fa-camera
            ("\u{f1fc}", "Paint"),           // fa-paintbrush
            ("\u{f040}", "Pencil"),          // fa-pencil
            ("\u{f02e}", "Bookmark"),        // fa-bookmark
            ("\u{eb1c}", "Mail"),            // cod-mail
            ("\u{f29f}", "Diamond"),         // fa-diamond
        ],
    ),
];
```

**Step 2: Register the module in lib.rs**

In `par-term-settings-ui/src/lib.rs`, after line 33 (`pub use profile_modal_ui::{ProfileModalAction, ProfileModalUI};`), add:

```rust
// Nerd Font integration (font loading + icon presets)
pub mod nerd_font;
```

**Step 3: Verify it compiles**

```bash
cargo check -p par-term-settings-ui
```

**Step 4: Commit**

```bash
git add par-term-settings-ui/src/nerd_font.rs par-term-settings-ui/src/lib.rs
git commit -m "feat(settings-ui): add nerd_font module with font loading and icon presets"
```

---

### Task 3: Wire font loading into both egui contexts

**Files:**
- Modify: `src/app/renderer_init.rs:393-408` (main window egui context)
- Modify: `src/settings_window.rs:150-160` (settings window egui context)

**Step 1: Add font setup to main window (renderer_init.rs)**

After line 393 (`let egui_ctx = egui::Context::default();`), before the `if let Some(memory)` block, add:

```rust
        crate::settings_ui::nerd_font::configure_nerd_font(&egui_ctx);
```

The full block becomes:
```rust
        let egui_ctx = egui::Context::default();
        crate::settings_ui::nerd_font::configure_nerd_font(&egui_ctx);

        if let Some(memory) = previous_memory {
```

**Step 2: Add font setup to settings window (settings_window.rs)**

After line 152 (`let egui_ctx = egui::Context::default();`), add:

```rust
        crate::settings_ui::nerd_font::configure_nerd_font(&egui_ctx);
```

**Step 3: Verify it compiles and runs**

```bash
cargo check
```

Then do a quick visual test:
```bash
cargo run
```

Open the settings window and check that existing text still renders normally.

**Step 4: Commit**

```bash
git add src/app/renderer_init.rs src/settings_window.rs
git commit -m "feat: configure Nerd Font in both egui contexts (main + settings window)"
```

---

### Task 4: Update profile icon picker to use Nerd Font presets

**Files:**
- Modify: `par-term-settings-ui/src/profile_modal_ui.rs:10-45` (replace EMOJI_PRESETS)
- Modify: `par-term-settings-ui/src/profile_modal_ui.rs:825-860` (update picker UI)

**Step 1: Replace EMOJI_PRESETS with NERD_FONT_PRESETS import**

Delete the entire `EMOJI_PRESETS` constant (lines 10-45) and replace with:

```rust
use crate::nerd_font::NERD_FONT_PRESETS;
```

**Step 2: Update the picker UI loop**

Replace the emoji picker popup body (the `for` loop iterating over `EMOJI_PRESETS`, approximately lines 830-853) with a loop over `NERD_FONT_PRESETS`. The new format includes labels as tooltips:

```rust
                                            for (category, icons) in NERD_FONT_PRESETS {
                                                ui.label(
                                                    egui::RichText::new(*category)
                                                        .small()
                                                        .strong(),
                                                );
                                                ui.horizontal_wrapped(|ui| {
                                                    for (icon, label) in *icons {
                                                        let btn = ui.add_sized(
                                                            [28.0, 28.0],
                                                            egui::Button::new(
                                                                egui::RichText::new(*icon)
                                                                    .size(16.0),
                                                            )
                                                            .frame(false),
                                                        );
                                                        if btn.on_hover_text(*label).clicked() {
                                                            self.temp_icon =
                                                                icon.to_string();
                                                            egui::Popup::close_all(
                                                                ui.ctx(),
                                                            );
                                                        }
                                                    }
                                                });
                                                ui.add_space(2.0);
                                            }
```

Key changes from the emoji version:
- Iterates `(icon, label)` tuples instead of plain `&str`
- Uses `RichText::new(*icon).size(16.0)` for consistent icon sizing
- Adds `.on_hover_text(*label)` tooltip so users can identify icons

**Step 3: Update the picker button label fallback**

Change the fallback icon (line ~816) from emoji to a Nerd Font icon:

```rust
                            let picker_label = if self.temp_icon.is_empty() {
                                "\u{ea7b}" // cod-file (Nerd Font default)
                            } else {
                                &self.temp_icon
                            };
```

**Step 4: Verify it compiles**

```bash
cargo check
```

**Step 5: Run and visually test**

```bash
cargo run
```

1. Open Settings → Profiles → Edit a profile
2. Click the icon picker button
3. Verify all icons render (no blank boxes)
4. Hover over icons to see labels
5. Select an icon, verify it appears in the text field and on the picker button
6. Create a new tab with the profile, verify the icon shows in the tab bar

**Step 6: Commit**

```bash
git add par-term-settings-ui/src/profile_modal_ui.rs
git commit -m "feat(profiles): replace emoji presets with Nerd Font icons in picker"
```

---

### Task 5: Update the main crate's profile_modal_ui.rs (if duplicated)

**Files:**
- Check: `src/profile_modal_ui.rs` — there appears to be a copy in the main crate

The grep results showed `src/profile_modal_ui.rs` also contains `EMOJI_PRESETS`. Check if this is a re-export or a separate copy. If it's a separate copy, apply the same changes as Task 4. If it re-exports from par-term-settings-ui, no changes needed.

**Step 1: Check if the file has its own EMOJI_PRESETS or imports from settings-ui**

Read `src/profile_modal_ui.rs` and determine if changes are needed.

**Step 2: Apply same changes as Task 4 if needed**

**Step 3: Verify and commit**

```bash
cargo check
git add src/profile_modal_ui.rs
git commit -m "feat(profiles): update main crate profile_modal_ui to use Nerd Font icons"
```

---

### Task 6: Run full checks and final commit

**Step 1: Format**

```bash
cargo fmt
```

**Step 2: Lint**

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

**Step 3: Test**

```bash
cargo test
```

**Step 4: Fix any issues from the above**

**Step 5: Final commit if needed**

```bash
git add -A
git commit -m "chore: format and lint fixes for Nerd Font integration"
```
