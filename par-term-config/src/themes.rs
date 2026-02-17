/// Color theme definitions for the terminal
use serde::{Deserialize, Serialize};

/// A color in RGB format
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    #[allow(dead_code)]
    pub fn as_array(&self) -> [u8; 3] {
        [self.r, self.g, self.b]
    }
}

/// Terminal color theme with 16 ANSI colors plus foreground/background
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub foreground: Color,
    pub background: Color,
    pub cursor: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,

    // ANSI colors (0-15)
    pub black: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
    pub white: Color,
    pub bright_black: Color,
    pub bright_red: Color,
    pub bright_green: Color,
    pub bright_yellow: Color,
    pub bright_blue: Color,
    pub bright_magenta: Color,
    pub bright_cyan: Color,
    pub bright_white: Color,
}

impl Theme {
    /// Get ANSI color by index (0-15)
    #[allow(dead_code)]
    pub fn ansi_color(&self, index: u8) -> Color {
        match index {
            0 => self.black,
            1 => self.red,
            2 => self.green,
            3 => self.yellow,
            4 => self.blue,
            5 => self.magenta,
            6 => self.cyan,
            7 => self.white,
            8 => self.bright_black,
            9 => self.bright_red,
            10 => self.bright_green,
            11 => self.bright_yellow,
            12 => self.bright_blue,
            13 => self.bright_magenta,
            14 => self.bright_cyan,
            15 => self.bright_white,
            _ => self.foreground,
        }
    }

    /// Dracula theme
    pub fn dracula() -> Self {
        Self {
            name: "Dracula".to_string(),
            foreground: Color::new(248, 248, 242),
            background: Color::new(40, 42, 54),
            cursor: Color::new(248, 248, 240),
            selection_bg: Color::new(68, 71, 90),
            selection_fg: Color::new(248, 248, 242),
            black: Color::new(0, 0, 0),
            red: Color::new(255, 85, 85),
            green: Color::new(80, 250, 123),
            yellow: Color::new(241, 250, 140),
            blue: Color::new(189, 147, 249),
            magenta: Color::new(255, 121, 198),
            cyan: Color::new(139, 233, 253),
            white: Color::new(255, 255, 255),
            bright_black: Color::new(98, 114, 164),
            bright_red: Color::new(255, 110, 103),
            bright_green: Color::new(90, 247, 142),
            bright_yellow: Color::new(244, 244, 161),
            bright_blue: Color::new(189, 147, 249),
            bright_magenta: Color::new(255, 121, 198),
            bright_cyan: Color::new(139, 233, 253),
            bright_white: Color::new(255, 255, 255),
        }
    }

    /// Solarized Dark theme
    pub fn solarized_dark() -> Self {
        Self {
            name: "Solarized Dark".to_string(),
            foreground: Color::new(131, 148, 150),
            background: Color::new(0, 43, 54),
            cursor: Color::new(147, 161, 161),
            selection_bg: Color::new(7, 54, 66),
            selection_fg: Color::new(147, 161, 161),
            black: Color::new(7, 54, 66),
            red: Color::new(220, 50, 47),
            green: Color::new(133, 153, 0),
            yellow: Color::new(181, 137, 0),
            blue: Color::new(38, 139, 210),
            magenta: Color::new(211, 54, 130),
            cyan: Color::new(42, 161, 152),
            white: Color::new(238, 232, 213),
            bright_black: Color::new(0, 43, 54),
            bright_red: Color::new(203, 75, 22),
            bright_green: Color::new(88, 110, 117),
            bright_yellow: Color::new(101, 123, 131),
            bright_blue: Color::new(131, 148, 150),
            bright_magenta: Color::new(108, 113, 196),
            bright_cyan: Color::new(147, 161, 161),
            bright_white: Color::new(253, 246, 227),
        }
    }

    /// Nord theme
    pub fn nord() -> Self {
        Self {
            name: "Nord".to_string(),
            foreground: Color::new(216, 222, 233),
            background: Color::new(46, 52, 64),
            cursor: Color::new(216, 222, 233),
            selection_bg: Color::new(59, 66, 82),
            selection_fg: Color::new(216, 222, 233),
            black: Color::new(59, 66, 82),
            red: Color::new(191, 97, 106),
            green: Color::new(163, 190, 140),
            yellow: Color::new(235, 203, 139),
            blue: Color::new(129, 161, 193),
            magenta: Color::new(180, 142, 173),
            cyan: Color::new(136, 192, 208),
            white: Color::new(229, 233, 240),
            bright_black: Color::new(76, 86, 106),
            bright_red: Color::new(191, 97, 106),
            bright_green: Color::new(163, 190, 140),
            bright_yellow: Color::new(235, 203, 139),
            bright_blue: Color::new(129, 161, 193),
            bright_magenta: Color::new(180, 142, 173),
            bright_cyan: Color::new(143, 188, 187),
            bright_white: Color::new(236, 239, 244),
        }
    }

