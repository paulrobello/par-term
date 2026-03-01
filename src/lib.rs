// Library exports for testing and potential library use
//
// # Mutex Usage Policy
//
// par-term uses three mutex types for different concurrency scenarios.
// New code should follow these rules:
//
//   - `tokio::sync::Mutex`    — use for terminal/async state accessed from both
//                               async tasks and sync event-loop code. Access via
//                               `try_lock()` from sync contexts (non-blocking) and
//                               `.await` or `blocking_lock()` from std threads.
//
//   - `parking_lot::Mutex`    — use for sync-only state where you need a fast,
//                               non-async lock (e.g. upload error field, watcher state).
//                               Never call `blocking_lock()` on a tokio mutex from
//                               within an async context — use parking_lot instead.
//
//   - `std::sync::Mutex`      — acceptable for simple, short-lived locks in code
//                               that cannot depend on parking_lot (e.g. platform
//                               FFI modules). Prefer parking_lot for new code.
//
// See `docs/MUTEX_PATTERNS.md` for detailed patterns, deadlock avoidance rules,
// and examples showing correct lock acquisition in each context.

/// Application version (root crate version, for use by sub-crates).
/// Sub-crates should receive this via parameter rather than using
/// `env!("CARGO_PKG_VERSION")` which resolves to the sub-crate's version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[macro_use]
pub mod debug;

pub mod ai_inspector;
pub mod app;
pub mod arrangements;
pub mod audio_bell;
pub mod badge;
pub mod cell_renderer {
    //! Cell renderer re-exports from par-term-render crate.
    pub use par_term_render::cell_renderer::{
        Cell, CellRenderer, PaneViewport, atlas, background, block_chars, pipeline, render, types,
    };
}
pub mod cli;
pub mod clipboard_history_ui;
pub mod close_confirmation_ui;
pub mod command_history;
pub mod command_history_ui;
pub mod config;
pub mod copy_mode;
pub mod font_metrics;
pub mod help_ui;
pub mod http;
pub mod input;
pub mod integrations_ui;
pub mod keybindings;
pub mod macos_blur; // macOS window blur using private CGS API
pub mod macos_metal; // macOS-specific CAMetalLayer configuration
pub mod macos_space; // macOS Space (virtual desktop) targeting using private SLS API
pub(crate) mod manifest;
pub use par_term_mcp as mcp_server;
pub mod menu;
pub mod pane;
pub mod paste_special_ui;
pub mod paste_transform;
pub mod platform;
pub mod prettifier;
pub mod profile;
pub mod profile_drawer_ui;
pub mod profile_modal_ui;
pub mod progress_bar;
pub mod quit_confirmation_ui;
pub mod remote_shell_install_ui;
pub(crate) mod renderer {
    //! Renderer re-exports from the `par-term-render` sub-crate.
    pub use par_term_render::renderer::{
        DividerRenderInfo, PaneDividerSettings, PaneRenderInfo, PaneTitleInfo, Renderer,
        RendererParams, compute_visible_separator_marks,
    };
}
pub mod scripting;
pub mod scroll_state;
pub(crate) mod scrollback_metadata {
    //! Scrollback metadata re-exports from par-term-terminal crate.
    pub use par_term_terminal::scrollback_metadata::ScrollbackMark;
}
pub mod search;
pub mod selection;
pub mod self_updater {
    //! Self-update functionality re-exports from `par-term-update`.
    pub use par_term_update::self_updater::{
        DownloadUrls, InstallationType, UpdateResult, cleanup_old_binary, compute_data_hash,
        detect_installation, get_asset_name, get_binary_download_url, get_checksum_asset_name,
        get_download_urls, perform_update,
    };
}
pub mod session;
pub mod session_logger;
pub use par_term_settings_ui as settings_ui;
pub mod settings_window;
pub mod shader_install_ui;
pub mod shader_installer;
pub mod shader_watcher;
pub mod shell_detection;
pub mod shell_integration_installer;
pub mod shell_quote;
pub mod smart_selection;
pub mod snippets;
pub mod ssh;
pub mod ssh_connect_ui;
pub mod status_bar;
pub mod tab;
pub mod tab_bar_ui;
pub mod terminal {
    //! Terminal manager re-exports from `par-term-terminal` sub-crate.
    pub use par_term_terminal::terminal::clipboard;
    pub use par_term_terminal::terminal::coprocess_env;
    pub use par_term_terminal::terminal::graphics;
    pub use par_term_terminal::terminal::hyperlinks;
    pub use par_term_terminal::terminal::rendering;
    pub use par_term_terminal::terminal::spawn;
    pub use par_term_terminal::terminal::{
        ClipboardEntry, ClipboardSlot, ShellLifecycleEvent, TerminalManager,
    };
    pub use par_term_terminal::{HyperlinkInfo, SearchMatch};
}
pub mod text_shaper {
    //! Text shaping re-exports from par-term-fonts crate.
    pub use par_term_fonts::text_shaper::{ShapedGlyph, ShapedRun, ShapingOptions, TextShaper};
}
pub(crate) mod themes {
    //! Terminal color themes re-exports from par-term-config crate.
    pub use par_term_config::Theme;
}
pub use par_term_tmux as tmux;
pub mod tmux_session_picker_ui;
pub mod tmux_status_bar_ui;
pub mod traits;
pub mod traits_impl;
pub mod ui_constants;
pub(crate) mod update_checker {
    //! Update checker re-exports from `par-term-update`.
    pub use par_term_update::update_checker::{
        UpdateCheckResult, UpdateChecker, UpdateInfo, current_timestamp, fetch_latest_release,
    };
}
pub mod update_dialog;
pub mod url_detection;
