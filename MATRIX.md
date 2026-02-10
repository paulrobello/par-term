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
| **Shader progress uniforms** | ❌ | ✅ `iProgress` vec4 | ✅ | - | - | **par-term exclusive** - progress bar state in shaders |

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
| Minimum contrast | ✅ `Minimum Contrast` | ✅ `minimum_contrast` | ✅ | - | - | WCAG luminance-based contrast adjustment (1.0-21.0) |
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
| Unicode normalization | ✅ NFC/NFD/HFS+ | ✅ NFC/NFD/NFKC/NFKD/None | ✅ | ⭐ | 🟢 | Text normalization form; configurable in Settings > Terminal > Unicode (core v0.35.0+) |
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
| Semantic history | ✅ Open in editor | ✅ `semantic_history_*` | ✅ | - | - | Ctrl+click file paths to open in editor with line:column support. Editor modes: Custom, $EDITOR, System Default |

---

## 18. Triggers & Automation

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Regex triggers | ✅ Full trigger system | ✅ `TriggerConfig` | ✅ | - | - | Core `TriggerRegistry` + Settings UI for CRUD with regex validation |
| Trigger actions | ✅ Many actions | ✅ 7 action types | ✅ | - | - | Highlight, Notify, MarkLine, SetVariable, RunCommand, PlaySound, SendText |
| Trigger highlight rendering | ✅ | ✅ Cell overlay | ✅ | - | - | Overlays fg/bg colors on matched cells with automatic expiry |
| Trigger marks on scrollbar | ✅ | ✅ MarkLine marks | ✅ | - | - | Color-coded trigger marks with labels in scrollbar tooltips |
| SetVariable → badge sync | ✅ | ✅ Custom variables | ✅ | - | - | Trigger-captured variables (e.g., git branch) displayed in badge overlay |
| Coprocesses | ✅ | ✅ `CoprocessManager` | ✅ | - | - | Per-tab coprocess with auto-start, restart policy (Never/Always/OnFailure), output viewer, start/stop controls, config persistence, Settings UI |
| Shell integration | ✅ Full integration | ✅ OSC 133/7/1337 | ✅ | - | - | Command tracking, marks, CWD, badges |
| **Automation Settings Tab** | ❌ | ✅ Settings > Automation | ✅ | - | - | **par-term exclusive** - Full CRUD for triggers and coprocesses |
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
| Maximize throughput | ✅ | ✅ `maximize_throughput` | ✅ | - | - | Toggle with Cmd+Shift+T |
| Disable GPU when unplugged | ✅ | ❌ | ❌ | ➖ | ➖ | Won't implement - par-term requires GPU |
| Prefer integrated GPU | ✅ | ✅ `power_preference` | ✅ | - | - | None/LowPower/HighPerformance GPU selection |
| Reduce flicker | ✅ `Reduce Flicker` | ✅ `reduce_flicker` | ✅ | - | - | Delay redraws while cursor hidden (DECTCEM off) |

---

## 21. Accessibility

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Minimum contrast | ✅ | ✅ `minimum_contrast` | ✅ | - | - | WCAG luminance-based contrast (1.0-21.0) |
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

## 23. Status Bar

iTerm2 has a comprehensive status bar system for displaying session and system information.

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Status bar visibility | ✅ `Show Status Bar` | ❌ | ❌ | ⭐⭐ | 🟡 | Toggle status bar on/off |
| Status bar position | ✅ Top/Bottom | ❌ | ❌ | ⭐⭐ | 🟡 | Choose status bar location |
| Status bar components | ✅ Configurable widgets | ❌ | ❌ | ⭐⭐ | 🔴 | Add/remove components (time, battery, network, etc.) |
| Status bar auto-hide | ✅ `Status Bar Location` (Automatic) | ❌ | ❌ | ⭐ | 🟡 | Hide when fullscreen/no mouse |
| Status bar color | ✅ Per-profile | ❌ | ❌ | ⭐ | 🟢 | Custom colors |
| Status bar font | ✅ `Status Bar Font` | ❌ | ❌ | ⭐ | 🟢 | Custom typography |
| Git branch in status bar | ✅ Component | ❌ | ❌ | ⭐⭐ | 🟡 | Show current branch |
| Network status | ✅ Component | ❌ | ❌ | ⭐ | 🟡 | Show network info |
| CPU/memory usage | ✅ Component | ❌ | ❌ | ⭐ | 🟡 | System monitoring |
| Username@hostname | ✅ Component | ❌ | ❌ | ⭐⭐ | 🟡 | Session info |

---

## 24. Toolbelt

