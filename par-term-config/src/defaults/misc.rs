//! Default values that do not belong to a single focused subsystem.

// ── Primitive helpers ──────────────────────────────────────────────────────

/// Serde default returning `false`.
pub fn bool_false() -> bool {
    false
}

/// Serde default returning `true`.
pub fn bool_true() -> bool {
    true
}

/// Serde default returning `0`.
pub fn zero() -> usize {
    0
}

/// Default mDNS service discovery timeout in seconds.
pub fn mdns_timeout() -> u32 {
    3
}

// ── Update ─────────────────────────────────────────────────────────────────

/// Default update check frequency.
pub fn update_check_frequency() -> crate::types::UpdateCheckFrequency {
    crate::types::UpdateCheckFrequency::Daily
}

// ── Keybindings ────────────────────────────────────────────────────────────

pub fn keybindings() -> Vec<crate::types::KeyBinding> {
    // macOS: Cmd+key is safe because Cmd is separate from Ctrl (terminal control codes).
    // Windows/Linux: Ctrl+key conflicts with terminal control codes (Ctrl+C=SIGINT, Ctrl+D=EOF, etc.)
    // so we use Ctrl+Shift+key following standard terminal emulator conventions
    // (WezTerm, Kitty, Alacritty, GNOME Terminal, Windows Terminal).
    #[cfg(target_os = "macos")]
    let mut bindings = vec![
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+B".to_string(),
            action: "toggle_background_shader".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+U".to_string(),
            action: "toggle_cursor_shader".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+V".to_string(),
            action: "paste_special".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+R".to_string(),
            action: "toggle_session_logging".to_string(),
        },
        // Split pane shortcuts with iTerm2's meaning (UX.md I1/I2): Cmd+D puts
        // the new pane to the right, Cmd+Shift+D below. Close pane is Cmd+W
        // (I15), in `defaults::menu_chords` with the File menu's Close item.
        crate::types::KeyBinding {
            key: "CmdOrCtrl+D".to_string(),
            action: "split_right".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+D".to_string(),
            action: "split_down".to_string(),
        },
        // Pane navigation shortcuts
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+Left".to_string(),
            action: "navigate_pane_left".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+Right".to_string(),
            action: "navigate_pane_right".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+Up".to_string(),
            action: "navigate_pane_up".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+Down".to_string(),
            action: "navigate_pane_down".to_string(),
        },
        // tmux display-panes style: letter badges, type a letter to focus
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+P".to_string(),
            action: "select_pane_hint".to_string(),
        },
        // Pane resize on iTerm2's Move Divider chords (UX.md I6)
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Ctrl+Left".to_string(),
            action: "resize_pane_left".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Ctrl+Right".to_string(),
            action: "resize_pane_right".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Ctrl+Up".to_string(),
            action: "resize_pane_up".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Ctrl+Down".to_string(),
            action: "resize_pane_down".to_string(),
        },
        // Pane swap with the neighbor in that direction, on the chord resize
        // used to hold (UX.md I7)
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+Shift+Left".to_string(),
            action: "swap_pane_left".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+Shift+Right".to_string(),
            action: "swap_pane_right".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+Shift+Up".to_string(),
            action: "swap_pane_up".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+Shift+Down".to_string(),
            action: "swap_pane_down".to_string(),
        },
        // Broadcast input mode (iTerm2's "all panes in current tab", I18)
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+I".to_string(),
            action: "toggle_broadcast_input".to_string(),
        },
        // Session picker moves off Cmd+Opt+T, which UX.md I14 reserves for
        // New Tab Next to Current (I24).
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Ctrl+S".to_string(),
            action: "toggle_tmux_session_picker".to_string(),
        },
        // Copy mode (vi-style keyboard-driven selection) - matches iTerm2
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+C".to_string(),
            action: "toggle_copy_mode".to_string(),
        },
        // Command history on iTerm2's Cmd+Shift+; (UX.md I28), which gives
        // Cmd+R back to the shell. Spelled with the shifted character: the
        // matcher compares logical keys and macOS reports Cmd+Shift+; as ':'.
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+:".to_string(),
            action: "toggle_command_history".to_string(),
        },
        // Reopen recently closed tab: iTerm2's Undo Close (UX.md I17, D4),
        // with the previous Cmd+Z kept as an alias. Throughput mode moved to
        // the palette.
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+T".to_string(),
            action: "reopen_closed_tab".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Z".to_string(),
            action: "reopen_closed_tab".to_string(),
        },
        // SSH Quick Connect
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+S".to_string(),
            action: "ssh_quick_connect".to_string(),
        },
        // Duplicate Tab. Cmd+D / Cmd+Shift+D are the two splits and
        // Cmd+Shift+T reopens a closed tab, so J — free in both the default
        // keybindings and every hardcoded key layer.
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+J".to_string(),
            action: "duplicate_tab".to_string(),
        },
        // Command palette (UX.md I35, D2); the profile drawer moves to
        // iTerm2's Open Profiles chord (I36). Both are advertised by the
        // Profiles / View menus through `registry_accel`.
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+P".to_string(),
            action: "toggle_command_palette".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+O".to_string(),
            action: "toggle_profile_drawer".to_string(),
        },
    ];

    #[cfg(not(target_os = "macos"))]
    let mut bindings = vec![
        crate::types::KeyBinding {
            key: "Ctrl+Shift+B".to_string(),
            action: "toggle_background_shader".to_string(),
        },
        crate::types::KeyBinding {
            key: "Ctrl+Shift+U".to_string(),
            action: "toggle_cursor_shader".to_string(),
        },
        // Ctrl+Shift+V is standard paste on Linux terminals, so use Ctrl+Alt+V for paste special
        crate::types::KeyBinding {
            key: "Ctrl+Alt+V".to_string(),
            action: "paste_special".to_string(),
        },
        crate::types::KeyBinding {
            key: "Ctrl+Shift+R".to_string(),
            action: "toggle_session_logging".to_string(),
        },
        // Split pane shortcuts: Terminator's pair (UX.md D1, K11/K12) —
        // Ctrl+Shift+E splits right, Ctrl+Shift+O splits down, and the
        // previous Ctrl+Shift+D stays as a split-down alias. Close pane is
        // Ctrl+Shift+W (I15 in the K1 family), in `defaults::menu_chords`.
        crate::types::KeyBinding {
            key: "Ctrl+Shift+E".to_string(),
            action: "split_right".to_string(),
        },
        crate::types::KeyBinding {
            key: "Ctrl+Shift+O".to_string(),
            action: "split_down".to_string(),
        },
        crate::types::KeyBinding {
            key: "Ctrl+Shift+D".to_string(),
            action: "split_down".to_string(),
        },
        // Pane navigation shortcuts
        crate::types::KeyBinding {
            key: "Ctrl+Alt+Left".to_string(),
            action: "navigate_pane_left".to_string(),
        },
        crate::types::KeyBinding {
            key: "Ctrl+Alt+Right".to_string(),
            action: "navigate_pane_right".to_string(),
        },
        crate::types::KeyBinding {
            key: "Ctrl+Alt+Up".to_string(),
            action: "navigate_pane_up".to_string(),
        },
        crate::types::KeyBinding {
            key: "Ctrl+Alt+Down".to_string(),
            action: "navigate_pane_down".to_string(),
        },
        // tmux display-panes style: letter badges, type a letter to focus
        crate::types::KeyBinding {
            key: "Ctrl+Alt+P".to_string(),
            action: "select_pane_hint".to_string(),
        },
        // Pane resize shortcuts
        crate::types::KeyBinding {
            key: "Ctrl+Alt+Shift+Left".to_string(),
            action: "resize_pane_left".to_string(),
        },
        crate::types::KeyBinding {
            key: "Ctrl+Alt+Shift+Right".to_string(),
            action: "resize_pane_right".to_string(),
        },
        crate::types::KeyBinding {
            key: "Ctrl+Alt+Shift+Up".to_string(),
            action: "resize_pane_up".to_string(),
        },
        crate::types::KeyBinding {
            key: "Ctrl+Alt+Shift+Down".to_string(),
            action: "resize_pane_down".to_string(),
        },
        // Pane swap ships unbound here (UX.md K16): editors use Alt+Shift+Arrow
        // and Windows uses Alt+Shift to switch the input language.
        // Broadcast input mode
        crate::types::KeyBinding {
            key: "Ctrl+Alt+I".to_string(),
            action: "toggle_broadcast_input".to_string(),
        },
        // Ctrl+Shift+T is standard new tab - use Ctrl+Shift+M for throughput mode
        crate::types::KeyBinding {
            key: "Ctrl+Shift+M".to_string(),
            action: "toggle_throughput_mode".to_string(),
        },
        // tmux session picker. Ubuntu opens a terminal on Ctrl+Alt+T, so
        // Ctrl+Alt+S is the alternative UX.md K18 names.
        crate::types::KeyBinding {
            key: "Ctrl+Alt+T".to_string(),
            action: "toggle_tmux_session_picker".to_string(),
        },
        crate::types::KeyBinding {
            key: "Ctrl+Alt+S".to_string(),
            action: "toggle_tmux_session_picker".to_string(),
        },
        // Copy mode (vi-style keyboard-driven selection)
        // Ctrl+Shift+C is standard copy on Linux, so use Ctrl+Shift+Space
        crate::types::KeyBinding {
            key: "Ctrl+Shift+Space".to_string(),
            action: "toggle_copy_mode".to_string(),
        },
        // Command history fuzzy search
        // Ctrl+R conflicts with terminal reverse search, so use Ctrl+Shift+R
        // Note: Ctrl+Shift+R is session logging on Linux; users can reassign
        crate::types::KeyBinding {
            key: "Ctrl+Alt+R".to_string(),
            action: "toggle_command_history".to_string(),
        },
        // Reopen recently closed tab
        crate::types::KeyBinding {
            key: "Ctrl+Shift+Z".to_string(),
            action: "reopen_closed_tab".to_string(),
        },
        // SSH Quick Connect
        crate::types::KeyBinding {
            key: "Ctrl+Shift+S".to_string(),
            action: "ssh_quick_connect".to_string(),
        },
        // Duplicate Tab. Ctrl+Shift+D is a split-down alias here, and the menu
        // model's `cmd_or_ctrl` is itself Ctrl+Shift off macOS, so every letter
        // it uses (N, W, Q, T, C, V, A) is spoken for too. J is free on both.
        crate::types::KeyBinding {
            key: "Ctrl+Shift+J".to_string(),
            action: "duplicate_tab".to_string(),
        },
        // Command palette (UX.md K10, D2). The profile drawer gives this chord
        // up and ships unbound here — its menu item stays (D2) — because the
        // K1 translation of iTerm2's Cmd+O, Ctrl+Shift+O, is split down.
        crate::types::KeyBinding {
            key: "Ctrl+Shift+P".to_string(),
            action: "toggle_command_palette".to_string(),
        },
    ];

    // Menu-advertised chords (UX.md K2) — see `defaults::menu_chords`.
    bindings.extend(super::menu_chords::menu_chords());

    // Hardcoded-layer chords (UX.md K2) — see `defaults::layer_chords`.
    bindings.extend(super::layer_chords::layer_chords());

    bindings
}