    /// Monokai theme
    pub fn monokai() -> Self {
        Self {
            name: "Monokai".to_string(),
            foreground: Color::new(248, 248, 242),
            background: Color::new(39, 40, 34),
            cursor: Color::new(253, 254, 236),
            selection_bg: Color::new(73, 72, 62),
            selection_fg: Color::new(248, 248, 242),
            black: Color::new(39, 40, 34),
            red: Color::new(249, 38, 114),
            green: Color::new(166, 226, 46),
            yellow: Color::new(244, 191, 117),
            blue: Color::new(102, 217, 239),
            magenta: Color::new(174, 129, 255),
            cyan: Color::new(161, 239, 228),
            white: Color::new(248, 248, 242),
            bright_black: Color::new(117, 113, 94),
            bright_red: Color::new(249, 38, 114),
            bright_green: Color::new(166, 226, 46),
            bright_yellow: Color::new(244, 191, 117),
            bright_blue: Color::new(102, 217, 239),
            bright_magenta: Color::new(174, 129, 255),
            bright_cyan: Color::new(161, 239, 228),
            bright_white: Color::new(249, 248, 245),
        }
    }

    /// One Dark theme
    pub fn one_dark() -> Self {
        Self {
            name: "One Dark".to_string(),
            foreground: Color::new(171, 178, 191),
            background: Color::new(40, 44, 52),
            cursor: Color::new(171, 178, 191),
            selection_bg: Color::new(56, 61, 71),
            selection_fg: Color::new(171, 178, 191),
            black: Color::new(40, 44, 52),
            red: Color::new(224, 108, 117),
            green: Color::new(152, 195, 121),
            yellow: Color::new(229, 192, 123),
            blue: Color::new(97, 175, 239),
            magenta: Color::new(198, 120, 221),
            cyan: Color::new(86, 182, 194),
            white: Color::new(171, 178, 191),
            bright_black: Color::new(92, 99, 112),
            bright_red: Color::new(224, 108, 117),
            bright_green: Color::new(152, 195, 121),
            bright_yellow: Color::new(229, 192, 123),
            bright_blue: Color::new(97, 175, 239),
            bright_magenta: Color::new(198, 120, 221),
            bright_cyan: Color::new(86, 182, 194),
            bright_white: Color::new(255, 255, 255),
        }
    }

    /// Default dark background theme
    pub fn default_dark() -> Self {
        Self {
            name: "Default Dark".to_string(),
            foreground: Color::new(255, 255, 255),
            background: Color::new(0, 0, 0),
            cursor: Color::new(255, 255, 255),
            selection_bg: Color::new(100, 100, 100),
            selection_fg: Color::new(255, 255, 255),
            black: Color::new(0, 0, 0),
            red: Color::new(205, 0, 0),
            green: Color::new(0, 205, 0),
            yellow: Color::new(205, 205, 0),
            blue: Color::new(0, 0, 238),
            magenta: Color::new(205, 0, 205),
            cyan: Color::new(0, 205, 205),
            white: Color::new(229, 229, 229),
            bright_black: Color::new(127, 127, 127),
            bright_red: Color::new(255, 0, 0),
            bright_green: Color::new(0, 255, 0),
            bright_yellow: Color::new(255, 255, 0),
            bright_blue: Color::new(92, 92, 255),
            bright_magenta: Color::new(255, 0, 255),
            bright_cyan: Color::new(0, 255, 255),
            bright_white: Color::new(255, 255, 255),
        }
    }

    /// Dark Background theme (iTerm2)
    pub fn dark_background() -> Self {
        Self {
            name: "Dark Background".to_string(),
            foreground: Color::new(187, 187, 187),
            background: Color::new(0, 0, 0),
            cursor: Color::new(187, 187, 187),
            selection_bg: Color::new(181, 213, 255),
            selection_fg: Color::new(0, 0, 0),
            black: Color::new(0, 0, 0),
            red: Color::new(187, 0, 0),
            green: Color::new(0, 187, 0),
            yellow: Color::new(187, 187, 0),
            blue: Color::new(0, 0, 187),
            magenta: Color::new(187, 0, 187),
            cyan: Color::new(0, 187, 187),
            white: Color::new(187, 187, 187),
            bright_black: Color::new(85, 85, 85),
            bright_red: Color::new(255, 85, 85),
            bright_green: Color::new(85, 255, 85),
            bright_yellow: Color::new(255, 255, 85),
            bright_blue: Color::new(85, 85, 255),
            bright_magenta: Color::new(255, 85, 255),
            bright_cyan: Color::new(85, 255, 255),
            bright_white: Color::new(255, 255, 255),
        }
    }

