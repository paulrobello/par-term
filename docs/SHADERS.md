# Included Shaders

Par-term includes 73 ready-to-use GLSL shaders (61 background + 12 cursor). This document lists all available shaders organized by category.

**[View Shader Gallery with Screenshots](https://paulrobello.github.io/par-term/)**

For information on how to use and configure shaders, see [CUSTOM_SHADERS.md](CUSTOM_SHADERS.md).

## Table of Contents

- [Background Shaders](#background-shaders)
  - [Terminal-Aware & Productivity](#terminal-aware--productivity)
  - [Animated Backgrounds](#animated-backgrounds)
  - [Abstract & Procedural](#abstract--procedural)
  - [CRT & Retro Effects](#crt--retro-effects)
  - [Distortion Effects](#distortion-effects)
  - [Lighting & Glow](#lighting--glow)
  - [Cubemap-Based](#cubemap-based)
- [Cursor Shaders](#cursor-shaders)
- [Included Textures](#included-textures)
  - [Included Texture Packs](#included-texture-packs)

---

## Background Shaders

Background shaders are full-screen post-processing effects applied to the terminal. Configure them with `custom_shader` in your config.

### Terminal-Aware & Productivity

| Shader | Description |
|--------|-------------|
| `progress_reactive_theme.glsl` | Calm ambient background that reacts to `iProgress` with normal glow, amber warning pulse, red error edge bloom, and indeterminate stripes. |
| `command_state_backdrop.glsl` | Uses `iCommand` to briefly tint after command start, success, or failure. |
| `pane_focus_regions.glsl` | Uses `iFocusedPane` in full-content mode to frame the active split pane and dim inactive terminal content. |
| `scrollback_parallax.glsl` | Uses `iScroll` to add depth fog and timeline bands as you move through scrollback. |
| `blueprint_grid.glsl` | CAD-style grid that brightens around the cursor and active progress bars. |
| `build_reactor.glsl` | Progress-aware reactor/core glow that charges with `iProgress.y` and vents on warnings/errors. |
| `matrix_rain_2.glsl` | Less distracting full-content Matrix rain that avoids dense terminal text and reacts to typing bursts. |
| `low_power_ambience.glsl` | Static-to-ultra-slow ambience intended for reduced frame cadence and battery-friendly sessions. |

### Animated Backgrounds

| Shader | Description |
|--------|-------------|
| `starfield.glsl` | Animated starfield with parallax layers behind terminal text |
| `starfield-colors.glsl` | Colorful variant of the starfield effect with rainbow stars |
| `galaxy.glsl` | Swirling galaxy background animation with cosmic dust |
| `clouds.glsl` | Animated procedural clouds with blue sky gradient |
| `rain.glsl` | Rain on glass effect with fog and water droplets (Heartfelt port). **Example of:** [`iChannel0`](CUSTOM_SHADERS.md#texture-channel-uniforms) usage. |
| `rain-glass.glsl` | Rain on glass with a procedural dark nebula background — no texture required. Configurable color palette, noise parameters, and fog settings via `#define` knobs at the top of the file. |
| `jellyfish.glsl` | Animated procedural jellyfish with dark water background, neon blue/purple bioluminescent jellyfish, depth layers, and floating particles. Supports optional `iChannel0` background texture. |
| `just-snow.glsl` | Falling snow particles overlay |
| `fireworks.glsl` | Fireworks particle explosion animation |
| `fireworks-rockets.glsl` | Fireworks with rocket trails before explosion |
| `sparks-from-fire.glsl` | Rising fire sparks effect |
| `water.glsl` | Water ripple/wave effect with caustics |
| `underwater.glsl` | Underwater caustics effect with subtle distortion |
| `aurora_terminal.glsl` | Soft northern-light ribbons with color controls, slow motion, and readability-first defaults |

### Abstract & Procedural

| Shader | Description |
|--------|-------------|
| `animated-gradient-shader.glsl` | Smooth animated color gradient background |
| `gradient-background.glsl` | Static diagonal gradient background |
| `ink_wash.glsl` | Low-contrast paper and ink diffusion using procedural noise for a calm writing environment |
| `solarized_nebula.glsl` | Solarized-inspired nebula that blends active terminal background color with configurable accents |
| `universe-within.glsl` | Mystical neural network with pulsing nodes and connecting lines |
| `singularity.glsl` | Whirling blackhole with red/blue accretion disk and spiraling waves |
| `convergence.glsl` | Two swirling voronoi patterns (teal/red) split by animated lightning bolt |
| `sin-interference.glsl` | Sine wave interference patterns |
| `gyroid.glsl` | Raymarched gyroid tunnel with colorful lighting and reflections |
| `dodecagon-pattern.glsl` | Raymarched dodecagon tile pattern with BRDF metallic frames. Uses `iChannel0` and `iCubemap`. |
| `happy_fractal.glsl` | Raymarched fractal landscape with animated happy face and rainbow trail |
| `cubes.glsl` | 3D rotating cubes background |
| `gears-and-belts.glsl` | Mechanical gears animation |
| `inside-the-matrix.glsl` | Matrix-style cascading green code effect |
| `cineShader-Lava.glsl` | Flowing lava/plasma effect (Shadertoy port) |
| `arcane-portal.glsl` | Mystical portal animation with raymarching (chronos port) |
| `bumped_sinusoidal_warp.glsl` | Metallic sinusoidal warp with bump-mapped lighting (Shane port). **Example of:** `iChannel0` texture. |
| `circuit-3d.glsl` | Raymarched circuit-board structure with configurable march/detail controls. |
| `infinite-zoom-1.glsl` | Burning Ship fractal infinite zoom with full-content terminal blending controls. |
| `infinite-zoom-2.glsl` | Multibrot z^3 infinite zoom with warm amber/magenta palette and terminal blend controls. |
| `infinite-zoom-3.glsl` | Julia set infinite zoom with configurable center, constant, and glow controls. |
| `magic-ball.glsl` | Raymarched glowing magic sphere with rotation, glow, and mouse-control settings. |

### CRT & Retro Effects

These shaders typically use [`full_content: true`](CUSTOM_SHADERS.md#shader-modes) to manipulate terminal text.

| Shader | Description |
|--------|-------------|
| `crt.glsl` | Full CRT simulation with curvature, scanlines, and phosphor mask. **Example of:** `full_content` mode. |
| `retro-terminal.glsl` | Classic green-tint terminal with scanlines |
| `bloom.glsl` | Soft glow/bloom effect around bright text (golden spiral sampling). **Example of:** `full_content` mode. |
| `dither.glsl` | Ordered dithering effect using 4x4 Bayer matrix. **Example of:** `full_content` mode. |
| `glitchy.glsl` | Digital glitch/corruption effect. Uses `iChannel0`. |
| `glow-rgbsplit-twitchy.glsl` | RGB split with glow and glitch effects. Uses `iChannel0`. |

### Distortion Effects

| Shader | Description |
|--------|-------------|
| `drunkard.glsl` | Wobbly distortion effect using Perlin noise (moni-dz, CC BY-NC-SA 4.0) |
| `glass-sphere-bounce.glsl` | Bouncing glass sphere that refracts a background image (or fallback gradient) with configurable size, bounce speed, IOR, chromatic aberration, and optional thin-film iridescence. Uses `iChannel0` for background texture. |

### Lighting & Glow

| Shader | Description |
|--------|-------------|
| `spotlight.glsl` | Moving spotlight/vignette effect. Uses `iChannel0` for optional background. |
| `industrial1.glsl` | Optimized industrial raymarched structure with configurable glow, relief, and accent colors. |
| `keypress_ring_fullcontent.glsl` | Full-content cursor-centered ring and flash on each keystroke. **Example of:** [`iTimeKeyPress`](CUSTOM_SHADERS.md#window--content-uniforms) uniform. |
| `debug-coords.glsl` | Minimal coordinate-gradient shader useful for verifying Shadertoy `fragCoord` orientation. |

### Cubemap-Based

These shaders use cubemap textures for environment mapping effects. **Example of:** [`iCubemap`](CUSTOM_SHADERS.md#texture-channel-uniforms) uniform.

| Shader | Description |
|--------|-------------|
| `cubemap-atmospheric-sky.glsl` | Low-distraction atmospheric cubemap gradient using `textures/cubemaps/env-outside`. |
| `cubemap-metallic-ambience.glsl` | Slow metallic reflections with low contrast using `textures/cubemaps/env-outside`. |
| `cubemap-neon-room.glsl` | Subdued neon room ambience using `textures/cubemaps/env-test`. |
| `cubemap-skybox.glsl` | Rotating cubemap skybox environment. **Example of:** `iCubemap` usage. |
| `cubemap-test.glsl` | Simple cubemap test/demo with mouse-controlled pitch |

---

## Cursor Shaders

Cursor shaders create visual effects that follow your cursor position. Configure them with `cursor_shader` in your config.

These shaders demonstrate usage of [cursor uniforms](CUSTOM_SHADERS.md#cursor-uniforms) like `iCurrentCursor`, `iPreviousCursor`, and `iCurrentCursorColor`.

| Shader | Description |
|--------|-------------|
| `cursor_glow.glsl` | Soft radial glow around cursor position. **Simple example** of cursor uniforms. |
| `cursor_trail.glsl` | Persistent fading trail from previous to current cursor position. **Simple example** of cursor uniforms. |
| `cursor_sweep.glsl` | Smooth trailing sweep effect when cursor moves |
| `cursor_blaze.glsl` | Combined glow + trail effect (fire/blaze aesthetic) |
| `cursor_ripple.glsl` | Expanding ripple waves emanating from cursor position |
| `cursor_ripple_rectangle.glsl` | Rectangular ripple variant |
| `cursor_sonic_boom.glsl` | Expanding shockwave effect from cursor |
| `cursor_rectangle_boom.glsl` | Rectangular expanding shockwave |
| `cursor_warp.glsl` | Space-time warp distortion around cursor |
| `cursor_orbit.glsl` | Particles orbiting around cursor position with fading trail |
| `cursor_pacman.glsl` | Animated Pac-Man character at cursor (faces movement direction) |
| `cursor_water_tank.glsl` | Water tank at cursor with sloshing liquid that tilts based on movement |

**Tip:** For shaders that fully replace the cursor (like `cursor_pacman`, `cursor_orbit`, or `cursor_water_tank`), set `cursor_shader_hides_cursor: true` in your config.

---

## Included Textures

Par-term includes textures in `shaders/textures/` for use with shader channels.

### Cubemaps

Located in `shaders/textures/cubemaps/`:

| Cubemap Prefix | Description |
|----------------|-------------|
| `env-outside` | Outdoor environment cubemap |
| `env-test` | Test environment cubemap |

### Material Textures

| Texture | Description |
|---------|-------------|
| `metalic1.jpg` | Metallic surface texture for bump mapping |

### Included Texture Packs

Texture packs are installed under `shaders/textures/packs/` and tracked by `shaders/manifest.json`.

| Pack | Path | Included texture |
|------|------|------------------|
| Noise | `textures/packs/noise/` | `soft-value-128.png` |
| Gradients | `textures/packs/gradients/` | `deep-violet-128.png` |
| Paper | `textures/packs/paper/` | `warm-paper-128.png` |
| Metal | `textures/packs/metal/` | `brushed-metal-128.png` |
| Starfields | `textures/packs/starfields/` | `dim-stars-128.png` |

### Wallpapers

Located in `shaders/textures/wallpaper/`:

| Texture | Description |
|---------|-------------|
| `DNA.png` | DNA helix pattern |
| `HexBalls.png` | Hexagonal ball pattern |
| `EarthProcedural.png` | Procedural Earth-like texture |
| `Bulbs.png` | Light bulb pattern |
| `SciFi1.png` | Sci-fi aesthetic texture |
| `BarsAndOrbs.png` | Bars and spheres pattern |
| `Abstract1.png` | Abstract artistic texture |
| `MagicMushrooms.png` | Mushroom pattern |

---

## Shader Credits

Many shaders are ports or adaptations from the shader community:

- **crt.glsl**: Timothy Lottes (public domain), adapted by Qwerasd
- **rain.glsl**: Martijn Steinrucken aka BigWings (Heartfelt)
- **drunkard.glsl**: moni-dz (CC BY-NC-SA 4.0)
- **arcane-portal.glsl**: chronos (Shadertoy)
- **bumped_sinusoidal_warp.glsl**: Shane (Shadertoy)
- **animated-gradient-shader.glsl**: unkn0wncode (GitHub)

---

## Related Documentation

- [Custom Shaders Guide](CUSTOM_SHADERS.md) - Configuration, uniforms, and creating custom shaders
- [Compositor Details](COMPOSITOR.md) - Deep dive into the rendering pipeline
- [Shadertoy](https://www.shadertoy.com) - Shader inspiration and examples
- [Ghostty](https://ghostty.org/) - Compatible shader format reference
