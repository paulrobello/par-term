# Configuration Reference

Complete reference for `~/.config/par-term/config.yaml` (Linux/macOS) or
`%APPDATA%\par-term\config.yaml` (Windows).

Fields are grouped by functional area. All fields are optional — omitting a
field uses its documented default value.

> **Environment variable substitution**: Use `${VAR}` in string values. Only
> safe variables (HOME, USER, SHELL, XDG_*, PAR_TERM_*, LC_*) are substituted
> by default. Set `allow_all_env_vars: true` to allow all variables.

---

## Window / General

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `cols` | `usize` | `220` | Number of terminal columns |
| `rows` | `usize` | `50` | Number of terminal rows |
| `window_title` | `string` | `"par-term"` | Window title bar text |
| `allow_title_change` | `bool` | `true` | Allow OSC sequences to change the window title |
| `window_padding` | `f32` | `4.0` | Padding in pixels around terminal content |
| `hide_window_padding_on_split` | `bool` | `true` | Remove padding when panes are split |
| `window_opacity` | `f32` | `1.0` | Window transparency (0.0=transparent, 1.0=opaque) |
| `window_always_on_top` | `bool` | `false` | Keep window above all other windows |
| `window_decorations` | `bool` | `true` | Show window title bar and borders |
| `window_type` | `enum` | `normal` | `normal`, `fullscreen`, `edge_top`, `edge_bottom`, `edge_left`, `edge_right` |
| `target_monitor` | `usize?` | `null` | Monitor index for window placement (0=primary) |
| `target_space` | `u32?` | `null` | macOS Space (virtual desktop) index, 1-based |
| `lock_window_size` | `bool` | `false` | Prevent user from resizing window |
| `show_window_number` | `bool` | `false` | Show window number in title bar |
| `transparency_affects_only_default_background` | `bool` | `true` | Only make default background transparent, not colored areas |
| `keep_text_opaque` | `bool` | `true` | Render text at full opacity regardless of window transparency |
| `blur_enabled` | `bool` | `false` | macOS: blur content visible through transparent window |
| `blur_radius` | `u32` | `10` | macOS: blur radius in points (0–64) |
| `screenshot_format` | `string` | `"png"` | Screenshot file format: `png`, `jpeg`, `svg`, `html` |

---

## Fonts

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `font_size` | `f32` | `13.0` | Font size in points |
| `font_family` | `string` | `"Menlo"` | Regular/normal font family name |
| `font_family_bold` | `string?` | `null` | Bold font family (falls back to `font_family`) |
| `font_family_italic` | `string?` | `null` | Italic font family (falls back to `font_family`) |
| `font_family_bold_italic` | `string?` | `null` | Bold italic font family (falls back to `font_family`) |
| `font_ranges` | `array` | `[]` | Custom font mappings for Unicode ranges; each entry: `{start, end, font_family}` |
| `line_spacing` | `f32` | `1.2` | Line height multiplier (1.0=tight, 1.5=spacious) |
| `char_spacing` | `f32` | `0.6` | Character width multiplier |
| `enable_text_shaping` | `bool` | `true` | Enable HarfBuzz text shaping for ligatures and complex scripts |
| `enable_ligatures` | `bool` | `true` | Render font ligatures (requires `enable_text_shaping`) |
| `enable_kerning` | `bool` | `true` | Apply kerning adjustments (requires `enable_text_shaping`) |
| `font_antialias` | `bool` | `true` | Anti-aliased font rendering |
| `font_hinting` | `bool` | `true` | Font hinting for pixel-aligned rendering |
| `font_thin_strokes` | `enum` | `retina_only` | Stroke weight mode: `never`, `retina_only`, `dark_backgrounds_only`, `retina_dark_backgrounds_only`, `always` |
| `minimum_contrast` | `f32` | `1.0` | WCAG contrast ratio enforcement (1.0=off, 4.5=AA, 7.0=AAA) |

---

