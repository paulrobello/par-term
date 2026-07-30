//! Gate: `Config`'s `#[serde(flatten)]` decomposition must not change the wire
//! format or any default value.
//!
//! `Config` is drained into sub-structs that are flattened back to the top
//! level, so a `config.yaml` written by any release keeps loading unchanged.
//! Three things can go wrong when a field moves, and none of them fail to
//! compile:
//!
//! 1. **The field stops being top-level.** A sub-struct added without
//!    `#[serde(flatten)]` nests its keys under the member name; every existing
//!    config silently loses those settings.
//!    → [`serialized_default_is_flat_and_complete`]
//! 2. **The default changes.** More than half of `Config`'s fields do *not*
//!    default to their type's `Default` — non-zero durations, opacities, `true`
//!    flags. `#[derive(Default)]` on a receiving sub-struct compiles and resets
//!    every one. → [`non_type_default_seeds_survive_decomposition`]
//! 3. **A `#[serde(default = "…")]` is dropped while the `Default` impl keeps
//!    the seed** (or the reverse). The field then reads one value from an
//!    absent key and another from `Config::default()`.
//!    → [`default_config_equals_empty_yaml_document`]
//!
//! `default_config_equals_empty_yaml_document` is the broadest of the three: it
//! pins every field of `Config` against class 3 with no per-field maintenance.
//! It is also the claim `config_struct/default_impl.rs` makes in prose and that
//! nothing previously checked.
//!
//! Comparison is by *serialized* value, not `PartialEq` — `Config` does not
//! implement it — normalised into a `BTreeMap` so that the key reordering
//! `flatten` introduces (flattened members serialise at their member's
//! position) cannot make a passing test fail.

use par_term_config::Config;
use serde_yaml_ng::Value;
use std::collections::BTreeMap;

/// Serialise a `Config` into a key-sorted top-level map.
///
/// Sorting is what makes the comparison immune to field reordering; a raw
/// string diff of the YAML would fail on every move even when nothing changed.
fn as_map(config: &Config) -> BTreeMap<String, Value> {
    let value = serde_yaml_ng::to_value(config).expect("Config serialises");
    let Value::Mapping(mapping) = value else {
        panic!("Config must serialise as a mapping, got {value:?}");
    };
    mapping
        .into_iter()
        .map(|(k, v)| {
            let key = k.as_str().expect("config keys are strings").to_string();
            (key, v)
        })
        .collect()
}

fn parse(yaml: &str) -> Config {
    serde_yaml_ng::from_str(yaml).unwrap_or_else(|e| panic!("config must load: {e}\n---\n{yaml}"))
}

/// Floor on the number of top-level keys a default `Config` serialises to.
///
/// A sub-struct that loses its `#[serde(flatten)]` collapses many keys into
/// one, so the count drops sharply. The live count is 350; the floor sits just
/// below it so adding a setting never fails the gate but losing a group always
/// does.
const MIN_TOP_LEVEL_KEYS: usize = 345;

// ---------------------------------------------------------------------------
// 1. Flatness — every setting is still a top-level YAML key
// ---------------------------------------------------------------------------

