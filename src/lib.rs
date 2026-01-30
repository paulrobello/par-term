// Library exports for testing and potential library use

#[macro_use]
pub mod debug;

pub mod app;
pub mod audio_bell;
pub mod cell_renderer;
pub mod cli;
pub mod clipboard_history_ui;
pub mod config;
pub mod custom_shader_renderer;
pub mod font_manager;
pub mod font_metrics;
pub mod gpu_utils;
pub mod graphics_renderer;
pub mod help_ui;
pub mod input;
pub mod keybindings;
pub mod macos_blur; // macOS window blur using private CGS API
pub mod macos_metal; // macOS-specific CAMetalLayer configuration
pub mod menu;
pub mod renderer;
pub mod scroll_state;
pub mod scrollbar;
pub mod selection;
pub mod settings_ui;
pub mod settings_window;
pub mod shader_install_ui;
pub mod shader_installer;
pub mod shader_watcher;
pub mod styled_content;
pub mod tab;
pub mod tab_bar_ui;
pub mod terminal;
pub mod text_shaper;
pub mod themes;
pub mod url_detection;