## Rendering

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_fps` | `u32` | `60` | Maximum frames per second target |
| `vsync_mode` | `enum` | `immediate` | VSync: `immediate`, `mailbox`, `fifo` |
| `power_preference` | `enum` | `none` | GPU preference: `none`, `low_power`, `high_performance` |
| `reduce_flicker` | `bool` | `true` | Delay redraws while cursor is hidden to reduce visual noise |
| `reduce_flicker_delay_ms` | `u32` | `16` | Max delay in ms before forced redraw during flicker reduction |
| `maximize_throughput` | `bool` | `false` | Throttle rendering during large outputs for lower CPU usage |
| `throughput_render_interval_ms` | `u32` | `100` | Render interval when throughput mode is active (50–500ms) |
| `pause_shaders_on_blur` | `bool` | `true` | Pause shader animations when window loses focus |
| `pause_refresh_on_blur` | `bool` | `false` | Reduce refresh rate when window is unfocused |
| `unfocused_fps` | `u32` | `10` | Target FPS when window is not focused (if `pause_refresh_on_blur`) |
| `inactive_tab_fps` | `u32` | `2` | Target FPS for background tabs (reduces CPU usage) |

---

## Background & Images

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `background_mode` | `enum` | `default` | `default` (theme color), `color` (solid), `image` |
| `background_color` | `[u8;3]` | `[0,0,0]` | Custom solid background color `[R, G, B]` (0-255) |
| `background_image` | `string?` | `null` | Path to background image (supports `~`) |
| `background_image_enabled` | `bool` | `true` | Enable/disable background image rendering |
| `background_image_mode` | `enum` | `stretch` | `fit`, `fill`, `stretch`, `tile`, `center` |
| `background_image_opacity` | `f32` | `0.5` | Background image opacity (0.0–1.0) |
| `image_scaling_mode` | `enum` | `linear` | Inline image scaling: `nearest` (sharp), `linear` (smooth) |
| `image_preserve_aspect_ratio` | `bool` | `true` | Preserve aspect ratio when scaling inline images |
| `pane_backgrounds` | `array` | `[]` | Per-pane background configs: `{index, image, mode, opacity, darken}` |

---

## Custom Shaders (Background)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `custom_shader` | `string?` | `null` | Path to GLSL background shader file |
| `custom_shader_enabled` | `bool` | `true` | Enable/disable the shader |
| `custom_shader_animation` | `bool` | `true` | Animate the shader (update `iTime` each frame) |
| `custom_shader_animation_speed` | `f32` | `1.0` | Animation speed multiplier |
| `custom_shader_text_opacity` | `f32` | `1.0` | Text opacity over shader background (0.0–1.0) |
| `custom_shader_brightness` | `f32` | `0.5` | Shader brightness multiplier (dims background) |
| `custom_shader_full_content` | `bool` | `false` | Pass full terminal content to shader for distortion effects |
| `custom_shader_channel0` | `string?` | `null` | Texture path for `iChannel0` |
| `custom_shader_channel1` | `string?` | `null` | Texture path for `iChannel1` |
| `custom_shader_channel2` | `string?` | `null` | Texture path for `iChannel2` |
| `custom_shader_channel3` | `string?` | `null` | Texture path for `iChannel3` |
| `custom_shader_cubemap` | `string?` | `null` | Cubemap path prefix for `iCubemap` (expects `-px/-nx/-py/-ny/-pz/-nz` suffixes) |
| `custom_shader_cubemap_enabled` | `bool` | `true` | Enable cubemap sampling |
| `custom_shader_use_background_as_channel0` | `bool` | `false` | Bind background image as `iChannel0` |
| `shader_hot_reload` | `bool` | `false` | Reload shader automatically when file is modified |
| `shader_hot_reload_delay` | `u64` | `250` | Debounce delay in ms before hot-reload triggers |

---

## Custom Shaders (Cursor)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `cursor_shader` | `string?` | `null` | Path to GLSL cursor shader file |
| `cursor_shader_enabled` | `bool` | `false` | Enable/disable cursor shader |
| `cursor_shader_animation` | `bool` | `true` | Animate cursor shader |
| `cursor_shader_animation_speed` | `f32` | `1.0` | Cursor shader animation speed |
| `cursor_shader_color` | `[u8;3]` | `[255,255,255]` | Cursor color passed to shader via `iCursorShaderColor` |
| `cursor_shader_trail_duration` | `f32` | `0.5` | Trail effect duration in seconds |
| `cursor_shader_glow_radius` | `f32` | `80.0` | Glow effect radius in pixels |
| `cursor_shader_glow_intensity` | `f32` | `0.3` | Glow intensity (0.0–1.0) |
| `cursor_shader_hides_cursor` | `bool` | `false` | Hide the default cursor when cursor shader is active |
| `cursor_shader_disable_in_alt_screen` | `bool` | `true` | Disable cursor shader in alt screen (vim, less, htop) |

---

## Keyboard Input

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `left_option_key_mode` | `enum` | `normal` | Left Option/Alt key: `normal`, `meta`, `esc` |
| `right_option_key_mode` | `enum` | `normal` | Right Option/Alt key: `normal`, `meta`, `esc` |
| `modifier_remapping` | `object` | `{}` | Remap modifier keys: fields `left_ctrl`, `right_ctrl`, `left_alt`, `right_alt`, `left_super`, `right_super` |
| `use_physical_keys` | `bool` | `false` | Use physical key positions for keybindings (layout-independent) |
| `keybindings` | `array` | (built-in defaults) | Custom keybindings: `[{key: "CmdOrCtrl+B", action: "toggle_tab_bar"}]` |

---

## Selection & Clipboard

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `auto_copy_selection` | `bool` | `true` | Auto-copy selected text to clipboard |
| `copy_trailing_newline` | `bool` | `false` | Include trailing newline when copying lines |
| `middle_click_paste` | `bool` | `true` | Paste on middle mouse button click |
| `paste_delay_ms` | `u64` | `0` | Delay between pasted lines in ms (for slow connections) |
| `dropped_file_quote_style` | `enum` | `single_quotes` | Quote style for dropped paths: `single_quotes`, `double_quotes`, `backslash`, `none` |
| `clipboard_max_sync_events` | `usize` | `100` | Maximum clipboard sync events retained |
| `clipboard_max_event_bytes` | `usize` | `1048576` | Maximum bytes per clipboard sync event |

---

## Mouse

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `mouse_scroll_speed` | `f32` | `3.0` | Mouse wheel scroll speed multiplier |
| `mouse_double_click_threshold` | `u64` | `300` | Double-click timing threshold in ms |
| `mouse_triple_click_threshold` | `u64` | `300` | Triple-click timing threshold in ms |
| `option_click_moves_cursor` | `bool` | `true` | Option+Click / Alt+Click moves text cursor to clicked position |
| `focus_follows_mouse` | `bool` | `false` | Focus window when mouse enters (no click required) |
| `report_horizontal_scroll` | `bool` | `true` | Report horizontal scroll to terminal applications |

---

## Word Selection & Copy Mode

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `word_characters` | `string` | `"/-+\\~_."` | Extra characters considered part of a word for double-click selection |
| `smart_selection_enabled` | `bool` | `true` | Enable pattern-based smart selection on double-click |
| `smart_selection_rules` | `array` | (built-in) | Custom smart selection rules: `{name, regex, precision, enabled}` |
| `copy_mode_enabled` | `bool` | `true` | Enable vi-style copy mode |
| `copy_mode_auto_exit_on_yank` | `bool` | `true` | Exit copy mode after yanking text |
| `copy_mode_show_status` | `bool` | `true` | Show status bar during copy mode |

---

## Scrollback & Unicode

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `scrollback_lines` | `usize` | `10000` | Maximum scrollback buffer size in lines |
| `unicode_version` | `enum` | `auto` | Unicode width table version: `unicode_9` … `unicode_16`, `auto` |
| `ambiguous_width` | `enum` | `narrow` | East Asian Ambiguous character width: `narrow`, `wide` |
| `normalization_form` | `enum` | `nfc` | Unicode normalization: `nfc`, `nfd`, `nfkc`, `nfkd`, `none` |

---

## Cursor

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `cursor_style` | `enum` | `block` | Cursor shape: `block`, `beam`, `underline` |
| `cursor_color` | `[u8;3]` | `[255,255,255]` | Cursor color `[R, G, B]` |
| `cursor_text_color` | `[u8;3]?` | `null` | Text color under block cursor (null=auto contrast) |
| `cursor_blink` | `bool` | `false` | Enable cursor blinking |
| `cursor_blink_interval` | `u64` | `500` | Cursor blink interval in ms |
| `unfocused_cursor_style` | `enum` | `hollow` | Cursor when unfocused: `hollow`, `same`, `hidden` |
| `lock_cursor_visibility` | `bool` | `false` | Prevent applications from hiding the cursor |
| `lock_cursor_style` | `bool` | `false` | Prevent applications from changing cursor style |
| `lock_cursor_blink` | `bool` | `false` | Prevent applications from enabling blink |
| `cursor_guide_enabled` | `bool` | `false` | Show horizontal highlight line at cursor row |
| `cursor_guide_color` | `[u8;4]` | `[128,128,128,30]` | Cursor guide color `[R, G, B, A]` |
| `cursor_shadow_enabled` | `bool` | `false` | Show drop shadow behind cursor |
| `cursor_shadow_color` | `[u8;4]` | (dark) | Shadow color `[R, G, B, A]` |
| `cursor_shadow_offset` | `[f32;2]` | `[2.0,2.0]` | Shadow offset in pixels `[x, y]` |
| `cursor_shadow_blur` | `f32` | `4.0` | Shadow blur radius in pixels |
| `cursor_boost` | `f32` | `0.0` | Cursor glow intensity (0.0=off, 1.0=max) |
| `cursor_boost_color` | `[u8;3]` | `[255,255,255]` | Cursor glow color `[R, G, B]` |

---

## Scrollbar

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `scrollbar_position` | `string` | `"right"` | Scrollbar position: `"left"` or `"right"` |
| `scrollbar_width` | `f32` | `8.0` | Scrollbar width in pixels |
| `scrollbar_thumb_color` | `[f32;4]` | `[0.5,0.5,0.5,0.6]` | Scrollbar thumb color RGBA (0.0–1.0 each) |
| `scrollbar_track_color` | `[f32;4]` | `[0.0,0.0,0.0,0.0]` | Scrollbar track color RGBA |
| `scrollbar_autohide_delay` | `u64` | `2000` | Milliseconds before scrollbar auto-hides (0=never) |
| `scrollbar_command_marks` | `bool` | `true` | Show command markers on scrollbar (requires shell integration) |
| `scrollbar_mark_tooltips` | `bool` | `false` | Show tooltips on scrollbar command markers |

---

## Theme & Colors

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `theme` | `string` | `"Builtin Dark"` | Color theme name |
| `auto_dark_mode` | `bool` | `false` | Automatically switch theme based on system light/dark mode |
| `light_theme` | `string` | `"Builtin Light"` | Theme to use in system light mode |
| `dark_theme` | `string` | `"Builtin Dark"` | Theme to use in system dark mode |

---

## Shell Behavior

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `custom_shell` | `string?` | `null` | Custom shell path (defaults to `$SHELL`) |
| `shell_args` | `[string]?` | `null` | Arguments to pass to the shell |
| `login_shell` | `bool` | `true` | Launch shell as login shell (`-l` flag) |
| `shell_exit_action` | `enum` | `close` | On shell exit: `close`, `keep`, `restart_immediately`, `restart_with_prompt`, `restart_after_delay` |
| `startup_directory_mode` | `enum` | `home` | Where new sessions start: `home`, `previous`, `custom` |
| `startup_directory` | `string?` | `null` | Custom startup directory (when mode is `custom`) |
| `working_directory` | `string?` | `null` | Legacy startup directory override |
| `shell_env` | `{string:string}?` | `null` | Extra environment variables for the shell |
| `initial_text` | `string` | `""` | Text sent to shell on session start |
| `initial_text_delay_ms` | `u64` | `100` | Delay before sending initial text (ms) |
| `initial_text_send_newline` | `bool` | `false` | Append newline after initial text |
| `answerback_string` | `string` | `""` | Response to ENQ (terminal identification, disabled by default) |
| `prompt_on_quit` | `bool` | `false` | Confirm before closing window with active sessions |
| `confirm_close_running_jobs` | `bool` | `false` | Confirm before closing tab with running commands |
| `jobs_to_ignore` | `[string]` | (shell names) | Process names that don't trigger close confirmation |
| `command_history_max_entries` | `usize` | `1000` | Max commands in fuzzy search history |

---

## Semantic History (File/URL Detection)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `semantic_history_enabled` | `bool` | `true` | Enable file path and URL detection on Cmd/Ctrl+Click |
| `semantic_history_editor_mode` | `enum` | `environment_variable` | Editor selection: `custom`, `environment_variable`, `system_default` |
| `semantic_history_editor` | `string` | `""` | Editor command when mode is `custom` (use `{file}` and `{line}` placeholders) |
| `link_highlight_color` | `[u8;3]` | `[0,150,255]` | URL and file path highlight color |
| `link_highlight_underline` | `bool` | `true` | Underline highlighted links |
| `link_underline_style` | `enum` | `stipple` | Underline style: `solid`, `stipple` |
| `link_handler_command` | `string` | `""` | Custom URL open command (use `{url}` placeholder; empty=system default) |

---

## Tabs

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `tab_style` | `enum` | `dark` | Tab visual preset: `dark`, `light`, `compact`, `minimal`, `high_contrast`, `automatic` |
| `light_tab_style` | `enum` | `light` | Tab style for system light mode (when `tab_style: automatic`) |
| `dark_tab_style` | `enum` | `dark` | Tab style for system dark mode (when `tab_style: automatic`) |
| `tab_bar_mode` | `enum` | `always` | Tab bar visibility: `always`, `when_multiple`, `never` |
| `tab_title_mode` | `enum` | `auto` | How tab titles update: `auto`, `osc_only` |
| `tab_bar_height` | `f32` | `28.0` | Tab bar height in pixels |
| `tab_bar_position` | `enum` | `top` | Tab bar position: `top`, `bottom`, `left` |
| `tab_bar_width` | `f32` | `200.0` | Tab bar width in pixels (when position is `left`) |
| `tab_show_close_button` | `bool` | `true` | Show close (×) button on each tab |
| `tab_show_index` | `bool` | `false` | Show tab index number (for Cmd+1-9) |
| `tab_inherit_cwd` | `bool` | `true` | New tabs inherit working directory from active tab |
| `max_tabs` | `usize` | `0` | Maximum tabs per window (0=unlimited) |
| `tab_min_width` | `f32` | `80.0` | Minimum tab width before horizontal scrolling |
| `tab_stretch_to_fill` | `bool` | `false` | Stretch tabs to fill available tab bar width |
| `tab_html_titles` | `bool` | `false` | Render tab titles as limited HTML |
| `tab_border_width` | `f32` | `0.0` | Tab border width in pixels (0=no border) |
| `tab_inactive_outline_only` | `bool` | `false` | Render inactive tabs as outline only |
| `dim_inactive_tabs` | `bool` | `true` | Visually dim inactive tabs |
| `inactive_tab_opacity` | `f32` | `0.6` | Inactive tab opacity (0.0–1.0) |
| `new_tab_shortcut_shows_profiles` | `bool` | `false` | Show profile selector instead of opening default tab |

---

## Split Panes

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `pane_divider_width` | `f32?` | `1.0` | Divider line width in pixels |
| `pane_divider_hit_width` | `f32` | `5.0` | Drag-target width for resizing panes |
| `pane_padding` | `f32` | `0.0` | Padding inside each pane in pixels |
| `pane_min_size` | `usize` | `2` | Minimum pane size in terminal cells |
| `pane_background_opacity` | `f32` | `1.0` | Pane background opacity (allows shader/image show-through) |
| `pane_divider_style` | `enum` | `solid` | Divider style: `solid`, `double`, `dashed`, `shadow` |
| `max_panes` | `usize` | `16` | Maximum panes per tab (0=unlimited) |
| `dim_inactive_panes` | `bool` | `false` | Visually dim inactive panes |
| `inactive_pane_opacity` | `f32` | `0.7` | Inactive pane opacity |
| `show_pane_titles` | `bool` | `false` | Show title bar on each pane |
| `pane_title_height` | `f32` | `18.0` | Pane title bar height in pixels |
| `pane_title_position` | `enum` | `top` | Title bar position: `top`, `bottom` |
| `pane_focus_indicator` | `bool` | `true` | Show border around focused pane |
| `pane_focus_width` | `f32` | `1.0` | Focused pane border width in pixels |

---

## tmux Integration

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `tmux_enabled` | `bool` | `false` | Enable tmux control mode integration |
| `tmux_path` | `string` | `"tmux"` | Path to tmux executable |
| `tmux_auto_attach` | `bool` | `false` | Auto-attach to existing tmux session on startup |
| `tmux_auto_attach_session` | `string?` | `null` | Session name to auto-attach to |
| `tmux_default_session` | `string?` | `null` | Default session name for new sessions |
| `tmux_clipboard_sync` | `bool` | `true` | Sync clipboard with tmux paste buffer |
| `tmux_show_status_bar` | `bool` | `false` | Show tmux status bar in par-term UI |
| `tmux_prefix_key` | `string` | `"C-b"` | tmux prefix key combination |
| `tmux_status_bar_refresh_ms` | `u64` | `1000` | Status bar refresh interval in ms |
| `tmux_status_bar_use_native_format` | `bool` | `false` | Use native tmux format strings for status bar |
| `tmux_status_bar_left` | `string` | `"[{session}] {windows}"` | Left status bar format |
| `tmux_status_bar_right` | `string` | `"{pane} \| {time:%H:%M}"` | Right status bar format |

---

## Notifications

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `notification_bell_desktop` | `bool` | `false` | Forward BEL to desktop notification center |
| `notification_bell_sound` | `u8` | `50` | Bell sound volume (0=disabled, 1–100) |
| `notification_bell_visual` | `bool` | `true` | Show visual flash on BEL |
| `notification_activity_enabled` | `bool` | `false` | Notify when activity resumes after inactivity |
| `notification_activity_threshold` | `u64` | `30` | Seconds of inactivity before activity alert fires |
| `notification_silence_enabled` | `bool` | `false` | Notify after prolonged silence |
| `notification_silence_threshold` | `u64` | `60` | Seconds of silence before alert fires |
| `notification_session_ended` | `bool` | `false` | Notify when session exits |
| `suppress_notifications_when_focused` | `bool` | `true` | Suppress desktop notifications when window is focused |
| `notification_max_buffer` | `usize` | `100` | Max OSC 9/777 notifications retained |
| `alert_sounds` | `{event: config}` | `{}` | Per-event sound config: keys are `bell`, `command_complete`, `new_tab`, `tab_close` |
| `anti_idle_enabled` | `bool` | `false` | Send keep-alive after idle period |
| `anti_idle_seconds` | `u64` | `60` | Idle seconds before sending keep-alive |
| `anti_idle_code` | `u8` | `0` | ASCII code to send as keep-alive (0=NUL) |

---

## SSH

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enable_mdns_discovery` | `bool` | `false` | Enable mDNS/Bonjour SSH host discovery |
| `mdns_scan_timeout_secs` | `u32` | `5` | mDNS scan timeout in seconds |
| `ssh_auto_profile_switch` | `bool` | `true` | Auto-switch profile based on SSH hostname |
| `ssh_revert_profile_on_disconnect` | `bool` | `true` | Revert profile when SSH session disconnects |