/// One representative key per flattened sub-struct.
///
/// Each entry lives in a sub-struct rather than on `Config` itself, so its
/// presence at the top level proves that group's `flatten` is intact. A group
/// added without one fails here rather than in a user's `config.yaml`.
const FLATTENED_GROUP_WITNESSES: &[&str] = &[
    // Sub-structs that predate the ENH-007 decomposition
    "font_antialias",           // FontRenderingConfig
    "window_opacity",           // WindowConfig
    "custom_shader_enabled",    // GlobalShaderConfig
    "mouse_scroll_speed",       // MouseConfig
    "copy_mode_enabled",        // CopyModeConfig
    "scrollback_lines",         // ScrollbackConfig
    "unicode_version",          // UnicodeConfig
    "cursor_blink",             // CursorConfig
    "notification_bell_visual", // NotificationConfig
    "ssh_auto_profile_switch",  // SshConfig
    "update_check_frequency",   // UpdateConfig
    "search_case_sensitive",    // SearchConfig
    "status_bar_enabled",       // StatusBarConfig
    "ai_inspector_enabled",     // AiInspectorConfig
    // Added by the ENH-007 decomposition
    "tab_style",                 // TabConfig
    "tab_bar_background",        // TabBarColorsConfig
    "pane_divider_width",        // PaneConfig
    "tmux_enabled",              // TmuxConfig
    "badge_enabled",             // BadgeConfig
    "progress_bar_enabled",      // ProgressBarConfig
    "semantic_history_enabled",  // SemanticHistoryConfig
    "command_separator_enabled", // CommandSeparatorConfig
    "auto_log_sessions",         // SessionLogConfig
    "scrollbar_position",        // ScrollbarConfig
    "shell_exit_action",         // ShellConfig
    "theme",                     // ThemeColorsConfig
    "auto_copy_selection",       // SelectionConfig
    "word_characters",           // WordSelectionConfig
    "image_scaling_mode",        // ImageConfig
    "left_option_key_mode",      // InputConfig
    "pause_shaders_on_blur",     // PowerConfig
    "clipboard_max_sync_events", // ClipboardConfig
    "shader_hot_reload",         // ShaderWatchConfig
    "shader_install_prompt",     // IntegrationConfig
    "restore_session",           // SessionRestoreConfig
    "max_fps",                   // RenderingConfig
    "window_type",               // WindowPlacementConfig
    "background_image_enabled",  // BackgroundConfig
    "shader_configs",            // ShaderOverridesConfig
    "triggers",                  // AutomationConfig
    "allow_all_env_vars",        // SecurityConfig
];