iTerm2's Toolbelt is a sidebar providing quick access to various utilities.

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Toolbelt sidebar | ✅ `Enable Toolbelt` | ❌ | ❌ | ⭐ | 🔴 | Collapsible sidebar with utilities |
| Toolbelt notes | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Per-session notes/scratchpad |
| Toolbelt paste history | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Quick paste from history |
| Toolbelt jobs | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Manage background jobs |
| Toolbelt actions | ✅ | ❌ | ❌ | ⭐ | 🔴 | Custom actions in sidebar |
| Toolbelt profiles | ✅ | ❌ | ❌ | ⭐ | 🟡 | Profile switcher in sidebar |
| Toolbelt directory history | ✅ | ❌ | ❌ | ⭐ | 🟡 | Navigate visited directories |
| Toolbelt autocomplete | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Command history search |

---

## 25. Composer & Auto-Complete

iTerm2's Composer provides intelligent command completion suggestions.

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Composer UI | ✅ `Enable Composer` | ❌ | ❌ | ⭐⭐ | 🔵 | AI-style command completion |
| Command history search | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Fuzzy search history |
| Suggestion ranking | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Smart relevance scoring |
| Man page integration | ✅ | ❌ | ❌ | ⭐⭐ | 🔴 | Show man info inline |
| Command preview | ✅ | ❌ | ❌ | ⭐ | 🟡 | Preview command output |
| Shell integration auto-install | ✅ | ❌ | ❌ | ⭐⭐ | 🟢 | Auto-download shell integration scripts |

---

## 26. Copy Mode

iTerm2's Copy Mode provides vi-style navigation for selection.

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Copy Mode | ✅ `Copy Mode` | ❌ | ❌ | ⭐⭐ | 🟡 | Vi-style navigation for selection |
| Vi key bindings in copy mode | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | hjkl navigation |
| Copy mode activation | ✅ `Copy Mode Key Binding` | ❌ | ❌ | ⭐⭐ | 🟢 | Custom hotkey |
| Copy mode indicator | ✅ | ❌ | ❌ | ⭐ | 🟢 | Visual indicator when active |
| Character/word/line motion | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | w, b, e, 0, $, etc. |
| Search in copy mode | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | / and ? search |
| Mark positions | ✅ | ❌ | ❌ | ⭐ | 🟡 | m and ' marks |
| Copy to clipboard | ✅ | ❌ | ❌ | ⭐⭐ | 🟢 | y operation |

---

## 27. Snippets & Actions

iTerm2 has a system for saved text snippets and custom actions.

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Text snippets | ✅ Snippets | ✅ | ✅ | ⭐⭐ | 🟡 | Saved text blocks for quick insertion |
| Snippet shortcuts | ✅ | ✅ | ✅ | ⭐⭐ | 🟡 | Keyboard shortcuts for snippets |
| Snippet variables | ✅ | ✅ | ✅ | ⭐ | 🟡 | Dynamic values in snippets (10 built-in variables) |
| Snippet library | ✅ | ✅ Partial | ✅ | ⭐⭐ | 🟡 | Organize snippets into folders (no import/export yet) |
| Custom actions | ✅ | ✅ Partial | ✅ | ⭐ | 🔴 | Shell commands and text insertion (key sequences TODO) |
| Action key bindings | ✅ | ✅ | ✅ | ⭐ | 🟡 | Assign keys to actions via UI or config (auto-generated on load) |

### Implementation Details (v0.11.0+)

**Data Structures** (`src/config/snippets.rs`):
- `SnippetConfig`: id, title, content, keybinding, folder, enabled, description, variables (HashMap)
- `CustomActionConfig`: Tagged enum with ShellCommand, InsertText, KeySequence variants
- `BuiltInVariable`: Enum for 10 built-in variables with runtime resolution

**Variable Substitution** (`src/snippets/mod.rs`):
- `VariableSubstitutor`: Regex engine matching `\(variable)` syntax
- Built-in variable resolution (date, time, hostname, user, path, git_branch, git_commit, uuid, random)
- Custom variable support via HashMap
- 15 unit tests, all passing

**Settings UI**:
- **Snippets tab** (`src/settings_ui/snippets_tab.rs`): CRUD operations, folder grouping, variables reference
- **Actions tab** (`src/settings_ui/actions_tab.rs`): Type selector, form fields, CRUD operations
- Both added to sidebar navigation with icons (📝 Snippets, 🚀 Actions)
- Right-anchored Edit/Delete buttons with auto-truncating content preview (prevents overflow)
- Platform-specific keybinding display (shows `Cmd` on macOS, `Ctrl` on Linux/Windows instead of `CmdOrCtrl`)

**Execution Engine** (`src/app/input_events.rs`):
- `execute_snippet()`: Variable substitution + terminal write
- `execute_custom_action()`: Shell command execution, text insertion
- Keybinding integration via "snippet:<id>" and "action:<id>" prefixes
- Toast notifications for errors and success feedback

**Configuration** (`src/config/mod.rs`):
- `generate_snippet_action_keybindings()`: Auto-generate keybindings during config load
- Added to Config: `snippets: Vec<SnippetConfig>`, `actions: Vec<CustomActionConfig>`
- YAML persistence via serde

