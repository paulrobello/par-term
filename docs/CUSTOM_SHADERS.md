# Custom Shaders Guide

Par-term supports custom GLSL shaders for background effects and post-processing, compatible with Ghostty and Shadertoy shader formats. This guide covers configuration, available uniforms, and creating your own shaders.

For a list of all included shaders, see [SHADERS.md](SHADERS.md).

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Installing Shaders](#installing-shaders)
- [Configuration](#configuration)
  - [Background Shader Settings](#background-shader-settings)
  - [Cursor Shader Settings](#cursor-shader-settings)
  - [Channel Textures](#channel-textures)
  - [Cubemap Textures](#cubemap-textures)
  - [Power Saving](#power-saving)
  - [Shader Hot Reload](#shader-hot-reload)
  - [Per-Shader Overrides](#per-shader-overrides)
- [Available Uniforms](#available-uniforms)
  - [Core Shadertoy Uniforms](#core-shadertoy-uniforms)
  - [Window & Content Uniforms](#window--content-uniforms)
  - [Texture Channel Uniforms](#texture-channel-uniforms)
  - [Cursor Uniforms](#cursor-uniforms)
  - [Cursor Shader Configuration Uniforms](#cursor-shader-configuration-uniforms)
- [Creating Custom Shaders](#creating-custom-shaders)
  - [Basic Structure](#basic-structure)
  - [Shader Modes](#shader-modes)
  - [Shader Metadata Format](#shader-metadata-format)
  - [Porting Shadertoy Shaders](#porting-shadertoy-shaders)
- [Examples](#examples)
- [Troubleshooting](#troubleshooting)
- [Related Documentation](#related-documentation)

## Overview

Par-term's shader system provides two types of customization:

1. **Background Shaders**: Post-processing effects applied to the entire terminal (CRT effects, color grading, animated backgrounds)
2. **Cursor Shaders**: Visual effects that follow the cursor (trails, glows, ripples)

Shaders are written in GLSL (OpenGL Shading Language) and automatically transpiled to WGSL for the GPU backend.

## Quick Start

1. Copy a shader file to your config directory:
   ```bash
   # macOS/Linux
   mkdir -p ~/.config/par-term/shaders
   cp shaders/starfield.glsl ~/.config/par-term/shaders/
   ```

2. Enable the shader in your config:
   ```yaml
   # ~/.config/par-term/config.yaml
   custom_shader: "starfield.glsl"
   custom_shader_enabled: true
   ```

3. Restart par-term or press `F5` to reload configuration

## Installing Shaders

### From the Included Collection

Par-term includes 49 ready-to-use shaders in the `shaders/` directory. See [SHADERS.md](SHADERS.md) for the complete list. Copy any shader to your configuration directory to use it:

```bash
# macOS/Linux
cp shaders/crt.glsl ~/.config/par-term/shaders/

# Windows (PowerShell)
Copy-Item shaders\crt.glsl $env:APPDATA\par-term\shaders\
```

### Shader Directory Location

| Platform | Path |
|----------|------|
| macOS/Linux | `~/.config/par-term/shaders/` |
| Windows | `%APPDATA%\par-term\shaders\` |

The directory is created automatically when par-term first starts.

## Configuration

### Background Shader Settings

Configure background/post-processing shaders in your config file:

```yaml
# ~/.config/par-term/config.yaml

# Shader file name (in shaders/ directory)
custom_shader: "starfield.glsl"

# Enable/disable the shader
custom_shader_enabled: true

# Enable animation (updates iTime uniform each frame)
# When false, iTime remains at 0.0
custom_shader_animation: true

# Animation speed multiplier (1.0 = normal, 0.5 = half speed, 2.0 = double)
custom_shader_animation_speed: 1.0

# Text opacity when shader is active (0.0 - 1.0)
custom_shader_text_opacity: 1.0

# Shader brightness (0.05 - 1.0, default 1.0)
# Dims the shader background to improve text readability
custom_shader_brightness: 0.5

# Full content mode: shader can distort/modify text
# false = text composited on top of shader output (recommended)
# true = shader receives and can modify terminal content via iChannel4
custom_shader_full_content: false
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `custom_shader` | `String?` | `null` | Path to GLSL shader file (absolute or relative to shaders dir) |
| `custom_shader_enabled` | `bool` | `true` | Enable/disable background shader rendering |
| `custom_shader_animation` | `bool` | `true` | Enable animation (updates iTime each frame) |
| `custom_shader_animation_speed` | `f32` | `1.0` | Animation speed multiplier |
| `custom_shader_text_opacity` | `f32` | `1.0` | Text opacity when shader is active (0.0-1.0) |
| `custom_shader_brightness` | `f32` | `1.0` | Brightness multiplier (0.05-1.0) |
| `custom_shader_full_content` | `bool` | `false` | When true, shader can manipulate terminal content |

### Cursor Shader Settings

Cursor shaders are configured separately and have additional controls:

```yaml
# Cursor shader file name
cursor_shader: "cursor_glow.glsl"

# Enable/disable cursor shader
cursor_shader_enabled: true

# Animation controls
cursor_shader_animation: true
cursor_shader_animation_speed: 1.0

# Visibility controls
cursor_shader_hides_cursor: false          # Hide default cursor (set true for replacement shaders)
cursor_shader_disable_in_alt_screen: true  # Pause cursor shader in alt-screen apps (vim/less/htop)

# Cursor effect parameters
cursor_shader_color: [255, 255, 255]       # Cursor color RGB (0-255)
cursor_shader_trail_duration: 0.5          # Trail duration in seconds
cursor_shader_glow_radius: 80.0            # Glow radius in pixels
cursor_shader_glow_intensity: 0.3          # Glow intensity (0.0-1.0)
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `cursor_shader` | `String?` | `null` | Path to cursor shader GLSL file |
| `cursor_shader_enabled` | `bool` | `false` | Enable/disable cursor shader effects |
| `cursor_shader_animation` | `bool` | `true` | Enable animation in cursor shader |
| `cursor_shader_animation_speed` | `f32` | `1.0` | Animation speed multiplier |
| `cursor_shader_color` | `[u8; 3]` | `[255, 255, 255]` | Cursor color RGB (0-255) |
| `cursor_shader_trail_duration` | `f32` | `0.5` | Trail duration in seconds |
| `cursor_shader_glow_radius` | `f32` | `80.0` | Glow radius in pixels |
| `cursor_shader_glow_intensity` | `f32` | `0.3` | Glow intensity (0.0-1.0) |
| `cursor_shader_hides_cursor` | `bool` | `false` | Hide default cursor when shader is enabled |
| `cursor_shader_disable_in_alt_screen` | `bool` | `true` | Disable cursor shader in alt screen apps |

### Channel Textures

Par-term supports Shadertoy-compatible texture channels (iChannel0-3) for passing custom images to shaders:

```yaml
# Texture paths for shader channels (supports ~ for home directory)
custom_shader_channel0: "~/textures/noise.png"
custom_shader_channel1: "~/textures/metal.jpg"
custom_shader_channel2: null  # Not used
custom_shader_channel3: null  # Not used
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `custom_shader_channel0` | `String?` | `null` | Path to iChannel0 texture |
| `custom_shader_channel1` | `String?` | `null` | Path to iChannel1 texture |
| `custom_shader_channel2` | `String?` | `null` | Path to iChannel2 texture |
| `custom_shader_channel3` | `String?` | `null` | Path to iChannel3 texture |

**Notes:**
- `iChannel0-3` are user-defined texture inputs (Shadertoy compatible)
- `iChannel4` is the terminal content texture (par-term specific)
- Channels without a configured texture use a 1x1 transparent placeholder
- Supports common image formats: PNG, JPEG, BMP, etc.
- Textures can also be configured via Settings UI under "Shader Channel Textures"
- Sample textures are included in `shaders/textures/` directory

### Cubemap Textures

Par-term supports cubemap textures for environment mapping and skybox effects via the `iCubemap` uniform:

```yaml
# Path prefix for cubemap faces
# Expects 6 files: {prefix}-px.{ext}, -nx.{ext}, -py.{ext}, -ny.{ext}, -pz.{ext}, -nz.{ext}
custom_shader_cubemap: "shaders/textures/cubemaps/env-outside"

# Enable cubemap sampling
custom_shader_cubemap_enabled: true
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `custom_shader_cubemap` | `String?` | `null` | Path prefix for cubemap face files |
| `custom_shader_cubemap_enabled` | `bool` | `true` | Enable cubemap sampling |

**Face naming convention:**
- `{prefix}-px.{ext}` - Positive X (+X, right)
- `{prefix}-nx.{ext}` - Negative X (-X, left)
- `{prefix}-py.{ext}` - Positive Y (+Y, top)
- `{prefix}-ny.{ext}` - Negative Y (-Y, bottom)
- `{prefix}-pz.{ext}` - Positive Z (+Z, front)
- `{prefix}-nz.{ext}` - Negative Z (-Z, back)

**Supported formats:** PNG, JPEG, HDR

**Notes:**
- HDR cubemaps (.hdr) are supported with automatic Rgba16Float conversion
- LDR cubemaps use Rgba8UnormSrgb format
- Sample cubemaps are included in `shaders/textures/cubemaps/`
- Use `iCubemapResolution.xy` for cubemap face dimensions

### Power Saving

Control shader behavior when the window loses focus:

```yaml
# Pause shader animations when window loses focus
pause_shaders_on_blur: true

# Reduce refresh rate when unfocused
pause_refresh_on_blur: false

# Target FPS when unfocused (only if pause_refresh_on_blur=true)
unfocused_fps: 30
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `pause_shaders_on_blur` | `bool` | `true` | Pause shader animations when window loses focus |
| `pause_refresh_on_blur` | `bool` | `false` | Reduce refresh rate when unfocused |
| `unfocused_fps` | `u32` | `30` | Target FPS when unfocused |

### Shader Hot Reload

Enable automatic shader reloading when files change:

```yaml
# Auto-reload shaders when files are modified
shader_hot_reload: false

# Debounce delay in milliseconds before reloading
shader_hot_reload_delay: 100
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `shader_hot_reload` | `bool` | `false` | Auto-reload shaders when files are modified |
| `shader_hot_reload_delay` | `u64` | `100` | Debounce delay in milliseconds |

### Per-Shader Overrides

Override settings for specific shaders without changing global defaults:

```yaml
# Per-shader configuration overrides
shader_configs:
  "crt.glsl":
    animation_speed: 0.3
    brightness: 0.8
    full_content: true
  "starfield.glsl":
    brightness: 0.5

# Per-cursor-shader configuration overrides
cursor_shader_configs:
  "cursor_glow.glsl":
    glow_radius: 100.0
    glow_intensity: 0.5
  "cursor_trail.glsl":
    trail_duration: 0.8
```

**Per-shader override fields:**
- `animation_speed`: Override animation speed
- `brightness`: Override brightness
- `text_opacity`: Override text opacity
- `full_content`: Override full content mode
- `channel0`, `channel1`, `channel2`, `channel3`: Override texture paths
- `cubemap`: Override cubemap path
- `cubemap_enabled`: Override cubemap enable
- `use_background_as_channel0`: Use app's background image as iChannel0

**Per-cursor-shader additional fields:**
- `glow_radius`: Override glow radius
- `glow_intensity`: Override glow intensity
- `trail_duration`: Override trail duration
- `cursor_color`: Override cursor color `[R, G, B]` (0-255)

**Configuration resolution priority (highest to lowest):**
1. User overrides from `shader_configs` / `cursor_shader_configs` map
2. Shader metadata defaults embedded in shader file
3. Global defaults from config

---

## Available Uniforms

Par-term provides comprehensive Shadertoy-compatible uniforms plus Ghostty-compatible cursor uniforms.

### Core Shadertoy Uniforms

These are fully compatible with Shadertoy shaders:

| Uniform | Type | Description |
|---------|------|-------------|
| `iResolution` | `vec3` | Viewport size: `xy` = pixels, `z` = pixel aspect ratio (usually 1.0) |
| `iTime` | `float` | Time in seconds since shader started (0.0 if animation disabled) |
| `iTimeDelta` | `float` | Time elapsed since last frame in seconds |
| `iFrame` | `float` | Frame counter (incremented each frame) |
| `iFrameRate` | `float` | Current frame rate in FPS |
| `iMouse` | `vec4` | Mouse state: `xy` = current position, `zw` = click position; sign indicates button state |
| `iDate` | `vec4` | Date/time: `x` = year, `y` = month (0-11), `z` = day (1-31), `w` = seconds since midnight |

### Window & Content Uniforms

Par-term specific uniforms for terminal integration:

| Uniform | Type | Description |
|---------|------|-------------|
| `iOpacity` | `float` | Window opacity setting (0.0-1.0) |
| `iTextOpacity` | `float` | Text opacity setting (0.0-1.0) |
| `iBrightness` | `float` | Shader brightness multiplier (0.05-1.0) |
| `iFullContentMode` | `float` | 1.0 = shader receives full terminal content; 0.0 = background only |
| `iTimeKeyPress` | `float` | Time when last key was pressed (same timebase as iTime). See [`keypress_pulse.glsl`](../shaders/keypress_pulse.glsl) for example. |

### Texture Channel Uniforms

Shadertoy-compatible texture channels:

| Uniform | Type | Description |
|---------|------|-------------|
| `iChannel0` | `sampler2D` | User texture channel 0. See [`rain.glsl`](../shaders/rain.glsl), [`bumped_sinusoidal_warp.glsl`](../shaders/bumped_sinusoidal_warp.glsl) for examples. |
| `iChannel1` | `sampler2D` | User texture channel 1 |
| `iChannel2` | `sampler2D` | User texture channel 2 |
| `iChannel3` | `sampler2D` | User texture channel 3 |
| `iChannel4` | `sampler2D` | Terminal content texture (par-term specific) |
| `iChannelResolution[0]` | `vec4` | Channel 0 resolution `[width, height, 1.0, 0.0]` |
| `iChannelResolution[1]` | `vec4` | Channel 1 resolution |
| `iChannelResolution[2]` | `vec4` | Channel 2 resolution |
| `iChannelResolution[3]` | `vec4` | Channel 3 resolution |
| `iChannelResolution[4]` | `vec4` | Channel 4 (terminal) resolution |
| `iCubemap` | `samplerCube` | Cubemap texture for environment mapping. See [`cubemap-skybox.glsl`](../shaders/cubemap-skybox.glsl) for example. |
| `iCubemapResolution` | `vec4` | Cubemap face size `[size, size, 1.0, 0.0]` |

### Cursor Uniforms

Ghostty-compatible cursor tracking uniforms (available in both background and cursor shaders). See [`cursor_trail.glsl`](../shaders/cursor_trail.glsl) and [`cursor_glow.glsl`](../shaders/cursor_glow.glsl) for simple examples.

| Uniform | Type | Description |
|---------|------|-------------|
| `iCurrentCursor` | `vec4` | Current cursor: `xy` = position (top-left, pixels), `zw` = cell size (pixels) |
| `iPreviousCursor` | `vec4` | Previous cursor: `xy` = position, `zw` = cell size |
| `iCurrentCursorColor` | `vec4` | Current cursor RGBA color (with blink opacity in alpha, 0.0-1.0) |
| `iPreviousCursorColor` | `vec4` | Previous cursor RGBA color |
| `iTimeCursorChange` | `float` | Time when cursor last moved (same timebase as iTime) |

**Cursor position details:**
- `iCurrentCursor.xy` is the top-left corner of the cursor cell in pixels
- `iCurrentCursor.zw` is the cell width and height in pixels
- To get cursor center: `iCurrentCursor.xy + iCurrentCursor.zw * 0.5`

### Cursor Shader Configuration Uniforms

These uniforms pass cursor shader configuration values to the shader:

| Uniform | Type | Description |
|---------|------|-------------|
| `iCursorTrailDuration` | `float` | Trail duration in seconds (from config) |
| `iCursorGlowRadius` | `float` | Glow radius in pixels (from config) |
| `iCursorGlowIntensity` | `float` | Glow intensity 0.0-1.0 (from config) |
| `iCursorShaderColor` | `vec4` | User-configured cursor color `[R, G, B, 1.0]` (0.0-1.0 normalized) |

---

## Creating Custom Shaders

### Basic Structure

Every shader must define a `mainImage` function:

```glsl
void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    // Normalize coordinates to 0-1 range
    vec2 uv = fragCoord / iResolution.xy;

    // Sample terminal content (iChannel4 in par-term)
    vec4 terminal = texture(iChannel4, uv);

    // Apply your effect
    vec3 color = terminal.rgb;

    // Output with alpha (1.0 = opaque)
    fragColor = vec4(color, 1.0);
}
```

### Shader Modes

**Background-Only Mode** (default, `custom_shader_full_content: false`):
- Shader output is used as background
- Terminal text is composited on top, remaining sharp
- Best for animated backgrounds and non-distorting effects

**Full Content Mode** (`custom_shader_full_content: true`):
- Shader receives full terminal content via `iChannel4`
- Shader can distort, warp, or transform text
- Required for CRT curvature, underwater distortion, etc.
- See [`crt.glsl`](../shaders/crt.glsl), [`bloom.glsl`](../shaders/bloom.glsl), [`dither.glsl`](../shaders/dither.glsl) for examples

### Shader Metadata Format

Shaders can include embedded configuration via YAML block comment at the start:

```glsl
/*! par-term shader metadata
name: My Custom Shader
author: Your Name
description: Brief description of what this shader does
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.8
  text_opacity: 0.9
  full_content: false
  channel0: textures/noise.png
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: true
  use_background_as_channel0: false
*/
```

All metadata fields are optional. Values that are `null` fall through to global defaults.

### Porting Shadertoy Shaders

Par-term is fully Shadertoy compatible. When adapting shaders:

1. **Terminal content is on iChannel4**: Use `texture(iChannel4, uv)` to sample terminal content. iChannel0-3 are available for user textures (same as Shadertoy)
2. **Y-axis matches Shadertoy**: No modifications needed - fragCoord.y=0 at bottom, same as Shadertoy
3. **iMouse is vec4**: Full Shadertoy compatibility (xy=current position, zw=click position)
4. **mat2(vec4) construction**: May need to expand to `mat2(v.x, v.y, v.z, v.w)` for GLSL 450 compatibility

---

## Examples

### Simple Background Gradient

A static diagonal gradient:

```glsl
void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord / iResolution.xy;

    // Diagonal gradient
    float t = (uv.x + uv.y) * 0.5;

    // Dark blue to dark purple
    vec3 color1 = vec3(0.05, 0.05, 0.15);
    vec3 color2 = vec3(0.15, 0.05, 0.15);

    vec3 bg = mix(color1, color2, t);

    fragColor = vec4(bg, 1.0);
}
```

### Animated Background

Pulsing color effect:

```glsl
void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord / iResolution.xy;

    // Animated pulse
    float pulse = sin(iTime * 2.0) * 0.5 + 0.5;

    // Dark base with subtle color variation
    vec3 color = vec3(0.05, 0.05, 0.1);
    color += vec3(0.02, 0.0, 0.05) * pulse;

    // Add radial gradient from center
    float dist = length(uv - 0.5);
    color *= 1.0 - dist * 0.5;

    fragColor = vec4(color, 1.0);
}
```

### Custom Cursor Trail

Simple fading trail effect:

```glsl
void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord / iResolution.xy;

    // Sample terminal content
    vec4 terminal = texture(iChannel4, uv);

    // Get cursor center
    vec2 cursorCenter = iCurrentCursor.xy + iCurrentCursor.zw * 0.5;
    vec2 prevCenter = iPreviousCursor.xy + iPreviousCursor.zw * 0.5;

    // Distance from cursor
    float dist = length(fragCoord - cursorCenter);

    // Glow falloff using config values
    float glow = 1.0 - smoothstep(0.0, iCursorGlowRadius, dist);
    glow = pow(glow, 2.0) * iCursorGlowIntensity;

    // Blend glow with cursor color
    vec3 color = terminal.rgb + iCursorShaderColor.rgb * glow;

    fragColor = vec4(color, terminal.a);
}
```

### Key Press Pulse Effect

Visual feedback on keystrokes:

```glsl
void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord / iResolution.xy;
    vec4 terminal = texture(iChannel4, uv);

    // Calculate time since last key press
    float timeSinceKey = iTime - iTimeKeyPress;

    // Exponential decay for smooth falloff
    float pulse = exp(-timeSinceKey * 6.0);
    pulse *= step(timeSinceKey, 1.0);  // Only show for 1 second

    // Screen-wide brightness flash
    vec3 color = terminal.rgb * (1.0 + pulse * 0.15);

    fragColor = vec4(color, terminal.a);
}
```

### Using Cubemap Environment

Skybox with animated rotation:

```glsl
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution.xy) / min(iResolution.x, iResolution.y);

    // Create ray direction from camera
    vec3 rayDir = normalize(vec3(uv.x, uv.y, -1.0));

    // Rotate over time for animation
    float angle = iTime * 0.2;
    float c = cos(angle), s = sin(angle);
    rayDir = vec3(rayDir.x * c - rayDir.z * s, rayDir.y, rayDir.x * s + rayDir.z * c);

    // Sample cubemap
    vec4 sky = texture(iCubemap, rayDir);

    // Blend with terminal content
    vec4 terminal = texture(iChannel4, fragCoord / iResolution.xy);
    fragColor = terminal.a > 0.01 ? terminal : sky;
}
```

### Using Channel Textures

Blend a noise texture with the terminal content:

```glsl
void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord / iResolution.xy;

    // Sample terminal content (iChannel4)
    vec4 terminal = texture(iChannel4, uv);

    // Sample noise texture (iChannel0 - Shadertoy compatible)
    // Configure via: custom_shader_channel0: "path/to/noise.png"
    vec4 noise = texture(iChannel0, uv * 2.0);  // Scale UV for tiling

    // Get texture dimensions if needed
    vec2 noiseSize = iChannelResolution[0].xy;

    // Blend noise with terminal (subtle overlay)
    vec3 color = terminal.rgb + noise.rgb * 0.1;

    fragColor = vec4(color, terminal.a);
}
```

---

## Troubleshooting

### Shader Not Loading

**Symptom:** No visual effect after enabling shader

**Solutions:**
- Verify file exists in `~/.config/par-term/shaders/`
- Check `custom_shader_enabled: true` in config
- Press `F5` to reload configuration
- Check terminal output for compilation errors

### Black or White Screen

**Symptom:** Terminal content not visible

**Solutions:**
- Ensure `fragColor.a = 1.0` for opaque output
- Verify UV coordinates are in 0.0-1.0 range
- Check `texture(iChannel4, uv)` sampling for terminal content

### Text Hard to Read

**Symptom:** Text blurry or distorted, or shader background is too bright

**Solutions:**
- Use `custom_shader_full_content: false` (background-only mode)
- Increase `custom_shader_text_opacity`
- Lower `custom_shader_brightness` (e.g., 0.3-0.5) to dim bright shader backgrounds
- Reduce effect intensity in shader

### Low Frame Rate

**Symptom:** Stuttering or choppy animation

**Solutions:**
- Reduce shader complexity (fewer loops, simpler math)
- Lower `custom_shader_animation_speed`
- Disable animation: `custom_shader_animation: false`

### Default Cursor Showing Through Cursor Shader

**Symptom:** The default block/beam cursor is visible behind or through your cursor shader effect

**Solution:**
- Set `cursor_shader_hides_cursor: true` in config
- This tells the renderer to skip drawing the default cursor when a cursor shader is active
- Recommended for shaders that fully replace the cursor (e.g., `cursor_pacman`, `cursor_orbit`, `cursor_water_tank`)

### Compilation Errors

**Common GLSL issues:**
- Use `texture()` not `texture2D()`
- Declare constants with `const` keyword
- Arrays: `vec3[N] arr = vec3[N](...)`
- No `#version` directive needed (added automatically)

### Debugging Tips

- Transpiled WGSL is written to `/tmp/par_term_<shader_name>_shader.wgsl`
- Wrapped GLSL is written to `/tmp/par_term_debug_wrapped.glsl` (last shader only)
- Enable `shader_hot_reload: true` for faster iteration

---

## Related Documentation

- [Included Shaders](SHADERS.md) - Complete list of all available shaders
- [Compositor Details](COMPOSITOR.md) - Deep dive into the rendering pipeline
- [README.md](../README.md) - Configuration reference
- [Shadertoy](https://www.shadertoy.com) - Shader inspiration and examples
- [Ghostty](https://ghostty.org/) - Compatible shader format reference
