//! Default values that do not belong to a single focused subsystem.

// ── Primitive helpers ──────────────────────────────────────────────────────

pub fn bool_false() -> bool {
    false
}

pub fn bool_true() -> bool {
    true
}

pub fn zero() -> usize {
    0
}

pub fn mdns_timeout() -> u32 {
    3
}

// ── Update ─────────────────────────────────────────────────────────────────

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
    let bindings = vec![
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
        // Split pane shortcuts (Cmd+D / Cmd+Shift+D matches iTerm2)
        crate::types::KeyBinding {
            key: "CmdOrCtrl+D".to_string(),
            action: "split_horizontal".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+D".to_string(),
            action: "split_vertical".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+W".to_string(),
            action: "close_pane".to_string(),
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
        // Pane resize shortcuts
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+Shift+Left".to_string(),
            action: "resize_pane_left".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+Shift+Right".to_string(),
            action: "resize_pane_right".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+Shift+Up".to_string(),
            action: "resize_pane_up".to_string(),
        },
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+Shift+Down".to_string(),
            action: "resize_pane_down".to_string(),
        },
        // Broadcast input mode
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+I".to_string(),
            action: "toggle_broadcast_input".to_string(),
        },
        // Throughput mode toggle
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+T".to_string(),
            action: "toggle_throughput_mode".to_string(),
        },
        // tmux session picker
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Alt+T".to_string(),
            action: "toggle_tmux_session_picker".to_string(),
        },
        // Copy mode (vi-style keyboard-driven selection) - matches iTerm2
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+C".to_string(),
            action: "toggle_copy_mode".to_string(),
        },
        // Command history fuzzy search
        crate::types::KeyBinding {
            key: "CmdOrCtrl+R".to_string(),
            action: "toggle_command_history".to_string(),
        },
        // Reopen recently closed tab
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Z".to_string(),
            action: "reopen_closed_tab".to_string(),
        },
        // SSH Quick Connect
        crate::types::KeyBinding {
            key: "CmdOrCtrl+Shift+S".to_string(),
            action: "ssh_quick_connect".to_string(),
        },
    ];

    #[cfg(not(target_os = "macos"))]
    let bindings = vec![
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
        // Split pane shortcuts
        // Ctrl+D is EOF/logout - use Ctrl+Shift+D for horizontal split
        crate::types::KeyBinding {
            key: "Ctrl+Shift+D".to_string(),
            action: "split_horizontal".to_string(),
        },
        // Ctrl+Shift+E for vertical split (Tilix/Terminator convention)
        crate::types::KeyBinding {
            key: "Ctrl+Shift+E".to_string(),
            action: "split_vertical".to_string(),
        },
        // Ctrl+Shift+W is standard close tab - use Ctrl+Shift+X for close pane
        crate::types::KeyBinding {
            key: "Ctrl+Shift+X".to_string(),
            action: "close_pane".to_string(),
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
        // tmux session picker
        crate::types::KeyBinding {
            key: "Ctrl+Alt+T".to_string(),
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
    ];

    bindings
}

// ── Command separator ──────────────────────────────────────────────────────

pub fn command_separator_thickness() -> f32 {
    1.0 // 1 pixel line
}

pub fn command_separator_opacity() -> f32 {
    0.4 // Subtle by default
}

// ── Cursor shadow / boost ──────────────────────────────────────────────────

pub fn cursor_shadow_offset() -> [f32; 2] {
    [2.0, 2.0] // 2 pixels offset in both directions
}

pub fn cursor_shadow_blur() -> f32 {
    3.0 // 3 pixel blur radius
}

pub fn cursor_boost() -> f32 {
    0.0 // Disabled by default
}

// ── Badge ──────────────────────────────────────────────────────────────────

pub fn badge_format() -> String {
    "\\(session.username)@\\(session.hostname)".to_string()
}

pub fn badge_color_alpha() -> f32 {
    0.5 // 50% opacity (semi-transparent)
}

pub fn badge_top_margin() -> f32 {
    0.0 // 0 pixels from top
}

pub fn badge_right_margin() -> f32 {
    16.0 // 16 pixels from right
}

pub fn badge_max_width() -> f32 {
    0.5 // 50% of terminal width
}

pub fn badge_max_height() -> f32 {
    0.2 // 20% of terminal height
}

// ── Progress bar ───────────────────────────────────────────────────────────

pub fn progress_bar_height() -> f32 {
    4.0 // Height in pixels
}

pub fn progress_bar_opacity() -> f32 {
    0.8
}

// ── Unicode ────────────────────────────────────────────────────────────────

pub fn unicode_version() -> par_term_emu_core_rust::UnicodeVersion {
    par_term_emu_core_rust::UnicodeVersion::Auto
}

pub fn ambiguous_width() -> par_term_emu_core_rust::AmbiguousWidth {
    par_term_emu_core_rust::AmbiguousWidth::Narrow
}

pub fn normalization_form() -> par_term_emu_core_rust::NormalizationForm {
    par_term_emu_core_rust::NormalizationForm::NFC
}

// ── Pane layout ────────────────────────────────────────────────────────────

pub fn pane_divider_width() -> Option<f32> {
    Some(2.0) // 2 pixel divider between panes
}

pub fn pane_divider_hit_width() -> f32 {
    8.0 // 8 pixel hit area for drag-to-resize (larger than visual for easier grabbing)
}

pub fn pane_padding() -> f32 {
    4.0 // 4 pixel padding inside panes (space between content and border/divider)
}

pub fn pane_min_size() -> usize {
    10 // Minimum pane size in cells (columns or rows)
}

pub fn pane_background_opacity() -> f32 {
    0.85 // 85% opacity allows background/shader to show through slightly
}

pub fn inactive_pane_opacity() -> f32 {
    0.7 // 70% opacity for inactive panes
}

pub fn max_panes() -> usize {
    16 // Maximum panes per tab
}

pub fn pane_title_height() -> f32 {
    20.0 // 20 pixel title bar height for panes
}

pub fn pane_focus_width() -> f32 {
    2.0 // 2 pixel border around focused pane
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

pub fn tmux_default_session() -> Option<String> {
    None // No default session name
}

pub fn tmux_auto_attach_session() -> Option<String> {
    None // No auto-attach session
}

pub fn tmux_prefix_key() -> String {
    "C-b".to_string() // Standard tmux prefix (Ctrl+B)
}

pub fn tmux_status_bar_refresh_ms() -> u64 {
    1000 // Default: 1 second refresh interval
}

pub fn tmux_status_bar_left() -> String {
    "[{session}] {windows}".to_string()
}

pub fn tmux_status_bar_right() -> String {
    "{pane} | {time:%H:%M}".to_string()
}
