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
| Window type (normal/fullscreen/edge) | ✅ Multiple types | ❌ | ❌ | ⭐⭐ | 🟡 | Edge-anchored windows useful for dropdown terminal |
| Open on specific screen | ✅ `Screen` | ❌ | ❌ | ⭐ | 🟢 | Multi-monitor support |
| Open in specific Space | ✅ `Space` | ❌ | ❌ | ⭐ | 🟢 | macOS Spaces integration |
| Maximize vertically only | ✅ | ❌ | ❌ | ⭐ | 🟢 | Niche use case |
| Lock window size | ✅ `Lock Window Size Automatically` | ❌ | ❌ | ⭐ | 🟢 | Prevent accidental resize |
| Proxy icon in title bar | ✅ `Enable Proxy Icon` | ❌ | ❌ | ⭐ | 🟡 | macOS feature for current directory |
| Window number display | ✅ `Show Window Number` | ❌ | ❌ | ⭐ | 🟢 | Useful for multi-window |
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
| Cursor text color | ✅ `Cursor Text Color` | ❌ | ❌ | ⭐⭐ | 🟢 | Text color under block cursor |
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
| Bold color | ✅ | 🔶 | 🔶 | ⭐⭐ | 🟢 | Font weight only, no color intensity |
| Selection color | ✅ | ✅ | ✅ | - | - | Theme-controlled |
| Cursor color | ✅ | ✅ | ✅ | - | - | - |
| Link color | ✅ `Link Color` | 🔶 | 🔶 | ⭐⭐ | 🟢 | OSC 8 tracked but not colored |
| Theme presets | ✅ Many built-in | ✅ 17 themes | ✅ | - | - | Dracula, Nord, Monokai, Solarized, etc. |
| Light/Dark mode variants | ✅ Separate colors per mode | ❌ | ❌ | ⭐⭐ | 🟡 | Auto-switch with system theme |
| Minimum contrast | ✅ `Minimum Contrast` | ❌ | ❌ | ⭐⭐ | 🟡 | Accessibility feature |
| Smart cursor color | ✅ `Smart Cursor Color` | ❌ | ❌ | ⭐⭐ | 🟢 | Auto-choose readable cursor |
| Faint text alpha | ✅ `Faint Text Alpha` | ❌ | ❌ | ⭐ | 🟢 | Dim faint text |
| Underline color | ✅ `Underline Color` | ❌ | ❌ | ⭐⭐ | 🟢 | Uses text foreground color |
| Badge color | ✅ `Badge Color` | ❌ | ❌ | ⭐ | 🟢 | Part of badge feature |
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
| Tab index numbers | ✅ `Hide Tab Number` | 🔶 `tab_show_index` | 🔶 | ⭐⭐ | 🟢 | Config exists, rendering stubbed |
| New output indicator | ✅ `Show New Output Indicator` | ✅ Activity indicator | ✅ | - | - | - |
| Bell indicator | ✅ | ✅ `tab_bell_indicator` | ✅ | - | - | - |
| Activity indicator | ✅ `Hide Tab Activity Indicator` | ✅ `tab_activity_indicator` | ✅ | - | - | - |
| Tab colors (active/inactive/hover) | ✅ | ✅ Full color customization | ✅ | - | - | - |
| Dim inactive tabs | ✅ | ✅ `dim_inactive_tabs`, `inactive_tab_opacity` | ✅ | - | - | - |
| Tab min width | ❌ | ✅ `tab_min_width` | ✅ | - | - | par-term exclusive |
| Stretch tabs to fill | ✅ `Stretch Tabs to Fill Bar` | ❌ | ❌ | ⭐ | 🟢 | Equal-width vs stretched |
| New tabs at end | ✅ `New Tabs Open at End` | ✅ | ✅ | - | - | Default behavior |
| Inherit working directory | ✅ | ✅ `tab_inherit_cwd` | ✅ | - | - | - |
| Max tabs limit | ❌ | ✅ `max_tabs` | ✅ | - | - | par-term exclusive |
| Tab style (visual theme) | ✅ Light/Dark/Minimal/Compact | ❌ | ❌ | ⭐ | 🟡 | Different visual styles |
| HTML tab titles | ✅ `HTML Tab Titles` | ❌ | ❌ | ⭐ | 🟡 | Rich text in tabs |

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
| Timestamps | ✅ `Show Timestamps` | ❌ | ❌ | ⭐⭐ | 🟡 | Command timing info |
| Mark indicators | ✅ `Show Mark Indicators` | ❌ | ❌ | ⭐⭐ | 🟡 | Shell integration marks |