---

## Session Logging

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `auto_log_sessions` | `bool` | `false` | Automatically record all terminal sessions |
| `session_log_format` | `enum` | `asciicast` | Log format: `plain`, `html`, `asciicast` |
| `session_log_directory` | `string` | `"~/.local/share/par-term/logs/"` | Directory for session log files |
| `archive_on_close` | `bool` | `true` | Flush session log when tab closes |
| `session_log_redact_passwords` | `bool` | `true` | Redact password prompt input in session logs |

---

## Search

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `search_highlight_color` | `[u8;4]` | `[255,200,0,180]` | Highlight color for search matches `[R, G, B, A]` |
| `search_current_highlight_color` | `[u8;4]` | `[255,100,0,220]` | Highlight color for current/active match |
| `search_case_sensitive` | `bool` | `false` | Case-sensitive search by default |
| `search_regex` | `bool` | `false` | Enable regex mode by default |
| `search_wrap_around` | `bool` | `true` | Wrap search results at buffer boundaries |

---

## Status Bar

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `status_bar_enabled` | `bool` | `false` | Show the status bar |
| `status_bar_position` | `enum` | `bottom` | Status bar position: `top`, `bottom` |
| `status_bar_height` | `f32` | `22.0` | Status bar height in pixels |
| `status_bar_font` | `string` | `""` | Status bar font family (empty=terminal font) |
| `status_bar_font_size` | `f32` | `12.0` | Status bar font size in points |
| `status_bar_separator` | `string` | `" │ "` | Separator between widgets |
| `status_bar_auto_hide_fullscreen` | `bool` | `true` | Auto-hide status bar in fullscreen |
| `status_bar_auto_hide_mouse_inactive` | `bool` | `false` | Auto-hide when mouse is inactive |
| `status_bar_mouse_inactive_timeout` | `f32` | `3.0` | Timeout in seconds before hiding on mouse inactivity |
| `status_bar_system_poll_interval` | `f32` | `2.0` | CPU/memory/network polling interval in seconds |
| `status_bar_git_poll_interval` | `f32` | `5.0` | Git branch detection polling interval in seconds |
| `status_bar_time_format` | `string` | `"%H:%M"` | Clock widget time format (chrono strftime) |
| `status_bar_git_show_status` | `bool` | `true` | Show ahead/behind and dirty indicators in git widget |
| `status_bar_widgets` | `array` | (built-in defaults) | Widget list with `{id, enabled, ...}` entries |

