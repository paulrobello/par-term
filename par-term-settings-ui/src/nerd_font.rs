//! Nerd Font integration for egui.
//!
//! Provides font configuration and curated icon presets for the profile icon picker.
//! Uses SymbolsNerdFontMono-Regular.ttf (Nerd Fonts v3.4.0).

/// Embedded Nerd Font Symbols (Mono variant, ~2.5MB).
const NERD_FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/SymbolsNerdFontMono-Regular.ttf");

/// The full fallback stack par-term ships on top of egui's defaults: the
/// Braille fallback and the Nerd Font symbols, last in the Proportional and
/// Monospace families. One builder so a late font install composes with it —
/// `Context::set_fonts` replaces the whole stack, so a caller starting from
/// `FontDefinitions::default()` silently drops these fallbacks (the IME CJK
/// overlay once did; tab icons and spinner braille then rendered as □).
pub fn font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "nerd_font_symbols".to_owned(),
        egui::FontData::from_static(NERD_FONT_BYTES).into(),
    );

    // A system font covering the Braille Patterns block (U+2800–U+28FF) so
    // CLI spinner characters (⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏) render in the tab bar. None of
    // egui's default fonts nor SymbolsNerdFontMono cover the block.
    if let Some(braille_bytes) = load_braille_font() {
        fonts.font_data.insert(
            "braille_fallback".to_owned(),
            egui::FontData::from_owned(braille_bytes).into(),
        );
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("braille_fallback".to_owned());
        }
    }

    // Nerd Font last, so Nerd Font codepoints render everywhere (profile
    // and tab icons, badges) while standard Latin text keeps egui's default.
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("nerd_font_symbols".to_owned());
    }
    fonts
}

/// Configure egui to use Nerd Font Symbols as a fallback font.
///
/// Call this once after creating each `egui::Context` (main window and settings window).
pub fn configure_nerd_font(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions());
}

/// Whether the embedded nerd-font symbols fallback carries `ch`.
///
/// The in-repo coverage oracle for icon glyphs drawn in egui text: nerd PUA
/// is the only icon vocabulary every shipped stack carries, and glyphs like
/// U+2713/U+2715 sit in NO font of the stack (egui defaults, braille fallback,
/// this font) and render as tofu. Icon-code tests assert against this instead
/// of trusting a literal looks right.
pub fn nerd_font_covers(ch: char) -> bool {
    static COVERED: std::sync::OnceLock<std::collections::HashSet<char>> =
        std::sync::OnceLock::new();
    COVERED.get_or_init(parse_cmap_codepoints).contains(&ch)
}