**Testing** (`tests/snippets_actions_tests.rs`):
- 26 integration tests covering all major functionality
- Config persistence, serialization, keybinding generation
- All 41 tests passing (26 integration + 15 unit)

**Documentation** (`docs/SNIPPETS.md`):
- Comprehensive user guide with examples
- Variable reference table
- Action configuration guide
- Tips and best practices

### Usage Examples

**Snippet with variables:**
```yaml
snippets:
  - id: "git_commit"
    title: "Git Commit"
    content: "git commit -m 'feat(\\(user)): \\(datetime)'"
    keybinding: "Ctrl+Shift+C"
    folder: "Git"
```

**Shell command action:**
```yaml
actions:
  - id: "run_tests"
    title: "Run Tests"
    type: "shell_command"
    command: "npm"
    args: ["test"]
    notify_on_success: true
```

### Future Enhancements

- [ ] Key sequence simulation (parsing and keyboard event injection)
- [ ] Import/export snippet libraries
- [ ] Custom variables UI editor

---

## 28. Window Arrangements & Placement

iTerm2 has sophisticated window state management.

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Save window arrangements | ✅ `Save Window Arrangements` | ❌ | ❌ | ⭐⭐ | 🟡 | Save window positions and layouts |
| Restore arrangements | ✅ `Restore Window Arrangements` | ❌ | ❌ | ⭐⭐ | 🟡 | Restore saved layouts |
| Arrange windows by app | ✅ | ❌ | ❌ | ⭐ | 🔴 | Auto-arrange windows |
| Hotkey window type | ✅ | ❌ | ❌ | ⭐⭐⭐ | 🔴 | Quake-style dropdown terminal |
| Hotkey window profile | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Different profile for hotkey window |
| Hotkey window animation | ✅ `Animate Hotkey Window` | ❌ | ❌ | ⭐ | 🟡 | Slide/fade animations |
| Hotkey window dock | ✅ | ❌ | ❌ | ⭐ | 🟡 | Show dock icon for hotkey window |
| Hotkey window hide on defocus | ✅ `Hotkey Window Hides When App Deactivated` | ❌ | ❌ | ⭐ | 🟢 | Auto-hide when losing focus |
| Hotkey window float | ✅ `Hotkey Window Floats` | ❌ | ❌ | ⭐ | 🟢 | Floating window style |
| Window screen memory | ✅ `Open Arrangement on Screen` | ❌ | ❌ | ⭐ | 🟡 | Remember screen per arrangement |

---

## 29. Session Management & Quit Behavior

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Prompt on quit | ✅ `Prompt When Quitting` | ✅ `prompt_on_quit` | ✅ | ⭐⭐ | 🟢 | Confirm before closing app with sessions |
| Confirm closing multiple sessions | ✅ `Confirm Closing Multiple Sessions` | ✅ Partial | ✅ | ⭐⭐ | 🟢 | Partial - jobs confirmation exists |
| Only confirm when there are jobs | ✅ | ✅ | ✅ | - | - | Already implemented |
| Session undo timeout | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Reopen closed tabs within timeout |
| Session restore on launch | ✅ `Restore Arrangement on Launch` | ❌ | ❌ | ⭐⭐ | 🔴 | Reopen previous sessions |
| Session restore at startup | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Auto-restore last sessions |
| Open saved arrangement | ✅ `Open Arrangement` | ❌ | ❌ | ⭐⭐ | 🟡 | Load saved window arrangement |

---

## 30. Tab Styles & Appearance

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Tab style variants | ✅ `Tab Style` (Automatic/Compact/High Contrast/Light/Minimal) | ❌ | ❌ | ⭐ | 🟡 | Different visual styles for tabs |
| Automatic tab style | ✅ | ❌ | ❌ | ⭐ | 🟡 | Auto-switch based on theme |
| Compact tab style | ✅ | ❌ | ❌ | ⭐ | 🟢 | Smaller tabs, more visible content |
| Minimal tab style | ✅ | ❌ | ❌ | ⭐ | 🟢 | Clean, minimal tab appearance |
| High contrast tab style | ✅ | ❌ | ❌ | ⭐ | 🟢 | Enhanced contrast for accessibility |
| Light tab style | ✅ | ❌ | ❌ | ⭐ | 🟢 | Light theme tabs |
| Dark tab style | ✅ | ❌ | ❌ | ⭐ | 🟢 | Dark theme tabs |
| Tab color overrides | ✅ `Tab Color` | ✅ | ✅ | - | - | Already implemented |

---