// ── Command separator ──────────────────────────────────────────────────────

/// Default command separator line thickness in pixels.
pub fn command_separator_thickness() -> f32 {
    1.0 // 1 pixel line
}

/// Default command separator line opacity (0.0–1.0).
pub fn command_separator_opacity() -> f32 {
    0.4 // Subtle by default
}

// ── Cursor shadow / boost ──────────────────────────────────────────────────

/// Default cursor drop shadow pixel offset as `[x, y]`.
pub fn cursor_shadow_offset() -> [f32; 2] {
    [2.0, 2.0] // 2 pixels offset in both directions
}

/// Default cursor drop shadow blur radius in pixels.
pub fn cursor_shadow_blur() -> f32 {
    3.0 // 3 pixel blur radius
}

/// Default cursor brightness boost amount (0.0 = disabled).
pub fn cursor_boost() -> f32 {
    0.0 // Disabled by default
}

// ── Badge ──────────────────────────────────────────────────────────────────

/// Default badge format string.
pub fn badge_format() -> String {
    "\\(session.username)@\\(session.hostname)".to_string()
}

/// Default badge text opacity (0.0–1.0).
pub fn badge_color_alpha() -> f32 {
    0.5 // 50% opacity (semi-transparent)
}

/// Default badge top margin in pixels.
pub fn badge_top_margin() -> f32 {
    0.0 // 0 pixels from top
}

