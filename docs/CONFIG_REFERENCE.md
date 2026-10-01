# Configuration Reference

Complete reference for `~/.config/par-term/config.yaml` (Linux/macOS) or
`%APPDATA%\par-term\config.yaml` (Windows).

Fields are grouped by functional area. All fields are optional — omitting a
field uses its documented default value.

> **Settings column**: the last column of each table names the one Settings
> section that edits the key, as Tab › Page › Section (see
> [Settings Reference](features/SETTINGS.md)). `YAML only` marks keys with no
> Settings control: edit them in `config.yaml`. `Internal` marks keys par-term
> writes itself. Fields of list rows (agents, profiles) name the editor that
> owns the row. The column is generated: run `make docs-settings` after
> changing Settings, and do not edit it by hand.

> **Environment variable substitution**: Use `${VAR}` in string values. Only
> allowlisted safe variables (e.g. HOME, USER, SHELL, TERM, LANG, PATH,
> EDITOR, XDG_*, PAR_TERM_*, LC_*) are substituted by default. Set
> `allow_all_env_vars: true` to allow all variables.

## Table of Contents
- [Window / General](#window--general)
- [Fonts](#fonts)
- [Rendering](#rendering)
- [Background & Images](#background--images)
- [Custom Shaders (Background)](#custom-shaders-background)
- [Per-Shader Configuration Overrides](#per-shader-configuration-overrides)
- [File Transfers](#file-transfers)
- [Custom Shaders (Cursor)](#custom-shaders-cursor)
- [Keyboard Input](#keyboard-input)
- [Selection & Clipboard](#selection--clipboard)
- [Mouse](#mouse)
- [Word Selection & Copy Mode](#word-selection--copy-mode)
- [Scrollback & Unicode](#scrollback--unicode)
- [Cursor](#cursor)
- [Scrollbar](#scrollbar)
- [Theme & Colors](#theme--colors)
- [Shell Behavior](#shell-behavior)
- [Semantic History (File/URL Detection)](#semantic-history-fileurl-detection)
- [Tabs](#tabs)
- [Split Panes](#split-panes)
- [tmux Integration](#tmux-integration)
- [Notifications](#notifications)
- [SSH](#ssh)
- [Output Recording](#output-recording)
- [Search](#search)
- [Status Bar](#status-bar)
- [Agent Usage](#agent-usage)
- [Agents (Launcher)](#agents-launcher)
- [Progress Bar](#progress-bar)
- [Badge](#badge)
- [Automation & Scripting](#automation--scripting)
- [Assistant Panel](#assistant-panel)
- [Update Checking](#update-checking)
- [Security](#security)
- [Settings UI](#settings-ui)
- [Restore & Arrangements](#restore--arrangements)
- [Profiles](#profiles)
- [Command Separator Lines](#command-separator-lines)
- [Debug Logging](#debug-logging)
- [Related Documentation](#related-documentation)

---

## Window / General

> **v0.30.0:** Window appearance fields (`window_opacity`, `window_always_on_top`, `window_decorations`, `blur_enabled`, `blur_radius`, `window_padding`, `hide_window_padding_on_split`, `snap_window_to_grid`) are now internally grouped under a `WindowConfig` sub-struct. Existing YAML configs are fully backward-compatible.

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `cols` | `usize` | `80` | Number of terminal columns | Windows & Tabs › Window › Display |
| `rows` | `usize` | `24` | Number of terminal rows | Windows & Tabs › Window › Display |
| `window_title` | `string` | `"par-term"` | Window title bar text | Windows & Tabs › Window › Display |
| `allow_title_change` | `bool` | `true` | Allow OSC sequences to change the window title | Windows & Tabs › Window › Display |
| `window_padding` | `f32` | `1.0` | Padding in pixels around terminal content | Windows & Tabs › Window › Display |
| `hide_window_padding_on_split` | `bool` | `true` | Remove padding when panes are split | Windows & Tabs › Window › Display |
| `snap_window_to_grid` | `bool` | `true` | Snap window dimensions to exact terminal cell boundaries during resize, eliminating blank background gaps. Disabled automatically in split-pane mode. | Windows & Tabs › Window › Display |
| `window_opacity` | `f32` | `1.0` | Window transparency (0.0=transparent, 1.0=opaque) | Windows & Tabs › Window › Transparency |
| `window_always_on_top` | `bool` | `false` | Keep window above all other windows | Windows & Tabs › Window › Window Behavior |
| `window_decorations` | `bool` | `true` | Show window title bar and borders | Windows & Tabs › Window › Window Behavior |
| `window_type` | `enum` | `normal` | `normal`, `fullscreen`, `edge_top`, `edge_bottom`, `edge_left`, `edge_right` | Windows & Tabs › Window › Window Behavior |
| `target_monitor` | `usize?` | `null` | Monitor index for window placement (0=primary) | Windows & Tabs › Window › Window Behavior |
| `target_space` | `u32?` | `null` | macOS Space (virtual desktop) index, 1-based | Windows & Tabs › Window › Window Behavior |
| `lock_window_size` | `bool` | `false` | Prevent user from resizing window | Windows & Tabs › Window › Window Behavior |
| `show_window_number` | `bool` | `false` | Show window number in title bar | Windows & Tabs › Window › Window Behavior |
| `transparency_affects_only_default_background` | `bool` | `true` | Only make default background transparent, not colored areas | Windows & Tabs › Window › Transparency |
| `keep_text_opaque` | `bool` | `true` | Render text at full opacity regardless of window transparency | Windows & Tabs › Window › Transparency |
| `blur_enabled` | `bool` | `false` | macOS: blur content visible through transparent window | Windows & Tabs › Window › Transparency |
| `blur_radius` | `u32` | `8` | macOS: blur radius in points (0–64) | Windows & Tabs › Window › Transparency |
| `screenshot_format` | `string` | `"png"` | Screenshot file format: `png`, `jpeg`, `svg`, `html` | General › Integration & Files › Screenshots |

---

## Fonts

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `font_size` | `f32` | `13.0` | Font size in points | Appearance › Text & Fonts › Fonts |
| `font_family` | `string` | `"JetBrains Mono"` | Regular/normal font family name | Appearance › Text & Fonts › Fonts |
| `font_family_bold` | `string?` | `null` | Bold font family (falls back to `font_family`) | Appearance › Text & Fonts › Font Variants |
| `font_family_italic` | `string?` | `null` | Italic font family (falls back to `font_family`) | Appearance › Text & Fonts › Font Variants |
| `font_family_bold_italic` | `string?` | `null` | Bold italic font family (falls back to `font_family`) | Appearance › Text & Fonts › Font Variants |
| `font_ranges` | `array` | `[]` | Custom font mappings for Unicode ranges; each entry: `{start, end, font_family}` | YAML only |
| `line_spacing` | `f32` | `1.0` | Line height multiplier (1.0=tight, 1.5=spacious) | Appearance › Text & Fonts › Fonts |
| `char_spacing` | `f32` | `1.0` | Character width multiplier | Appearance › Text & Fonts › Fonts |
| `enable_text_shaping` | `bool` | `true` | Enable HarfBuzz text shaping for ligatures and complex scripts | YAML only |
| `enable_ligatures` | `bool` | `true` | Render font ligatures (requires `enable_text_shaping`) | YAML only |
| `enable_kerning` | `bool` | `true` | Apply kerning adjustments (requires `enable_text_shaping`) | YAML only |

> **v0.30.0:** The following rendering fields are now internally grouped under a `FontRenderingConfig` sub-struct. Existing YAML configs are fully backward-compatible.

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `font_antialias` | `bool` | `true` | Anti-aliased font rendering | Appearance › Text & Fonts › Font Rendering |
| `font_hinting` | `bool` | `true` | Font hinting for pixel-aligned rendering | Appearance › Text & Fonts › Font Rendering |
| `font_thin_strokes` | `enum` | `retina_only` | Stroke weight mode: `never`, `retina_only`, `dark_backgrounds_only`, `retina_dark_backgrounds_only`, `always` | Appearance › Text & Fonts › Font Rendering |
| `minimum_contrast` | `f32` | `0.0` | Perceived brightness contrast enforcement on a 0.0–1.0 scale (0.0=disabled, 1.0=maximum). Uses iTerm2-compatible perceived brightness model. Changed from WCAG scale in v0.25.0 — if migrating from an earlier version, set this to `0.0` to disable or `0.5` for moderate enforcement. | Appearance › Text & Fonts › Font Rendering |

---

## Rendering

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `max_fps` | `u32` | `60` | Maximum frames per second target | Advanced › Performance & Power › Performance |
| `vsync_mode` | `enum` | `fifo` | VSync: `immediate`, `mailbox`, `fifo` | Advanced › Performance & Power › Performance |
| `power_preference` | `enum` | `none` | GPU preference: `none`, `low_power`, `high_performance` | Advanced › Performance & Power › Performance |
| `reduce_flicker` | `bool` | `true` | Delay redraws while cursor is hidden to reduce visual noise | Advanced › Performance & Power › Performance |
| `reduce_flicker_delay_ms` | `u32` | `16` | Max delay in ms before forced redraw during flicker reduction | Advanced › Performance & Power › Performance |
| `maximize_throughput` | `bool` | `false` | Throttle rendering during large outputs for lower CPU usage | Advanced › Performance & Power › Performance |
| `throughput_render_interval_ms` | `u32` | `100` | Render interval when throughput mode is active (50–500ms) | Advanced › Performance & Power › Performance |
| `pause_shaders_on_blur` | `bool` | `true` | Pause shader animations when window loses focus | Advanced › Performance & Power › Performance |
| `pause_refresh_on_blur` | `bool` | `true` | Reduce refresh rate when window is unfocused | Advanced › Performance & Power › Performance |
| `unfocused_fps` | `u32` | `30` | Target FPS when window is not focused (if `pause_refresh_on_blur`) | Advanced › Performance & Power › Performance |
| `inactive_tab_fps` | `u32` | `2` | Target FPS for background tabs (reduces CPU usage) | Advanced › Performance & Power › Performance |

---

## Background & Images

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `background_mode` | `enum` | `default` | `default` (theme color), `color` (solid), `image` | Effects & Shaders › Background & Shader › Background & Effects |
| `background_color` | `[u8;3]` | `[30,30,30]` | Custom solid background color `[R, G, B]` (0-255) | Effects & Shaders › Background & Shader › Background & Effects |
| `background_image` | `string?` | `null` | Path to background image (supports `~`) | Effects & Shaders › Background & Shader › Background & Effects |
| `background_image_enabled` | `bool` | `true` | Enable/disable background image rendering | Effects & Shaders › Background & Shader › Background & Effects |
| `background_image_mode` | `enum` | `stretch` | `fit`, `fill`, `stretch`, `tile`, `center` | Effects & Shaders › Background & Shader › Background & Effects |
| `background_image_opacity` | `f32` | `1.0` | Background image opacity (0.0–1.0) | Effects & Shaders › Background & Shader › Background & Effects |
| `image_scaling_mode` | `enum` | `linear` | Inline image scaling: `nearest` (sharp), `linear` (smooth) | Effects & Shaders › Inline Images › Inline Images (Sixel, iTerm2, Kitty) |
| `image_preserve_aspect_ratio` | `bool` | `true` | Preserve aspect ratio when scaling inline images | Effects & Shaders › Inline Images › Inline Images (Sixel, iTerm2, Kitty) |
| `pane_backgrounds` | `array` | `[]` | Per-pane background configs: `{index, image, mode, opacity, darken}` | Internal |

---

## Custom Shaders (Background)

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `custom_shader` | `string?` | `null` | Path to GLSL background shader file | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_enabled` | `bool` | `true` | Enable/disable the shader | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_animation` | `bool` | `true` | Animate the shader (update `iTime` each frame) | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_animation_speed` | `f32` | `1.0` | Animation speed multiplier | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_text_opacity` | `f32` | `1.0` | Text opacity over shader background (0.0–1.0) | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_brightness` | `f32` | `0.15` | Shader brightness multiplier (dims background) | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_full_content` | `bool` | `false` | Pass full terminal content to shader for distortion effects | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_channel0` | `string?` | `null` | Texture path for `iChannel0` | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_channel1` | `string?` | `null` | Texture path for `iChannel1` | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_channel2` | `string?` | `null` | Texture path for `iChannel2` | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_channel3` | `string?` | `null` | Texture path for `iChannel3` | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_cubemap` | `string?` | `null` | Cubemap path prefix for `iCubemap` (expects `-px/-nx/-py/-ny/-pz/-nz` suffixes) | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_cubemap_enabled` | `bool` | `true` | Enable cubemap sampling | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_use_background_as_channel0` | `bool` | `false` | Bind background image as `iChannel0` | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_background_channel0_blend_mode` | `enum` | `replace` | Blend-mode hint exposed as `iBackgroundBlendMode` when using background as `iChannel0`; values: `replace`, `multiply`, `screen`, `overlay`, `luminance_mask` | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_auto_dim_under_text` | `bool` | `false` | Reduce shader intensity under terminal text for readability | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_auto_dim_strength` | `f32` | `0.35` | Auto-dim strength under text (0.0 = no extra dim, 1.0 = black) | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_readability_mode` | `bool` | `false` | Temporary low-power/readability mode for quick toggles | Effects & Shaders › Background & Shader › Background & Effects |
| `custom_shader_readability_brightness` | `f32` | `0.35` | Brightness cap while readability mode is enabled | Effects & Shaders › Background & Shader › Background & Effects |
| `shader_hot_reload` | `bool` | `false` | Reload shader automatically when file is modified | Effects & Shaders › Background & Shader › Background & Effects |
| `shader_hot_reload_delay` | `u64` | `100` | Debounce delay in ms before hot-reload triggers | Effects & Shaders › Background & Shader › Background & Effects |

---

## Per-Shader Configuration Overrides

Override shader settings per-file. Keys are shader filenames (without path).

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `shader_configs` | `map` | `{}` | Per-background-shader overrides. Each value: `{animation_speed?, brightness?, text_opacity?, full_content?, channel0–3?, cubemap?, cubemap_enabled?, use_background_as_channel0?, background_channel0_blend_mode?, auto_dim_under_text?, auto_dim_strength?, uniforms?}` | Internal |
| `cursor_shader_configs` | `map` | `{}` | Per-cursor-shader overrides. Same fields as `shader_configs` plus `hides_cursor?`, `disable_in_alt_screen?`, `glow_radius?`, `glow_intensity?`, `trail_duration?`, and `cursor_color?` | Internal |

---

## File Transfers

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `download_save_location` | `enum` | `downloads` | Default save location for downloaded files: `downloads`, `last_used`, `cwd`, or `!custom /path/to/dir` (a YAML tag, not a mapping) | General › Integration & Files › File Transfers |

---

## Custom Shaders (Cursor)

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `cursor_shader` | `string?` | `null` | Path to GLSL cursor shader file | Appearance › Cursor › Cursor Shader |
| `cursor_shader_enabled` | `bool` | `false` | Enable/disable cursor shader | Appearance › Cursor › Cursor Shader |
| `cursor_shader_animation` | `bool` | `true` | Animate cursor shader | Appearance › Cursor › Cursor Shader |
| `cursor_shader_animation_speed` | `f32` | `1.0` | Cursor shader animation speed | Appearance › Cursor › Cursor Shader |
| `cursor_shader_color` | `[u8;3]` | `[255,255,255]` | Cursor color passed to shader via `iCursorShaderColor` | Appearance › Cursor › Cursor Shader |
| `cursor_shader_trail_duration` | `f32` | `0.5` | Trail effect duration in seconds | Appearance › Cursor › Cursor Shader |
| `cursor_shader_glow_radius` | `f32` | `80.0` | Glow effect radius in pixels | Appearance › Cursor › Cursor Shader |
| `cursor_shader_glow_intensity` | `f32` | `0.3` | Glow intensity (0.0–1.0) | Appearance › Cursor › Cursor Shader |
| `cursor_shader_hides_cursor` | `bool` | `false` | Hide the default cursor when cursor shader is active | Appearance › Cursor › Cursor Shader |
| `cursor_shader_disable_in_alt_screen` | `bool` | `true` | Disable cursor shader in alt screen (vim, less, htop) | Appearance › Cursor › Cursor Shader |

---

## Keyboard Input

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `left_option_key_mode` | `enum` | `esc` | Left Option/Alt key: `normal`, `meta`, `esc` | Keys › Option/Alt › Keyboard |
| `right_option_key_mode` | `enum` | `esc` | Right Option/Alt key: `normal`, `meta`, `esc` | Keys › Option/Alt › Keyboard |
| `modifier_remapping` | `object` | `{}` | Remap modifier keys: fields `left_ctrl`, `right_ctrl`, `left_alt`, `right_alt`, `left_super`, `right_super` | Keys › Modifiers › Modifier Remapping |
| `use_physical_keys` | `bool` | `false` | Use physical key positions for keybindings (layout-independent) | Keys › Option/Alt › Keyboard |
| `leader_key` | `string` | `Cmd+B` (macOS), `Ctrl+Shift+B` (Linux/Windows) | The leader chord, in keybinding syntax: arms a one-key table of window, tab, pane, and par-mux session actions ([Leader Key](features/LEADER_KEY.md)). Empty turns the leader off | YAML only |
| `leader_timeout_ms` | `integer` | `2000` | How long the armed leader waits for its next key | YAML only |
| `leader_overlay_delay_ms` | `integer` | `400` | How long after the leader its which-key overlay appears | YAML only |
| `leader_vim_keys` | `bool` | `false` | Add `h j k l` (focus) and `H J K L` (swap) to the leader table; last-used tab moves from `l` to `Tab` | YAML only |
| `keybindings` | `array` | (built-in defaults) | Custom keybindings: `[{key: "CmdOrCtrl+Shift+B", action: "toggle_background_shader"}]` | Keys › Key Bindings › Keybindings |

---

## Selection & Clipboard

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `auto_copy_selection` | `bool` | `true` | Auto-copy selected text to clipboard | General › Selection & Clipboard › Selection & Clipboard |
| `copy_trailing_newline` | `bool` | `false` | Include trailing newline when copying lines | General › Selection & Clipboard › Selection & Clipboard |
| `middle_click_paste` | `bool` | `true` | Paste on middle mouse button click | General › Selection & Clipboard › Selection & Clipboard |
| `paste_delay_ms` | `u64` | `0` | Delay between pasted lines in ms (for slow connections) | General › Selection & Clipboard › Selection & Clipboard |
| `dropped_file_quote_style` | `enum` | `single_quotes` | Quote style for dropped paths: `single_quotes`, `double_quotes`, `backslash`, `none` | General › Selection & Clipboard › Selection & Clipboard |
| `clipboard_max_sync_events` | `usize` | `64` | Maximum clipboard sync events retained | General › Selection & Clipboard › Clipboard Limits |
| `clipboard_max_event_bytes` | `usize` | `2048` | Maximum bytes per clipboard sync event | General › Selection & Clipboard › Clipboard Limits |
| `osc52_clipboard` | `bool` | `true` | Apply OSC 52 clipboard-set sequences from programs to the system clipboard. Lets remote apps (tmux, herdr, etc.) copy to the local clipboard over SSH. | General › Selection & Clipboard › Selection & Clipboard |
| `warn_paste_control_chars` | `bool` | `true` | Log a warning when clipboard paste content contains VT escape sequences | General › Selection & Clipboard › Selection & Clipboard |

---

## Mouse

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `mouse_scroll_speed` | `f32` | `3.0` | Mouse wheel scroll speed multiplier | Pointer › Mouse › Mouse |
| `mouse_double_click_threshold` | `u64` | `500` | Double-click timing threshold in ms | Pointer › Mouse › Mouse |
| `mouse_triple_click_threshold` | `u64` | `500` | Triple-click timing threshold in ms | Pointer › Mouse › Mouse |
| `option_click_moves_cursor` | `bool` | `true` | Option+Click / Alt+Click moves text cursor to clicked position | Pointer › Mouse › Mouse |
| `focus_follows_mouse` | `bool` | `false` | Focus window when mouse enters (no click required) | Pointer › Mouse › Mouse |
| `pane_focus_follows_mouse` | `bool` | `false` | Focus the split pane under the pointer as it moves (no click required); waits while a button is held and does nothing while a pane is zoomed | Pointer › Mouse › Mouse |
| `report_horizontal_scroll` | `bool` | `true` | Report horizontal scroll to terminal applications | Pointer › Mouse › Mouse |

---

## Word Selection & Copy Mode

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `word_characters` | `string` | `"/-+\\~_."` | Extra characters considered part of a word for double-click selection | Pointer › Word Selection › Word Selection |
| `smart_selection_enabled` | `bool` | `true` | Enable pattern-based smart selection on double-click | Pointer › Word Selection › Word Selection |
| `smart_selection_rules` | `array` | (built-in) | Custom smart selection rules: `{name, regex, precision, enabled}` | Pointer › Word Selection › Word Selection |
| `copy_mode_enabled` | `bool` | `true` | Enable vi-style copy mode | Pointer › Copy Mode › Copy Mode |
| `copy_mode_auto_exit_on_yank` | `bool` | `true` | Exit copy mode after yanking text | Pointer › Copy Mode › Copy Mode |
| `copy_mode_show_status` | `bool` | `true` | Show status bar during copy mode | Pointer › Copy Mode › Copy Mode |

---

## Scrollback & Unicode

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `scrollback_lines` | `usize` | `10000` | Maximum scrollback buffer size in lines | Advanced › Terminal Emulation › Scrollback |
| `unicode_version` | `enum` | `auto` | Unicode width table version: `unicode9`, `unicode10`, `unicode11`, `unicode12`, `unicode13`, `unicode14`, `unicode15`, `unicode15_1`, `unicode16`, `auto` (no underscore before the digit) | Advanced › Terminal Emulation › Unicode |
| `ambiguous_width` | `enum` | `narrow` | East Asian Ambiguous character width: `narrow`, `wide` | Advanced › Terminal Emulation › Unicode |
| `normalization_form` | `enum` | `NFC` | Unicode normalization: `NFC`, `NFD`, `NFKC`, `NFKD`, `none` (uppercase — this enum has no `rename_all`) | Advanced › Terminal Emulation › Unicode |

---

## Cursor

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `cursor_style` | `enum` | `block` | Cursor shape: `block`, `beam`, `underline` | Appearance › Cursor › Cursor |
| `cursor_color` | `[u8;3]` | `[255,255,255]` | Cursor color `[R, G, B]` | Appearance › Cursor › Cursor |
| `cursor_text_color` | `[u8;3]?` | `null` | Text color under block cursor (null=auto contrast) | Appearance › Cursor › Cursor |
| `cursor_blink` | `bool` | `false` | Enable cursor blinking | Appearance › Cursor › Cursor |
| `cursor_blink_interval` | `u64` | `500` | Cursor blink interval in ms | Appearance › Cursor › Cursor |
| `unfocused_cursor_style` | `enum` | `hollow` | Cursor when unfocused: `hollow`, `same`, `hidden` | Appearance › Cursor › Cursor |
| `lock_cursor_visibility` | `bool` | `false` | Prevent applications from hiding the cursor | Appearance › Cursor › Cursor Locks |
| `lock_cursor_style` | `bool` | `false` | Prevent applications from changing cursor style | Appearance › Cursor › Cursor Locks |
| `lock_cursor_blink` | `bool` | `false` | Prevent applications from enabling blink | Appearance › Cursor › Cursor Locks |
| `cursor_guide_enabled` | `bool` | `false` | Show horizontal highlight line at cursor row | Appearance › Cursor › Cursor Effects |
| `cursor_guide_color` | `[u8;4]` | `[255,255,255,20]` | Cursor guide color `[R, G, B, A]` | Appearance › Cursor › Cursor Effects |
| `cursor_shadow_enabled` | `bool` | `false` | Show drop shadow behind cursor | Appearance › Cursor › Cursor Effects |
| `cursor_shadow_color` | `[u8;4]` | `[0,0,0,128]` | Shadow color `[R, G, B, A]` (semi-transparent black) | Appearance › Cursor › Cursor Effects |
| `cursor_shadow_offset` | `[f32;2]` | `[2.0,2.0]` | Shadow offset in pixels `[x, y]` | Appearance › Cursor › Cursor Effects |
| `cursor_shadow_blur` | `f32` | `3.0` | Shadow blur radius in pixels | Appearance › Cursor › Cursor Effects |
| `cursor_boost` | `f32` | `0.0` | Cursor glow intensity (0.0=off, 1.0=max) | Appearance › Cursor › Cursor Effects |
| `cursor_boost_color` | `[u8;3]` | `[255,255,255]` | Cursor glow color `[R, G, B]` | Appearance › Cursor › Cursor Effects |

---

## Scrollbar

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `scrollbar_position` | `string` | `"right"` | Scrollbar position: `"left"` or `"right"` | Windows & Tabs › Scrollbar › Scrollbar |
| `scrollbar_width` | `f32` | `15.0` | Scrollbar width in pixels | Windows & Tabs › Scrollbar › Scrollbar |
| `scrollbar_thumb_color` | `[f32;4]` | `[0.4,0.4,0.4,0.95]` | Scrollbar thumb color RGBA (0.0–1.0 each) | Windows & Tabs › Scrollbar › Scrollbar |
| `scrollbar_track_color` | `[f32;4]` | `[0.15,0.15,0.15,0.6]` | Scrollbar track color RGBA | Windows & Tabs › Scrollbar › Scrollbar |
| `scrollbar_autohide_delay` | `u64` | `0` | Milliseconds before scrollbar auto-hides (0=never/always visible) | Windows & Tabs › Scrollbar › Scrollbar |
| `scrollbar_command_marks` | `bool` | `true` | Show command markers on scrollbar (requires shell integration) | Windows & Tabs › Scrollbar › Scrollbar |
| `scrollbar_mark_tooltips` | `bool` | `false` | Show tooltips on scrollbar command markers | Windows & Tabs › Scrollbar › Scrollbar |

---

## Theme & Colors

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `theme` | `string` | `"dark-background"` | Color theme name | Appearance › Theme › Theme |
| `auto_dark_mode` | `bool` | `false` | Automatically switch theme based on system light/dark mode | Appearance › Theme › Auto Dark Mode |
| `light_theme` | `string` | `"light-background"` | Theme to use in system light mode | Appearance › Theme › Auto Dark Mode |
| `dark_theme` | `string` | `"dark-background"` | Theme to use in system dark mode | Appearance › Theme › Auto Dark Mode |

---

## Shell Behavior

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `custom_shell` | `string?` | `null` | Custom shell path (defaults to `$SHELL`) | General › Startup & Restore › Shell |
| `shell_args` | `[string]?` | `null` | Arguments to pass to the shell | General › Startup & Restore › Shell |
| `login_shell` | `bool` | `true` | Launch shell as login shell (`-l` flag) | General › Startup & Restore › Shell |
| `shell_exit_action` | `enum` | `close` | On shell exit: `close`, `keep`, `restart_immediately`, `restart_with_prompt`, `restart_after_delay` | General › Closing & Quitting › Closing & Quitting |
| `startup_directory_mode` | `enum` | `home` | Where new shells start: `home`, `previous`, `custom` | General › Startup & Restore › Shell |
| `startup_directory` | `string?` | `null` | Custom startup directory (when mode is `custom`) | General › Startup & Restore › Shell |
| `working_directory` | `string?` | `null` | Legacy startup directory override | YAML only |
| `shell_env` | `{string:string}?` | `null` | Extra environment variables for the shell | YAML only |
| `initial_text` | `string` | `""` | Text sent to shell when a new shell starts | General › Startup & Restore › Startup |
| `initial_text_delay_ms` | `u64` | `100` | Delay before sending initial text (ms) | General › Startup & Restore › Startup |
| `initial_text_send_newline` | `bool` | `true` | Append newline after initial text | General › Startup & Restore › Startup |
| `answerback_string` | `string` | `""` | Response to ENQ (terminal identification, disabled by default) | Advanced › Terminal Emulation › Unicode |
| `prompt_on_quit` | `bool` | `true` | Confirm before closing a non-empty window when `confirm_close_multiple_tabs` is off | General › Closing & Quitting › Closing & Quitting |
| `confirm_close_multiple_tabs` | `bool` | `true` | Confirm before closing a window that holds more than one tab | General › Closing & Quitting › Closing & Quitting |
| `confirm_close_running_jobs` | `bool` | `false` | Confirm before closing tab with running commands | General › Closing & Quitting › Closing & Quitting |
| `jobs_to_ignore` | `[string]` | (shell names) | Process names that don't trigger close confirmation | General › Closing & Quitting › Closing & Quitting |
| `command_history_max_entries` | `usize` | `1000` | Max commands in fuzzy search history | General › Search & Links › Command History |

---

## Semantic History (File/URL Detection)

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `semantic_history_enabled` | `bool` | `true` | Enable file path and URL detection on Cmd/Ctrl+Click | General › Search & Links › Semantic History |
| `semantic_history_editor_mode` | `enum` | `environment_variable` | Editor selection: `custom`, `environment_variable`, `system_default` | General › Search & Links › Semantic History |
| `semantic_history_editor` | `string` | `""` | Editor command when mode is `custom` (use `{file}` and `{line}` placeholders) | General › Search & Links › Semantic History |
| `link_highlight_color` | `[u8;3]` | `[79,195,247]` | URL and file path highlight color | General › Search & Links › Semantic History |
| `link_highlight_color_enabled` | `bool` | `true` | Enable link highlight color | General › Search & Links › Semantic History |
| `link_highlight_underline` | `bool` | `true` | Underline highlighted links | General › Search & Links › Semantic History |
| `link_underline_style` | `enum` | `stipple` | Underline style: `solid`, `stipple` | General › Search & Links › Semantic History |
| `link_handler_command` | `string` | `""` | Custom URL open command (use `{url}` placeholder; empty=system default) | General › Search & Links › Semantic History |
| `allow_file_scheme_urls` | `bool` | `false` | Allow Cmd/Ctrl+Click to open `file://` OSC 8 hyperlinks via the OS handler. Off by default (SEC-009): a remote program can emit `file://` links to open arbitrary local paths | General › Search & Links › Semantic History |

---

## Tabs

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `tab_style` | `enum` | `dark` | Tab visual preset: `dark`, `light`, `compact`, `minimal`, `high_contrast`, `automatic` | Windows & Tabs › Tab Bar › Tab Bar |
| `light_tab_style` | `enum` | `light` | Tab style for system light mode (when `tab_style: automatic`) | Windows & Tabs › Tab Bar › Tab Bar |
| `dark_tab_style` | `enum` | `dark` | Tab style for system dark mode (when `tab_style: automatic`) | Windows & Tabs › Tab Bar › Tab Bar |
| `tab_bar_mode` | `enum` | `always` | Tab bar visibility: `always`, `when_multiple`, `never` | Windows & Tabs › Tab Bar › Tab Bar |
| `tab_title_mode` | `enum` | `auto` | How tab titles update: `auto`, `osc_only` | Windows & Tabs › Tab Bar › Tab Bar |
| `remote_tab_title_format` | `enum` | `user_at_host` | Tab title format when shell integration detects a remote host: `user_at_host` (`user@host`), `host` (hostname only), `host_and_cwd` (`host:~/cwd`) | Windows & Tabs › Tab Bar › Tab Bar |
| `remote_tab_title_osc_priority` | `bool` | `true` | When `true`, explicit OSC title sequences take precedence over `remote_tab_title_format` | Windows & Tabs › Tab Bar › Tab Bar |
| `tab_bar_height` | `f32` | `28.0` | Tab bar height in pixels | Windows & Tabs › Tab Bar › Tab Bar |
| `tab_bar_position` | `enum` | `top` | Tab bar position: `top`, `bottom`, `left` | Windows & Tabs › Tab Bar › Tab Bar |
| `tab_bar_width` | `f32` | `160.0` | Tab bar width in pixels (when position is `left`) | Windows & Tabs › Tab Bar › Tab Bar |
| `tab_bar_background` | `[u8;3]` | `[40,40,40]` | Tab bar background color `[R, G, B]` | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `tab_active_background` | `[u8;3]` | `[60,60,60]` | Active tab background color | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `tab_inactive_background` | `[u8;3]` | `[40,40,40]` | Inactive tab background color | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `tab_hover_background` | `[u8;3]` | `[50,50,50]` | Tab background color on hover | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `tab_active_text` | `[u8;3]` | `[255,255,255]` | Active tab text color | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `tab_inactive_text` | `[u8;3]` | `[180,180,180]` | Inactive tab text color | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `tab_active_indicator` | `[u8;3]` | `[100,150,255]` | Active tab indicator color (underline) | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `tab_activity_indicator` | `[u8;3]` | `[100,180,255]` | Activity indicator dot color | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `tab_bell_indicator` | `[u8;3]` | `[255,200,100]` | Bell indicator icon color | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `tab_close_button` | `[u8;3]` | `[150,150,150]` | Close button color | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `tab_close_button_hover` | `[u8;3]` | `[255,100,100]` | Close button color on hover | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `tab_border_color` | `[u8;3]` | `[80,80,80]` | Tab border color | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `tab_show_close_button` | `bool` | `true` | Show close (×) button on each tab | Windows & Tabs › Tab Bar › Tab Bar |
| `tab_show_index` | `bool` | `false` | Show tab index number (for Cmd+1-9) | Windows & Tabs › Tab Bar › Tab Bar |
| `tab_inherit_cwd` | `bool` | `true` | New tabs inherit working directory from active tab | Windows & Tabs › Tab Bar › Tab Bar |
| `max_tabs` | `usize` | `0` | Maximum tabs per window (0=unlimited) | Windows & Tabs › Tab Bar › Tab Bar |
| `show_profile_drawer_button` | `bool` | `false` | Show the Open Profiles button in the tab bar (opens the profile launcher) | Windows & Tabs › Tab Bar › Tab Bar |
| `tab_min_width` | `f32` | `120.0` | Minimum tab width before horizontal scrolling | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `tab_stretch_to_fill` | `bool` | `true` | Stretch tabs to fill available tab bar width | Windows & Tabs › Tab Bar › Tab Bar |
| `tab_html_titles` | `bool` | `false` | Render tab titles as limited HTML | Windows & Tabs › Tab Bar › Tab Bar |
| `tab_border_width` | `f32` | `1.0` | Tab border width in pixels (0=no border) | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `tab_inactive_outline_only` | `bool` | `true` | Render inactive tabs as outline only | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `dim_inactive_tabs` | `bool` | `true` | Visually dim inactive tabs | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `inactive_tab_opacity` | `f32` | `0.6` | Inactive tab opacity (0.0–1.0) | Windows & Tabs › Tab Bar Colors › Tab Bar Appearance |
| `new_tab_shortcut_shows_profiles` | `bool` | `false` | The new-tab shortcut opens Open Profiles (the profile launcher) instead of a default tab | Windows & Tabs › Tab Bar › Tab Bar |
| `new_tab_position` | `enum` | `end` | Where new tabs are inserted: `end` (append to tab bar) or `after_active` (insert right of current tab) | Windows & Tabs › Tab Bar › Tab Bar |

---

## Split Panes

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `pane_divider_width` | `f32?` | `2.0` | Divider line width in pixels | Panes › Layout & Dividers › Split Panes |
| `pane_divider_hit_width` | `f32` | `8.0` | Drag-target width for resizing panes; double-click a divider to equalize its split | Panes › Layout & Dividers › Split Panes |
| `pane_padding` | `f32` | `1.0` | Padding inside each pane in pixels | Panes › Layout & Dividers › Split Panes |
| `pane_min_size` | `usize` | `10` | Minimum pane size in terminal cells; enforced on split, divider drag, keyboard resize, equalize, and layout presets | Panes › Layout & Dividers › Split Panes |
| `pane_resize_step` | `f32` | `5.0` | Percent of the enclosing split one keyboard resize press moves a divider (resize actions and resize mode arrows; `Shift` + arrow in resize mode moves one cell) | Panes › Layout & Dividers › Split Panes |
| `pane_background_opacity` | `f32` | `1.0` | Pane background opacity (allows shader/image show-through) | Panes › Appearance › Pane Appearance |
| `pane_divider_style` | `enum` | `solid` | Divider style: `solid`, `double`, `dashed`, `shadow` | Panes › Layout & Dividers › Split Panes |
| `pane_divider_color` | `[u8;3]` | `[80,80,80]` | Divider line color | Panes › Appearance › Pane Appearance |
| `pane_divider_hover_color` | `[u8;3]` | `[120,150,200]` | Divider color on hover (resize feedback) | Panes › Appearance › Pane Appearance |
| `max_panes` | `usize` | `16` | Maximum panes per tab (0=unlimited) | Panes › Layout & Dividers › Split Panes |
| `split_balance` | `enum` | `none` | Rebalance after a keyboard or menu split: `none` (halve the split pane), `siblings` (even out the row or column the new pane joined), `all` (equalize the whole tab). Trigger and snippet splits keep their own percent; par-mux tabs keep the daemon's layout | Panes › Layout & Dividers › Split Panes |
| `split_inherits_profile` | `bool` | `true` | A split of a profile tab runs the tab's profile program (its SSH connection, command, or shell) instead of the default shell; profiles that attach a tmux or par-mux session are never inherited | Panes › Layout & Dividers › Split Panes |
| `dim_inactive_panes` | `bool` | `false` | Visually dim inactive panes | Panes › Appearance › Pane Appearance |
| `inactive_pane_dim_mode` | `enum` | `darken` | How inactive panes dim: `darken` (colors toward black, text stays solid) or `fade` (the pane turns transparent, text included) | Panes › Appearance › Pane Appearance |
| `inactive_pane_opacity` | `f32` | `0.7` | Dim level for inactive panes: brightness in `darken` mode, opacity in `fade` mode | Panes › Appearance › Pane Appearance |
| `show_pane_titles` | `bool` | `false` | Show title bar on each pane | Panes › Appearance › Pane Appearance |
| `show_pane_numbers` | `bool` | `false` | Start each pane title with the pane's number in layout order (needs `show_pane_titles`) | Panes › Appearance › Pane Appearance |
| `pane_title_height` | `f32` | `20.0` | Pane title bar height in pixels | Panes › Appearance › Pane Appearance |
| `pane_title_position` | `enum` | `top` | Title bar position: `top`, `bottom` | Panes › Appearance › Pane Appearance |
| `pane_title_color` | `[u8;3]` | `[200,200,200]` | Pane title text color | Panes › Appearance › Pane Appearance |
| `pane_title_bg_color` | `[u8;3]` | `[40,40,50]` | Pane title background color | Panes › Appearance › Pane Appearance |
| `pane_title_font` | `string` | `""` | Pane title font family (empty=terminal font) | YAML only |
| `pane_focus_indicator` | `bool` | `true` | Show border around focused pane | Panes › Layout & Dividers › Split Panes |
| `pane_focus_color` | `[u8;3]` | `[100,150,255]` | Focused pane border color | Panes › Layout & Dividers › Split Panes |
| `pane_focus_width` | `f32` | `1.0` | Focused pane border width in pixels | Panes › Layout & Dividers › Split Panes |

---

## tmux Integration

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `tmux_enabled` | `bool` | `false` | Enable tmux control mode integration | Sessions › tmux › tmux Integration |
| `tmux_path` | `string` | `"tmux"` | Path to tmux executable | Sessions › tmux › tmux Integration |
| `tmux_auto_attach` | `bool` | `false` | Auto-attach to existing tmux session on startup | Sessions › tmux › tmux Integration |
| `tmux_auto_attach_session` | `string?` | `null` | Session name to auto-attach to | Sessions › tmux › tmux Integration |
| `mux_auto_attach` | `string?` | `null` | par-mux session the first window attaches to on launch, created when missing; works with tmux integration off. The `--attach <session>` flag overrides it | Sessions › par-mux › par-mux |
| `tmux_default_session` | `string?` | `null` | Session used when a tmux session is started without a name (auto-attach with no session name, session picker "new"); attaches if it already exists, creates it otherwise | Sessions › tmux › tmux Integration |
| `tmux_clipboard_sync` | `bool` | `true` | Sync clipboard with tmux paste buffer | Sessions › tmux › tmux Integration |
| `tmux_hide_gateway_tab` | `bool` | `false` | Hide the control-mode gateway tab from the tab bar while tmux windows are active; the tab is restored when the session ends | Sessions › tmux › tmux Integration |
| `tmux_profile` | `string?` | `null` | Profile to use for tmux sessions | Sessions › tmux › tmux Integration |
| `tmux_show_status_bar` | `bool` | `false` | Show tmux status bar in par-term UI | Sessions › tmux › tmux Integration |
| `tmux_prefix_key` | `string` | `"C-b"` | tmux prefix key combination | Sessions › tmux › tmux Integration |
| `tmux_status_bar_refresh_ms` | `u64` | `1000` | Status bar refresh interval in ms | Sessions › tmux › tmux Integration |
| `tmux_status_bar_use_native_format` | `bool` | `false` | Use native tmux format strings for status bar | YAML only |
| `tmux_status_bar_left` | `string` | `"[{session}] {windows}"` | Left status bar format | Sessions › tmux › tmux Integration |
| `tmux_status_bar_right` | `string` | `"{pane} \| {time:%H:%M}"` | Right status bar format | Sessions › tmux › tmux Integration |

---

## Notifications

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `notification_bell_desktop` | `bool` | `false` | Forward BEL to desktop notification center | Advanced › Notifications & Bell › Bell |
| `notification_bell_sound` | `u8` | `50` | Bell sound volume (0=disabled, 1–100). Not used while `alert_sounds.bell` is enabled, which sets the bell's sound instead | Advanced › Notifications & Bell › Bell |
| `notification_bell_visual` | `bool` | `true` | Show visual flash on BEL | Advanced › Notifications & Bell › Bell |
| `notification_visual_bell_color` | `[u8;3]` | `[255,255,255]` | Visual bell flash color | Advanced › Notifications & Bell › Bell |
| `notification_activity_enabled` | `bool` | `false` | Notify when activity resumes after inactivity | Advanced › Notifications & Bell › Activity |
| `notification_activity_threshold` | `u64` | `10` | Seconds of inactivity before activity alert fires | Advanced › Notifications & Bell › Activity |
| `notification_silence_enabled` | `bool` | `false` | Notify after prolonged silence | Advanced › Notifications & Bell › Activity |
| `notification_silence_threshold` | `u64` | `300` | Seconds of silence before alert fires (5 minutes) | Advanced › Notifications & Bell › Activity |
| `notification_session_ended` | `bool` | `false` | Notify when a shell exits | Advanced › Notifications & Bell › Activity |
| `suppress_notifications_when_focused` | `bool` | `true` | Suppress desktop notifications when window is focused | Advanced › Notifications & Bell › Behavior |
| `notification_max_buffer` | `usize` | `64` | Max OSC 9/777 notifications retained | Advanced › Notifications & Bell › Behavior |
| `alert_sounds` | `{event: config}` | `{}` | Per-event sound config: keys are `bell`, `command_complete`, `new_tab`, `tab_close`. An enabled `bell` entry replaces `notification_bell_sound` (it plays even when that is 0); a disabled entry leaves `notification_bell_sound` in charge | Advanced › Notifications & Bell › Alert Sounds |
| `anti_idle_enabled` | `bool` | `false` | Send keep-alive after idle period | Advanced › Terminal Emulation › Anti-Idle Keep-Alive |
| `anti_idle_seconds` | `u64` | `60` | Idle seconds before sending keep-alive | Advanced › Terminal Emulation › Anti-Idle Keep-Alive |
| `anti_idle_code` | `u8` | `0` | ASCII code to send as keep-alive (0=NUL) | Advanced › Terminal Emulation › Anti-Idle Keep-Alive |

---

## SSH

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `enable_mdns_discovery` | `bool` | `false` | Enable mDNS/Bonjour SSH host discovery | Profiles › SSH › SSH |
| `mdns_scan_timeout_secs` | `u32` | `3` | mDNS scan timeout in seconds | Profiles › SSH › SSH |
| `ssh_auto_profile_switch` | `bool` | `true` | Auto-switch profile based on SSH hostname | Profiles › SSH › SSH |
| `ssh_revert_profile_on_disconnect` | `bool` | `true` | Revert profile when SSH session disconnects | Profiles › SSH › SSH |

---

## Output Recording

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `auto_log_sessions` | `bool` | `false` | Automatically record every tab's output | Advanced › Logging › Output Recording |
| `session_log_format` | `enum` | `asciicast` | Log format: `plain`, `html`, `asciicast`, `asciicast_v3` | Advanced › Logging › Output Recording |
| `session_log_directory` | `string` | `"~/.local/share/par-term/logs/"` | Directory for recording files | Advanced › Logging › Output Recording |
| `session_log_redact_passwords` | `bool` | `true` | Redact password prompt input in recordings | Advanced › Logging › Output Recording |

---

## Search

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `search_highlight_color` | `[u8;4]` | `[255,200,0,180]` | Highlight color for search matches `[R, G, B, A]` | General › Search & Links › Search |
| `search_current_highlight_color` | `[u8;4]` | `[255,100,0,220]` | Highlight color for current/active match | General › Search & Links › Search |
| `search_case_sensitive` | `bool` | `false` | Case-sensitive search by default | General › Search & Links › Search |
| `search_regex` | `bool` | `false` | Enable regex mode by default | General › Search & Links › Search |
| `search_wrap_around` | `bool` | `true` | Wrap search results at buffer boundaries | General › Search & Links › Search |

---

## Status Bar

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `status_bar_enabled` | `bool` | `false` | Show the status bar | Advanced › Status Bar › General |
| `status_bar_position` | `enum` | `bottom` | Status bar position: `top`, `bottom` | Advanced › Status Bar › General |
| `status_bar_height` | `f32` | `22.0` | Status bar height in pixels | Advanced › Status Bar › General |
| `status_bar_bg_color` | `[u8;3]` | `[30,30,30]` | Status bar background color `[R, G, B]` | Advanced › Status Bar › Styling |
| `status_bar_bg_alpha` | `f32` | `0.95` | Status bar background alpha (0.0–1.0) | Advanced › Status Bar › Styling |
| `status_bar_fg_color` | `[u8;3]` | `[200,200,200]` | Status bar foreground/text color `[R, G, B]` | Advanced › Status Bar › Styling |
| `status_bar_font` | `string` | `""` | Status bar font family (empty=terminal font) | YAML only |
| `status_bar_font_size` | `f32` | `12.0` | Status bar font size in points | Advanced › Status Bar › Styling |
| `status_bar_separator` | `string` | `" \u{2502} "` | Separator between widgets (Unicode box-drawing vertical line) | Advanced › Status Bar › Styling |
| `status_bar_auto_hide_fullscreen` | `bool` | `true` | Auto-hide status bar in fullscreen | Advanced › Status Bar › Auto-Hide |
| `status_bar_auto_hide_mouse_inactive` | `bool` | `false` | Auto-hide when mouse is inactive | Advanced › Status Bar › Auto-Hide |
| `status_bar_mouse_inactive_timeout` | `f32` | `3.0` | Timeout in seconds before hiding on mouse inactivity | Advanced › Status Bar › Auto-Hide |
| `status_bar_system_poll_interval` | `f32` | `2.0` | CPU/memory/network polling interval in seconds | Advanced › Status Bar › Poll Intervals |
| `status_bar_git_poll_interval` | `f32` | `5.0` | Git branch detection polling interval in seconds | Advanced › Status Bar › Poll Intervals |
| `status_bar_time_format` | `string` | `"%H:%M:%S"` | Clock widget time format (chrono strftime) | Advanced › Status Bar › Widget Options |
| `status_bar_git_show_status` | `bool` | `true` | Show ahead/behind and dirty indicators in git widget | Advanced › Status Bar › Widget Options |
| `status_bar_disk_poll_interval` | `f32` | `60.0` | Disk free-space polling interval in seconds (5.0–600.0) | Advanced › Status Bar › Poll Intervals |
| `status_bar_disk_follow_cwd` | `bool` | `false` | Track the active tab's disk (`true`) vs the disk par-term launched from (`false`) | Advanced › Status Bar › Poll Intervals |
| `status_bar_widgets` | `array` | (built-in defaults) | Widget list with `{id, enabled, ...}` entries | Advanced › Status Bar › Widgets |

---

## Agent Usage

Agent subscription usage display (widget + popup panel) over a records
directory written by external collectors — see
[Agent Usage](features/AGENT_USAGE.md) for the record contract.

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `agent_usage_enabled` | `bool` | `true` | Enable the agent-usage subsystem (directory watch + panel) | Assistant & Agents › Agent Usage › Agent Usage |
| `agent_usage_update_command` | `string` | (none) | Optional command run through `sh -c` on the refresh interval and manual refresh, expected to rewrite the records directory; absent = pure watch mode | Assistant & Agents › Agent Usage › Agent Usage |
| `agent_usage_refresh_interval_sec` | `u64` | `300` | Seconds between periodic rescans (clamped to a 30 s floor) | Assistant & Agents › Agent Usage › Agent Usage |
| `agent_usage_hidden_agents` | `array` | `[]` | Agent ids to hide from the widget and panel | Assistant & Agents › Agent Usage › Agent Usage |

---

## Agents (Launcher)

Top-level `agents:` list — CLI coding agents exposed as command-palette
"Launch" rows. Managed under Settings → Assistant & Agents → Agents.

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `id` | `string` | required | Stable slug used in `launch-agent:<id>` / `launch-agent-autonomous:<id>` action names and keybindings; must be unique | Assistant & Agents › Agents › Agents |
| `name` | `string` | required | Human-readable palette label (`Launch <name>`) | Assistant & Agents › Agents › Agents |
| `command` | `string` | required | The command line typed into the new pane's shell | Assistant & Agents › Agents › Agents |
| `autonomy_args` | `string` | `""` | Arguments appended for the `(autonomous)` palette row. Empty/absent means no autonomous variant is offered | Assistant & Agents › Agents › Agents |
| `default` | `bool` | `false` | Whether "Launch Default Agent" resolves here. Multiple `default: true` entries are legal; the first wins | Assistant & Agents › Agents › Agents |

```yaml
agents:
  - id: claude
    name: Claude
    command: claude
    autonomy_args: --permission-mode auto
    default: true
  - id: omp
    name: Oh My Pi
    command: omp
```

Palette rows: every entry gets `Launch <name>`; a non-empty `autonomy_args`
additionally gets the labelled `Launch <name> (autonomous)` row — autonomy is
offered, never defaulted. The first `default: true` entry adds
`Launch Default Agent (<name>)`. In a par-mux tab a launch splits daemon-side
and types the command into the new daemon pane.

---

## Progress Bar

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `progress_bar_enabled` | `bool` | `true` | Show OSC 9;4 / OSC 934 progress bars | Appearance › Progress Bar › General |
| `progress_bar_style` | `enum` | `bar` | Style: `bar`, `barwithtext` (lowercased, no underscore) | Appearance › Progress Bar › General |
| `progress_bar_position` | `enum` | `top` | Position: `top`, `bottom` | Appearance › Progress Bar › General |
| `progress_bar_height` | `f32` | `4.0` | Bar height in pixels | Appearance › Progress Bar › General |
| `progress_bar_opacity` | `f32` | `0.8` | Bar opacity (0.0–1.0) | Appearance › Progress Bar › General |
| `progress_bar_normal_color` | `[u8;3]` | `[80,180,255]` | Color for normal progress state | Appearance › Progress Bar › State Colors |
| `progress_bar_warning_color` | `[u8;3]` | `[255,200,50]` | Color for warning state | Appearance › Progress Bar › State Colors |
| `progress_bar_error_color` | `[u8;3]` | `[255,80,80]` | Color for error state | Appearance › Progress Bar › State Colors |
| `progress_bar_indeterminate_color` | `[u8;3]` | `[150,150,150]` | Color for indeterminate state | Appearance › Progress Bar › State Colors |

---

## Badge

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `badge_enabled` | `bool` | `false` | Show the badge overlay | Appearance › Badge › General |
| `badge_format` | `string` | `"\\(session.username)@\\(session.hostname)"` | Badge text with `\(variable)` substitution | Appearance › Badge › General |
| `badge_color` | `[u8;3]` | `[255,0,0]` | Badge text color | Appearance › Badge › Appearance |
| `badge_color_alpha` | `f32` | `0.5` | Badge opacity (0.0–1.0) | Appearance › Badge › Appearance |
| `badge_font` | `string` | `"Helvetica"` | Badge font family | Appearance › Badge › Appearance |
| `badge_font_bold` | `bool` | `true` | Use bold badge font | Appearance › Badge › Appearance |
| `badge_top_margin` | `f32` | `0.0` | Top margin in pixels from terminal edge | Appearance › Badge › Position & Size |
| `badge_right_margin` | `f32` | `16.0` | Right margin in pixels from terminal edge | Appearance › Badge › Position & Size |
| `badge_max_width` | `f32` | `0.5` | Max badge width as fraction of terminal width (0.0–1.0) | Appearance › Badge › Position & Size |
| `badge_max_height` | `f32` | `0.2` | Max badge height as fraction of terminal height | Appearance › Badge › Position & Size |

---

## Automation & Scripting

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `triggers` | `array` | `[]` | Regex trigger definitions. Each entry: `{name, pattern, enabled, prompt_before_run, i_accept_the_risk, allowed_commands, actions}`. `prompt_before_run` (alias: `require_user_action`) defaults to `true`. When `prompt_before_run: false`, `i_accept_the_risk: true` is required; execution is blocked without it. `allowed_commands` is an optional command allowlist (binary-name substring match; when set, only listed commands may run via `run_command` actions, defaulting to deny-all). Actions include `highlight`, `notify`, `mark_line`, `set_variable`, `run_command`, `play_sound`, `send_text`, `split_pane` (accepts `split_percent` 10–90, default `66`). | Automation › Triggers › Triggers |
| `coprocesses` | `array` | `[]` | Coprocess definitions. Each entry: `{name, command, args, auto_start, copy_terminal_output, restart_policy, restart_delay_ms}` | Automation › Coprocesses › Coprocesses |
| `scripts` | `array` | `[]` | External observer script definitions | Automation › Observer Scripts › Observer Scripts |
| `snippets` | `array` | `[]` | Text snippets: `{id, title, content, keybinding, folder, enabled, auto_execute}` | Automation › Snippets › Snippets |
| `actions` | `array` | `[]` | Custom actions. All types share `{id, title, keybinding, prefix_char, keybinding_enabled, description}`. **Basic types**: `shell_command` (`command`, `args`, `capture_output`, `notify_on_success`, `timeout_secs`), `insert_text` (`text`, `variables`), `key_sequence` (`keys`), `new_tab` (`command`), `split_pane` (`direction`, `command`, `command_is_direct`, `focus_new_pane`, `delay_ms`, `split_percent`). **Workflow types**: `sequence` (`steps: [{action_id, delay_ms, on_failure: abort\|stop\|continue}]`), `condition` (`check: {kind: exit_code\|output_contains\|env_var\|dir_matches\|git_branch, ...}`, `on_true_id`, `on_false_id`), `repeat` (`action_id`, `count`, `delay_ms`, `stop_on_success`, `stop_on_failure`). See [SNIPPETS.md](features/SNIPPETS.md) for full field reference. | Automation › Custom Actions › Custom Actions |
| `custom_action_prefix_key` | `string` | `""` | Global prefix key for tmux-style two-stroke action triggers (e.g. `Ctrl+B` or `CmdOrCtrl+Alt+Z`). When set, actions with a `prefix_char` — or a single-character `keybinding` with no modifiers — can be triggered by pressing this key then that character. A single-character `keybinding` used this way is not also registered as a global shortcut. | Automation › Custom Actions › Custom Actions |



---

## Assistant Panel

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `ai_inspector_enabled` | `bool` | `true` | Enable AI Inspector panel | Assistant & Agents › Panel › Panel |
| `ai_inspector_open_on_startup` | `bool` | `false` | Open inspector automatically on startup | Assistant & Agents › Panel › Panel |
| `ai_inspector_width` | `f32` | `300.0` | Inspector panel width in pixels | Assistant & Agents › Panel › Panel |
| `ai_inspector_default_scope` | `string` | `"visible"` | Default capture scope: `visible`, `full`, or `recent_<n>` (the Settings UI offers `recent_5`, `recent_10`, `recent_25`, `recent_50`). Any unrecognized value silently falls back to `visible`. | Assistant & Agents › Panel › Panel |
| `ai_inspector_view_mode` | `string` | `"tree"` | View mode for inspector results: `cards`, `timeline`, `tree`, `list_detail`. Any unrecognized value silently falls back to `cards` — note this differs from the `tree` default applied when the field is absent. | Assistant & Agents › Panel › Panel |
| `ai_inspector_live_update` | `bool` | `false` | Automatically refresh inspector when terminal content changes | Assistant & Agents › Panel › Panel |
| `ai_inspector_show_zones` | `bool` | `true` | Show semantic zone overlays on terminal content | Assistant & Agents › Panel › Panel |
| `ai_inspector_agent` | `string` | `"claude.com"` | AI agent identifier for queries | Assistant & Agents › Agents › Agent |
| `ai_inspector_auto_launch` | `bool` | `false` | Auto-launch agent when inspector opens | Assistant & Agents › Agents › Agent |
| `ai_inspector_auto_context` | `bool` | `false` | Include terminal context with AI queries | Assistant & Agents › Agents › Agent |
| `ai_inspector_context_max_lines` | `usize` | `200` | Max terminal lines included as context | Assistant & Agents › Agents › Agent |
| `ai_inspector_auto_approve` | `bool` | `false` | Auto-approve AI-suggested actions | Assistant & Agents › Permissions › Permissions |
| `ai_inspector_agent_terminal_access` | `bool` | `false` | Allow AI agent to write input to terminal | Assistant & Agents › Permissions › Permissions |
| `ai_inspector_agent_screenshot_access` | `bool` | `true` | Allow AI agent to request screenshots | Assistant & Agents › Permissions › Permissions |
| `ai_inspector_chat_font_size` | `f32` | `14.0` | Font size for chat messages in points | Assistant & Agents › Panel › Panel |
| `ai_inspector_input_history_mode` | `string` | `"session"` | Assistant chat input history mode: `session` keeps prompts for the current panel/window session only; `persist` stores them in `assistant_input_history.yaml` under the par-term config directory | Assistant & Agents › Panel › Panel |
| `ai_inspector_extra_agent_roots` | `array` | `[]` | Additional filesystem roots made available to supported ACP agents; the par-term shaders directory is always included automatically | Assistant & Agents › Agents › Agent |
| `ai_inspector_custom_agents` | `array` | `[]` | Additional ACP agent definitions (overrides discovered agents with same identity) | Assistant & Agents › Agents › Custom Agents |

---

## Update Checking

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `update_check_frequency` | `enum` | `daily` | How often to check for updates: `never`, `hourly`, `daily`, `weekly`, `monthly` | General › Updates › Updates |

---

## Security

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `allow_all_env_vars` | `bool` | `false` | Allow all environment variables in `${VAR}` substitution (not just the safe allowlist) | Advanced › Security › Security |
| `allow_http_profiles` | `bool` | `false` | Allow plain `http://` URLs in `dynamic_profile_sources`. `https://` is always permitted; this flag only unlocks the insecure variant, and even then a source sending auth headers is refused. Every other scheme (`file:`, `ftp:`, `data:`, …) is rejected regardless of this setting | Advanced › Security › Security |
| `allow_file_scheme_urls` | `bool` | `false` | Allow opening `file://` OSC 8 hyperlinks via the OS handler (SEC-009). A remote program can emit `file://` links to open arbitrary local paths; enable only if you trust your sessions | General › Search & Links › Semantic History |
| `max_osc_data_length` | `usize` | `134217728` (128 MiB) | Maximum total OSC (escape sequence) payload size in bytes before a sequence is rejected as a memory-exhaustion guard (QA-012). Must be large enough for inline images (iTerm2/Kitty base64) if used | Advanced › Security › Security |

---

## Settings UI

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `collapsed_settings_sections` | `[string]` | `[]` | Ids of Settings sections whose open or closed state differs from the section's default (a section that starts open is listed when you close it, and the reverse). par-term writes it when you Save or close Settings, so it is not edited in Settings and never needs hand editing | Internal |

---

## Restore & Arrangements

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `restore_session` | `bool` | `false` | Restore windows (tabs, panes, CWDs) on launch | General › Startup & Restore › Startup |
| `auto_restore_arrangement` | `string?` | `null` | Name of arrangement to auto-restore on startup | General › Startup & Restore › Auto-Restore on Startup |
| `session_undo_timeout_secs` | `u32` | `5` | Seconds to keep closed tab metadata for undo (0=disabled) | General › Closing & Quitting › Closing & Quitting |
| `session_undo_max_entries` | `usize` | `10` | Maximum closed tabs remembered for undo | General › Closing & Quitting › Closing & Quitting |
| `session_undo_preserve_shell` | `bool` | `true` | Preserve shell process on tab close for undo | General › Closing & Quitting › Closing & Quitting |

---

## Profiles

Profiles are stored in a separate `~/.config/par-term/profiles.yaml` file.
Each profile can override shell, working directory, badge, SSH host, and more.
See [PROFILES.md](features/PROFILES.md) for full documentation.

**Per-profile tmux auto-connect fields** (in `profiles.yaml`):

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `tmux_session_name` | `string?` | `null` | tmux session to auto-connect when this profile opens. Uses create-or-attach semantics (`tmux new-session -A -s <name>`). Requires `tmux_enabled: true`. | Profiles › Profiles (profile editor, Session sub-tab) |
| `tmux_connection_mode` | `enum` | `control_mode` | How to connect: `control_mode` (full par-term integration via `tmux -CC`) or `normal` (plain tmux UI in the PTY) | Profiles › Profiles (profile editor, Session sub-tab) |

Dynamic profiles can be fetched from remote URLs:

```yaml
dynamic_profile_sources:
  - url: "https://example.com/profiles.yaml"
    conflict_resolution: local_wins  # or remote_wins
```

---

## Command Separator Lines

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `command_separator_enabled` | `bool` | `false` | Show horizontal separator lines between commands | Appearance › Theme › Command Separators |
| `command_separator_thickness` | `f32` | `1.0` | Separator line thickness in pixels | Appearance › Theme › Command Separators |
| `command_separator_opacity` | `f32` | `0.4` | Separator line opacity (0.0–1.0) | Appearance › Theme › Command Separators |
| `command_separator_exit_color` | `bool` | `true` | Color separators by exit code (green=success, red=failure) | Appearance › Theme › Command Separators |
| `command_separator_color` | `[u8;3]` | `[128,128,128]` | Custom separator color when `exit_color` is disabled | Appearance › Theme › Command Separators |

---

## Debug Logging

| Field | Type | Default | Description | Settings |
|-------|------|---------|-------------|--------|
| `log_level` | `enum` | `warn` | Debug log verbosity: `off`, `error`, `warn`, `info`, `debug`, `trace`. Overrides the RUST_LOG environment variable; overridden by the --log-level CLI flag | Advanced › Logging › Debug Logging |

---

## Related Documentation

- [Settings Reference](features/SETTINGS.md) — Every Settings control by tab, page, and section, with the key it edits
- [Custom Shaders](features/CUSTOM_SHADERS.md) — Background and cursor shader creation, uniforms, and debugging
- [Snippets & Actions](features/SNIPPETS.md) — Full field reference for snippets, actions, and keybindings
- [Profiles](features/PROFILES.md) — Per-profile configuration and dynamic profile sources
- [SSH Support](features/SSH.md) — SSH host discovery and profile switching
- [Restoring Windows and Reopening Tabs](features/SESSION_MANAGEMENT.md) — Restore windows on launch and reopen closed tabs
- [Automation](features/AUTOMATION.md) — Triggers, coprocesses, and observer scripts
- [Assistant Panel](ASSISTANT_PANEL.md) — Assistant panel and ACP agent configuration
- [Logging](LOGGING.md) — Debug logging categories and DEBUG_LEVEL values