## 31. Pane & Split Customization

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Pane title format | ✅ `Show Pane Titles` | ✅ OSC/CWD/fallback titles | ✅ | ⭐⭐ | 🟡 | Configurable title display with text/bg colors |
| Pane title position | ✅ | ✅ top/bottom | ✅ | ⭐ | 🟢 | Top/bottom placement via settings |
| Pane title color | ✅ | ✅ text + background colors | ✅ | ⭐ | 🟢 | Configurable via settings UI |
| Pane title font | ✅ | ✅ uses terminal font | ✅ | ⭐ | 🟢 | Config field ready, uses terminal font |
| Division view | ✅ `Enable Division View` | ✅ configurable dividers | ✅ | - | - | Already implemented |
| Division thickness | ✅ `Division Thickness` | ✅ configurable width | ✅ | ⭐ | 🟢 | 1-10px slider in settings |
| Division color | ✅ `Division Color` | ✅ | ✅ | ⭐ | 🟢 | Already implemented |
| Division style | ✅ `Double/Shadow` | ✅ solid/double/dashed/shadow | ✅ | ⭐ | 🟢 | Four styles via settings UI |
| Per-pane backgrounds | ✅ | 🔶 Data model ready | 🔶 | ⭐ | 🟡 | Renderer support pending |

---

## 32. Profile Switching & Dynamic Profiles

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Hostname-based switching | ✅ | ✅ | ✅ | - | - | Already implemented |
| Directory-based switching | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Auto-switch profile by directory |
| Command-based switching | ✅ | ❌ | ❌ | ⭐ | 🟡 | Auto-switch by running command |
| User-based switching | ✅ | ❌ | ❌ | ⭐ | 🟡 | Switch by SSH user |
| Dynamic profiles from URL | ✅ `Dynamic Profiles` | ❌ | ❌ | ⭐⭐ | 🔴 | Load profiles from remote URL |
| Dynamic profiles reload | ✅ `Reload Dynamic Profiles` | ❌ | ❌ | ⭐⭐ | 🟡 | Refresh dynamic profiles |
| Dynamic profiles automatic reload | ✅ `Automatically Reload` | ❌ | ❌ | ⭐ | 🟡 | Auto-refresh on change |
| Profile inheritance | ✅ Parent profiles | ✅ `parent_id` | ✅ | - | - | Already implemented |

---

## 33. Image Protocol Enhancements

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Sixel support | ✅ | ✅ | ✅ | - | - | Already implemented |
| iTerm2 inline images | ✅ | ✅ | ✅ | - | - | Already implemented |
| Kitty graphics protocol | ✅ | ✅ | ✅ | - | - | Already implemented |
| Kitty animations | ✅ | ✅ | ✅ | - | - | Already implemented |
| Image compression | ✅ | ✅ | ✅ | - | - | Core handles zlib decompression for Kitty protocol transparently |
| Image scaling quality | ✅ | ✅ `image_scaling_mode` | ✅ | - | - | Nearest (sharp/pixel art) and linear (smooth) filtering |
| Image placement modes | ✅ | ✅ | ✅ | - | - | Core ImagePlacement with inline/download, requested dimensions (cells/pixels/percent), z-index, sub-cell offsets |
| Preserve aspect ratio | ✅ | ✅ `image_preserve_aspect_ratio` | ✅ | - | - | Global config + per-image flag from core |
| Image metadata in files | ✅ | ✅ | ✅ | - | - | Core SerializableGraphic/GraphicsSnapshot with export/import JSON, base64 or file-backed pixel data |

---

## 34. Audio & Haptic Feedback

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Sound for ESC key | ✅ `Play Sound When Esc Is Pressed` | ❌ | ❌ | ➖ | ➖ | Touch Bar feature - won't implement |
| Haptic feedback for ESC | ✅ `Haptic Feedback For Esc` | ❌ | ❌ | ➖ | ➖ | Touch Bar feature - won't implement |
| Bell sound selection | ✅ `Bell Sound` | ✅ `notification_bell_sound_file` | ✅ | - | - | Already implemented |
| Custom bell sounds | ✅ | ✅ | ✅ | - | - | Already implemented |
| Alert sounds | ✅ | ❌ | ❌ | ⭐ | 🟢 | Sound effects for events |

---

## 35. Advanced GPU & Rendering Settings

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| GPU renderer selection | ✅ `Use GPU Renderer` | ✅ wgpu | ✅ | - | - | Always GPU in par-term |
| Metal backend | ✅ | ✅ Metal on macOS | ✅ | - | - | Already implemented |
| Reduce flicker | ✅ `Reduce Flicker` | ✅ `reduce_flicker` | ✅ | - | - | Already implemented |
| Minimum frame time | ✅ | ✅ `max_fps` | ✅ | - | - | Config + Settings UI slider (1-240), separate `unfocused_fps` |
| Subpixel anti-aliasing | ✅ | ❌ | 🚫 | - | - | Won't implement; industry moving away (macOS dropped in Mojave), thin strokes covers most benefit, incompatible with transparency/bg images/shaders |
| Font smoothing | ✅ `ASCII/Non-ASCII Antialiased` | ✅ | ✅ | - | - | Already implemented |

---