/// Default badge right margin in pixels.
pub fn badge_right_margin() -> f32 {
    16.0 // 16 pixels from right
}

/// Default badge maximum width as a fraction of terminal width (0.0–1.0).
pub fn badge_max_width() -> f32 {
    0.5 // 50% of terminal width
}

/// Default badge maximum height as a fraction of terminal height (0.0–1.0).
pub fn badge_max_height() -> f32 {
    0.2 // 20% of terminal height
}

// ── Progress bar ───────────────────────────────────────────────────────────

/// Default progress bar height in pixels.
pub fn progress_bar_height() -> f32 {
    4.0 // Height in pixels
}

/// Default progress bar opacity (0.0–1.0).
pub fn progress_bar_opacity() -> f32 {
    0.8
}

// ── Unicode ────────────────────────────────────────────────────────────────

/// Default Unicode version for character width calculations.
pub fn unicode_version() -> crate::types::UnicodeVersion {
    crate::types::UnicodeVersion::Auto
}

/// Default treatment of ambiguous-width Unicode characters.
pub fn ambiguous_width() -> crate::types::AmbiguousWidth {
    crate::types::AmbiguousWidth::Narrow
}

/// Default Unicode normalization form applied to terminal input.
pub fn normalization_form() -> crate::types::NormalizationForm {
    crate::types::NormalizationForm::NFC
}

