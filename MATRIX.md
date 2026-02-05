# iTerm2 vs par-term Feature Comparison Matrix

This document compares features between iTerm2 and par-term, including assessment of usefulness and implementation effort for features par-term doesn't yet have.

**Legend:**
- **Status**: ✅ = Implemented | 🔶 = Partial | ❌ = Not Implemented
- **Useful**: ⭐⭐⭐ = Essential | ⭐⭐ = Nice to have | ⭐ = Low priority | ➖ = Not applicable
- **Effort**: 🟢 = Low (1-2 days) | 🟡 = Medium (3-7 days) | 🔴 = High (1-2 weeks) | 🔵 = Very High (2+ weeks)

---

## 1. Terminal Dimensions & Window

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Configurable columns | ✅ `Columns` | ✅ `cols` | ✅ | - | - | - |
| Configurable rows | ✅ `Rows` | ✅ `rows` | ✅ | - | - | - |
| Window title | ✅ `Custom Window Title` | ✅ `window_title` | ✅ | - | - | - |
| Allow title change via OSC | ✅ `Allow Title Setting` | ✅ `allow_title_change` | ✅ | - | - | - |
| Window padding | ✅ `Side Margins`, `Top/Bottom Margins` | ✅ `window_padding` | ✅ | - | - | par-term uses single value for all sides |
| Window opacity/transparency | ✅ `Transparency` | ✅ `window_opacity` | ✅ | - | - | - |
| Blur effect | ✅ `Blur`, `Blur Radius` | ✅ `blur_enabled`, `blur_radius` | ✅ | - | - | macOS only |
| Always on top | ✅ | ✅ `window_always_on_top` | ✅ | - | - | - |
| Window decorations toggle | ❌ | ✅ `window_decorations` | ✅ | - | - | par-term exclusive |
| Fullscreen mode | ✅ Lion Fullscreen, Traditional | ✅ F11 toggle | ✅ | - | - | - |
| Window type (normal/fullscreen/edge) | ✅ Multiple types | ✅ `window_type` | ✅ | - | - | Normal/Fullscreen/Edge-anchored windows |
| Open on specific screen | ✅ `Screen` | ✅ `target_monitor` | ✅ | - | - | Multi-monitor support |
| Open in specific Space | ✅ `Space` | ❌ | ❌ | ⭐ | 🟢 | macOS Spaces integration |
| Maximize vertically only | ✅ | ✅ Shift+F11 | ✅ | - | - | Menu and keybinding |
| Lock window size | ✅ `Lock Window Size Automatically` | ✅ `lock_window_size` | ✅ | - | - | Prevent resize via config/settings |
| Proxy icon in title bar | ✅ `Enable Proxy Icon` | ❌ | ❌ | ⭐ | 🟡 | macOS feature for current directory |
| Window number display | ✅ `Show Window Number` | ✅ `show_window_number` | ✅ | - | - | Window index in title bar |
| Transparency only for default BG | ✅ | ✅ `transparency_affects_only_default_background` | ✅ | - | - | - |
| Keep text opaque | ❌ | ✅ `keep_text_opaque` | ✅ | - | - | par-term exclusive |

---

## 2. Typography & Fonts

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Primary font family | ✅ `Normal Font` | ✅ `font_family` | ✅ | - | - | - |
| Font size | ✅ | ✅ `font_size` | ✅ | - | - | - |
| Bold font variant | ✅ `Use Bold Font` | ✅ `font_family_bold` | ✅ | - | - | - |
| Italic font variant | ✅ `Use Italic Font` | ✅ `font_family_italic` | ✅ | - | - | - |
| Bold-italic font variant | ✅ | ✅ `font_family_bold_italic` | ✅ | - | - | - |
| Non-ASCII font (fallback) | ✅ `Non-ASCII Font` | 🔶 | 🔶 | - | - | par-term has font_ranges for Unicode ranges |
| Unicode range-specific fonts | ❌ | ✅ `font_ranges` | ✅ | - | - | par-term exclusive, more flexible |
| Horizontal spacing | ✅ `Horizontal Spacing` | ✅ `char_spacing` | ✅ | - | - | - |
| Vertical/line spacing | ✅ `Vertical Spacing` | ✅ `line_spacing` | ✅ | - | - | - |
| Text shaping (HarfBuzz) | ✅ | ✅ `enable_text_shaping` | ✅ | - | - | - |
| Ligatures | ✅ `ASCII Ligatures`, `Non-ASCII Ligatures` | ✅ `enable_ligatures` | ✅ | - | - | - |
| Kerning | ✅ | ✅ `enable_kerning` | ✅ | - | - | - |
| Anti-aliasing control | ✅ `ASCII/Non-ASCII Anti Aliased` | ✅ `font_antialias`, `font_hinting` | ✅ | - | - | Toggle anti-aliasing and hinting |
| Thin strokes | ✅ Multiple modes | ✅ `font_thin_strokes` | ✅ | - | - | 5 modes: never/retina_only/dark_backgrounds_only/retina_dark_backgrounds_only/always |
| Powerline glyphs | ✅ `Draw Powerline Glyphs` | ✅ | ✅ | - | - | Built into font rendering |
| Use bold color | ✅ `Use Bold Color` | ✅ | ✅ | - | - | Theme-controlled |
| Brighten bold text | ✅ `Use Bright Bold` | ✅ | ✅ | - | - | Theme-controlled |

---