## 36. Advanced Configuration

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Save preferences mode | ✅ `Save Preferences` | ✅ Auto-saves on change | ✅ | - | - | Auto-saves when settings changed in UI |
| Preference file location | ✅ | ✅ XDG-compliant | ✅ | - | - | Already implemented |
| Import preferences | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Import from file/URL |
| Export preferences | ✅ | ❌ | ❌ | ⭐⭐ | 🟢 | Export config to file |
| Preference validation | ✅ | ✅ Serde validation | ✅ | - | - | Serde deserialization with defaults and backward compat |
| Preference profiles | ✅ | ✅ Full profile system | ✅ | - | - | Tags, inheritance, shortcuts, hostname/tmux auto-switching |
| Shell integration download | ✅ | ✅ Embedded auto-install | ✅ | - | - | bash/zsh/fish scripts embedded and auto-installed to RC files |
| Shell integration version | ✅ | ✅ Version tracking | ✅ | - | - | Tracks installed/prompted versions, prompts on update |

---

## 37. Unicode & Text Processing

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Unicode normalization | ✅ `Unicode Normalization` (NFC/NFD/HFS+) | ✅ NFC/NFD/NFKC/NFKD/None | ✅ | - | - | Already implemented; Settings > Terminal > Unicode (core v0.35.0+) |
| Unicode version selection | ✅ `Unicode Version` | ✅ | ✅ | - | - | Already implemented |
| Ambiguous width characters | ✅ `Ambiguous Width Characters` | ✅ | ✅ | - | - | Already implemented |
| Unicode box drawing | ✅ | ✅ | ✅ | - | - | Already implemented |
| Emoji variation sequences | ✅ | ✅ Grapheme + FE0F font selection | ✅ | - | - | VS15/VS16 preserved via grapheme strings, FE0F forces emoji font |
| Right-to-left text | ✅ `Bidi` | ❌ | ❌ | ⭐⭐ | 🔴 | Bidirectional text support |

---

## 38. Browser Integration

iTerm2 has a built-in browser for web-based workflows.

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Built-in browser | ✅ `Enable Browser Integration` | ❌ | ❌ | ⭐ | 🔴 | Embedded web browser |
| Browser per tab | ✅ | ❌ | ❌ | ⭐ | 🔴 | Individual browser tabs |
| Browser profile sync | ✅ | ❌ | ❌ | ⭐ | 🟡 | Sync with external browser |
| Open links in browser | ✅ | ❌ | ❌ | ⭐ | 🟡 | Configurable link handler |

---

## 39. Progress Bars

iTerm2 supports showing progress for long-running commands.

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Progress bar protocol | ✅ `Progress Bar` (OSC 934) | ✅ | ✅ | ⭐⭐ | 🟡 | OSC 9;4 simple progress bar |
| Progress bar style | ✅ | ✅ | ✅ | ⭐ | 🟢 | Bar and bar-with-text styles |
| Progress bar position | ✅ | ✅ | ✅ | ⭐ | 🟡 | Top/bottom placement |
| Multiple progress bars | ✅ | ✅ | ✅ | ⭐ | 🟡 | OSC 934 named concurrent bars |
| **Progress bar shader uniforms** | ❌ | ✅ `iProgress` | ✅ | - | - | **par-term exclusive** - expose state to custom shaders |

---

## 40. Advanced Paste & Input

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Paste from clipboard history | ✅ | ✅ | ✅ | - | - | Already implemented |
| Paste special transformations | ✅ | ✅ `Cmd/Ctrl+Shift+V` | ✅ | - | - | Already implemented |
| Paste multi-line behavior | ✅ `Paste Special` | ✅ | ✅ | - | - | Already implemented |
| Paste bracketing | ✅ `Allow Paste Bracketing` | ✅ | ✅ | - | - | Already implemented |
| Paste delay | ✅ | ✅ `paste_delay_ms` config | ✅ | - | - | Configurable delay between pasted lines (0-500ms) |
| Paste as single line | ✅ | ✅ Paste Special transform | ✅ | - | - | `Newline: Paste as Single Line` transform |
| Paste with newlines | ✅ | ✅ Paste Special transforms | ✅ | - | - | `Newline: Add/Remove Newlines` transforms |

---

## 41. Advanced Shell Integration

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Shell integration auto-install | ✅ | ✅ Embedded auto-install | ✅ | - | - | bash/zsh/fish scripts embedded, auto-installed to RC files |
| Shell integration version check | ✅ | ✅ Version tracking | ✅ | - | - | Tracks installed/prompted versions, prompts on update |
| Disable shell integration | ✅ | ✅ Uninstall in Settings | ✅ | - | - | Uninstall button cleanly removes from all RC files |
| Shell integration features | ✅ `Features` | ✅ OSC 133/7/1337 | ✅ | - | - | Partial - marks/CWD/badges |
| Current command in window title | ✅ | ✅ Title bar + badge var | ✅ | - | - | Shows `[cmd]` in title when running; `\(session.current_command)` badge var |
| Command duration tracking | ✅ | ✅ Via tooltips | ✅ | - | - | Already implemented |
| Command exit code in badge | ✅ | ✅ Title bar + badge var | ✅ | - | - | Shows `[Exit: N]` in title on failure; `\(session.exit_code)` badge var |
| Remote host integration | ✅ | ✅ OSC 7 + OSC 1337 RemoteHost | ✅ | - | - | Hostname/username from OSC 7 file:// URLs and OSC 1337 RemoteHost; auto profile switching |