#[test]
fn serialized_default_is_flat_and_complete() {
    let map = as_map(&Config::default());

    assert!(
        map.len() >= MIN_TOP_LEVEL_KEYS,
        "a default Config serialised to only {} top-level keys (floor {MIN_TOP_LEVEL_KEYS}). \
         A sub-config that lost its #[serde(flatten)] nests its keys instead, which silently \
         drops those settings from every existing config.yaml.",
        map.len(),
    );

    for witness in FLATTENED_GROUP_WITNESSES {
        assert!(
            map.contains_key(*witness),
            "`{witness}` is not a top-level key. Its sub-config is missing #[serde(flatten)], \
             so existing config.yaml files no longer see that whole group of settings.",
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Defaults — an absent key and `Config::default()` must agree
// ---------------------------------------------------------------------------

#[test]
fn default_config_equals_empty_yaml_document() {
    let from_empty = parse("{}");
    let programmatic = Config::default();

    let (left, right) = (as_map(&from_empty), as_map(&programmatic));
    let mismatched: Vec<&String> = left
        .keys()
        .filter(|k| left.get(*k) != right.get(*k))
        .collect();

    assert!(
        mismatched.is_empty(),
        "these keys deserialise from an empty config differently than Config::default() \
         produces them: {mismatched:?}. A #[serde(default = \"…\")] and its Default impl \
         initialiser disagree — a config that omits the key now behaves differently from a \
         fresh install.",
    );
    assert_eq!(
        left.keys().collect::<Vec<_>>(),
        right.keys().collect::<Vec<_>>(),
    );
}

/// Fields whose default is deliberately *not* their type's `Default`.
///
/// Every one of these becomes `0`, `false` or `""` under `#[derive(Default)]`,
/// so a sub-config that derives instead of copying the original initialiser
/// fails here with the field named. Values that vary by machine (a `PATH`
/// lookup, a home directory) are pinned against the seed function instead of a
/// literal.
#[test]
fn non_type_default_seeds_survive_decomposition() {
    let c = Config::default();

    // Tabs — non-zero geometry and `true` flags
    assert_eq!(c.tabs.tab_bar_height, 28.0, "tab_bar_height");
    assert_eq!(c.tabs.tab_bar_width, 160.0, "tab_bar_width");
    assert!(c.tabs.tab_show_close_button, "tab_show_close_button");
    assert!(c.tabs.tab_inherit_cwd, "tab_inherit_cwd");
    assert!(
        c.tabs.remote_tab_title_osc_priority,
        "remote_tab_title_osc_priority"
    );

    // Tab bar colours — dimming, sizing and outlines
    assert!(c.tab_colors.dim_inactive_tabs, "dim_inactive_tabs");
    assert_eq!(
        c.tab_colors.inactive_tab_opacity, 0.6,
        "inactive_tab_opacity"
    );
    assert_eq!(c.tab_colors.tab_min_width, 120.0, "tab_min_width");
    assert!(c.tab_colors.tab_stretch_to_fill, "tab_stretch_to_fill");
    assert_eq!(c.tab_colors.tab_border_width, 1.0, "tab_border_width");
    assert!(
        c.tab_colors.tab_inactive_outline_only,
        "tab_inactive_outline_only"
    );

    // Panes — non-zero geometry, opaque backgrounds, focus indicator on
    assert_eq!(c.panes.pane_divider_width, Some(2.0), "pane_divider_width");
    assert_eq!(
        c.panes.pane_divider_hit_width, 5.0,
        "pane_divider_hit_width"
    );
    assert_eq!(c.panes.pane_padding, 1.0, "pane_padding");
    assert_eq!(c.panes.pane_min_size, 10, "pane_min_size");
    assert_eq!(
        c.panes.pane_background_opacity, 1.0,
        "pane_background_opacity"
    );
    assert_eq!(c.panes.inactive_pane_opacity, 0.7, "inactive_pane_opacity");
    assert_eq!(c.panes.pane_title_height, 20.0, "pane_title_height");
    assert_eq!(c.panes.max_panes, 16, "max_panes");
    assert!(c.panes.pane_focus_indicator, "pane_focus_indicator");
    assert_eq!(c.panes.pane_focus_width, 1.0, "pane_focus_width");

    // tmux — a discovered binary path, a `true` flag, non-zero polling
    assert!(!c.tmux.tmux_path.is_empty(), "tmux_path");
    assert!(c.tmux.tmux_clipboard_sync, "tmux_clipboard_sync");
    assert_eq!(
        c.tmux.tmux_status_bar_refresh_ms, 1000,
        "tmux_status_bar_refresh_ms"
    );
    assert_eq!(c.tmux.tmux_prefix_key, "C-b", "tmux_prefix_key");
    assert!(
        !c.tmux.tmux_status_bar_left.is_empty(),
        "tmux_status_bar_left"
    );
    assert!(
        !c.tmux.tmux_status_bar_right.is_empty(),
        "tmux_status_bar_right"
    );

    // Badge — format string, alpha, font and margins
    assert!(!c.badge.badge_format.is_empty(), "badge_format");
    assert_eq!(c.badge.badge_color_alpha, 0.5, "badge_color_alpha");
    assert_eq!(c.badge.badge_font, "Helvetica", "badge_font");
    assert!(c.badge.badge_font_bold, "badge_font_bold");
    assert_eq!(c.badge.badge_right_margin, 16.0, "badge_right_margin");
    assert_eq!(c.badge.badge_max_width, 0.5, "badge_max_width");
    assert_eq!(c.badge.badge_max_height, 0.2, "badge_max_height");

    // Progress bar — on by default, non-zero height and opacity
    assert!(c.progress_bar.progress_bar_enabled, "progress_bar_enabled");
    assert_eq!(
        c.progress_bar.progress_bar_height, 4.0,
        "progress_bar_height"
    );
    assert_eq!(
        c.progress_bar.progress_bar_opacity, 0.8,
        "progress_bar_opacity"
    );

    // Semantic history — enabled with link highlighting on
    assert!(
        c.semantic_history.semantic_history_enabled,
        "semantic_history_enabled"
    );
    assert!(
        c.semantic_history.link_highlight_color_enabled,
        "link_highlight_color_enabled"
    );
    assert!(
        c.semantic_history.link_highlight_underline,
        "link_highlight_underline"
    );

    // Command separators — non-zero thickness/opacity, exit colouring on
    assert_eq!(
        c.command_separator.command_separator_thickness, 1.0,
        "command_separator_thickness"
    );
    assert_eq!(
        c.command_separator.command_separator_opacity, 0.4,
        "command_separator_opacity"
    );
    assert!(
        c.command_separator.command_separator_exit_color,
        "command_separator_exit_color"
    );

    // Session logging — archives and redacts by default, into a real directory
    assert!(c.session_log.archive_on_close, "archive_on_close");
    assert!(
        c.session_log.session_log_redact_passwords,
        "session_log_redact_passwords"
    );
    assert!(
        !c.session_log.session_log_directory.is_empty(),
        "session_log_directory"
    );

    // Scrollbar — right-hand side, non-zero width, marks on
    assert_eq!(
        c.scrollbar.scrollbar_position, "right",
        "scrollbar_position"
    );
    assert_eq!(c.scrollbar.scrollbar_width, 15.0, "scrollbar_width");
    assert!(
        c.scrollbar.scrollbar_command_marks,
        "scrollbar_command_marks"
    );

    // Shell — login shell on, non-empty jobs allowlist, non-zero send delay
    assert!(c.shell.login_shell, "login_shell");
    assert!(
        c.shell.initial_text_send_newline,
        "initial_text_send_newline"
    );
    assert_eq!(c.shell.initial_text_delay_ms, 100, "initial_text_delay_ms");
    assert!(!c.shell.jobs_to_ignore.is_empty(), "jobs_to_ignore");

    // Theme — named themes, not empty strings
    assert_eq!(c.theme_colors.theme, "dark-background", "theme");
    assert_eq!(
        c.theme_colors.light_theme, "light-background",
        "light_theme"
    );
    assert_eq!(c.theme_colors.dark_theme, "dark-background", "dark_theme");

    // Selection and clipboard — `true` flags and non-zero caps
    assert!(c.selection.auto_copy_selection, "auto_copy_selection");
    assert!(c.selection.middle_click_paste, "middle_click_paste");
    assert!(
        c.selection.warn_paste_control_chars,
        "warn_paste_control_chars"
    );
    assert!(c.clipboard.osc52_clipboard, "osc52_clipboard");
    assert_eq!(
        c.clipboard.clipboard_max_sync_events, 64,
        "clipboard_max_sync_events"
    );
    assert_eq!(
        c.clipboard.clipboard_max_event_bytes, 2048,
        "clipboard_max_event_bytes"
    );

    // Word selection — the iTerm2-compatible character set and its rules
    assert_eq!(
        c.word_selection.word_characters, "/-+\\~_.",
        "word_characters"
    );
    assert!(
        c.word_selection.smart_selection_enabled,
        "smart_selection_enabled"
    );
    assert!(
        !c.word_selection.smart_selection_rules.is_empty(),
        "smart_selection_rules"
    );

    // Inline images and background — aspect ratio kept, non-black background
    assert!(
        c.image.image_preserve_aspect_ratio,
        "image_preserve_aspect_ratio"
    );
    assert_eq!(c.image.background_color, [30, 30, 30], "background_color");
    assert!(
        c.background.transparency_affects_only_default_background,
        "transparency_affects_only_default_background"
    );
    assert!(c.background.keep_text_opaque, "keep_text_opaque");
    assert!(
        c.background.background_image_enabled,
        "background_image_enabled"
    );
    assert_eq!(
        c.background.background_image_opacity, 1.0,
        "background_image_opacity"
    );

    // Frame pacing — 60 fps target and non-zero batching intervals
    assert_eq!(c.rendering.max_fps, 60, "max_fps");
    assert_eq!(
        c.rendering.reduce_flicker_delay_ms, 16,
        "reduce_flicker_delay_ms"
    );
    assert_eq!(
        c.rendering.throughput_render_interval_ms, 100,
        "throughput_render_interval_ms"
    );

    // Power saving — both blur pauses on, non-zero unfocused rates
    assert!(c.power.pause_shaders_on_blur, "pause_shaders_on_blur");
    assert!(c.power.pause_refresh_on_blur, "pause_refresh_on_blur");
    assert_eq!(c.power.unfocused_fps, 30, "unfocused_fps");
    assert_eq!(c.power.inactive_tab_fps, 2, "inactive_tab_fps");

    // Shader hot reload — non-zero debounce
    assert_eq!(
        c.shader_watch.shader_hot_reload_delay, 100,
        "shader_hot_reload_delay"
    );

    // Closed-tab undo — non-zero retention window and entry cap
    assert_eq!(
        c.session_restore.session_undo_timeout_secs, 5,
        "session_undo_timeout_secs"
    );
    assert_eq!(
        c.session_restore.session_undo_max_entries, 10,
        "session_undo_max_entries"
    );
}

// ---------------------------------------------------------------------------
// 3. Round trip — a realistic, fully-populated config
// ---------------------------------------------------------------------------

/// A config with a non-default value in every group the decomposition touches.
///
/// Default values would exercise nothing: a field that stopped deserialising
/// would fall back to the value the fixture asked for. Every value here differs
/// from the default, so a lost key shows up as a failed assertion.
const POPULATED_CONFIG: &str = r#"
cols: 120
rows: 40
font_size: 15.5
font_family: "Fira Code"
theme: "nord"
auto_dark_mode: true
light_theme: "solarized-light"
dark_theme: "dracula"
max_fps: 144
vsync_mode: mailbox
reduce_flicker_delay_ms: 33
throughput_render_interval_ms: 250
window_type: fullscreen
target_monitor: 1
lock_window_size: true
show_window_number: true
keep_text_opaque: false
background_image: "~/wall.png"
background_image_enabled: false
background_image_mode: tile
background_image_opacity: 0.35
tab_style: light
tab_bar_mode: never
tab_bar_height: 44.0
tab_bar_position: bottom
tab_bar_width: 210.0
tab_show_close_button: false
tab_show_index: true
tab_inherit_cwd: false
max_tabs: 12
tab_bar_background: [10, 20, 30]
tab_active_background: [40, 50, 60]
dim_inactive_tabs: false
inactive_tab_opacity: 0.42
tab_min_width: 88.0
tab_border_width: 3.0
pane_divider_width: 5.0
pane_divider_hit_width: 11.0
pane_padding: 7.0
pane_min_size: 3
pane_background_opacity: 0.55
show_pane_titles: true
pane_title_height: 26.0
pane_title_position: bottom
max_panes: 4
pane_focus_width: 6.0
tmux_enabled: true
tmux_path: "/opt/tmux/bin/tmux"
tmux_auto_attach: true
tmux_clipboard_sync: false
tmux_status_bar_refresh_ms: 250
tmux_prefix_key: "C-a"
tmux_status_bar_left: "<<{session}>>"
badge_enabled: true
badge_format: "host"
badge_color_alpha: 0.85
badge_font_bold: false
badge_max_width: 0.75
progress_bar_enabled: false
progress_bar_style: barwithtext
progress_bar_position: bottom
progress_bar_height: 9.0
progress_bar_opacity: 0.25
semantic_history_enabled: false
semantic_history_editor_mode: custom
semantic_history_editor: "code -g {file}:{line}"
link_highlight_underline: false
link_handler_command: "firefox {url}"
allow_file_scheme_urls: true
command_separator_enabled: true
command_separator_thickness: 2.5
command_separator_opacity: 0.8
command_separator_exit_color: false
auto_log_sessions: true
session_log_format: html
session_log_directory: "/var/log/par-term"
archive_on_close: false
session_log_redact_passwords: false
scrollbar_position: "left"
scrollbar_width: 20.0
scrollbar_command_marks: false
scrollbar_mark_tooltips: true
scrollbar_autohide_delay: 2500
shell_exit_action: restart_with_prompt
custom_shell: "/bin/fish"
login_shell: false
initial_text: "echo hi"
initial_text_delay_ms: 750
initial_text_send_newline: false
answerback_string: "par-term"
prompt_on_quit: true
jobs_to_ignore: ["bash", "vim"]
auto_copy_selection: false
copy_trailing_newline: true
middle_click_paste: false
paste_delay_ms: 33
warn_paste_control_chars: false
dropped_file_quote_style: double_quotes
word_characters: "/-+_.@"
smart_selection_enabled: false
image_scaling_mode: nearest
image_preserve_aspect_ratio: false
background_mode: color
background_color: [12, 34, 56]
left_option_key_mode: esc
right_option_key_mode: meta
use_physical_keys: true
clipboard_max_sync_events: 7
clipboard_max_event_bytes: 128
osc52_clipboard: false
pause_shaders_on_blur: false
pause_refresh_on_blur: false
unfocused_fps: 5
inactive_tab_fps: 1
shader_hot_reload: true
shader_hot_reload_delay: 1500
shader_install_prompt: never
allow_all_env_vars: true
allow_http_profiles: true
restore_session: true
session_undo_timeout_secs: 90
session_undo_max_entries: 3
session_undo_preserve_shell: true
auto_restore_arrangement: "work"
last_download_directory: "/tmp/dl"
collapsed_settings_sections: ["appearance"]
"#;

#[test]
fn populated_config_round_trips_unchanged() {
    let first = parse(POPULATED_CONFIG);
    let serialized = serde_yaml_ng::to_string(&first).expect("Config serialises");
    let second = parse(&serialized);

    assert_eq!(
        as_map(&first),
        as_map(&second),
        "serialize → deserialize → serialize is not stable",
    );
}

#[test]
fn populated_config_values_survive_the_flattening() {
    let c = parse(POPULATED_CONFIG);

    // At least one assertion per group: if a group's flatten is wrong its keys
    // are ignored and the field holds its default instead of the fixture value.
    assert_eq!(c.cols, 120);
    assert_eq!(c.font_size, 15.5);
    assert_eq!(c.theme_colors.theme, "nord");
    assert_eq!(c.theme_colors.dark_theme, "dracula");
    assert_eq!(c.rendering.max_fps, 144);
    assert_eq!(c.rendering.throughput_render_interval_ms, 250);
    assert_eq!(c.placement.target_monitor, Some(1));
    assert!(c.placement.lock_window_size);
    assert!(!c.background.keep_text_opaque);
    assert_eq!(c.background.background_image.as_deref(), Some("~/wall.png"));
    assert_eq!(c.background.background_image_opacity, 0.35);
    assert_eq!(c.tabs.tab_bar_height, 44.0);
    assert_eq!(c.tabs.max_tabs, 12);
    assert!(!c.tabs.tab_inherit_cwd);
    assert_eq!(c.tab_colors.tab_bar_background, [10, 20, 30]);
    assert_eq!(c.tab_colors.inactive_tab_opacity, 0.42);
    assert_eq!(c.panes.pane_divider_width, Some(5.0));
    assert_eq!(c.panes.max_panes, 4);
    assert!(c.panes.show_pane_titles);
    assert!(c.tmux.tmux_enabled);
    assert_eq!(c.tmux.tmux_prefix_key, "C-a");
    assert_eq!(c.tmux.tmux_status_bar_refresh_ms, 250);
    assert!(c.badge.badge_enabled);
    assert_eq!(c.badge.badge_color_alpha, 0.85);
    assert!(!c.progress_bar.progress_bar_enabled);
    assert_eq!(c.progress_bar.progress_bar_height, 9.0);
    assert!(!c.semantic_history.semantic_history_enabled);
    assert_eq!(c.semantic_history.link_handler_command, "firefox {url}");
    assert!(c.command_separator.command_separator_enabled);
    assert_eq!(c.command_separator.command_separator_thickness, 2.5);
    assert!(c.session_log.auto_log_sessions);
    assert_eq!(c.session_log.session_log_directory, "/var/log/par-term");
    assert_eq!(c.scrollbar.scrollbar_position, "left");
    assert_eq!(c.scrollbar.scrollbar_autohide_delay, 2500);
    assert_eq!(c.shell.custom_shell.as_deref(), Some("/bin/fish"));
    assert!(!c.shell.login_shell);
    assert_eq!(c.shell.initial_text_delay_ms, 750);
    assert!(!c.selection.auto_copy_selection);
    assert_eq!(c.selection.paste_delay_ms, 33);
    assert_eq!(c.word_selection.word_characters, "/-+_.@");
    assert_eq!(c.image.background_color, [12, 34, 56]);
    assert!(!c.image.image_preserve_aspect_ratio);
    assert!(c.input.use_physical_keys);
    assert_eq!(c.clipboard.clipboard_max_sync_events, 7);
    assert!(!c.power.pause_shaders_on_blur);
    assert_eq!(c.power.unfocused_fps, 5);
    assert!(c.shader_watch.shader_hot_reload);
    assert_eq!(c.shader_watch.shader_hot_reload_delay, 1500);
    assert!(c.security.allow_all_env_vars);
    assert!(c.security.allow_http_profiles);
    assert!(c.session_restore.restore_session);
    assert_eq!(c.session_restore.session_undo_timeout_secs, 90);
    assert_eq!(
        c.session_restore.auto_restore_arrangement.as_deref(),
        Some("work")
    );
}

// ---------------------------------------------------------------------------
// 4. Legacy spellings — `alias` attributes must travel with their field
// ---------------------------------------------------------------------------

/// Every `#[serde(alias = "…")]` on a moved field, exercised through the
/// flattened path.
///
/// An alias is a string inside an attribute: dropping one while moving the
/// field compiles cleanly and breaks only the configs that still use the old
/// spelling — exactly the users a compatibility gate exists for.
#[test]
fn legacy_key_spellings_still_load() {
    assert_eq!(
        parse("refresh_rate: 144").rendering.max_fps,
        144,
        "refresh_rate → max_fps",
    );

    assert!(
        parse("strip_trailing_newline_on_copy: true")
            .selection
            .copy_trailing_newline,
        "strip_trailing_newline_on_copy → copy_trailing_newline",
    );

    assert_eq!(
        parse("max_clipboard_sync_events: 5")
            .clipboard
            .clipboard_max_sync_events,
        5,
        "max_clipboard_sync_events → clipboard_max_sync_events",
    );
    assert_eq!(
        parse("max_clipboard_event_bytes: 64")
            .clipboard
            .clipboard_max_event_bytes,
        64,
        "max_clipboard_event_bytes → clipboard_max_event_bytes",
    );
}

/// `shell_exit_action` carries both aliases *and* a custom `deserialize_with`
/// accepting the pre-enum boolean form. That attribute names a bare function
/// resolved in the module declaring the field, so moving the field to another
/// file changes what the string refers to unless it is re-pathed.
#[test]
fn shell_exit_action_accepts_every_historical_form() {
    use par_term_config::ShellExitAction;

    assert_eq!(
        parse("exit_on_shell_exit: true").shell.shell_exit_action,
        ShellExitAction::Close,
        "legacy boolean alias exit_on_shell_exit: true",
    );
    assert_eq!(
        parse("close_on_shell_exit: false").shell.shell_exit_action,
        ShellExitAction::Keep,
        "legacy boolean alias close_on_shell_exit: false",
    );
    assert_eq!(
        parse("shell_exit_action: restart_after_delay")
            .shell
            .shell_exit_action,
        ShellExitAction::RestartAfterDelay,
        "current string form",
    );
}

// ---------------------------------------------------------------------------
// 5. `skip_serializing_if` — omitted at default, written when set
// ---------------------------------------------------------------------------

#[test]
fn optional_keys_are_omitted_at_their_default() {
    let map = as_map(&Config::default());

    for key in [
        "last_download_directory",
        "auto_restore_arrangement",
        "collapsed_settings_sections",
        "dynamic_profile_sources",
    ] {
        assert!(
            !map.contains_key(key),
            "`{key}` has #[serde(skip_serializing_if)] and must not be written at its default; \
             the attribute was lost when the field moved.",
        );
    }
}

#[test]
fn optional_keys_are_written_once_set() {
    let map = as_map(&parse(POPULATED_CONFIG));

    for key in [
        "last_download_directory",
        "auto_restore_arrangement",
        "collapsed_settings_sections",
    ] {
        assert!(
            map.contains_key(key),
            "`{key}` was set in the fixture but did not survive the round trip",
        );
    }
}