    /// High Contrast theme
    pub fn high_contrast() -> Self {
        Self {
            name: "High Contrast".to_string(),
            foreground: Color::new(255, 255, 255),
            background: Color::new(0, 0, 0),
            cursor: Color::new(255, 255, 255),
            selection_bg: Color::new(51, 51, 51),
            selection_fg: Color::new(255, 255, 255),
            black: Color::new(0, 0, 0),
            red: Color::new(255, 0, 0),
            green: Color::new(0, 255, 0),
            yellow: Color::new(255, 255, 0),
            blue: Color::new(0, 0, 255),
            magenta: Color::new(255, 0, 255),
            cyan: Color::new(0, 255, 255),
            white: Color::new(255, 255, 255),
            bright_black: Color::new(127, 127, 127),
            bright_red: Color::new(255, 127, 127),
            bright_green: Color::new(127, 255, 127),
            bright_yellow: Color::new(255, 255, 127),
            bright_blue: Color::new(127, 127, 255),
            bright_magenta: Color::new(255, 127, 255),
            bright_cyan: Color::new(127, 255, 255),
            bright_white: Color::new(255, 255, 255),
        }
    }

    /// Light Background theme (iTerm2)
    pub fn light_background() -> Self {
        Self {
            name: "Light Background".to_string(),
            foreground: Color::new(0, 0, 0),
            background: Color::new(255, 255, 255),
            cursor: Color::new(0, 0, 0),
            selection_bg: Color::new(203, 228, 255),
            selection_fg: Color::new(0, 0, 0),
            black: Color::new(0, 0, 0),
            red: Color::new(187, 0, 0),
            green: Color::new(0, 187, 0),
            yellow: Color::new(187, 187, 0),
            blue: Color::new(0, 0, 187),
            magenta: Color::new(187, 0, 187),
            cyan: Color::new(0, 187, 187),
            white: Color::new(187, 187, 187),
            bright_black: Color::new(85, 85, 85),
            bright_red: Color::new(255, 85, 85),
            bright_green: Color::new(85, 255, 85),
            bright_yellow: Color::new(255, 255, 85),
            bright_blue: Color::new(85, 85, 255),
            bright_magenta: Color::new(255, 85, 255),
            bright_cyan: Color::new(85, 255, 255),
            bright_white: Color::new(255, 255, 255),
        }
    }

    /// Pastel theme (Dark Background)
    pub fn pastel_dark() -> Self {
        Self {
            name: "Pastel (Dark Background)".to_string(),
            foreground: Color::new(187, 187, 187),
            background: Color::new(0, 0, 0),
            cursor: Color::new(255, 165, 96),
            selection_bg: Color::new(54, 57, 131),
            selection_fg: Color::new(242, 242, 242),
            black: Color::new(79, 79, 79),
            red: Color::new(255, 108, 96),
            green: Color::new(168, 255, 96),
            yellow: Color::new(255, 255, 182),
            blue: Color::new(150, 203, 254),
            magenta: Color::new(255, 115, 253),
            cyan: Color::new(198, 197, 254),
            white: Color::new(238, 238, 238),
            bright_black: Color::new(124, 124, 124),
            bright_red: Color::new(255, 182, 176),
            bright_green: Color::new(206, 255, 172),
            bright_yellow: Color::new(255, 255, 204),
            bright_blue: Color::new(181, 220, 255),
            bright_magenta: Color::new(255, 156, 254),
            bright_cyan: Color::new(223, 223, 254),
            bright_white: Color::new(255, 255, 255),
        }
    }

    /// Regular theme (light)
    pub fn regular() -> Self {
        Self {
            name: "Regular".to_string(),
            foreground: Color::new(16, 16, 16),
            background: Color::new(250, 250, 250),
            cursor: Color::new(0, 0, 0),
            selection_bg: Color::new(179, 215, 255),
            selection_fg: Color::new(0, 0, 0),
            black: Color::new(20, 25, 30),
            red: Color::new(180, 60, 42),
            green: Color::new(0, 194, 0),
            yellow: Color::new(199, 196, 0),
            blue: Color::new(39, 68, 199),
            magenta: Color::new(192, 64, 190),
            cyan: Color::new(0, 197, 199),
            white: Color::new(199, 199, 199),
            bright_black: Color::new(104, 104, 104),
            bright_red: Color::new(221, 121, 117),
            bright_green: Color::new(88, 231, 144),
            bright_yellow: Color::new(236, 225, 0),
            bright_blue: Color::new(167, 171, 242),
            bright_magenta: Color::new(225, 126, 225),
            bright_cyan: Color::new(96, 253, 255),
            bright_white: Color::new(255, 255, 255),
        }
    }