---

## 42. Network & Discovery

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Bonjour discovery | ✅ `Bonjour Hosts` | ❌ | ❌ | ⭐ | 🟡 | Auto-discover SSH hosts |
| SSH hosts auto-discover | ✅ | ❌ | ❌ | ⭐⭐ | 🔴 | Scan network for SSH |
| Host profiles | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Per-host profiles |
| Quick connect | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Fast SSH connections |

---

## 43. Miscellaneous (Remaining)

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Config file location (XDG) | ✅ | ✅ | ✅ | - | - | Already implemented |
| Settings UI | ✅ Full GUI | ✅ Full GUI (F12) | ✅ | - | - | Already implemented |
| Reload config (F5) | ❌ | ✅ | ✅ | - | - | par-term exclusive |
| Window arrangements | ✅ Save/restore layouts | ❌ | ❌ | ⭐⭐ | 🟡 | Save window positions |
| Bonjour host discovery | ✅ | ❌ | ❌ | ⭐ | 🟡 | Auto-discover SSH hosts |
| Password manager | ✅ | ❌ | ❌ | ⭐ | 🔴 | Secure credential storage |
| Search in terminal | ✅ Cmd+F | ✅ Cmd/Ctrl+F | ✅ | - | - | Already implemented |
| CLI command (`par-term`) | ❌ | ✅ Full CLI | ✅ | - | - | par-term exclusive |
| First-run shader install prompt | ❌ | ✅ Auto-detect & install | ✅ | - | - | par-term exclusive |
| Shader gallery | ❌ | ✅ Online gallery | ✅ | - | - | par-term exclusive |
| Automatic update checking | ✅ Built-in updater | ✅ `update_check_frequency` | ✅ | - | - | Notify-only (no auto-install) |
| Quit when last session closes | ✅ | ❌ | ❌ | ⭐ | 🟢 | Auto-exit when last tab closes |
| Open files in editor | ✅ `Semantic History` | ✅ `semantic_history_*` | ✅ | - | - | Already implemented |
| Report terminal type | ✅ | ✅ | ✅ | - | - | Already implemented |
| Character encoding | ✅ Multiple | ✅ UTF-8 | ✅ | - | - | UTF-8 only is fine |
| Check for updates automatically | ✅ | ✅ | ✅ | - | - | Already implemented |
| Open new viewer window | ✅ | ❌ | ❌ | ⭐ | 🟡 | Clone session in new window |
| Variable substitution | ✅ | ❌ | ❌ | ⭐ | 🟡 | Environment vars in config |

---

## 44. Badges

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
| Badge per-profile | ✅ Profile setting | ✅ Full badge config | ✅ | - | - | Per-profile badge text, color, alpha, font, bold, margins, and size |
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

### Feature Counts by Category

| Category | Implemented | Partial | Not Implemented |
|----------|-------------|---------|-----------------|
| Window & Display | 14 | 0 | 2 |
| Typography & Fonts | 16 | 1 | 0 |
| Cursor | 12 | 0 | 0 |
| Background & Effects | 11 | 0 | 1 |
| Colors & Themes | 16 | 0 | 1 |
| Tab Bar | 16 | 1 | 2 |
| Scrollback & Scrollbar | 11 | 1 | 1 |
| Selection & Clipboard | 12 | 0 | 0 |
| Mouse & Pointer | 9 | 0 | 1 |
| Keyboard & Input | 9 | 0 | 2 |
| Shell & Session | 14 | 0 | 2 |
| Notifications & Bell | 12 | 0 | 0 |
| Logging & Recording | 6 | 0 | 0 |
| Profiles | 12 | 0 | 0 |
| Split Panes | 9 | 1 | 0 |
| Inline Graphics | 5 | 0 | 0 |
| Hyperlinks & URLs | 5 | 0 | 0 |
| Triggers & Automation | 8 | 0 | 1 |
| tmux Integration | 17 | 0 | 0 |
| Performance & Power | 9 | 0 | 1 |
| Accessibility | 2 | 0 | 2 |
| AI Integration | 0 | 0 | 4 |
| Status Bar | 0 | 0 | 10 |
| Toolbelt | 0 | 0 | 8 |
| Composer & Auto-Complete | 0 | 0 | 5 |
| Copy Mode | 0 | 0 | 8 |
| Snippets & Actions | 0 | 0 | 6 |
| Window Arrangements & Placement | 1 | 0 | 9 |
| Session Management & Quit Behavior | 2 | 1 | 5 |
| Tab Styles & Appearance | 1 | 0 | 6 |
| Pane & Split Customization | 9 | 0 | 0 |
| Profile Switching & Dynamic Profiles | 2 | 0 | 5 |
| Image Protocol Enhancements | 9 | 0 | 0 |
| Audio & Haptic Feedback | 2 | 0 | 3 |
| Advanced GPU & Rendering Settings | 3 | 0 | 2 |
| Advanced Configuration | 1 | 0 | 7 |
| Unicode & Text Processing | 3 | 0 | 3 |
| Browser Integration | 0 | 0 | 4 |
| Progress Bars | 5 | 0 | 0 |
| Advanced Paste & Input | 6 | 0 | 0 |
| Advanced Shell Integration | 6 | 1 | 1 |
| Network & Discovery | 0 | 0 | 4 |
| Miscellaneous | 10 | 0 | 7 |
| Badges | 9 | 0 | 0 |
| **TOTAL** | **~287** | **~5** | **~124** |