---

## 8. Selection & Clipboard

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Auto-copy selection | ✅ `Selection Copies Text` | ✅ `auto_copy_selection` | ✅ | - | - | - |
| Copy trailing newline | ✅ `Copy Last Newline` | ✅ `copy_trailing_newline` | ✅ | - | - | - |
| Middle-click paste | ✅ | ✅ `middle_click_paste` | ✅ | - | - | - |
| Clipboard history | ✅ | ✅ Cmd/Ctrl+Shift+H | ✅ | - | - | - |
| Block/rectangular selection | ✅ | ✅ | ✅ | - | - | - |
| Word selection | ✅ | ✅ | ✅ | - | - | - |
| Line selection | ✅ | ✅ | ✅ | - | - | - |
| Triple-click selects wrapped lines | ✅ `Triple Click Selects Full Wrapped Lines` | ✅ | ✅ | - | - | - |
| Smart selection rules | ✅ Custom regex patterns | ❌ | ❌ | ⭐⭐ | 🟡 | Double-click selection patterns |
| Word boundary characters | ✅ `Characters Considered Part of Word` | ❌ | ❌ | ⭐⭐ | 🟢 | Customize word selection |
| Paste bracketing | ✅ `Allow Paste Bracketing` | ✅ | ✅ | - | - | - |
| Paste special options | ✅ Many transformations | ❌ | ❌ | ⭐⭐ | 🟡 | Tab→spaces, escape, etc. |
| Allow terminal clipboard access | ✅ `Allow Clipboard Access From Terminal` | ✅ OSC 52 | ✅ | - | - | - |
| Wrap filenames in quotes | ✅ | ❌ | ❌ | ⭐ | 🟢 | Auto-quote dropped files |

---

## 9. Mouse & Pointer

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Mouse scroll speed | ✅ | ✅ `mouse_scroll_speed` | ✅ | - | - | - |
| Double-click threshold | ✅ | ✅ `mouse_double_click_threshold` | ✅ | - | - | - |
| Triple-click threshold | ✅ | ✅ `mouse_triple_click_threshold` | ✅ | - | - | - |
| Mouse reporting | ✅ `Mouse Reporting` | ✅ | ✅ | - | - | ANSI mouse sequences |
| Cmd+click opens URLs | ✅ `Cmd Click Opens URLs` | ✅ Ctrl+click | ✅ | - | - | Different modifier |
| Option+click moves cursor | ✅ `Option Click Moves Cursor` | ❌ | ❌ | ⭐⭐ | 🟢 | Position cursor at click |
| Focus follows mouse | ✅ `Focus Follows Mouse` | ❌ | ❌ | ⭐ | 🟢 | Auto-focus on hover |
| Three-finger middle click | ✅ `Three Finger Emulates Middle` | ❌ | ❌ | ⭐ | 🟢 | Trackpad gesture |
| Right-click context menu | ✅ | ✅ | ✅ | - | - | - |
| Horizontal scroll reporting | ✅ `Report Horizontal Scroll Events` | ❌ | ❌ | ⭐ | 🟢 | Niche use case |

---

## 10. Keyboard & Input

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Custom keybindings | ✅ Full keyboard map | ✅ `keybindings` | ✅ | - | - | - |
| Modifier remapping | ✅ Per-modifier remapping | ❌ | ❌ | ⭐⭐ | 🟡 | Remap Ctrl/Alt/Cmd |
| Option as Meta/Esc | ✅ `Option Key Sends` | ✅ `left/right_option_key_mode` | ✅ | - | - | Normal/Meta/Esc modes per key |
| Hotkey window | ✅ Global hotkey | ❌ | ❌ | ⭐⭐⭐ | 🔴 | Quake-style dropdown |
| Haptic/sound feedback for Esc | ✅ | ❌ | ❌ | ⭐ | 🟢 | Touch Bar feedback |
| Language-agnostic key bindings | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Non-US keyboard support |
| Application keypad mode | ✅ `Application Keypad Allowed` | ✅ | ✅ | - | - | - |
| Touch Bar customization | ✅ `Touch Bar Map` | ❌ | ❌ | ⭐ | 🟡 | macOS Touch Bar |
| modifyOtherKeys protocol | ✅ `Allow Modify Other Keys` | ❌ | ❌ | ⭐⭐ | 🟡 | Extended key reporting |