    /// Smoooooth theme (dark)
    pub fn smoooooth() -> Self {
        Self {
            name: "Smoooooth".to_string(),
            foreground: Color::new(220, 220, 220),
            background: Color::new(21, 25, 31),
            cursor: Color::new(255, 255, 255),
            selection_bg: Color::new(179, 215, 255),
            selection_fg: Color::new(0, 0, 0),
            black: Color::new(20, 25, 30),
            red: Color::new(180, 60, 42),
            green: Color::new(0, 194, 0),
            yellow: Color::new(199, 196, 0),
            blue: Color::new(39, 68, 199),
            magenta: Color::new(192, 64, 190),
            cyan: Color::new(0, 197, 199),
            white: Color::new(199, 199, 199),
            bright_black: Color::new(104, 104, 104),
            bright_red: Color::new(221, 121, 117),
            bright_green: Color::new(88, 231, 144),
            bright_yellow: Color::new(236, 225, 0),
            bright_blue: Color::new(167, 171, 242),
            bright_magenta: Color::new(225, 126, 225),
            bright_cyan: Color::new(96, 253, 255),
            bright_white: Color::new(255, 255, 255),
        }
    }

    /// Solarized base theme (dark)
    pub fn solarized() -> Self {
        Self {
            name: "Solarized".to_string(),
            foreground: Color::new(131, 148, 150),
            background: Color::new(0, 43, 54),
            cursor: Color::new(131, 148, 150),
            selection_bg: Color::new(7, 54, 66),
            selection_fg: Color::new(147, 161, 161),
            black: Color::new(7, 54, 66),
            red: Color::new(220, 50, 47),
            green: Color::new(133, 153, 0),
            yellow: Color::new(181, 137, 0),
            blue: Color::new(38, 139, 210),
            magenta: Color::new(211, 54, 130),
            cyan: Color::new(42, 161, 152),
            white: Color::new(238, 232, 213),
            bright_black: Color::new(0, 43, 54),
            bright_red: Color::new(203, 75, 22),
            bright_green: Color::new(88, 110, 117),
            bright_yellow: Color::new(101, 123, 131),
            bright_blue: Color::new(131, 148, 150),
            bright_magenta: Color::new(108, 113, 196),
            bright_cyan: Color::new(147, 161, 161),
            bright_white: Color::new(253, 246, 227),
        }
    }

    /// Solarized Light theme
    pub fn solarized_light() -> Self {
        Self {
            name: "Solarized Light".to_string(),
            foreground: Color::new(101, 123, 131),
            background: Color::new(253, 246, 227),
            cursor: Color::new(88, 110, 117),
            selection_bg: Color::new(238, 232, 213),
            selection_fg: Color::new(88, 110, 117),
            black: Color::new(238, 232, 213),
            red: Color::new(220, 50, 47),
            green: Color::new(133, 153, 0),
            yellow: Color::new(181, 137, 0),
            blue: Color::new(38, 139, 210),
            magenta: Color::new(211, 54, 130),
            cyan: Color::new(42, 161, 152),
            white: Color::new(7, 54, 66),
            bright_black: Color::new(253, 246, 227),
            bright_red: Color::new(203, 75, 22),
            bright_green: Color::new(147, 161, 161),
            bright_yellow: Color::new(131, 148, 150),
            bright_blue: Color::new(101, 123, 131),
            bright_magenta: Color::new(108, 113, 196),
            bright_cyan: Color::new(88, 110, 117),
            bright_white: Color::new(0, 43, 54),
        }
    }

    /// iTerm2 Dark default theme
    pub fn iterm2_dark() -> Self {
        Self {
            name: "iTerm2 Dark".to_string(),
            foreground: Color::new(178, 178, 178),
            background: Color::new(0, 0, 0),
            cursor: Color::new(255, 255, 255),
            selection_bg: Color::new(179, 215, 255),
            selection_fg: Color::new(0, 0, 0),
            black: Color::new(0, 0, 0),
            red: Color::new(171, 53, 37),
            green: Color::new(87, 191, 55),
            yellow: Color::new(198, 196, 63),
            blue: Color::new(45, 66, 192),
            magenta: Color::new(177, 72, 184),
            cyan: Color::new(88, 194, 197),
            white: Color::new(199, 199, 199),
            bright_black: Color::new(103, 103, 103),
            bright_red: Color::new(207, 126, 119),
            bright_green: Color::new(129, 227, 151),
            bright_yellow: Color::new(233, 221, 0),
            bright_blue: Color::new(167, 170, 236),
            bright_magenta: Color::new(211, 130, 219),
            bright_cyan: Color::new(142, 249, 253),
            bright_white: Color::new(254, 254, 254),
        }
    }