## 3. Cursor

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Cursor style (block/beam/underline) | ✅ `Cursor Type` | ✅ `cursor_style` | ✅ | - | - | - |
| Cursor color | ✅ `Cursor Color` | ✅ `cursor_color` | ✅ | - | - | - |
| Cursor text color | ✅ `Cursor Text Color` | ✅ `cursor_text_color` | ✅ | - | - | Text color under block cursor |
| Cursor blinking | ✅ `Blinking Cursor` | ✅ `cursor_blink` | ✅ | - | - | - |
| Blink interval | ✅ | ✅ `cursor_blink_interval` | ✅ | - | - | - |
| Allow app to change cursor blink | ✅ `Allow Change Cursor Blink` | ✅ `lock_cursor_blink` | ✅ | - | - | Inverted logic |
| Lock cursor visibility | ❌ | ✅ `lock_cursor_visibility` | ✅ | - | - | par-term exclusive |
| Lock cursor style | ❌ | ✅ `lock_cursor_style` | ✅ | - | - | par-term exclusive |
| Cursor guide (horizontal line) | ✅ `Use Cursor Guide` | ✅ `cursor_guide_enabled` | ✅ | - | - | With customizable RGBA color |
| Cursor shadow | ✅ `Cursor Shadow` | ✅ `cursor_shadow_*` | ✅ | - | - | Color, offset, blur configurable |
| Cursor boost | ✅ `Cursor Boost` | ✅ `cursor_boost` | ✅ | - | - | Intensity and color control |
| Hide cursor when unfocused | ✅ `Cursor Hidden When Unfocused` | ✅ `unfocused_cursor_style` | ✅ | - | - | Hidden/Hollow/Same options |
| Hollow block cursor | ✅ | ✅ `unfocused_cursor_style` | ✅ | - | - | Via Hollow option |
| **Cursor shader effects** | ❌ | ✅ `cursor_shader*` | ✅ | - | - | **par-term exclusive** - GPU cursor effects |

---

## 4. Background & Visual Effects

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Solid background color | ✅ `Background Color` | ✅ `background_color` | ✅ | - | - | - |
| Background image | ✅ `Background Image Location` | ✅ `background_image` | ✅ | - | - | - |
| Background image modes | ✅ Stretch/Tile/Scale Aspect | ✅ fit/fill/stretch/tile/center | ✅ | - | - | - |
| Background image opacity | ✅ `Blend` | ✅ `background_image_opacity` | ✅ | - | - | - |
| Per-pane background image | ✅ | ❌ | ❌ | ⭐ | 🟡 | Per-pane/tab backgrounds |
| **Custom GLSL shaders** | ❌ | ✅ `custom_shader*` | ✅ | - | - | **par-term exclusive** - 49+ shaders |
| **Shader hot reload** | ❌ | ✅ `shader_hot_reload` | ✅ | - | - | **par-term exclusive** |
| **Per-shader configuration** | ❌ | ✅ `shader_configs` | ✅ | - | - | **par-term exclusive** |
| **Shader texture channels** | ❌ | ✅ `custom_shader_channel0-3` | ✅ | - | - | **par-term exclusive** - Shadertoy compatible |
| **Shader cubemap support** | ❌ | ✅ `custom_shader_cubemap` | ✅ | - | - | **par-term exclusive** |

---

## 5. Colors & Themes

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Foreground color | ✅ | ✅ | ✅ | - | - | Theme-controlled |
| Background color | ✅ | ✅ | ✅ | - | - | Theme-controlled |
| ANSI colors (0-15) | ✅ | ✅ | ✅ | - | - | Theme-controlled |
| Bold color | ✅ | ✅ `bold_brightening`, `bold_color` | ✅ | - | - | Core supports both bright variant and custom color |
| Selection color | ✅ | ✅ | ✅ | - | - | Theme-controlled |
| Cursor color | ✅ | ✅ | ✅ | - | - | - |
| Link color | ✅ `Link Color` | ✅ `link_color` | ✅ | - | - | Core tracks and styles OSC 8 hyperlinks |
| Theme presets | ✅ Many built-in | ✅ 17 themes | ✅ | - | - | Dracula, Nord, Monokai, Solarized, etc. |
| Light/Dark mode variants | ✅ Separate colors per mode | ❌ | ❌ | ⭐⭐ | 🟡 | Auto-switch with system theme |
| Minimum contrast | ✅ `Minimum Contrast` | ❌ | ❌ | ⭐⭐ | 🟡 | Accessibility feature |
| Smart cursor color | ✅ `Smart Cursor Color` | ✅ `smart_cursor_color` | ✅ | - | - | Core exposes setting, frontend implements |
| Faint text alpha | ✅ `Faint Text Alpha` | ✅ `faint_text_alpha` | ✅ | - | - | Core exposes 0.0-1.0 alpha multiplier |
| Underline color | ✅ `Underline Color` | ✅ SGR 58/59 | ✅ | - | - | Full colored underline support in core |
| Badge color | ✅ `Badge Color` | ✅ `badge_color`, `badge_color_alpha` | ✅ | - | - | RGBA color via config and Settings UI |
| Tab color per profile | ✅ `Tab Color` | ✅ per-tab colors | ✅ | - | - | - |
| Selection foreground color | ✅ | ✅ `selection_fg` | ✅ | - | - | Separate fg and bg colors |
| **Scrollbar colors** | ❌ | ✅ thumb/track colors | ✅ | - | - | **par-term exclusive** |
| **Cursor guide color** | ❌ | ✅ `cursor_guide_color` | ✅ | - | - | **par-term exclusive** - RGBA |
| **Cursor shadow color** | ❌ | ✅ `cursor_shadow_color` | ✅ | - | - | **par-term exclusive** - RGBA |
| **Cursor boost/glow color** | ❌ | ✅ `cursor_boost_color` | ✅ | - | - | **par-term exclusive** |
| **Tab bar colors (13+ options)** | 🔶 Limited | ✅ Full customization | ✅ | - | - | **par-term exclusive** - bg/text/indicators/borders |