---

## 11. Shell & Session

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Custom shell command | ✅ `Command` | ✅ `custom_shell` | ✅ | - | - | - |
| Shell arguments | ✅ | ✅ `shell_args` | ✅ | - | - | - |
| Working directory | ✅ `Working Directory` | ✅ `working_directory` | ✅ | - | - | - |
| Login shell | ✅ | ✅ `login_shell` | ✅ | - | - | - |
| Environment variables | ✅ | ✅ `shell_env` | ✅ | - | - | - |
| Exit behavior | ✅ Close/Restart | ✅ `exit_on_shell_exit` | 🔶 | ⭐⭐ | 🟢 | Add restart option |
| Initial text to send | ✅ `Initial Text` | ❌ | ❌ | ⭐⭐ | 🟢 | Send command on start |
| Anti-idle (keep-alive) | ✅ `Send Code When Idle` | ❌ | ❌ | ⭐⭐ | 🟢 | Prevent SSH timeouts |
| Jobs to ignore | ✅ | ❌ | ❌ | ⭐ | 🟢 | Ignore specific processes |
| Session close undo timeout | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Recover closed tabs |
| TERM variable | ✅ `Terminal Type` | ✅ | ✅ | - | - | Set via environment |
| Character encoding | ✅ Multiple | ✅ UTF-8 | ✅ | - | - | UTF-8 only |
| Unicode version | ✅ | ❌ | ❌ | ⭐ | 🟢 | Unicode standard version |
| Unicode normalization | ✅ NFC/NFD/HFS+ | ❌ | ❌ | ⭐ | 🟡 | Text normalization |
| Answerback string | ✅ | ❌ | ❌ | ⭐ | 🟢 | Terminal identification |

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
| Session ended notification | ✅ `Send Session Ended Alert` | ❌ | ❌ | ⭐⭐ | 🟢 | Notify when process exits |
| Suppress alerts when focused | ✅ `Suppress Alerts in Active Session` | ❌ | ❌ | ⭐⭐ | 🟢 | Smart notification filtering |
| Flashing bell | ✅ `Flashing Bell` | ✅ Visual bell | ✅ | - | - | - |
| OSC 9/777 notifications | ✅ | ✅ `notification_max_buffer` | ✅ | - | - | - |

---

## 13. Logging & Recording

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Automatic session logging | ✅ `Automatically Log` | ❌ | ❌ | ⭐⭐ | 🟡 | Record all output |
| Log format (plain/HTML/asciicast) | ✅ Multiple formats | ❌ | ❌ | ⭐⭐ | 🟡 | Different log formats |
| Log directory | ✅ `Log Directory` | ❌ | ❌ | ⭐⭐ | 🟢 | Where to save logs |
| Archive on closure | ✅ `Archive on Closure` | ❌ | ❌ | ⭐ | 🟡 | Save session on close |
| Screenshot | ✅ | ✅ Ctrl+Shift+S | ✅ | - | - | - |
| Screenshot format | ✅ | ✅ `screenshot_format` | ✅ | - | - | png/jpeg/svg/html |

---