**Overall Parity: ~69% of iTerm2 features implemented** (287 implemented out of ~416 total tracked features)

**Note: This includes many low-priority features. Core terminal functionality parity is much higher (80%+).**

### par-term Exclusive Features (Not in iTerm2)
- 49+ custom GLSL background shaders with hot reload
- 12+ cursor shader effects (GPU-powered cursor animations)
- Per-shader configuration system with metadata
- Shadertoy-compatible texture channels and cubemaps
- Progress bar shader uniforms (`iProgress` — react to OSC 9;4 / OSC 934 state)
- First-run shader install prompt (auto-detect missing shaders)
- Scrollbar customization (position, colors, width, auto-hide)
- Scrollbar mark tooltips (command, time, duration, exit code)
- FPS control and VSync modes
- GPU power preference (low power/high performance)
- Power saving options (pause shaders/refresh on blur)
- Reduce flicker mode with configurable delay
- Maximize throughput mode for bulk output
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
- Keep text opaque (separate from window transparency)
- Window decorations toggle
- modifyOtherKeys protocol support (modes 0, 1, 2)
- Semantic history with 3 editor modes (Custom/$EDITOR/System Default)
- WCAG-compliant minimum contrast enforcement (1.0-21.0 range)
- Shell exit action with 5 modes (close/keep/restart variants)
- Close confirmation for running jobs with configurable ignore list
- tmux profile auto-switching via session name patterns
- Automation Settings Tab with full CRUD for triggers and coprocesses
- Trigger-to-badge variable sync (SetVariable action updates badge overlay)
- Configurable log level (config/CLI/Settings UI) with unified log file routing

### Remaining High-Priority Features

| Feature | Usefulness | Effort | Notes |
|---------|------------|--------|-------|
| Hotkey window (Quake-style) | ⭐⭐⭐ | 🔴 High | Dropdown terminal with global hotkey |
| Copy Mode (vi-style navigation) | ⭐⭐⭐ | 🟡 Medium | Vi-style navigation for selection |
| Status Bar | ⭐⭐⭐ | 🔴 High | Customizable status bar with widgets |
| Snippets system | ⭐⭐⭐ | 🟡 Medium | Saved text blocks for quick insertion |
| Directory-based profile switching | ⭐⭐⭐ | 🟡 Medium | Auto-switch profile by directory |
| Session undo timeout | ⭐⭐ | 🟡 Medium | Recover accidentally closed tabs |
| Window arrangements | ⭐⭐ | 🟡 Medium | Save/restore window layouts |
| ~~Progress bars (OSC 934)~~ | ⭐⭐ | 🟡 Medium | ✅ Complete (OSC 9;4 + OSC 934) |
| Composer (auto-complete) | ⭐⭐ | 🔵 Very High | AI-style command completion |
| Toolbelt sidebar | ⭐⭐ | 🔴 High | Notes, paste history, jobs panel |
| Shell integration auto-install | ⭐⭐ | 🟢 Low | Auto-download shell scripts |
| Light/Dark mode switching | ⭐⭐ | 🟡 Medium | Auto-switch with system theme |
| Tab bar position (left/bottom) | ⭐⭐ | 🟡 Medium | Top/Bottom/Left options |
| Tab style variants | ⭐ | 🟢 Low | Different visual styles for tabs |
| Paste delay options | ⭐ | 🟢 Low | Delay between pasted lines |
| Command in window title | ⭐⭐ | 🟡 Medium | Show running command |
| Dynamic profiles from URL | ⭐⭐ | 🔴 High | Load profiles from remote URL |
| ~~Pane title customization~~ | ~~⭐⭐~~ | ~~🟡 Medium~~ | ✅ Implemented |
| ~~Division thickness/style~~ | ~~⭐~~ | ~~🟢 Low~~ | ✅ Implemented |
| Instant Replay | ⭐⭐ | 🔵 Very High | Rewind terminal state |
| AI integration | ⭐⭐ | 🔵 Very High | Command help and generation |
| VoiceOver/accessibility | ⭐⭐ | 🔵 Very High | Screen reader support |
| Bidirectional text | ⭐⭐ | 🔴 High | RTL language support |
| Browser integration | ⭐ | 🔴 High | Embedded web browser |
| Bonjour/SSH discovery | ⭐⭐ | 🔴 High | Auto-discover SSH hosts |

