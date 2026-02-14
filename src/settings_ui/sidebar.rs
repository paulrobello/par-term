//! Vertical sidebar navigation for settings tabs.
//!
//! This component provides a vertical tab list on the left side of the settings UI,
//! replacing the previous horizontal tab bar for better organization.

use super::SettingsUI;

/// The available settings tabs in the reorganized UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Appearance,
    Window,
    Input,
    Terminal,
    Effects,
    Badge,
    ProgressBar,
    StatusBar,
    Profiles,
    Ssh,
    Notifications,
    Integrations,
    Automation,
    Snippets,
    Actions,
    Arrangements,
    Advanced,
}

impl SettingsTab {
    /// Get the display name for this tab.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Window => "Window",
            Self::Input => "Input",
            Self::Terminal => "Terminal",
            Self::Effects => "Effects",
            Self::Badge => "Badge",
            Self::ProgressBar => "Progress Bar",
            Self::StatusBar => "Status Bar",
            Self::Profiles => "Profiles",
            Self::Ssh => "SSH",
            Self::Notifications => "Notifications",
            Self::Integrations => "Integrations",
            Self::Automation => "Automation",
            Self::Snippets => "Snippets",
            Self::Actions => "Actions",
            Self::Arrangements => "Arrangements",
            Self::Advanced => "Advanced",
        }
    }

    /// Get the icon for this tab (using emoji for simplicity).
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Appearance => "🎨",
            Self::Window => "🪟",
            Self::Input => "⌨",
            Self::Terminal => "📟",
            Self::Effects => "✨",
            Self::Badge => "🏷",
            Self::ProgressBar => "📊",
            Self::StatusBar => "\u{2501}",
            Self::Profiles => "👤",
            Self::Ssh => "🔗",
            Self::Notifications => "🔔",
            Self::Integrations => "🔌",
            Self::Automation => "⚡",
            Self::Snippets => "📝",
            Self::Actions => "🚀",
            Self::Arrangements => "📐",
            Self::Advanced => "⚙",
        }
    }

    /// Get all available tabs in order.
    pub fn all() -> &'static [Self] {
        &[
            Self::Appearance,
            Self::Window,
            Self::Input,
            Self::Terminal,
            Self::Effects,
            Self::Badge,
            Self::ProgressBar,
            Self::StatusBar,
            Self::Profiles,
            Self::Ssh,
            Self::Notifications,
            Self::Integrations,
            Self::Automation,
            Self::Snippets,
            Self::Actions,
            Self::Arrangements,
            Self::Advanced,
        ]
    }
}

/// Render the sidebar navigation.
///
/// Returns true if the selected tab changed.
pub fn show(ui: &mut egui::Ui, current_tab: &mut SettingsTab, search_query: &str) -> bool {
    let mut tab_changed = false;

    // Add some vertical spacing at the top
    ui.add_space(8.0);

    for tab in SettingsTab::all() {
        let is_selected = *current_tab == *tab;

        // Check if this tab has any matches for the search query
        let has_matches = search_query.is_empty() || tab_matches_search(*tab, search_query);

        // Dim tabs that don't match search
        let text_color = if !has_matches {
            egui::Color32::from_rgb(80, 80, 80)
        } else if is_selected {
            egui::Color32::from_rgb(255, 255, 255)
        } else {
            egui::Color32::from_rgb(180, 180, 180)
        };

        let bg_color = if is_selected {
            egui::Color32::from_rgb(60, 60, 70)
        } else {
            egui::Color32::TRANSPARENT
        };

        // Create a selectable button-like widget
        let response = ui.add_sized(
            [140.0, 32.0],
            egui::Button::new(
                egui::RichText::new(format!("{} {}", tab.icon(), tab.display_name()))
                    .color(text_color),
            )
            .fill(bg_color)
            .stroke(if is_selected {
                egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 100, 120))
            } else {
                egui::Stroke::NONE
            }),
        );

        if response.clicked() && has_matches {
            *current_tab = *tab;
            tab_changed = true;
        }

        // Show tooltip with tab contents summary
        response.on_hover_text(tab_contents_summary(*tab));
    }

    ui.add_space(8.0);

    tab_changed
}