---

## 6. Tab Bar

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Tab bar visibility modes | ✅ Show/Hide | ✅ always/when_multiple/never | ✅ | - | - | - |
| Tab bar position | ✅ Top/Bottom/Left | ❌ Top only | 🔶 | ⭐⭐ | 🟡 | Left tabs are useful |
| Tab bar height | ✅ | ✅ `tab_bar_height` | ✅ | - | - | - |
| Tab close button | ✅ `Tabs Have Close Button` | ✅ `tab_show_close_button` | ✅ | - | - | - |
| Smart close (Cmd+W) | ✅ | ✅ `Cmd/Ctrl+W` | ✅ | - | - | Closes tab if multiple, window if single |
| Tab index numbers | ✅ `Hide Tab Number` | ✅ Hotkey indicators (⌘1-9) | ✅ | - | - | Shows shortcut on tab right side |
| New output indicator | ✅ `Show New Output Indicator` | ✅ Activity indicator | ✅ | - | - | - |
| Bell indicator | ✅ | ✅ `tab_bell_indicator` | ✅ | - | - | - |
| Activity indicator | ✅ `Hide Tab Activity Indicator` | ✅ `tab_activity_indicator` | ✅ | - | - | - |
| Tab colors (active/inactive/hover) | ✅ | ✅ Full color customization | ✅ | - | - | - |
| Dim inactive tabs | ✅ | ✅ `dim_inactive_tabs`, `inactive_tab_opacity` | ✅ | - | - | - |
| Tab min width | ❌ | ✅ `tab_min_width` | ✅ | - | - | par-term exclusive |
| Stretch tabs to fill | ✅ `Stretch Tabs to Fill Bar` | ✅ `tab_stretch_to_fill` (default on) | ✅ | ⭐ | 🟢 | Equal-width distribution with `tab_min_width` floor |
| New tabs at end | ✅ `New Tabs Open at End` | ✅ | ✅ | - | - | Default behavior |
| Inherit working directory | ✅ | ✅ `tab_inherit_cwd` | ✅ | - | - | - |
| Max tabs limit | ❌ | ✅ `max_tabs` | ✅ | - | - | par-term exclusive |
| Tab style (visual theme) | ✅ Light/Dark/Minimal/Compact | ❌ | ❌ | ⭐ | 🟡 | Different visual styles |
| HTML tab titles | ✅ `HTML Tab Titles` | ✅ `tab_html_titles` | ✅ | ⭐ | 🟡 | Limited tags: <b>, <i>, <u>, <span style=\"color\"> |

---

## 7. Scrollback & Scrollbar

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Scrollback buffer size | ✅ | ✅ `scrollback_lines` | ✅ | - | - | - |
| Scrollbar visibility | ✅ `Hide Scrollbar` | ✅ | ✅ | - | - | - |
| Scrollbar position | ❌ | ✅ `scrollbar_position` (left/right) | ✅ | - | - | par-term exclusive |
| Scrollbar width | ❌ | ✅ `scrollbar_width` | ✅ | - | - | par-term exclusive |
| Scrollbar colors | ❌ | ✅ thumb/track colors | ✅ | - | - | par-term exclusive |
| Scrollbar auto-hide | ❌ | ✅ `scrollbar_autohide_delay` | ✅ | - | - | par-term exclusive |
| Scrollback in alt screen | ✅ `Scrollback in Alternate Screen` | ✅ | ✅ | - | - | - |
| Instant Replay | ✅ `Instant Replay Memory` | ❌ | ❌ | ⭐⭐ | 🔵 | Rewind terminal state |
| Timestamps | ✅ `Show Timestamps` | 🔶 via tooltips | 🔶 | - | - | Hover scrollbar marks for timing info |
| Mark indicators | ✅ `Show Mark Indicators` | ✅ `scrollbar_command_marks` | ✅ | - | - | Color-coded marks on scrollbar (green=success, red=fail) |
| Mark tooltips | ❌ | ✅ `scrollbar_mark_tooltips` | ✅ | - | - | **par-term exclusive** - command, time, duration, exit code |
| Mark navigation | ✅ | ✅ Cmd+Up/Down | ✅ | - | - | Jump between command marks |

---