### Newly Identified Features (This Update)

The following iTerm2 features were identified and added to the matrix in this update:

**Status Bar (10 features)**
- Status bar visibility, position, auto-hide
- Configurable components (time, battery, network, git branch, etc.)
- Custom colors and fonts

**Toolbelt (8 features)**
- Sidebar with notes, paste history, jobs, actions
- Profile switcher and directory history
- Command history search/autocomplete

**Composer & Auto-Complete (5 features)**
- AI-style command completion UI
- Command history search with fuzzy matching
- Man page integration and command preview

**Copy Mode (8 features)**
- Vi-style navigation for text selection
- Vi key bindings (hjkl, w, b, e, 0, $, etc.)
- Search (/ and ?) and marks (m and ')
- y operation to copy to clipboard

**Snippets & Actions (6 features)**
- Saved text snippets with shortcuts
- Dynamic variables in snippets
- Custom user-defined actions/macros

**Window Arrangements (9 features)**
- Save/restore window arrangements
- Hotkey window type (Quake-style dropdown)
- Hotkey window animations and profiles
- Screen memory per arrangement

**Session Management (5 features)**
- Prompt on quit with sessions
- Session undo timeout (recover closed tabs)
- Session restore on launch

**Tab Styles (6 features)**
- Multiple tab style variants (Automatic/Compact/High Contrast/Light/Minimal/Dark)
- Auto-switch tab style based on theme

**Pane Customization (1 remaining feature)**
- ~~Pane title format, position, color, font~~ ✅ Implemented
- ~~Division thickness and style variants~~ ✅ Implemented
- Per-pane backgrounds (data model ready, renderer pending)

**Profile Switching (5 features)**
- Directory-based auto-switching
- Command-based auto-switching
- User-based auto-switching
- Dynamic profiles from URL with auto-reload

**Advanced Configuration (7 features)**
- Save preferences mode (auto-save/ask on quit)
- Import/export preferences
- Preference validation and profiles

**Unicode & Text Processing (2 features)**
- Emoji variation sequences
- Right-to-left text support

**Browser Integration (4 features)**
- Built-in browser for web-based workflows
- Browser per tab, profile sync

**Progress Bars (0 remaining - COMPLETE)**
- ~~OSC 9;4 protocol support~~ ✅
- ~~Progress bar style and position~~ ✅
- ~~Multiple concurrent progress bars (OSC 934)~~ ✅

**Advanced Paste (3 features)**
- ~~Paste delay between lines~~ ✅
- ~~Paste as single line~~ ✅
- ~~Add/remove newlines on paste~~ ✅

**Advanced Shell Integration (1 feature)**
- ~~Current command in window title~~ ✅
- ~~Command exit code in badge~~ ✅
- ~~Remote host integration~~ ✅

**Network & Discovery (4 features)**
- Bonjour discovery
- SSH hosts auto-discovery
- Host profiles and quick connect

**Total: 131 new features remaining across 21 new categories**

---

### Recently Completed (v0.12.0)
- ✅ Snippets & actions system (text snippets, custom actions, keybinding recording, auto-execute)
- ✅ Progress bar rendering (OSC 9;4 and OSC 934 named bars, iProgress shader uniform)
- ✅ Paste delay and newline control transforms
- ✅ Current command in window title, exit code and current command badge variables
- ✅ Remote host integration: OSC 1337 RemoteHost support, hostname/username synced to badge variables
- ✅ Image scaling quality and aspect ratio control
- ✅ Pane title bars and divider style customization
- ✅ Prompt on quit confirmation dialog
- ✅ Cross-platform keybindings overhaul (Linux/Windows)

### Previously Completed (v0.11.0)
- ✅ Triggers & automation (regex triggers, 7 action types, coprocesses with restart policy & output viewer)
- ✅ Unified logging with configurable log level
- ✅ Semantic history (click file paths to open in editor)
- ✅ Minimum contrast enforcement (WCAG compliance)
- ✅ Shell exit action modes, close confirmation for running jobs
- ✅ Modifier key remapping, physical key bindings, modifyOtherKeys protocol

---

*Updated: 2026-02-10 (Snippets, progress bars, paste enhancements, pane customization)*
*iTerm2 Version: Latest (from source)*
*par-term Version: 0.12.0*
