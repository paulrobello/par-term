//! Nerd Font integration for egui.
//!
//! Provides font configuration and curated icon presets for the profile icon picker.
//! Uses SymbolsNerdFontMono-Regular.ttf (Nerd Fonts v3.4.0).

/// Embedded Nerd Font Symbols (Mono variant, ~2.5MB).
const NERD_FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/SymbolsNerdFontMono-Regular.ttf");

/// Configure egui to use Nerd Font Symbols as a fallback font.
///
/// Call this once after creating each `egui::Context` (main window and settings window).
/// Adds the Nerd Font as the last fallback in the Proportional and Monospace families
/// so that standard Latin text still uses egui's default font, but Nerd Font codepoints render.
pub fn configure_nerd_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "nerd_font_symbols".to_owned(),
        egui::FontData::from_static(NERD_FONT_BYTES).into(),
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
            ("\u{e795}", "Terminal"),
            ("\u{ebca}", "Bash"),
            ("\u{ebc7}", "PowerShell"),
            ("\u{ebc8}", "tmux"),
            ("\u{ea85}", "Console"),
            ("\u{ebc6}", "Linux Term"),
            ("\u{ebc5}", "Debian Term"),
            ("\u{ebc4}", "Cmd"),
            ("\u{f120}", "Prompt"),
            ("\u{e84f}", "Oh My Zsh"),
            ("\u{f489}", "Octicons Term"),
        ],
    ),
    (
        "Dev & Tools",
        &[
            ("\u{f121}", "Code"),
            ("\u{f09b}", "GitHub"),
            ("\u{e7ba}", "React"),
            ("\u{e73c}", "Python"),
            ("\u{e7a8}", "Rust"),
            ("\u{e718}", "Node.js"),
            ("\u{e738}", "Java"),
            ("\u{e755}", "Swift"),
            ("\u{e81b}", "Kotlin"),
            ("\u{e826}", "Lua"),
            ("\u{e73d}", "PHP"),
            ("\u{e605}", "Ruby"),
            ("\u{e62b}", "Vim"),
            ("\u{e6ae}", "Neovim"),
            ("\u{f188}", "Bug"),
            ("\u{f0ad}", "Wrench"),
        ],
    ),
    (
        "Files & Data",
        &[
            ("\u{ea7b}", "File"),
            ("\u{eae9}", "File Code"),
            ("\u{ea83}", "Folder"),
            ("\u{eaf7}", "Folder Open"),
            ("\u{f1c0}", "Database"),
            ("\u{eb4b}", "Save"),
            ("\u{f02d}", "Book"),
            ("\u{ea66}", "Tag"),
            ("\u{f1b2}", "Cube"),
            ("\u{f487}", "Package"),
            ("\u{f019}", "Download"),
            ("\u{f093}", "Upload"),
        ],
    ),
    (
        "Network & Cloud",
        &[
            ("\u{f0ac}", "Globe"),
            ("\u{f1eb}", "WiFi"),
            ("\u{ebaa}", "Cloud"),
            ("\u{f233}", "Server"),
            ("\u{ef09}", "Network"),
            ("\u{f0e8}", "Sitemap"),
            ("\u{eb2d}", "Plug"),
            ("\u{e8b1}", "SSH"),
            ("\u{e7ad}", "AWS"),
            ("\u{eac2}", "Cloud DL"),
            ("\u{eac3}", "Cloud UL"),
            ("\u{f27a}", "Message"),
        ],
    ),
    (
        "Security",
        &[
            ("\u{f023}", "Lock"),
            ("\u{eb74}", "Unlock"),
            ("\u{f132}", "Shield"),
            ("\u{ed25}", "Shield Check"),
            ("\u{eb11}", "Key"),
            ("\u{f49c}", "Oct Shield"),
            ("\u{ea70}", "Eye"),
            ("\u{eae7}", "Eye Closed"),
            ("\u{f06a}", "Warning"),
            ("\u{f05a}", "Info"),
            ("\u{edcf}", "User Shield"),
            ("\u{f12e}", "Puzzle"),
        ],
    ),
    (
        "Git & VCS",
        &[
            ("\u{e725}", "Branch"),
            ("\u{e727}", "Merge"),
            ("\u{e729}", "Commit"),
            ("\u{f09b}", "GitHub"),
            ("\u{e65c}", "GitLab"),
            ("\u{e702}", "Git"),
            ("\u{e65d}", "Gitignore"),
            ("\u{e5fb}", "Git Folder"),
        ],
    ),
    (
        "Containers & Infra",
        &[
            ("\u{f308}", "Docker"),
            ("\u{e81d}", "Kubernetes"),
            ("\u{f1b3}", "Cubes"),
            ("\u{f4b7}", "Container"),
            ("\u{f4bc}", "CPU"),
            ("\u{f2db}", "Chip"),
            ("\u{efc5}", "Memory"),
            ("\u{f013}", "Gear"),
            ("\u{f085}", "Gears"),
            ("\u{f1de}", "Sliders"),
            ("\u{eb06}", "Home"),
            ("\u{f0e8}", "Sitemap"),
        ],
    ),
    (
        "OS & Platforms",
        &[
            ("\u{f179}", "Apple"),
            ("\u{f17a}", "Windows"),
            ("\u{f17c}", "Linux"),
            ("\u{f31a}", "Tux"),
            ("\u{e712}", "Linux Dev"),
            ("\u{e70f}", "Windows Dev"),
            ("\u{e7ad}", "AWS"),
            ("\u{e7e9}", "GitHub Actions"),
            ("\u{e71e}", "npm"),
            ("\u{e7fd}", "Homebrew"),
        ],
    ),
    (
        "Status & Alerts",
        &[
            ("\u{f05d}", "Check"),
            ("\u{f057}", "Times"),
            ("\u{f06a}", "Exclamation"),
            ("\u{f0e7}", "Bolt"),
            ("\u{f0eb}", "Lightbulb"),
            ("\u{f135}", "Rocket"),
            ("\u{f140}", "Crosshairs"),
            ("\u{f06d}", "Fire"),
            ("\u{f0f3}", "Bell"),
            ("\u{f005}", "Star"),
            ("\u{eb05}", "Heart"),
            ("\u{ea74}", "Info"),
        ],
    ),
    (
        "People & Misc",
        &[
            ("\u{f007}", "User"),
            ("\u{f0c0}", "Users"),
            ("\u{ea67}", "Person"),
            ("\u{ee0d}", "Robot"),
            ("\u{f11b}", "Gamepad"),
            ("\u{f001}", "Music"),
            ("\u{f030}", "Camera"),
            ("\u{f1fc}", "Paint"),
            ("\u{f040}", "Pencil"),
            ("\u{f02e}", "Bookmark"),
            ("\u{eb1c}", "Mail"),
            ("\u{f29f}", "Diamond"),
        ],
    ),
];
