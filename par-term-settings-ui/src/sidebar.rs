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
    Scripts,
    Snippets,
    Actions,
    ContentPrettifier,
    Arrangements,
    AiInspector,
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
            Self::Scripts => "Scripts",
            Self::Snippets => "Snippets",
            Self::Actions => "Actions",
            Self::ContentPrettifier => "Prettifier",
            Self::Arrangements => "Arrangements",
            Self::AiInspector => "Assistant",
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
            Self::StatusBar => "🖥",
            Self::Profiles => "👤",
            Self::Ssh => "🔗",
            Self::Notifications => "🔔",
            Self::Integrations => "🔌",
            Self::Automation => "⚡",
            Self::Scripts => "📜",
            Self::Snippets => "📝",
            Self::Actions => "🚀",
            Self::ContentPrettifier => "🔮",
            Self::Arrangements => "📐",
            Self::AiInspector => "💬",
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
            Self::Scripts,
            Self::Snippets,
            Self::Actions,
            Self::ContentPrettifier,
            Self::Arrangements,
            Self::AiInspector,
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
            // Font variants
            "bold-italic",
            "bold italic",
            "font variant",
            "variant",
        ],
        SettingsTab::Window => &[
            // Display
            "window",
            "title",
            "size",
            "columns",
            "rows",
            "padding",
            "hide padding on split",
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
            "inactive tab",
            "inactive tab fps",
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
            "tab title mode",
            "tab title",
            "osc only",
            "cwd title",
            "rename tab",
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
            "outline only",
            "outline tab",
            "dimming",
            "dim inactive",
            "tab background",
            "tab text",
            "tab indicator",
            "activity indicator",
            "bell indicator",
            "close button color",
            "tab style",
            "auto tab style",
            "automatic tab",
            "system tab style",
            // Tab bar layout
            "tab bar position",
            "tab bar width",
            // Split panes
            "panes",
            "split",
            "divider",
            "divider width",
            "hit width",
            "pane padding",
            "divider style",
            "focus indicator",
            "focus indicator color",
            "focus indicator width",
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
            "pane title height",
            "pane title position",
            "pane title color",
            "pane background",
            // Performance extras
            "latency",
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
            // Copy mode
            "copy mode",
            "yank",
            // Paste
            "paste delay",
            // Smart selection
            "rules",
            "smart selection rules",
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
            "link handler",
            "link highlight color",
            "link highlight underline",
            "link underline style",
            "stipple",
            "link color",
            "url color",
            "browser",
            "open url",
            "open links",
            "url handler",
            // Scrollbar
            "scrollbar",
            "thumb",
            "track",
            "autohide",
            "command marks",
            "marker",
            "mark",
            "tooltips",
            "scrollbar width",
            // Unicode extras
            "normalization",
            "text normalization",
            "nfc",
            "nfd",
            // Command history
            "command history",
            "history entries",
            "max history",
            // Command separators
            "command separator",
            "separator",
            "separator line",
            "separator thickness",
            "separator opacity",
            "exit code",
            // Session restore
            "restore session",
            "undo timeout",
            "undo entries",
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
            // Per-pane background
            "per-pane background",
            "pane image",
            "split background",
            "per pane",
            "darken",
            "pane darken",
            // Hot reload extras
            "hot reload delay",
            "reload delay",
            // Shader overrides
            "per-shader",
            "shader override",
            "shader defaults",
            // Cubemap extras
            "cubemap enabled",
            "enable cubemap",
            // Per-pane extras
            "identify panes",
            // Background as texture
            "background as ichannel",
            "background as texture",
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
            "opacity",
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
            // Position and size
            "position",
            "height",
            // Styling
            "background",
            "background color",
            "background opacity",
            "text color",
            "foreground",
            "font size",
            // Auto-hide extras
            "fullscreen",
            "inactivity",
            "inactivity timeout",
            // Custom widgets
            "custom text",
            "custom widget",
            // Time format
            "strftime",
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
            "dynamic",
            "dynamic profiles",
            "remote url",
            "fetch",
            "refresh",
            "team",
            "shared",
            "download",
            "sync",
            // Profile management
            "duplicate",
            "default profile",
            "set default",
            // Dynamic profile extras
            "conflict resolution",
            "http headers",
            "headers",
            "max download",
            "download size",
            "fetch timeout",
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
            // Auto-switch extras
            "revert profile",
            "disconnect",
            // mDNS extras
            "scan timeout",
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
            // Alert sound extras
            "frequency",
            "duration",
            "sound file",
            "custom sound",
            // Anti-idle character
            "character",
            "ascii",
            "nul",
            "enq",
            "esc",
            "space",
            // Sound file formats
            "wav",
            "ogg",
            "flac",
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
            "reinstall",
            "bundle",
            "curl",
            "manual",
            "open folder",
            "shaders folder",
            "overwrite",
            // Status and info
            "detected",
            "version",
            "status",
            "location",
            "copy",
            "modified",
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
            // Trigger action extras
            "mark line",
            "set variable",
            "variable",
            "foreground",
            "foreground color",
            // Prettify action
            "prettify",
            "prettifier",
            "scope",
            "command output",
            // Coprocess extras
            "restart",
            "restart policy",
            "restart delay",
        ],
        SettingsTab::Scripts => &[
            "script",
            "scripting",
            "python",
            "automation",
            "observer",
            "event",
            "subprocess",
            "external",
            "panel",
            "subscriptions",
            // Script management
            "script path",
            "arguments",
            "args",
            "start",
            "stop",
            "auto-start",
            "auto start",
            "auto-launch",
            "restart",
            "restart policy",
            "restart delay",
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
            // Snippet management
            "title",
            "name",
            "content",
            "body",
            "description",
            "category",
            "auto-execute",
            "auto execute",
            "record",
            // Import/export
            "export",
            "import",
            "yaml",
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
            // Action details
            "keybinding",
            "binding",
            "record",
            "title",
            "name",
            "arguments",
        ],
        SettingsTab::ContentPrettifier => &[
            "prettifier",
            "prettify",
            "pretty",
            "content",
            "detect",
            "detection",
            "render",
            "markdown",
            "json",
            "yaml",
            "toml",
            "xml",
            "csv",
            "diff",
            "diagram",
            "mermaid",
            "log",
            "stack trace",
            "confidence",
            "gutter",
            "badge",
            "toggle",
            "source",
            "rendered",
            "custom",
            "claude code",
            "external command",
            "test detection",
            "sample",
            // Diagram engine
            "engine",
            "kroki",
            "native",
            "text fallback",
            // Display options
            "alternate screen",
            "per-block",
            "per block",
            "block",
            // Clipboard
            "clipboard",
            "copy",
            // Cache
            "cache",
            "max entries",
            // Detection tuning
            "scope",
            "threshold",
            "debounce",
            "scan",
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
            // Arrangement management
            "rename",
            "delete",
            "reorder",
            "move up",
            "move down",
            "overwrite",
            "startup",
        ],
        SettingsTab::AiInspector => &[
            "ai",
            "inspector",
            "agent",
            "acp",
            "llm",
            "assistant",
            "zone",
            "command",
            "history",
            "context",
            "auto",
            "approve",
            "yolo",
            "terminal",
            "access",
            "drive",
            "execute",
            "live",
            "update",
            "scope",
            "cards",
            "timeline",
            "tree",
            "env",
            "environment",
            "anthropic",
            "ollama",
            "startup",
            "open",
            "width",
            // Panel
            "panel",
            // Permissions
            "permissions",
            "auto-approve",
            "auto approve",
            "screenshot",
            "screenshot access",
            // Agent extras
            "auto-send",
            "auto send",
            "max context",
            "context lines",
            "auto-launch",
            "auto launch",
            // View modes
            "list",
            "list detail",
            "recent",
            // Custom agents
            "custom",
            "identity",
            "short name",
            "install command",
            "connector",
            "run command",
            "active",
            "protocol",
            // Platform-specific
            "macos",
            "linux",
            "windows",
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
            "hourly",
            "skipped version",
            // File Transfers
            "download",
            "upload",
            "transfer",
            "file transfer",
            "save location",
            "save directory",
            // Debug Logging
            "debug",
            "debug logging",
            "log level",
            "log file",
            "trace",
            "verbose",
            "diagnostics",
            // Import/export preferences
            "import",
            "export",
            "preferences",
            "merge",
            "url",
            // Logging format extras
            "plain",
            "plain text",
            // tmux status format
            "left format",
            "right format",
            // Updates extras
            "check now",
            "daily",
            "weekly",
            "monthly",
            "homebrew",
            "brew",
            "cargo",
            "self-update",
            // Import/export extras
            "backup",
            "config",
            "url import",
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
        SettingsTab::Scripts => "External observer scripts that receive terminal events",
        SettingsTab::Snippets => "Text snippets with variable substitution",
        SettingsTab::Actions => "Custom actions (shell, text, keys)",
        SettingsTab::ContentPrettifier => {
            "Content detection, renderers, custom renderers, Claude Code integration"
        }
        SettingsTab::Arrangements => "Save and restore window layouts",
        SettingsTab::AiInspector => "Assistant agent integration, panel settings, permissions",
        SettingsTab::Advanced => {
            "tmux integration, logging, file transfers, updates, debug logging"
        }
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