    /// Tango Dark theme
    pub fn tango_dark() -> Self {
        Self {
            name: "Tango Dark".to_string(),
            foreground: Color::new(211, 215, 207),
            background: Color::new(46, 52, 54),
            cursor: Color::new(211, 215, 207),
            selection_bg: Color::new(238, 238, 236),
            selection_fg: Color::new(85, 87, 83),
            black: Color::new(46, 52, 54),
            red: Color::new(204, 0, 0),
            green: Color::new(78, 154, 6),
            yellow: Color::new(196, 160, 0),
            blue: Color::new(52, 101, 164),
            magenta: Color::new(117, 80, 123),
            cyan: Color::new(6, 152, 154),
            white: Color::new(211, 215, 207),
            bright_black: Color::new(85, 87, 83),
            bright_red: Color::new(239, 41, 41),
            bright_green: Color::new(138, 226, 52),
            bright_yellow: Color::new(252, 233, 79),
            bright_blue: Color::new(114, 159, 207),
            bright_magenta: Color::new(173, 127, 168),
            bright_cyan: Color::new(52, 226, 226),
            bright_white: Color::new(238, 238, 236),
        }
    }

    /// Tango Light theme
    pub fn tango_light() -> Self {
        Self {
            name: "Tango Light".to_string(),
            foreground: Color::new(46, 52, 54),
            background: Color::new(255, 255, 255),
            cursor: Color::new(46, 52, 54),
            selection_bg: Color::new(203, 228, 255),
            selection_fg: Color::new(46, 52, 54),
            black: Color::new(46, 52, 54),
            red: Color::new(204, 0, 0),
            green: Color::new(78, 154, 6),
            yellow: Color::new(196, 160, 0),
            blue: Color::new(52, 101, 164),
            magenta: Color::new(117, 80, 123),
            cyan: Color::new(6, 152, 154),
            white: Color::new(211, 215, 207),
            bright_black: Color::new(85, 87, 83),
            bright_red: Color::new(239, 41, 41),
            bright_green: Color::new(138, 226, 52),
            bright_yellow: Color::new(252, 233, 79),
            bright_blue: Color::new(114, 159, 207),
            bright_magenta: Color::new(173, 127, 168),
            bright_cyan: Color::new(52, 226, 226),
            bright_white: Color::new(238, 238, 236),
        }
    }

    /// Get theme by name
    #[allow(dead_code)]
    pub fn by_name(name: &str) -> Option<Self> {
        let normalized = name.trim().to_lowercase().replace(['_', ' '], "-");

        match normalized.as_str() {
            "dracula" => Some(Self::dracula()),
            "solarized" => Some(Self::solarized()),
            "solarized-dark" => Some(Self::solarized_dark()),
            "solarized-light" => Some(Self::solarized_light()),
            "nord" => Some(Self::nord()),
            "monokai" => Some(Self::monokai()),
            "one-dark" | "onedark" => Some(Self::one_dark()),
            "default-dark" | "default" => Some(Self::default_dark()),
            "dark-background" => Some(Self::dark_background()),
            "high-contrast" => Some(Self::high_contrast()),
            "light-background" => Some(Self::light_background()),
            "pastel-dark" => Some(Self::pastel_dark()),
            "regular" => Some(Self::regular()),
            "smoooooth" => Some(Self::smoooooth()),
            "iterm2-dark" | "iterm2" => Some(Self::iterm2_dark()),
            "tango-dark" => Some(Self::tango_dark()),
            "tango-light" => Some(Self::tango_light()),
            _ => None,
        }
    }

    /// Get all available theme names
    #[allow(dead_code)]
    pub fn available_themes() -> Vec<&'static str> {
        vec![
            "Dark Background",
            "Default Dark",
            "Dracula",
            "High Contrast",
            "iTerm2 Dark",
            "Light Background",
            "Monokai",
            "Nord",
            "One Dark",
            "Pastel (Dark Background)",
            "Regular",
            "Smoooooth",
            "Solarized",
            "Solarized Dark",
            "Solarized Light",
            "Tango Dark",
            "Tango Light",
        ]
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_dark()
    }
}