## 8. Selection & Clipboard

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Auto-copy selection | ✅ `Selection Copies Text` | ✅ `auto_copy_selection` | ✅ | - | - | - |
| Copy trailing newline | ✅ `Copy Last Newline` | ✅ `copy_trailing_newline` | ✅ | - | - | - |
| Middle-click paste | ✅ | ✅ `middle_click_paste` | ✅ | - | - | - |
| Clipboard history | ✅ | ✅ Cmd/Ctrl+Shift+H | ✅ | - | - | - |
| Block/rectangular selection | ✅ | ✅ | ✅ | - | - | Option+Cmd (matches iTerm2) |
| Word selection | ✅ | ✅ | ✅ | - | - | - |
| Line selection | ✅ | ✅ | ✅ | - | - | - |
| Triple-click selects wrapped lines | ✅ `Triple Click Selects Full Wrapped Lines` | ✅ | ✅ | - | - | - |
| Smart selection rules | ✅ Custom regex patterns | ✅ `smart_selection_rules` | ✅ | - | - | 11 default patterns with precision levels, Settings UI with enable/disable per rule |
| Word boundary characters | ✅ `Characters Considered Part of Word` | ✅ `word_characters` | ✅ | - | - | Default: `/-+\~_.` (iTerm2 compatible), Settings UI |
| Paste bracketing | ✅ `Allow Paste Bracketing` | ✅ | ✅ | - | - | - |
| Paste special options | ✅ Many transformations | ✅ `Cmd/Ctrl+Shift+V` | ✅ | - | - | 26 transforms: shell escape, case, whitespace, encoding |
| Allow terminal clipboard access | ✅ `Allow Clipboard Access From Terminal` | ✅ OSC 52 | ✅ | - | - | - |
| Wrap filenames in quotes | ✅ | ✅ `dropped_file_quote_style` | ✅ | - | - | Auto-quote dropped files with configurable style |

---

## 9. Mouse & Pointer

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Mouse scroll speed | ✅ | ✅ `mouse_scroll_speed` | ✅ | - | - | - |
| Double-click threshold | ✅ | ✅ `mouse_double_click_threshold` | ✅ | - | - | - |
| Triple-click threshold | ✅ | ✅ `mouse_triple_click_threshold` | ✅ | - | - | - |
| Mouse reporting | ✅ `Mouse Reporting` | ✅ | ✅ | - | - | ANSI mouse sequences |
| Cmd+click opens URLs | ✅ `Cmd Click Opens URLs` | ✅ Cmd/Ctrl+click | ✅ | - | - | Cmd on macOS, Ctrl elsewhere |
| Option+click moves cursor | ✅ `Option Click Moves Cursor` | ✅ `option_click_moves_cursor` | ✅ | - | - | Uses arrow keys for shell compatibility |
| Focus follows mouse | ✅ `Focus Follows Mouse` | ✅ `focus_follows_mouse` | ✅ | - | - | Auto-focus on hover (opt-in) |
| Three-finger middle click | ✅ `Three Finger Emulates Middle` | ❌ | ❌ | ⭐ | 🟡 | Requires platform gesture APIs |
| Right-click context menu | ✅ | ✅ | ✅ | - | - | - |
| Horizontal scroll reporting | ✅ `Report Horizontal Scroll Events` | ✅ `report_horizontal_scroll` | ✅ | - | - | Button codes 66/67 |

---

## 10. Keyboard & Input

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Custom keybindings | ✅ Full keyboard map | ✅ `keybindings` | ✅ | - | - | - |
| Modifier remapping | ✅ Per-modifier remapping | ✅ `modifier_remapping` | ✅ | - | - | Remap Ctrl/Alt/Super per-side |
| Option as Meta/Esc | ✅ `Option Key Sends` | ✅ `left/right_option_key_mode` | ✅ | - | - | Normal/Meta/Esc modes per key |
| Hotkey window | ✅ Global hotkey | ❌ | ❌ | ⭐⭐⭐ | 🔴 | Quake-style dropdown |
| Haptic/sound feedback for Esc | ✅ | ❌ | ❌ | ➖ | ➖ | Touch Bar feedback - won't implement (Touch Bar discontinued) |
| Language-agnostic key bindings | ✅ | ✅ `use_physical_keys` | ✅ | - | - | Match by scan code, works across layouts |
| Application keypad mode | ✅ `Application Keypad Allowed` | ✅ | ✅ | - | - | - |
| Touch Bar customization | ✅ `Touch Bar Map` | ❌ | ❌ | ➖ | ➖ | macOS Touch Bar - won't implement (Touch Bar discontinued) |
| modifyOtherKeys protocol | ✅ `Allow Modify Other Keys` | ✅ `CSI > 4 ; mode m` | ✅ | - | - | Extended key reporting (modes 0, 1, 2) |

---

## 11. Shell & Session

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Custom shell command | ✅ `Command` | ✅ `custom_shell` | ✅ | - | - | - |
| Shell arguments | ✅ | ✅ `shell_args` | ✅ | - | - | - |
| Working directory | ✅ `Working Directory` | ✅ `working_directory` | ✅ | - | - | - |
| **Startup directory mode** | ✅ Home/Recycle/Custom | ✅ `startup_directory_mode` | ✅ | - | - | Home/Previous/Custom with graceful fallback |
| Login shell | ✅ | ✅ `login_shell` | ✅ | - | - | - |
| Environment variables | ✅ | ✅ `shell_env` | ✅ | - | - | - |
| Exit behavior | ✅ Close/Restart | ✅ `shell_exit_action` | ✅ | - | - | Close/Keep/Restart immediately/Restart with prompt/Restart after delay |
| Initial text to send | ✅ `Initial Text` | ✅ `initial_text` | ✅ | ⭐⭐ | 🟢 | Send text on start with delay/newline + escapes |
| Anti-idle (keep-alive) | ✅ `Send Code When Idle` | ✅ `anti_idle_enabled` | ✅ | ⭐⭐ | 🟢 | Prevent SSH timeouts |
| Jobs to ignore | ✅ | ✅ `confirm_close_running_jobs`, `jobs_to_ignore` | ✅ | - | - | Confirmation dialog when closing tabs/panes with running jobs; configurable ignore list |
| Session close undo timeout | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Recover closed tabs |
| TERM variable | ✅ `Terminal Type` | ✅ | ✅ | - | - | Set via environment |
| Character encoding | ✅ Multiple | ✅ UTF-8 | ✅ | - | - | UTF-8 only |
| Unicode version | ✅ | ✅ | ✅ | ⭐ | 🟢 | Unicode 9.0-16.0 or Auto; ambiguous width narrow/wide; Settings > Terminal |
| Unicode normalization | ✅ NFC/NFD/HFS+ | ❌ | ❌ | ⭐ | 🟡 | Text normalization |
| Answerback string | ✅ | ✅ | ✅ | ⭐ | 🟢 | ENQ response; default empty for security; configurable in Settings > Shell (core v0.23.0+) |