---

## Progress Bar

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `progress_bar_enabled` | `bool` | `true` | Show OSC 9;4 / OSC 934 progress bars |
| `progress_bar_style` | `enum` | `bar` | Style: `bar`, `bar_with_text` |
| `progress_bar_position` | `enum` | `top` | Position: `top`, `bottom` |
| `progress_bar_height` | `f32` | `4.0` | Bar height in pixels |
| `progress_bar_opacity` | `f32` | `1.0` | Bar opacity (0.0–1.0) |
| `progress_bar_normal_color` | `[u8;3]` | `[0,180,80]` | Color for normal progress state |
| `progress_bar_warning_color` | `[u8;3]` | `[255,165,0]` | Color for warning state |
| `progress_bar_error_color` | `[u8;3]` | `[220,50,50]` | Color for error state |
| `progress_bar_indeterminate_color` | `[u8;3]` | `[80,150,255]` | Color for indeterminate state |

---

## Badge (Session Label)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `badge_enabled` | `bool` | `false` | Show the badge overlay |
| `badge_format` | `string` | `""` | Badge text with `\(variable)` substitution |
| `badge_color` | `[u8;3]` | `[255,0,0]` | Badge text color |
| `badge_color_alpha` | `f32` | `0.5` | Badge opacity (0.0–1.0) |
| `badge_font` | `string` | `""` | Badge font family |
| `badge_font_bold` | `bool` | `true` | Use bold badge font |
| `badge_top_margin` | `f32` | `5.0` | Top margin in pixels from terminal edge |
| `badge_right_margin` | `f32` | `5.0` | Right margin in pixels from terminal edge |
| `badge_max_width` | `f32` | `0.5` | Max badge width as fraction of terminal width (0.0–1.0) |
| `badge_max_height` | `f32` | `0.2` | Max badge height as fraction of terminal height |

