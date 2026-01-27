# Custom Shaders Guide

Par-term supports custom GLSL shaders for background effects and post-processing, compatible with Ghostty and Shadertoy shader formats. This guide covers installing included shaders and creating your own.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Installing Shaders](#installing-shaders)
  - [From the Included Collection](#from-the-included-collection)
  - [Shader Directory Location](#shader-directory-location)
- [Included Shaders](#included-shaders)
  - [Background Effects](#background-effects)
  - [CRT and Retro Effects](#crt-and-retro-effects)
  - [Cursor Effects](#cursor-effects)
- [Configuration](#configuration)
  - [Background Shaders](#background-shaders)
  - [Channel Textures](#channel-textures)
  - [Cursor Shaders](#cursor-shaders)
- [Creating Custom Shaders](#creating-custom-shaders)
  - [Basic Structure](#basic-structure)
  - [Available Uniforms](#available-uniforms)
  - [Cursor Shader Uniforms](#cursor-shader-uniforms)
  - [Shader Modes](#shader-modes)
- [Examples](#examples)
  - [Simple Background Gradient](#simple-background-gradient)
  - [Animated Background](#animated-background)
  - [Custom Cursor Trail](#custom-cursor-trail)
  - [Using Channel Textures](#using-channel-textures)
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

Par-term includes a collection of 40+ ready-to-use shaders in the `shaders/` directory. Copy any shader to your configuration directory to use it:

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

## Included Shaders

### Background Effects

| Shader | Description |
|--------|-------------|
| `starfield.glsl` | Animated starfield with parallax layers behind terminal text |
| `starfield-colors.glsl` | Colorful variant of the starfield effect |
| `galaxy.glsl` | Swirling galaxy background animation |
| `animated-gradient-shader.glsl` | Smoothly animated color gradient background |
| `gradient-background.glsl` | Static diagonal gradient background |
| `underwater.glsl` | Caustic water effect with subtle distortion |
| `water.glsl` | Simpler water caustic effect |
| `water-bg.glsl` | Water effect optimized for background use |
| `just-snow.glsl` | Falling snow particles overlay |
| `fireworks.glsl` | Fireworks particle effect |
| `fireworks-rockets.glsl` | Fireworks with rocket trails |
| `sparks-from-fire.glsl` | Rising fire sparks effect |
| `cubemap-skybox.glsl` | Cubemap environment with animated rotation |
| `smoke-and-ghost.glsl` | Ethereal smoke and ghosting effect |
| `cubes.glsl` | 3D rotating cubes background |
| `gears-and-belts.glsl` | Mechanical gears animation |
| `inside-the-matrix.glsl` | Matrix-style falling code effect |
| `cineShader-Lava.glsl` | Flowing lava/plasma effect |
| `sin-interference.glsl` | Wave interference pattern |
| `happy_fractal.glsl` | Raymarched fractal landscape with animated happy face and rainbow trail |
| `clouds.glsl` | Animated procedural clouds with blue sky gradient |
| `bumped_sinusoidal_warp.glsl` | Bump-mapped sinusoidal warp effect with point lighting (uses iChannel1 texture) |
| `gyroid.glsl` | Raymarched gyroid tunnel with colorful lighting and reflections |
| `dodecagon-pattern.glsl` | Raymarched dodecagon tile pattern with BRDF metallic frames (uses iChannel1 texture) |
| `convergence.glsl` | Two swirling voronoi patterns (teal/red) split by an animated lightning bolt |
| `singularity.glsl` | Whirling blackhole with red/blue accretion disk and spiraling waves |
| `universe-within.glsl` | Mystical neural network with pulsing nodes and connecting lines |

### CRT and Retro Effects

| Shader | Description |
|--------|-------------|
| `crt.glsl` | Full CRT simulation with curvature, scanlines, and phosphor mask |
| `bettercrt.glsl` | Simplified CRT effect with scanlines |
| `retro-terminal.glsl` | Classic green-tint terminal with scanlines |
| `in-game-crt.glsl` | Game-style CRT effect |
| `tft.glsl` | TFT/LCD subpixel simulation |
| `bloom.glsl` | Soft glow/bloom around bright text |
| `dither.glsl` | Retro dithering effect |
| `glitchy.glsl` | Digital glitch distortion |
| `glow-rgbsplit-twitchy.glsl` | RGB split with glow and glitch |
| `drunkard.glsl` | Wobbly distortion effect |
| `negative.glsl` | Simple color inversion |
| `spotlight.glsl` | Spotlight/vignette effect |

### Cursor Effects

Cursor shaders create visual effects that follow your cursor position. These use special uniforms for cursor tracking.

| Shader | Description |
|--------|-------------|
| `cursor_glow.glsl` | Soft radial glow around cursor |
| `cursor_sweep.glsl` | Smooth trailing sweep when cursor moves |
| `cursor_trail.glsl` | Persistent fading trail behind cursor |
| `cursor_warp.glsl` | Space warp effect emanating from cursor |
| `cursor_blaze.glsl` | Fire/blaze effect following cursor |
| `cursor_ripple.glsl` | Ripple waves emanating from cursor position |
| `cursor_ripple_rectangle.glsl` | Rectangular ripple variant |
| `cursor_sonic_boom.glsl` | Shockwave effect on cursor movement |
| `cursor_rectangle_boom.glsl` | Rectangular explosion effect |
| `cursor_pacman.glsl` | Animated Pac-Man cursor that faces movement direction |
| `cursor_orbit.glsl` | Ball with fading trail orbiting inside the cursor cell |

## Configuration

### Background Shaders

Configure background/post-processing shaders in your config file:

```yaml
# ~/.config/par-term/config.yaml

# Shader file name (in shaders/ directory)
custom_shader: "starfield.glsl"

# Enable/disable the shader
custom_shader_enabled: true

# Enable animation (updates iTime uniform each frame)
custom_shader_animation: true

# Animation speed multiplier (1.0 = normal, 0.5 = half speed)
custom_shader_animation_speed: 1.0

# Text opacity when shader is active (0.0 - 1.0)
custom_shader_text_opacity: 1.0

# Shader brightness (0.05 - 1.0, default 1.0)
# Dims the shader background to improve text readability
custom_shader_brightness: 0.5

# Full content mode: shader can distort/modify text
# false = text composited on top of shader output (recommended)
# true = shader receives and can modify terminal content
custom_shader_full_content: false
```

### Channel Textures

Par-term supports Shadertoy-compatible texture channels (iChannel0-3) for passing custom images to shaders. This enables effects like noise textures, normal maps, or any image-based input.

```yaml
# ~/.config/par-term/config.yaml

# Texture paths for shader channels (supports ~ for home directory)
custom_shader_channel0: "~/textures/noise.png"
custom_shader_channel1: "~/textures/metal.jpg"
custom_shader_channel2: null  # Not used
custom_shader_channel3: null  # Not used
```

**Notes:**
- `iChannel0-3` are user-defined texture inputs (Shadertoy compatible)
- `iChannel4` is the terminal content texture (par-term specific)
- Channels without a configured texture use a 1x1 transparent placeholder
- Supports common image formats: PNG, JPEG, BMP, etc.
- Textures can also be configured via Settings UI under "Shader Channel Textures"
- Sample textures are included in `shaders/textures/` directory

### Cubemap Textures

Par-term supports cubemap textures for environment mapping and skybox effects via the `iCubemap` uniform. Cubemaps consist of 6 face images that form a seamless cube.

```yaml
# ~/.config/par-term/config.yaml

# Path prefix for cubemap faces
# Expects 6 files: {prefix}-px.{ext}, -nx.{ext}, -py.{ext}, -ny.{ext}, -pz.{ext}, -nz.{ext}
# where {ext} is one of: png, jpg, jpeg, hdr
custom_shader_cubemap: "shaders/textures/cubemaps/env-outside"

# Enable cubemap sampling
custom_shader_cubemap_enabled: true
```

**Face naming convention:**
- `{prefix}-px.{ext}` - Positive X (+X, right)
- `{prefix}-nx.{ext}` - Negative X (-X, left)
- `{prefix}-py.{ext}` - Positive Y (+Y, top)
- `{prefix}-ny.{ext}` - Negative Y (-Y, bottom)
- `{prefix}-pz.{ext}` - Positive Z (+Z, front)
- `{prefix}-nz.{ext}` - Negative Z (-Z, back)

**Example usage in shader:**
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

**Notes:**
- HDR cubemaps (.hdr) are supported with automatic Rgba16Float conversion
- LDR cubemaps use Rgba8UnormSrgb format
- Sample cubemaps are included in `shaders/textures/cubemaps/`
- Use `iCubemapResolution.xy` for cubemap face dimensions

### Cursor Shaders

Cursor shaders are configured separately:

```yaml
# Cursor shader file name
cursor_shader: "cursor_glow.glsl"

# Enable/disable cursor shader
cursor_shader_enabled: true

# Animation controls
cursor_shader_animation: true
cursor_shader_animation_speed: 1.0

# Visibility controls
cursor_shader_hides_cursor: false          # Show normal cursor (set true to let shader fully replace it)
cursor_shader_disable_in_alt_screen: true  # Pause cursor shader in alt-screen TUIs (vim/less/htop)

# Cursor color (used by shaders via iCurrentCursorColor)
cursor_color: "#00ff00"
```

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

### Available Uniforms

Par-term provides Shadertoy-compatible uniforms:

| Uniform | Type | Description |
|---------|------|-------------|
| `iResolution` | `vec2` | Viewport size in pixels |
| `iTime` | `float` | Time in seconds since shader started |
| `iTimeDelta` | `float` | Time since last frame |
| `iFrame` | `float` | Frame counter |
| `iFrameRate` | `float` | Current FPS |
| `iMouse` | `vec4` | Mouse position and click state |
| `iDate` | `vec4` | Year, month (0-11), day (1-31), seconds since midnight |
| `iChannel0` | `sampler2D` | User texture channel 0 (Shadertoy compatible) |
| `iChannel1` | `sampler2D` | User texture channel 1 (Shadertoy compatible) |
| `iChannel2` | `sampler2D` | User texture channel 2 (Shadertoy compatible) |
| `iChannel3` | `sampler2D` | User texture channel 3 (Shadertoy compatible) |
| `iChannel4` | `sampler2D` | Terminal content texture |
| `iChannelResolution[n]` | `vec3` | Resolution of channel n (width, height, 1.0) |
| `iCubemap` | `samplerCube` | Cubemap texture for environment mapping |
| `iCubemapResolution` | `vec4` | Cubemap face size (size, size, 1.0, 0.0) |
| `iOpacity` | `float` | Window opacity setting |
| `iTextOpacity` | `float` | Text opacity setting |
| `iBrightness` | `float` | Shader brightness multiplier (0.05-1.0) |

### Cursor Shader Uniforms

Cursor shaders have additional uniforms:

| Uniform | Type | Description |
|---------|------|-------------|
| `iCurrentCursor` | `vec4` | Current cursor: `xy` = position, `zw` = cell size |
| `iPreviousCursor` | `vec4` | Previous cursor position and size |
| `iCurrentCursorColor` | `vec4` | Cursor color (RGB) with blink opacity in alpha (0.0-1.0, animated by `cursor_blink` settings) |
| `iTimeCursorChange` | `float` | Time when cursor last moved |

**Cursor position details:**
- `iCurrentCursor.xy` is the top-left corner of the cursor cell
- `iCurrentCursor.zw` is the cell width and height in pixels
- To get cursor center: `iCurrentCursor.xy + iCurrentCursor.zw * 0.5`

### Shader Modes

**Background-Only Mode** (default, `custom_shader_full_content: false`):
- Shader output is used as background
- Terminal text is composited on top, remaining sharp
- Best for animated backgrounds and non-distorting effects

**Full Content Mode** (`custom_shader_full_content: true`):
- Shader receives full terminal content via `iChannel4`
- Shader can distort, warp, or transform text
- Required for CRT curvature, underwater distortion, etc.

### Porting Shadertoy Shaders

Par-term is fully Shadertoy compatible. When adapting shaders:

1. **Terminal content is on iChannel4**: Use `texture(iChannel4, uv)` to sample terminal content. iChannel0-3 are available for user textures (same as Shadertoy)
2. **Y-axis matches Shadertoy**: No modifications needed - fragCoord.y=0 at bottom, same as Shadertoy
3. **iMouse is vec4**: Full Shadertoy compatibility (xy=current position, zw=click position)
4. **mat2(vec4) construction**: May need to expand to `mat2(v.x, v.y, v.z, v.w)` for GLSL 450 compatibility

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
const float TRAIL_INTENSITY = 0.5;
const float TRAIL_RADIUS = 50.0;

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord / iResolution.xy;

    // Sample terminal content
    vec4 terminal = texture(iChannel4, uv);

    // Get cursor center
    vec2 cursorCenter = iCurrentCursor.xy + iCurrentCursor.zw * 0.5;
    vec2 prevCenter = iPreviousCursor.xy + iPreviousCursor.zw * 0.5;

    // Distance from cursor path
    vec2 toCursor = fragCoord - cursorCenter;
    float dist = length(toCursor);

    // Glow falloff
    float glow = 1.0 - smoothstep(0.0, TRAIL_RADIUS, dist);
    glow = pow(glow, 2.0) * TRAIL_INTENSITY;

    // Blend glow with cursor color
    vec3 color = terminal.rgb + iCurrentCursorColor.rgb * glow;

    fragColor = vec4(color, terminal.a);
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

**Configuration for the above shader:**
```yaml
custom_shader: "my_noise_shader.glsl"
custom_shader_enabled: true
custom_shader_channel0: "~/textures/noise.png"
```

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
- Recommended for shaders that fully replace the cursor (e.g., `cursor_pacman`, `cursor_orbit`)

### Compilation Errors

**Common GLSL issues:**
- Use `texture()` not `texture2D()`
- Declare constants with `const` keyword
- Arrays: `vec3[N] arr = vec3[N](...)`
- No `#version` directive needed (added automatically)

## Related Documentation

- [Compositor Details](COMPOSITOR.md) - Deep dive into the rendering pipeline
- [README.md](../README.md) - Configuration reference
- [Shadertoy](https://www.shadertoy.com) - Shader inspiration and examples
- [Ghostty](https://ghostty.org/) - Compatible shader format reference