---

## 12. Notifications & Bell

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Visual bell | ✅ `Visual Bell` | ✅ `notification_bell_visual` | ✅ | - | - | - |
| Audio bell | ✅ | ✅ `notification_bell_sound` | ✅ | - | - | - |
| Desktop notification for bell | ✅ `Send Bell Alert` | ✅ `notification_bell_desktop` | ✅ | - | - | - |
| Silence bell | ✅ `Silence Bell` | ✅ volume=0 | ✅ | - | - | - |
| Activity notification | ✅ `Send New Output Alert` | ✅ `notification_activity_enabled` | ✅ | - | - | Notify when output resumes after inactivity |
| Idle notification | ✅ `Send Idle Alert` | ✅ `notification_silence_enabled` | ✅ | - | - | Notify after prolonged silence |
| Session ended notification | ✅ `Send Session Ended Alert` | ✅ `notification_session_ended` | ✅ | - | - | Notify when process exits |
| Suppress alerts when focused | ✅ `Suppress Alerts in Active Session` | ✅ `suppress_notifications_when_focused` | ✅ | - | - | Smart notification filtering |
| Flashing bell | ✅ `Flashing Bell` | ✅ Visual bell | ✅ | - | - | - |
| OSC 9/777 notifications | ✅ | ✅ `notification_max_buffer` | ✅ | - | - | - |

---

## 13. Logging & Recording

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Automatic session logging | ✅ `Automatically Log` | ✅ `auto_log_sessions` | ✅ | - | - | Record all terminal output |
| Log format (plain/HTML/asciicast) | ✅ Multiple formats | ✅ `session_log_format` | ✅ | - | - | Plain, HTML, asciicast formats |
| Log directory | ✅ `Log Directory` | ✅ `session_log_directory` | ✅ | - | - | XDG-compliant default |
| Archive on closure | ✅ `Archive on Closure` | ✅ `archive_on_close` | ✅ | - | - | Save session when tab closes |
| Screenshot | ✅ | ✅ Ctrl+Shift+S | ✅ | - | - | - |
| Screenshot format | ✅ | ✅ `screenshot_format` | ✅ | - | - | png/jpeg/svg/html |

---

## 14. Profiles

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Multiple profiles | ✅ Full profile system | ✅ `ProfileManager` | ✅ | - | - | Named configurations with YAML persistence |
| Profile selection | ✅ GUI + keyboard | ✅ Drawer + Modal | ✅ | - | - | Collapsible drawer, double-click to open |
| Profile creation/editing | ✅ | ✅ Modal UI | ✅ | - | - | Full CRUD operations |
| Profile reordering | ✅ | ✅ Move up/down | ✅ | - | - | Drag-free reorder buttons |
| Profile icon | ✅ Custom icons | ✅ Emoji icons | ✅ | - | - | Visual identification with emoji |
| Working directory | ✅ | ✅ Per-profile | ✅ | - | - | With directory browser |
| Custom command | ✅ | ✅ Per-profile | ✅ | - | - | Command + arguments |
| Custom tab name | ✅ | ✅ Per-profile | ✅ | - | - | Override default tab naming |
| Dynamic profiles (external files) | ✅ | ✅ `profiles.yaml` | ✅ | - | - | Loads from `~/.config/par-term/profiles.yaml` |
| Profile tags | ✅ Searchable tags | ✅ `tags` | ✅ | - | - | Filter/search profiles by tags in drawer |
| Profile inheritance | ✅ Parent profiles | ✅ `parent_id` | ✅ | - | - | Child inherits parent settings, can override |
| Profile keyboard shortcut | ✅ | ✅ `keyboard_shortcut` | ✅ | - | - | Quick profile launch via hotkey (e.g., "Cmd+1") |
| Automatic profile switching | ✅ Based on hostname | ✅ `hostname_patterns` | ✅ | - | - | OSC 7 hostname detection triggers profile match |
| Profile badge | ✅ `Badge Text` | ✅ `badge_text` | ✅ | - | - | Per-profile badge format override + session.profile_name |

---

## 15. Split Panes

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Horizontal split | ✅ | ✅ `Cmd+D` | ✅ | - | - | Split terminal vertically |
| Vertical split | ✅ | ✅ `Cmd+Shift+D` | ✅ | - | - | Split terminal horizontally |
| Pane navigation | ✅ | ✅ `Cmd+Opt+Arrow` | ✅ | - | - | Move between panes |
| Pane resizing | ✅ | ✅ keyboard + mouse drag | ✅ | - | - | Resize pane boundaries |
| Dim inactive panes | ✅ `Dim Inactive Split Panes` | ✅ `dim_inactive_panes` | ✅ | - | - | Visual focus indicator |
| Per-pane titles | ✅ `Show Pane Titles` | ✅ | ✅ | - | - | Pane identification via OSC/CWD |
| Per-pane background | ✅ | 🔶 Data model ready | 🔶 | ⭐ | 🟡 | Renderer support pending |
| Broadcast input | ✅ | ✅ `Cmd+Opt+I` | ✅ | - | - | Type to multiple panes |
| Division view | ✅ `Enable Division View` | ✅ configurable dividers | ✅ | - | - | Pane divider lines with colors |