---

## Automation & Scripting

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `triggers` | `array` | `[]` | Regex trigger definitions. Each entry: `{name, pattern, enabled, actions, require_user_action}` |
| `coprocesses` | `array` | `[]` | Coprocess definitions. Each entry: `{name, command, args, auto_start, copy_terminal_output, restart_policy, restart_delay_ms}` |
| `scripts` | `array` | `[]` | External observer script definitions |
| `snippets` | `array` | `[]` | Text snippets: `{id, title, content, keybinding, folder, enabled, auto_execute}` |
| `actions` | `array` | `[]` | Custom shell/text/key actions: `{type: shell_command|insert_text|key_sequence, id, title, ...}` |

---

## AI Inspector

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `ai_inspector_enabled` | `bool` | `false` | Enable AI Inspector panel |
| `ai_inspector_open_on_startup` | `bool` | `false` | Open inspector automatically on startup |
| `ai_inspector_width` | `f32` | `400.0` | Inspector panel width in pixels |
| `ai_inspector_default_scope` | `string` | `"visible"` | Default capture scope: `visible`, `scrollback`, `selection` |
| `ai_inspector_agent` | `string` | `"claude"` | AI agent identifier for queries |
| `ai_inspector_auto_launch` | `bool` | `false` | Auto-launch agent when inspector opens |
| `ai_inspector_auto_context` | `bool` | `true` | Include terminal context with AI queries |
| `ai_inspector_context_max_lines` | `usize` | `100` | Max terminal lines included as context |
| `ai_inspector_auto_approve` | `bool` | `false` | Auto-approve AI-suggested actions |
| `ai_inspector_agent_terminal_access` | `bool` | `false` | Allow AI agent to write input to terminal |
| `ai_inspector_agent_screenshot_access` | `bool` | `false` | Allow AI agent to request screenshots |
| `ai_inspector_custom_agents` | `array` | `[]` | Additional ACP agent definitions (overrides discovered agents with same identity) |