/// Check if a tab matches the search query.
fn tab_matches_search(tab: SettingsTab, query: &str) -> bool {
    let query = query.to_lowercase();
    let keywords = tab_search_keywords(tab);

    // Check tab name
    if tab.display_name().to_lowercase().contains(&query) {
        return true;
    }

    // Check keywords
    keywords.iter().any(|k| k.to_lowercase().contains(&query))
}

/// Get search keywords for a tab.
fn tab_search_keywords(tab: SettingsTab) -> &'static [&'static str] {
    match tab {
        SettingsTab::Appearance => &[
            // Theme
            "theme",
            "color",
            "scheme",
            "dark",
            "light",
            // Auto dark mode
            "auto dark mode",
            "auto",
            "dark mode",
            "light mode",
            "system theme",
            "system appearance",
            "automatic",
            // Fonts
            "font",
            "family",
            "size",
            "bold",
            "italic",
            "line spacing",
            "char spacing",
            // Text shaping
            "text shaping",
            "shaping",
            "ligatures",
            "kerning",
            // Font rendering
            "anti-alias",
            "antialias",
            "hinting",
            "thin strokes",
            "smoothing",
            "minimum contrast",
            "contrast",
            // Cursor style
            "cursor",
            "style",
            "block",
            "beam",
            "underline",
            "blink",
            "interval",
            // Cursor appearance
            "cursor color",
            "text color",
            "unfocused cursor",
            "hollow",
            // Cursor locks
            "lock",
            "visibility",
            // Cursor effects
            "cursor guide",
            "guide",
            "cursor shadow",
            "shadow",
            "cursor boost",
            "boost",
            "glow",
        ],
        SettingsTab::Window => &[
            // Display
            "window",
            "title",
            "size",
            "columns",
            "rows",
            "padding",
            "allow title change",
            // Transparency
            "opacity",
            "transparency",
            "transparent",
            "blur",
            "blur radius",
            "keep text opaque",
            // Performance
            "fps",
            "max fps",
            "vsync",
            "refresh",
            "power",
            "gpu",
            "unfocused",
            "pause shaders",
            "reduce flicker",
            "flicker",
            "maximize throughput",
            "throughput",
            "render interval",
            // Window behavior
            "decorations",
            "always on top",
            "lock window size",
            "window number",
            "window type",
            "monitor",
            "target monitor",
            "space",
            "spaces",
            "mission control",
            "virtual desktop",
            "macos space",
            "target space",
            // Tab bar
            "tab bar",
            "tabs",
            "tab bar mode",
            "tab height",
            "tab index",
            "close button",
            "stretch",
            "html titles",
            "inherit cwd",
            "inherit directory",
            "profile drawer",
            "new tab shortcut",
            "profile picker",
            "new tab profile",
            "max tabs",
            // Tab bar appearance
            "tab min width",
            "tab border",
            "tab color",
            "inactive tab",
            "dimming",
            "dim inactive",
            "tab background",
            "tab text",
            "tab indicator",
            "activity indicator",
            "bell indicator",
            "close button color",
            // Split panes
            "panes",
            "split",
            "divider",
            "divider width",
            "hit width",
            "pane padding",
            "focus indicator",
            "pane focus",
            "max panes",
            "min pane size",
            // Pane appearance
            "divider color",
            "hover color",
            "dim inactive panes",
            "inactive pane",
            "pane opacity",
            "pane title",
            "pane background",
        ],
        SettingsTab::Input => &[
            // Keyboard
            "keyboard",
            "option",
            "alt",
            "meta",
            "esc",
            "physical",
            "physical keys",
            // Modifier remapping
            "remap",
            "remapping",
            "swap",
            "ctrl",
            "super",
            "cmd",
            "modifier",
            // Mouse
            "mouse",
            "scroll",
            "scroll speed",
            "double-click",
            "triple-click",
            "click threshold",
            "option+click",
            "alt+click",
            "focus follows",
            "focus follows mouse",
            "horizontal scroll",
            // Selection & clipboard
            "selection",
            "clipboard",
            "copy",
            "paste",
            "auto-copy",
            "auto copy",
            "trailing newline",
            "middle-click",
            "middle click",
            "dropped file",
            "quote style",
            // Clipboard limits
            "max sync",
            "max bytes",
            "clipboard max",
            // Word selection
            "word characters",
            "smart selection",
            // Keybindings
            "keybindings",
            "shortcuts",
            "hotkey",
            "binding",
            "key",
        ],
        SettingsTab::Terminal => &[
            // Behavior
            "shell",
            "scrollback",
            "scrollback lines",
            "exit",
            "shell exit",
            "exit action",
            "confirm",
            "confirm close",
            "running jobs",
            "jobs",
            "jobs to ignore",
            // Unicode
            "unicode",
            "unicode version",
            "width",
            "ambiguous",
            "ambiguous width",
            "answerback",
            // Shell
            "custom shell",
            "shell args",
            "login shell",
            "login",
            "working directory",
            "startup directory",
            "previous session",
            "home",
            // Startup
            "initial text",
            "startup",
            "delay",
            "newline",
            "undo",
            "undo close",
            "reopen",
            "reopen tab",
            "closed tab",
            "preserve shell",
            "preserve",
            "hide tab",
            // Search
            "search",
            "highlight",
            "search highlight",
            "case sensitive",
            "regex",
            "wrap",
            "wrap around",
            // Semantic history
            "semantic",
            "semantic history",
            "file path",
            "click file",
            "editor",
            "editor mode",
            "editor command",
            // Scrollbar
            "scrollbar",
            "thumb",
            "track",
            "autohide",
            "command marks",
            "marker",
            "mark",
            "tooltips",
        ],
        SettingsTab::Effects => &[
            // Background
            "background",
            "background mode",
            "background image",
            "background color",
            "image",
            "image mode",
            "fit",
            "fill",
            "stretch",
            "tile",
            "center",
            // Background shader
            "shader",
            "custom shader",
            "animation",
            "animation speed",
            "hot reload",
            "brightness",
            "text opacity",
            "full content",
            // Shader channels
            "channel",
            "ichannel",
            "texture",
            "cubemap",
            // Inline images
            "inline image",
            "sixel",
            "iterm",
            "kitty",
            "scaling",
            "aspect ratio",
            "nearest",
            "linear",
            // Cursor shader
            "cursor shader",
            "cursor effect",
            "trail",
            "trail duration",
            "glow",
            "glow radius",
            "glow intensity",
            "hides cursor",
            "alt screen",
        ],
        SettingsTab::ProgressBar => &[
            "progress",
            "progress bar",
            "bar",
            "percent",
            "osc 934",
            "osc 9;4",
            "indeterminate",
            "normal",
            "warning",
            "error",
            "bar height",
            "bar style",
            "bar position",
            "bar color",
        ],
        SettingsTab::StatusBar => &[
            "status",
            "status bar",
            "widget",
            "widgets",
            "cpu",
            "memory",
            "network",
            "git branch",
            "git status",
            "ahead",
            "behind",
            "dirty",
            "clock",
            "time",
            "time format",
            "hostname",
            "username",
            "auto hide",
            "poll interval",
            "separator",
            "bell indicator",
            "current command",
            "directory",
            "section",
            "left",
            "center",
            "right",
        ],
        SettingsTab::Badge => &[
            // General
            "badge",
            "badge enabled",
            "badge format",
            // Appearance
            "badge color",
            "text color",
            "badge opacity",
            "opacity",
            "badge font",
            "font",
            "bold",
            // Position
            "margin",
            "top margin",
            "right margin",
            "max width",
            "max height",
            // Variables
            "variable",
            "session",
            "hostname",
            "username",
            "path",
            "overlay",
            "label",
        ],
        SettingsTab::Profiles => &[
            "profile",
            "profiles",
            "shell",
            "shell selection",
            "login shell",
            "login",
            "bash",
            "zsh",
            "fish",
            "powershell",
            "tags",
            "inheritance",
            "shortcut",
            "auto switch",
            "hostname",
            "ssh",
            "ssh host",
            "ssh user",
            "ssh port",
            "identity file",
            "remote",
            "connection",
            "profile drawer",
        ],
        SettingsTab::Ssh => &[
            "ssh",
            "remote",
            "host",
            "connect",
            "quick connect",
            "mdns",
            "bonjour",
            "discovery",
            "auto-switch",
            "auto switch",
            "profile switch",
            "hostname",
            "known hosts",
        ],
        SettingsTab::Notifications => &[
            // Bell
            "bell",
            "visual bell",
            "audio bell",
            "sound",
            "beep",
            "volume",
            "desktop notification",
            // Activity
            "notification",
            "activity",
            "activity notification",
            "activity threshold",
            "inactivity",
            // Silence
            "silence",
            "silence notification",
            "silence threshold",
            // Session
            "session ended",
            "shell exits",
            // Behavior
            "suppress",
            "focused",
            "suppress notifications",
            "buffer",
            "max buffer",
            "test notification",
            // Anti-idle
            "anti-idle",
            "anti idle",
            "keep-alive",
            "keepalive",
            "idle",
            "timeout",
            "ssh timeout",
            "connection timeout",
            "alert",
        ],
        SettingsTab::Integrations => &[
            "shell integration",
            "bash",
            "zsh",
            "fish",
            "shaders",
            "shader bundle",
            "install",
            "uninstall",
            "bundle",
        ],
        SettingsTab::Automation => &[
            "trigger",
            "triggers",
            "regex",
            "pattern",
            "match",
            "automation",
            "automate",
            "action",
            "highlight",
            "notify",
            "notification",
            "run command",
            "play sound",
            "send text",
            "coprocess",
            "coprocesses",
            "pipe",
            "subprocess",
            "auto start",
            "auto-start",
        ],
        SettingsTab::Snippets => &[
            "snippet",
            "snippets",
            "text",
            "insert",
            "template",
            "variable",
            "keybinding",
            "folder",
            "substitution",
            "date",
            "time",
            "hostname",
            "path",
        ],
        SettingsTab::Actions => &[
            "action",
            "actions",
            "custom action",
            "shell command",
            "text insert",
            "key sequence",
            "macro",
            "automation",
            "shortcut",
        ],
        SettingsTab::Arrangements => &[
            "arrangement",
            "arrangements",
            "layout",
            "workspace",
            "save",
            "restore",
            "monitor",
            "window layout",
            "auto-restore",
        ],
        SettingsTab::Advanced => &[
            // tmux
            "tmux",
            "tmux integration",
            "tmux path",
            "control mode",
            "session",
            "default session",
            "auto-attach",
            "attach",
            "clipboard sync",
            "tmux clipboard",
            "status bar",
            "tmux status",
            "refresh interval",
            "prefix key",
            "prefix",
            // Session logging
            "logging",
            "session logging",
            "auto log",
            "auto-log",
            "recording",
            "asciicast",
            "asciinema",
            "log format",
            "log directory",
            "archive",
            "archive on close",
            // Screenshots
            "screenshot",
            "screenshot format",
            "png",
            "jpeg",
            "svg",
            "html",
            // Updates
            "update",
            "version",
            "check",
            "release",
            "update check",
            "skipped version",
            // Debug Logging
            "debug",
            "debug logging",
            "log level",
            "log file",
            "trace",
            "verbose",
            "diagnostics",
        ],
    }
}