---

## 16. Inline Graphics

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Sixel graphics | ✅ | ✅ | ✅ | - | - | - |
| iTerm2 inline images | ✅ | ✅ | ✅ | - | - | - |
| Kitty graphics protocol | ✅ | ✅ | ✅ | - | - | - |
| Kitty animations | ✅ | ✅ | ✅ | - | - | - |
| GPU-accelerated rendering | ❌ | ✅ | ✅ | - | - | par-term uses wgpu |

---

## 17. Hyperlinks & URLs

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| OSC 8 hyperlinks | ✅ | ✅ | ✅ | - | - | - |
| Regex URL detection | ✅ | ✅ | ✅ | - | - | - |
| Click to open URLs | ✅ Cmd+click | ✅ Ctrl+click | ✅ | - | - | Different modifier |
| Hover highlighting | ✅ | ✅ | ✅ | - | - | - |
| Semantic history | ✅ Open in editor | ❌ | ❌ | ⭐⭐ | 🟡 | Click to open file in editor |

---

## 18. Triggers & Automation

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Regex triggers | ✅ Full trigger system | ❌ | ❌ | ⭐⭐ | 🔴 | Auto-respond to patterns |
| Trigger actions | ✅ Many actions | ❌ | ❌ | ⭐⭐ | 🔴 | Highlight, alert, run, etc. |
| Coprocesses | ✅ | ❌ | ❌ | ⭐ | 🔴 | Pipe output to process |
| Shell integration | ✅ Full integration | ✅ OSC 133/7/1337 | ✅ | - | - | Command tracking, marks, CWD, badges |
| Python API | ✅ Full scripting API | ❌ | ❌ | ⭐⭐ | 🔵 | Automation scripting |

---

## 19. tmux Integration

**Note:** par-term now has **native tmux integration** via control mode (`tmux -CC`), similar to iTerm2's approach.

### Current tmux Support in par-term

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Run tmux as shell | ✅ | ✅ | ✅ | - | - | Basic compatibility |
| Render tmux status bar | ✅ | ✅ | ✅ | - | - | Handles reverse video (SGR 7) correctly |
| Render tmux panes/windows | ✅ | ✅ | ✅ | - | - | Standard VT sequence rendering |
| tmux mouse support | ✅ | ✅ | ✅ | - | - | Mouse reporting works in tmux |

### Native tmux Integration (Control Mode)

par-term implements iTerm2-style native tmux integration via control mode (`tmux -CC`).

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| **tmux control mode (`-CC`)** | ✅ Full protocol | ✅ | ✅ | - | - | Core protocol for native integration |
| tmux windows as native tabs | ✅ | ✅ | ✅ | - | - | %window-add/%window-close handling |
| tmux panes as native splits | ✅ | ✅ | ✅ | - | - | %layout-change parsing |
| tmux session picker UI | ✅ | ✅ `Cmd+Opt+T` | ✅ | - | - | List/attach sessions from GUI |
| **Bidirectional pane resize** | ✅ | ✅ | ✅ | - | - | Resize in par-term updates tmux and vice versa |
| **Multi-client size sync** | ✅ | ✅ `window-size smallest` | ✅ | - | - | Sets smallest mode on connect for proper sizing |
| tmux status bar in UI | ✅ Native display | ✅ `tmux_show_status_bar` | ✅ | - | - | Display status outside terminal area |
| **Configurable status bar format** | ✅ Custom format | ✅ `tmux_status_bar_left/right` | ✅ | - | - | Format strings with variables: {session}, {windows}, {pane}, {time:FORMAT}, {hostname}, {user} |
| tmux clipboard sync | ✅ Bidirectional | ✅ `set-buffer` | ✅ | - | - | Sync with tmux paste buffers |
| tmux pause mode handling | ✅ | ✅ | ✅ | - | - | Handle slow connection pausing with buffering |
| Auto-attach on launch | ✅ | ✅ `tmux_auto_attach` | ✅ | - | - | Option to auto-attach to session |
| tmux profile auto-switching | ✅ | ✅ | ✅ | - | - | Glob pattern matching on session names (e.g., `work-*`, `*-production`) |

### How par-term's tmux Control Mode Works

1. **Protocol**: par-term connects via `tmux -CC` and parses structured notifications
2. **Window Management**: tmux windows map to par-term tabs via %window-add/%window-close
3. **Pane Management**: tmux panes map to par-term split panes via %layout-change parsing
4. **Bidirectional Resize**: Resizing panes in par-term sends `resize-pane` commands to tmux; layout changes from tmux update par-term
5. **Multi-Client Sizing**: Sets `window-size smallest` on connect so tmux respects par-term's smaller size when other clients are attached
6. **Seamless Experience**: Users interact with native UI while tmux manages sessions server-side
7. **Session Persistence**: Closing par-term doesn't kill tmux; sessions persist and can be reattached
8. **Broadcast Input**: Type to all panes simultaneously with Cmd+Opt+I

### Configuration Options

