//! Default value functions for configuration.
//!
//! Each sub-module groups related `default_*` free functions used as
//! `#[serde(default = "crate::defaults::...")]` attributes on `Config` fields.
//! Everything is re-exported from this module so that all call-sites using
//! `crate::defaults::*` continue to work without change.

mod colors;
mod font;
mod misc;
mod shader;
mod terminal;
mod window;

// ── Font & text rendering ──────────────────────────────────────────────────
pub use font::{
    badge_font, blur_radius, char_spacing, dark_tab_style, font_family, font_size, light_tab_style,
    line_spacing, minimum_contrast, text_shaping,
};

// ── Window & visual appearance ─────────────────────────────────────────────
pub use window::{
    background_color, background_image_opacity, cols, cubemap_enabled, dark_theme,
    inactive_tab_fps, inactive_tab_opacity, light_theme, max_fps, pane_background_darken, rows,
    screenshot_format, tab_bar_height, tab_bar_width, tab_border_width, tab_html_titles,
    tab_min_width, tab_stretch_to_fill, text_opacity, theme, unfocused_fps,
    use_background_as_channel0, window_opacity, window_padding, window_title,
};

// ── Terminal behaviour ─────────────────────────────────────────────────────
pub use terminal::{
    activity_threshold, answerback_string, anti_idle_code, anti_idle_seconds, bell_sound,
    clipboard_max_event_bytes, clipboard_max_sync_events, command_history_max_entries,
    cursor_blink_interval, double_click_threshold, initial_text, initial_text_delay_ms,
    initial_text_send_newline, jobs_to_ignore, login_shell, notification_max_buffer,
    paste_delay_ms, scroll_speed, scrollback, scrollbar_autohide_delay, scrollbar_position,
    scrollbar_width, semantic_history_editor, session_log_directory, session_undo_max_entries,
    session_undo_preserve_shell, session_undo_timeout_secs, silence_threshold,
    smart_selection_enabled, triple_click_threshold, word_characters,
};

// ── Shader & render pipeline ───────────────────────────────────────────────
pub use shader::{
    cursor_glow_intensity, cursor_glow_radius, cursor_shader_color,
    cursor_shader_disable_in_alt_screen, cursor_trail_duration, custom_shader_brightness,
    custom_shader_speed, maximize_throughput, reduce_flicker, reduce_flicker_delay_ms,
    shader_hot_reload_delay, throughput_render_interval_ms,
};

// ── Colors ─────────────────────────────────────────────────────────────────
pub use colors::{
    badge_color, command_separator_color, cursor_boost_color, cursor_color, cursor_guide_color,
    cursor_shadow_color, link_highlight_color, pane_divider_color, pane_divider_hover_color,
    pane_focus_color, pane_title_bg_color, pane_title_color, progress_bar_error_color,
    progress_bar_indeterminate_color, progress_bar_normal_color, progress_bar_warning_color,
    scrollbar_thumb_color, scrollbar_track_color, search_current_highlight_color,
    search_highlight_color, status_bar_bg_color, status_bar_fg_color, tab_active_background,
    tab_active_indicator, tab_active_text, tab_activity_indicator, tab_bar_background,
    tab_bell_indicator, tab_border_color, tab_close_button, tab_close_button_hover,
    tab_hover_background, tab_inactive_background, tab_inactive_text,
};

// ── Miscellaneous ──────────────────────────────────────────────────────────
pub use misc::{
    ai_inspector_agent, ai_inspector_agent_screenshot_access, ai_inspector_agent_terminal_access,
    ai_inspector_auto_approve, ai_inspector_auto_context, ai_inspector_auto_launch,
    ai_inspector_context_max_lines, ai_inspector_default_scope, ai_inspector_enabled,
    ai_inspector_live_update, ai_inspector_open_on_startup, ai_inspector_show_zones,
    ai_inspector_view_mode, ai_inspector_width, ambiguous_width, badge_color_alpha, badge_format,
    badge_max_height, badge_max_width, badge_right_margin, badge_top_margin, bool_false, bool_true,
    command_separator_opacity, command_separator_thickness, cursor_boost, cursor_shadow_blur,
    cursor_shadow_offset, inactive_pane_opacity, keybindings, max_panes, mdns_timeout,
    normalization_form, pane_background_opacity, pane_divider_hit_width, pane_divider_width,
    pane_focus_width, pane_min_size, pane_padding, pane_title_height, progress_bar_height,
    progress_bar_opacity, status_bar_bg_alpha, status_bar_font_size, status_bar_git_poll_interval,
    status_bar_height, status_bar_mouse_inactive_timeout, status_bar_separator,
    status_bar_system_poll_interval, status_bar_time_format, tmux_auto_attach_session,
    tmux_default_session, tmux_path, tmux_prefix_key, tmux_status_bar_left,
    tmux_status_bar_refresh_ms, tmux_status_bar_right, unicode_version, update_check_frequency,
    zero,
};