/// Every codepoint the embedded TTF maps to a non-.notdef glyph (cmap
/// formats 4 and 12, exact glyph-id resolution). An empty set means the
/// bytes failed to parse — tests then fail loudly rather than pass vacuously.
fn parse_cmap_codepoints() -> std::collections::HashSet<char> {
    let mut covered = std::collections::HashSet::new();
    let data = NERD_FONT_BYTES;
    let be16 = |off: usize| -> Option<u16> {
        data.get(off..off + 2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
    };
    let be32 = |off: usize| -> Option<u32> {
        data.get(off..off + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    let Some(num_tables) = be16(4) else {
        return covered;
    };
    let mut cmap_off = None;
    for i in 0..num_tables as usize {
        let rec = 12 + 16 * i;
        if data.get(rec..rec + 4) == Some(b"cmap")
            && let Some(off) = be32(rec + 8)
        {
            cmap_off = Some(off as usize);
            break;
        }
    }
    let Some(cmap) = cmap_off else {
        return covered;
    };
    let Some(num_sub) = be16(cmap + 2) else {
        return covered;
    };
    for i in 0..num_sub as usize {
        let rec = cmap + 4 + 8 * i;
        let (Some(_platform), Some(_encoding), Some(sub_off)) =
            (be16(rec), be16(rec + 2), be32(rec + 4))
        else {
            continue;
        };
        let b = cmap + sub_off as usize;
        let Some(fmt) = be16(b) else { continue };
        if fmt == 4 {
            let Some(seg_x2) = be16(b + 6) else { continue };
            let seg = seg_x2 as usize / 2;
            let ends_off = b + 14;
            let starts_off = ends_off + seg_x2 as usize + 2;
            let deltas_off = starts_off + seg_x2 as usize;
            let ranges_off = deltas_off + seg_x2 as usize;
            for si in 0..seg {
                let (Some(s), Some(e), Some(delta), Some(ro)) = (
                    be16(starts_off + 2 * si),
                    be16(ends_off + 2 * si),
                    be16(deltas_off + 2 * si),
                    be16(ranges_off + 2 * si),
                ) else {
                    continue;
                };
                if s == 0xFFFF {
                    continue;
                }
                for cp in s..=e {
                    let gid = if ro == 0 {
                        cp.wrapping_sub(s).wrapping_add(delta)
                    } else {
                        // idRangeOffset is relative to THIS entry's address.
                        let addr = ranges_off + 2 * si + ro as usize + 2 * (cp - s) as usize;
                        let Some(raw) = be16(addr) else { continue };
                        if raw == 0 { 0 } else { raw.wrapping_add(delta) }
                    };
                    if gid != 0
                        && let Some(ch) = char::from_u32(cp as u32)
                    {
                        covered.insert(ch);
                    }
                }
            }
        } else if fmt == 12 {
            let Some(n_groups) = be32(b + 12) else {
                continue;
            };
            for g in 0..n_groups as usize {
                let go = b + 16 + 12 * g;
                let (Some(s), Some(e), Some(gs)) = (be32(go), be32(go + 4), be32(go + 8)) else {
                    continue;
                };
                for cp in s..=e {
                    if gs + (cp - s) != 0
                        && let Some(ch) = char::from_u32(cp)
                    {
                        covered.insert(ch);
                    }
                }
            }
        }
    }
    covered
}

/// Try to find a system font that covers the Braille Patterns Unicode block (U+2800–U+28FF).
///
/// Returns the font file bytes if a suitable font is found, or `None` if no font is available.
/// The candidates are platform-specific well-known fonts that include the full Braille block.
fn load_braille_font() -> Option<Vec<u8>> {
    for path in braille_font_candidates() {
        if let Ok(data) = std::fs::read(path) {
            return Some(data);
        }
    }
    None
}

/// Platform-specific candidate paths for fonts that cover the Braille Patterns block.
fn braille_font_candidates() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        &[
            // Apple Braille — ships with every macOS, covers all 256 Braille patterns
            "/System/Library/Fonts/Apple Braille.ttf",
            "/System/Library/Fonts/Apple Braille Outline 6 Dot.ttf",
        ]
    }
    #[cfg(target_os = "linux")]
    {
        &[
            // DejaVu Sans Mono — excellent Unicode coverage including full Braille block
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
            // GNU FreeFont — also covers Braille
            "/usr/share/fonts/truetype/freefont/FreeMono.ttf",
            "/usr/share/fonts/gnu-free/FreeMono.ttf",
        ]
    }
    #[cfg(target_os = "windows")]
    {
        &[
            // Segoe UI Symbol covers Braille on modern Windows
            r"C:\Windows\Fonts\seguisym.ttf",
        ]
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        &[]
    }
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
            ("\u{e74a}", "TypeScript"),
            ("\u{e724}", "Go"),
            ("\u{e61d}", "C"),
            ("\u{e646}", "C++"),
            ("\u{e753}", "Angular"),
            ("\u{e6a0}", "Vue.js"),
            ("\u{e697}", "Svelte"),
            ("\u{e736}", "HTML5"),
            ("\u{e7a6}", "CSS3"),
            ("\u{e739}", "Haskell"),
            ("\u{e737}", "Scala"),
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
            ("\u{ea64}", "Pull Request"),
            ("\u{e72a}", "Bitbucket"),
            ("\u{ea6d}", "Diff"),
        ],
    ),
    (
        "Weather & Nature",
        &[
            ("\u{f185}", "Sun"),
            ("\u{f186}", "Moon"),
            ("\u{f2dc}", "Snowflake"),
            ("\u{f0e9}", "Umbrella"),
            ("\u{f0e7}", "Lightning"),
            ("\u{f06c}", "Leaf"),
            ("\u{f1bb}", "Tree"),
            ("\u{f2c9}", "Thermometer"),
            ("\u{f1b0}", "Paw"),
            ("\u{e30d}", "Day Sunny"),
            ("\u{e308}", "Rainy"),
            ("\u{e31a}", "Night Clear"),
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
            ("\u{f31b}", "Ubuntu"),
            ("\u{f303}", "Arch"),
            ("\u{f315}", "Raspi"),
            ("\u{f30a}", "Fedora"),
            ("\u{f17b}", "Android"),
        ],
    ),
    (
        "Status & Alerts",
        &[
            ("\u{f05d}", "Check"),
            ("\u{f057}", "Times"),
            ("\u{f058}", "Check Circle"),
            ("\u{f056}", "Minus Circle"),
            ("\u{f059}", "Question Circle"),
            ("\u{f06a}", "Exclamation"),
            ("\u{f071}", "Warning"),
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
        "UI Actions",
        &[
            ("\u{f002}", "Search"),
            ("\u{f044}", "Edit"),
            ("\u{f0c5}", "Copy"),
            ("\u{f0ea}", "Clipboard"),
            ("\u{f0c4}", "Cut"),
            ("\u{f1f8}", "Trash"),
            ("\u{f067}", "Plus"),
            ("\u{f00d}", "Close"),
            ("\u{f021}", "Refresh"),
            ("\u{f0b0}", "Filter"),
            ("\u{f03a}", "List"),
            ("\u{f0c1}", "Link"),
            ("\u{f08e}", "External Link"),
            ("\u{eb4b}", "Save"),
            ("\u{f00c}", "Apply"),
            ("\u{f05e}", "Ban/Cancel"),
        ],
    ),
    (
        "Navigation",
        &[
            ("\u{f062}", "Arrow Up"),
            ("\u{f063}", "Arrow Down"),
            ("\u{f060}", "Arrow Left"),
            ("\u{f061}", "Arrow Right"),
            ("\u{f106}", "Angle Up"),
            ("\u{f107}", "Angle Down"),
            ("\u{f104}", "Angle Left"),
            ("\u{f105}", "Angle Right"),
            ("\u{f0d8}", "Caret Up"),
            ("\u{f0d7}", "Caret Down"),
            ("\u{f176}", "Long Arrow Up"),
            ("\u{f175}", "Long Arrow Down"),
            ("\u{f112}", "Reply/Back"),
            ("\u{f148}", "Level Up"),
            ("\u{f149}", "Level Down"),
            ("\u{f01e}", "Rotate/Undo"),
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
    (
        "Fun & Seasonal",
        &[
            ("\u{f091}", "Trophy"),
            ("\u{f521}", "Crown"),
            ("\u{f1fd}", "Birthday Cake"),
            ("\u{f06b}", "Gift"),
            ("\u{eefe}", "Ghost"),
            ("\u{ee15}", "Skull"),
            ("\u{eeed}", "Cat"),
            ("\u{eef7}", "Dog"),
            ("\u{eef8}", "Dragon"),
            ("\u{f0d0}", "Magic Wand"),
            ("\u{f1e2}", "Bomb"),
            ("\u{f0fc}", "Beer"),
            ("\u{f0f4}", "Coffee"),
            ("\u{ef8c}", "Pizza"),
            ("\u{f522}", "Dice"),
            ("\u{eb2b}", "Sparkle"),
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_carries_the_nerd_glyphs_used_as_replacements() {
        for ch in [
            '\u{f00c}', // check (toast Success, tab-title ✅ sub)
            '\u{f00d}', // close/times (toast Error, dismiss buttons, ❌ sub)
            '\u{f002}', // search (tab-title 🔍 sub)
            '\u{f005}', // star (tab-title ✨ sub)
            '\u{f0c3}', // flask (tab-title 🧪 sub)
            '\u{f24d}', // clone (session chip, prior fix)
        ] {
            assert!(
                nerd_font_covers(ch),
                "U+{:04X} must be nerd-covered",
                ch as u32
            );
        }
    }

    #[test]
    fn oracle_lacks_the_dead_glyphs_and_any_latin() {
        // Verified against the shipped stack 2026-10-12: these five sit in NO
        // font (egui defaults, braille fallback, nerd symbols) — using any of
        // them in egui text renders tofu. The font is symbols-only, so Latin
        // is absent too — the negative control that keeps the parser honest.
        for ch in [
            '\u{2713}', '\u{2715}', '\u{2726}', '\u{2315}', '\u{2697}', 'A',
        ] {
            assert!(
                !nerd_font_covers(ch),
                "U+{:04X} unexpectedly covered; oracle is wrong",
                ch as u32
            );
        }
    }

    #[test]
    fn settings_ui_close_button_glyphs_are_carried() {
        // The dismiss buttons in settings_ui/banner.rs and
        // snippets_tab/editor.rs use U+F00D — never revert them to ✕.
        assert!(nerd_font_covers('\u{f00d}'));
    }
}