## 14. Profiles

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Multiple profiles | ✅ Full profile system | ❌ | ❌ | ⭐⭐⭐ | 🔵 | Named configurations |
| Profile selection | ✅ GUI + keyboard | ❌ | ❌ | ⭐⭐⭐ | 🔵 | Part of profile system |
| Profile tags | ✅ Searchable tags | ❌ | ❌ | ⭐⭐ | 🟡 | Organize profiles |
| Profile icon | ✅ Custom icons | ❌ | ❌ | ⭐ | 🟡 | Visual identification |
| Dynamic profiles (external files) | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Load from YAML/JSON |
| Profile inheritance | ✅ Parent profiles | ❌ | ❌ | ⭐⭐ | 🟡 | Base profile + overrides |
| Profile keyboard shortcut | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Quick profile launch |
| Automatic profile switching | ✅ Based on hostname | ❌ | ❌ | ⭐⭐ | 🟡 | SSH host detection |
| Profile badge | ✅ `Badge Text` | ❌ | ❌ | ⭐⭐ | 🟡 | Visual profile indicator |

---

## 15. Split Panes

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Horizontal split | ✅ | ❌ | ❌ | ⭐⭐⭐ | 🔵 | Split terminal vertically |
| Vertical split | ✅ | ❌ | ❌ | ⭐⭐⭐ | 🔵 | Split terminal horizontally |
| Pane navigation | ✅ | ❌ | ❌ | ⭐⭐⭐ | 🔵 | Move between panes |
| Pane resizing | ✅ | ❌ | ❌ | ⭐⭐⭐ | 🔵 | Resize pane boundaries |
| Dim inactive panes | ✅ `Dim Inactive Split Panes` | ❌ | ❌ | ⭐⭐ | 🟢 | Visual focus indicator |
| Per-pane titles | ✅ `Show Pane Titles` | ❌ | ❌ | ⭐⭐ | 🟡 | Pane identification |
| Per-pane background | ✅ | ❌ | ❌ | ⭐ | 🟡 | Different backgrounds |
| Broadcast input | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Type to multiple panes |
| Division view | ✅ `Enable Division View` | ❌ | ❌ | ⭐⭐ | 🟢 | Pane divider lines |

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
| Shell integration | ✅ Full integration | ❌ | ❌ | ⭐⭐⭐ | 🔵 | Command tracking, marks |
| Python API | ✅ Full scripting API | ❌ | ❌ | ⭐⭐ | 🔵 | Automation scripting |

---

## 19. tmux Integration

**Note:** par-term has **basic tmux compatibility** (can run tmux sessions and render output correctly) but does **not** have iTerm2-style native tmux integration via control mode.

### Current tmux Support in par-term

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| Run tmux as shell | ✅ | ✅ | ✅ | - | - | Basic compatibility |
| Render tmux status bar | ✅ | ✅ | ✅ | - | - | Handles reverse video (SGR 7) correctly |
| Render tmux panes/windows | ✅ | ✅ | ✅ | - | - | Standard VT sequence rendering |
| tmux mouse support | ✅ | ✅ | ✅ | - | - | Mouse reporting works in tmux |

### Missing: iTerm2-style Native tmux Integration

iTerm2's tmux integration uses **control mode** (`tmux -CC`) which provides a structured protocol for managing tmux sessions natively. This allows iTerm2 to represent tmux windows as native tabs and tmux panes as native split panes.

| Feature | iTerm2 | par-term | Status | Useful | Effort | Notes |
|---------|--------|----------|--------|--------|--------|-------|
| **tmux control mode (`-CC`)** | ✅ Full protocol | ❌ | ❌ | ⭐⭐⭐ | 🔵 | Core protocol for native integration |
| tmux windows as native tabs | ✅ | ❌ | ❌ | ⭐⭐⭐ | 🔵 | Requires control mode |
| tmux panes as native splits | ✅ | ❌ | ❌ | ⭐⭐⭐ | 🔵 | Requires control mode + split panes |
| tmux session picker UI | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | List/attach sessions from GUI |
| tmux status bar in UI | ✅ Native display | ❌ | ❌ | ⭐⭐ | 🟡 | Display status outside terminal area |
| tmux clipboard sync | ✅ Bidirectional | ❌ | ❌ | ⭐⭐ | 🟡 | Sync with tmux paste buffers |
| tmux pause mode handling | ✅ | ❌ | ❌ | ⭐⭐ | 🟡 | Handle slow connection pausing |
| Auto-attach on launch | ✅ | ❌ | ❌ | ⭐⭐ | 🟢 | Option to auto-attach to session |
| tmux profile auto-switching | ✅ | ❌ | ❌ | ⭐ | 🟡 | Different profile for tmux sessions |