/// Get a summary of tab contents for tooltip.
fn tab_contents_summary(tab: SettingsTab) -> &'static str {
    match tab {
        SettingsTab::Appearance => "Theme, fonts, cursor style and colors",
        SettingsTab::Window => "Window size, opacity, tab bar, split panes",
        SettingsTab::Input => "Keyboard shortcuts, mouse behavior, clipboard",
        SettingsTab::Terminal => "Shell, scrollback, search, scrollbar",
        SettingsTab::Effects => "Background image/shader, cursor effects",
        SettingsTab::StatusBar => {
            "Status bar widgets, layout, styling, auto-hide, and poll intervals"
        }
        SettingsTab::Badge => "Session info overlay (hostname, username, etc.)",
        SettingsTab::ProgressBar => "Progress bar style, position, and colors",
        SettingsTab::Profiles => "Create and manage terminal profiles",
        SettingsTab::Ssh => "SSH connection settings, mDNS discovery, auto-switch behavior",
        SettingsTab::Notifications => "Bell, activity alerts, desktop notifications",
        SettingsTab::Integrations => "Shell integration, shader bundle installation",
        SettingsTab::Automation => "Regex triggers, trigger actions, coprocesses",
        SettingsTab::Snippets => "Text snippets with variable substitution",
        SettingsTab::Actions => "Custom actions (shell, text, keys)",
        SettingsTab::Arrangements => "Save and restore window layouts",
        SettingsTab::Advanced => "tmux integration, logging, updates, debug logging",
    }
}

impl SettingsUI {
    /// Get the current selected tab.
    pub fn selected_tab(&self) -> SettingsTab {
        self.selected_tab
    }

    /// Set the selected tab.
    pub fn set_selected_tab(&mut self, tab: SettingsTab) {
        self.selected_tab = tab;
    }
}