// ── Pane layout ────────────────────────────────────────────────────────────

/// Default split-pane divider visual width in pixels (`None` = use theme default).
pub fn pane_divider_width() -> Option<f32> {
    Some(2.0) // 2 pixel divider between panes
}

/// Default split-pane divider drag hit area width in pixels.
pub fn pane_divider_hit_width() -> f32 {
    5.0 // 5 pixel hit area for drag-to-resize (larger than visual for easier grabbing)
}

/// Default padding in pixels inside each pane (between content and border/divider).
pub fn pane_padding() -> f32 {
    1.0 // 1 pixel padding inside panes (space between content and border/divider)
}

/// Default minimum pane size in terminal cells (applies to both columns and rows).
pub fn pane_min_size() -> usize {
    10 // Minimum pane size in cells (columns or rows)
}

/// Default pane background opacity (1.0 = fully opaque).
pub fn pane_background_opacity() -> f32 {
    1.0 // Fully opaque by default
}

/// Default opacity for inactive (unfocused) panes (0.0–1.0).
pub fn inactive_pane_opacity() -> f32 {
    0.7 // 70% opacity for inactive panes
}

/// Default maximum number of panes allowed per tab.
pub fn max_panes() -> usize {
    16 // Maximum panes per tab
}

/// Default pane title bar height in pixels.
pub fn pane_title_height() -> f32 {
    20.0 // 20 pixel title bar height for panes
}

/// Default focused pane border width in pixels.
pub fn pane_focus_width() -> f32 {
    1.0 // 1 pixel border around focused pane
}

// ── tmux integration ───────────────────────────────────────────────────────

pub fn tmux_path() -> String {
    // First, try to find tmux in the user's PATH environment variable
    if let Ok(path_env) = std::env::var("PATH") {
        let separator = if cfg!(windows) { ';' } else { ':' };
        let executable = if cfg!(windows) { "tmux.exe" } else { "tmux" };

        for dir in path_env.split(separator) {
            let candidate = std::path::Path::new(dir).join(executable);
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }

    // Fall back to common paths for environments where PATH might be incomplete
    // (e.g., macOS app bundles launched from Finder)
    #[cfg(target_os = "macos")]
    {
        let macos_paths = [
            "/opt/homebrew/bin/tmux", // Homebrew on Apple Silicon
            "/usr/local/bin/tmux",    // Homebrew on Intel / MacPorts
        ];
        for path in macos_paths {
            if std::path::Path::new(path).exists() {
                return path.to_string();
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let linux_paths = [
            "/usr/bin/tmux",       // Most distros
            "/usr/local/bin/tmux", // Manual install
            "/snap/bin/tmux",      // Snap package
        ];
        for path in linux_paths {
            if std::path::Path::new(path).exists() {
                return path.to_string();
            }
        }
    }

    // Final fallback - let the OS try to find it
    "tmux".to_string()
}

/// Default tmux session name to connect to (`None` = no default).
pub fn tmux_default_session() -> Option<String> {
    None // No default session name
}

/// Default tmux session to auto-attach on startup (`None` = disabled).
pub fn tmux_auto_attach_session() -> Option<String> {
    None // No auto-attach session
}

/// Default tmux prefix key string (standard Ctrl+B).
pub fn tmux_prefix_key() -> String {
    "C-b".to_string() // Standard tmux prefix (Ctrl+B)
}

/// Default custom action prefix key (empty = disabled).
pub fn custom_action_prefix_key() -> String {
    String::new() // Disabled by default
}

/// Default tmux status bar refresh interval in milliseconds.
pub fn tmux_status_bar_refresh_ms() -> u64 {
    1000 // Default: 1 second refresh interval
}

/// Default tmux status bar left-side format string.
pub fn tmux_status_bar_left() -> String {
    "[{session}] {windows}".to_string()
}

/// Default tmux status bar right-side format string.
pub fn tmux_status_bar_right() -> String {
    "{pane} | {time:%H:%M}".to_string()
}
