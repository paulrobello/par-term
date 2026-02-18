// Library exports for testing and potential library use

#[macro_use]
pub mod debug;

pub mod ai_inspector;
pub mod app;
pub mod arrangements;
pub mod audio_bell;
pub mod badge;
pub mod cell_renderer;
pub mod cli;
pub mod clipboard_history_ui;
pub mod close_confirmation_ui;
pub mod command_history;
pub mod command_history_ui;
pub mod config;
pub mod copy_mode;
pub mod custom_shader_renderer;
pub mod font_manager;
pub mod font_metrics;
pub mod gpu_utils;
pub mod graphics_renderer;
pub mod help_ui;
pub mod http;
pub mod input;
pub mod integrations_ui;
pub mod keybindings;
pub mod macos_blur; // macOS window blur using private CGS API
pub mod macos_metal; // macOS-specific CAMetalLayer configuration
pub mod macos_space; // macOS Space (virtual desktop) targeting using private SLS API
pub mod manifest;
pub use par_term_mcp as mcp_server;
pub mod menu;
pub mod pane;
pub mod paste_special_ui;
pub mod paste_transform;
pub mod profile;
pub mod profile_drawer_ui;
pub mod profile_modal_ui;
pub mod progress_bar;
pub mod quit_confirmation_ui;
pub mod remote_shell_install_ui;
pub mod renderer;
pub mod scripting;
pub mod scroll_state;
pub mod scrollback_metadata;
pub mod scrollbar;
pub mod search;
pub mod selection;
pub mod self_updater;
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
pub mod styled_content;
pub mod tab;
pub mod tab_bar_ui;
pub mod terminal;
pub mod text_shaper;
pub mod themes;
pub mod tmux;
pub mod tmux_session_picker_ui;
pub mod tmux_status_bar_ui;
pub mod update_checker;
pub mod url_detection;