### How iTerm2's tmux Control Mode Works

1. **Protocol**: iTerm2 connects via `tmux -CC` which outputs structured commands instead of terminal escape sequences
2. **Window Management**: tmux windows become iTerm2 tabs with native UI
3. **Pane Management**: tmux panes become iTerm2 split panes with native dividers
4. **Seamless Experience**: Users interact with native UI while tmux manages sessions server-side
5. **Session Persistence**: Closing iTerm2 doesn't kill tmux; sessions persist and can be reattached

### Implementation Complexity

Full tmux control mode integration would require:
- Parsing tmux control mode protocol (structured output format)
- Bidirectional command/response handling
- Mapping tmux window/pane IDs to par-term tabs/splits
- Session state synchronization
- Handling edge cases (window resize, pane creation/destruction)
- **Prerequisite**: Split pane support in par-term (currently not implemented)

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
| Disable GPU when unplugged | ✅ | ❌ | ❌ | ⭐ | 🟢 | Battery optimization |
| Prefer integrated GPU | ✅ | ❌ | ❌ | ⭐ | 🟢 | Power saving |
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
| Search in terminal | ✅ Cmd+F | ❌ | ❌ | ⭐⭐⭐ | 🟡 | Find text in scrollback |
| CLI command (`par-term`) | ❌ | ✅ Full CLI | ✅ | - | - | par-term exclusive |
| First-run shader install prompt | ❌ | ✅ Auto-detect & install | ✅ | - | - | par-term exclusive |
| Shader gallery | ❌ | ✅ Online gallery | ✅ | - | - | par-term exclusive |

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

### High-Priority Missing Features (⭐⭐⭐)
1. **Hotkey window** - Quake-style dropdown - 🔴 High effort
2. **Multiple profiles** - Named configurations - 🔵 Very high effort
3. **Split panes** - Divide terminal - 🔵 Very high effort
4. **Shell integration** - Command tracking - 🔵 Very high effort
5. **tmux control mode** - Native tmux integration (not basic compatibility) - 🔵 Very high effort
6. **Search in terminal** - Find in scrollback - 🟡 Medium effort

### Recommended Implementation Priority

**Phase 1 - Quick Wins (Low Effort, High Value)**
1. Cursor text color (⭐⭐, 🟢)
2. Smart cursor color (⭐⭐, 🟢)
3. Option+click moves cursor (⭐⭐, 🟢)
4. Word boundary characters (⭐⭐, 🟢)
5. Session ended notification (⭐⭐, 🟢)
6. Suppress alerts when focused (⭐⭐, 🟢)
7. Initial text to send on start (⭐⭐, 🟢)
8. Anti-idle keep-alive (⭐⭐, 🟢)
9. Tab index number rendering (⭐⭐, 🟢) - config exists, just needs rendering

**Phase 2 - Medium Effort, High Value**
1. Search in terminal (⭐⭐⭐, 🟡)
2. Tab bar position options (⭐⭐, 🟡)
3. Light/Dark mode theme switching (⭐⭐, 🟡)
4. Minimum contrast (⭐⭐, 🟡)
5. Timestamps in scrollback (⭐⭐, 🟡)
6. Mark indicators (⭐⭐, 🟡)
7. Smart selection rules (⭐⭐, 🟡)
8. Paste special options (⭐⭐, 🟡)
9. Session undo timeout (⭐⭐, 🟡)
10. Window arrangements (⭐⭐, 🟡)

**Phase 3 - High Effort, High Value**
1. Hotkey window (⭐⭐⭐, 🔴)
2. Triggers & automation (⭐⭐, 🔴)

**Phase 4 - Very High Effort (Major Features)**
1. Split panes (⭐⭐⭐, 🔵)
2. Multiple profiles (⭐⭐⭐, 🔵)
3. Shell integration (⭐⭐⭐, 🔵)
4. tmux control mode (⭐⭐⭐, 🔵) - requires split panes first
5. AI integration (⭐⭐, 🔵)

---

*Updated: 2026-01-30*
*iTerm2 Version: Latest (from source)*
*par-term Version: 0.6.0*