- `tmux_enabled`: Enable tmux control mode integration
- `tmux_path`: Path to tmux executable
- `tmux_auto_attach`: Automatically attach on startup
- `tmux_auto_attach_session`: Session name for auto-attach
- `tmux_clipboard_sync`: Sync clipboard with tmux paste buffer
- `tmux_show_status_bar`: Display tmux status bar at bottom when connected
- `tmux_status_bar_refresh_ms`: Status bar refresh interval in milliseconds (default: 1000)
- `tmux_status_bar_left`: Format string for left side (default: `[{session}] {windows}`)
- `tmux_status_bar_right`: Format string for right side (default: `{pane} | {time:%H:%M}`)
- `tmux_status_bar_use_native_format`: Use native tmux format strings (queries tmux directly)
- `tmux_profile`: Profile to use when connected (pending)

---

## 20. Performance & Power

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| GPU acceleration (Metal) | ✅ Optional | ✅ wgpu (required) | ✅ | - | - | par-term always GPU |
| Target FPS | ❌ | ✅ `max_fps` | ✅ | - | - | par-term exclusive |
| VSync mode | ❌ | ✅ `vsync_mode` | ✅ | - | - | par-term exclusive |
| Pause shaders when unfocused | ❌ | ✅ `pause_shaders_on_blur` | ✅ | - | - | par-term exclusive |
| Reduce FPS when unfocused | ❌ | ✅ `pause_refresh_on_blur`, `unfocused_fps` | ✅ | - | - | par-term exclusive |
| Maximize throughput | ✅ | ❌ | ❌ | ⭐ | 🟡 | Latency vs throughput |
| Disable GPU when unplugged | ✅ | ❌ | ❌ | ➖ | ➖ | Won't implement - par-term requires GPU |
| Prefer integrated GPU | ✅ | ✅ `power_preference` | ✅ | - | - | None/LowPower/HighPerformance GPU selection |
| Reduce flicker | ✅ `Reduce Flicker` | ❌ | ❌ | ⭐⭐ | 🟡 | Screen update optimization |

---

## 21. Accessibility

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Minimum contrast | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Ensure readable text |
| Focus on click | ✅ | ✅ | ✅ | - | - | - |
| Bidirectional text | ✅ `Bidi` | ❌ | ❌ | ⭐⭐ | 🔴 | RTL language support |
| VoiceOver support | ✅ | ❌ | ❌ | ⭐⭐ | 🔵 | Screen reader support |

---

## 22. AI Integration

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| AI assistant | ✅ Full AI integration | ❌ | ❌ | ⭐⭐ | 🔵 | Command help, completion |
| AI command generation | ✅ | ❌ | ❌ | ⭐⭐ | 🔵 | Natural language to commands |
| AI terminal inspection | ✅ | ❌ | ❌ | ⭐⭐ | 🔵 | AI reads terminal state |
| Multiple AI providers | ✅ OpenAI, Anthropic, etc. | ❌ | ❌ | ⭐⭐ | 🔵 | Provider selection |

---

## 23. Miscellaneous

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Config file location (XDG) | ✅ | ✅ | ✅ | - | - | - |
| Settings UI | ✅ Full GUI | ✅ Full GUI (F12) | ✅ | - | - | - |
| Reload config (F5) | ❌ | ✅ | ✅ | - | - | par-term exclusive |
| Window arrangements | ✅ Save/restore layouts | ❌ | ❌ | ⭐⭐ | 🟡 | Save window positions |
| Bonjour host discovery | ✅ | ❌ | ❌ | ⭐ | 🟡 | Auto-discover SSH hosts |
| Password manager | ✅ | ❌ | ❌ | ⭐ | 🔴 | Secure credential storage |
| Toolbelt sidebar | ✅ | ❌ | ❌ | ⭐ | 🔴 | Notes, jobs, paste history |
| Status bar | ✅ Customizable | ❌ | ❌ | ⭐⭐ | 🟡 | Show system info |
| Browser profile | ✅ | ❌ | ❌ | ⭐ | 🔴 | Web browser integration |
| Progress bar | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Show command progress |
| Snippets | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Saved text snippets |
| Search in terminal | ✅ Cmd+F | ✅ Cmd/Ctrl+F | ✅ | - | - | Regex, case, whole word options |
| CLI command (`par-term`) | ❌ | ✅ Full CLI | ✅ | - | - | par-term exclusive |
| First-run shader install prompt | ❌ | ✅ Auto-detect & install | ✅ | - | - | par-term exclusive |
| Shader gallery | ❌ | ✅ Online gallery | ✅ | - | - | par-term exclusive |
| Automatic update checking | ✅ Built-in updater | ✅ `update_check_frequency` | ✅ | - | - | Notify-only (no auto-install) |

---

## 24. Badges

Badges are semi-transparent text overlays displayed in the terminal corner showing dynamic session information.

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Badge text overlay | ✅ Top-right corner | ✅ `badge_enabled` | ✅ | - | - | Semi-transparent text label via egui overlay |
| Badge color | ✅ `Badge Color` | ✅ `badge_color`, `badge_color_alpha` | ✅ | - | - | Configurable RGB color with separate alpha |
| Badge font | ✅ `Badge Font` | ✅ `badge_font`, `badge_font_bold` | ✅ | - | - | Custom font family and bold toggle |
| Badge position margins | ✅ Top/Right margins | ✅ `badge_top_margin`, `badge_right_margin` | ✅ | - | - | Default 10px each |
| Badge max size | ✅ Width/Height fractions | ✅ `badge_max_width`, `badge_max_height` | ✅ | - | - | Default 50% width, 20% height |
| Dynamic badge variables | ✅ `\(session.*)` syntax | ✅ 12 built-in + custom | ✅ | - | - | hostname, username, path, job, etc. |
| Badge escape sequence | ✅ OSC 1337 SetBadgeFormat | ✅ Base64 decoding | ✅ | - | - | Update badge from shell with security checks |
| Badge per-profile | ✅ Profile setting | ❌ | ❌ | ⭐⭐ | 🟡 | Different badges per profile (pending profiles) |
| Badge configuration UI | ✅ Visual drag-and-drop | ✅ Settings tab | ✅ | - | - | Full settings with sliders and color picker |