---

## Update Checking

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `update_check_frequency` | `enum` | `daily` | How often to check for updates: `never`, `hourly`, `daily`, `weekly`, `monthly` |

---

## Security

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `allow_all_env_vars` | `bool` | `false` | Allow all environment variables in `${VAR}` substitution (not just the safe allowlist) |

---

## Content Prettifier

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enable_prettifier` | `bool` | `true` | Master switch for the content prettifier system |
| `content_prettifier` | `object` | `{}` | Detailed prettifier configuration (see [PRETTIFIER.md](PRETTIFIER.md)) |

---

## Sessions & Arrangements

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `restore_session` | `bool` | `false` | Restore previous session (tabs, panes, CWDs) on startup |
| `auto_restore_arrangement` | `string?` | `null` | Name of arrangement to auto-restore on startup |
| `session_undo_timeout_secs` | `u32` | `30` | Seconds to keep closed tab metadata for undo (0=disabled) |
| `session_undo_max_entries` | `usize` | `10` | Maximum closed tabs remembered for undo |
| `session_undo_preserve_shell` | `bool` | `false` | Preserve shell process on tab close for undo |

---

## Profiles

Profiles are stored in a separate `~/.config/par-term/profiles.yaml` file.
Each profile can override shell, working directory, badge, SSH host, and more.
See [PROFILES.md](PROFILES.md) for full documentation.

Dynamic profiles can be fetched from remote URLs:

```yaml
dynamic_profile_sources:
  - url: "https://example.com/profiles.yaml"
    conflict_resolution: keep_local  # or prefer_remote, merge
```

---

## Command Separator Lines

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `command_separator_enabled` | `bool` | `false` | Show horizontal separator lines between commands |
| `command_separator_thickness` | `f32` | `1.0` | Separator line thickness in pixels |
| `command_separator_opacity` | `f32` | `0.4` | Separator line opacity (0.0–1.0) |
| `command_separator_exit_color` | `bool` | `true` | Color separators by exit code (green=success, red=failure) |
| `command_separator_color` | `[u8;3]` | `[128,128,128]` | Custom separator color when `exit_color` is disabled |

---

## Debug Logging

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `log_level` | `enum` | `off` | Debug log verbosity: `off`, `error`, `warn`, `info`, `debug`, `trace` |