### Badge Variables Available

| Variable | Description | par-term |
|----------|-------------|----------|
| `session.hostname` | Remote hostname (SSH) | ✅ |
| `session.username` | Current user | ✅ |
| `session.path` | Current working directory | ✅ |
| `session.job` | Foreground job name | ✅ |
| `session.last_command` | Last executed command | ✅ |
| `session.profile_name` | Current profile name | ✅ |
| `session.tty` | TTY device name | ✅ |
| `session.columns` / `session.rows` | Terminal dimensions | ✅ |
| `session.bell_count` | Number of bells | ✅ |
| `session.selection` | Selected text | ✅ |
| `session.tmux_pane_title` | tmux pane title | ✅ |
| Custom variables | Via escape sequences | ✅ |

---

## Summary Statistics

### par-term Exclusive Features (Not in iTerm2)
- 49 custom GLSL background shaders with hot reload
- 12 cursor shader effects (GPU-powered cursor animations)
- Per-shader configuration system with metadata
- Shadertoy-compatible texture channels and cubemaps
- First-run shader install prompt (auto-detect missing shaders)
- Scrollbar customization (position, colors, width, auto-hide)
- FPS control and VSync modes
- Power saving options (pause shaders/refresh on blur)
- Tab minimum width and maximum tabs limit
- Configuration hot reload (F5)
- CLI with shader installation
- Cursor guide with customizable RGBA color
- Cursor shadow with color, offset, and blur
- Cursor boost/glow with intensity and color
- Unfocused cursor styles (Hidden/Hollow/Same)
- Lock cursor visibility and style
- 17 built-in color themes
- 13+ tab bar color customization options
- Selection foreground color (separate from background)
- Configurable update check frequency (never/daily/weekly/monthly)
- Paste special with 26 transformations (shell escape, case, whitespace, encoding)
- Edge-anchored window types (dropdown-style terminals)
- Target monitor selection for multi-monitor setups
- Native split panes with binary tree layout
- tmux control mode integration with session picker
- Broadcast input mode (type to all panes)
- Badge system with 12 dynamic variables and Settings UI tab
- Per-side modifier remapping (left/right Ctrl, Alt, Super independently)
- Physical key binding mode (language-agnostic keybindings via scan codes)

### High-Priority Missing Features (⭐⭐⭐)
1. **Hotkey window** - Quake-style dropdown - 🔴 High effort
2. **Multiple profiles** - Named configurations - 🔵 Very high effort
3. ~~**Split panes** - Divide terminal~~ - ✅ **IMPLEMENTED**
4. ~~**Shell integration** - Command tracking~~ - ✅ **IMPLEMENTED** (OSC 133/7/1337 in core)
5. ~~**tmux control mode** - Native tmux integration~~ - ✅ **IMPLEMENTED**

### Recommended Implementation Priority

**Phase 1 - Quick Wins (Low Effort, High Value)**
1. ~~Smart cursor color (⭐⭐, 🟢)~~ - ✅ **IMPLEMENTED** in core
2. ~~Faint text alpha (⭐, 🟢)~~ - ✅ **IMPLEMENTED** in core
3. ~~Bold color/brightening (⭐⭐, 🟢)~~ - ✅ **IMPLEMENTED** in core
4. ~~Link color (⭐⭐, 🟢)~~ - ✅ **IMPLEMENTED** in core
5. ~~Underline color SGR 58/59 (⭐⭐, 🟢)~~ - ✅ **IMPLEMENTED** in core

**Phase 2 - Medium Effort, High Value**
1. Tab bar position options (⭐⭐, 🟡)
2. Light/Dark mode theme switching (⭐⭐, 🟡)
3. Minimum contrast (⭐⭐, 🟡)
4. Timestamps in scrollback (⭐⭐, 🟡)
5. Mark indicators (⭐⭐, 🟡)
6. Session undo timeout (⭐⭐, 🟡)
7. Window arrangements (⭐⭐, 🟡)

**Phase 3 - High Effort, High Value**
1. Hotkey window (⭐⭐⭐, 🔴)
2. Triggers & automation (⭐⭐, 🔴)

**Phase 4 - Very High Effort (Major Features)**
1. ~~Split panes (⭐⭐⭐, 🔵)~~ - ✅ **IMPLEMENTED**
2. Multiple profiles (⭐⭐⭐, 🔵)
3. ~~Shell integration (⭐⭐⭐, 🔵)~~ - ✅ **IMPLEMENTED** (OSC 133/7/1337 in core)
4. ~~tmux control mode (⭐⭐⭐, 🔵)~~ - ✅ **IMPLEMENTED**
5. AI integration (⭐⭐, 🔵)

---

*Updated: 2026-02-04*
*iTerm2 Version: Latest (from source)*
*par-term Version: 0.9.0+*
