//! Context string builder for the AI Inspector shader assistant.
//!
//! [`build_shader_context`] assembles the structured prompt block injected into
//! ACP agent messages when shader-related questions are detected. The block
//! describes the current shader state, available shaders, debug file paths,
//! available uniforms, a minimal template, and how to apply changes.

use std::path::Path;

use crate::config::Config;

use super::helpers::{classify_shaders, scan_shaders};

/// Build a shader context block to inject into agent prompts.
///
/// The returned string contains sections on the current shader state,
/// available shaders, debug file paths, available uniforms, a minimal
/// shader template, and instructions for applying changes.
pub fn build_shader_context(config: &Config) -> String {
    let shaders_dir = Config::shaders_dir();

    let available = scan_shaders(&shaders_dir);
    let (bg_shaders, cursor_shaders) = classify_shaders(&available);

    let mut ctx = String::with_capacity(2048);

    ctx.push_str("[Shader Assistant Context]\n");
    ctx.push_str(
        "Use this block as reference context. Treat [Observation] sections as state,\n\
         [Constraint] sections as hard rules, and [Instruction] sections as guidance.\n\n",
    );

    // ---- Current Shader State ----
    ctx.push_str("## [Observation] Current Shader State\n");

    // Background shader
    if let Some(ref name) = config.shader.custom_shader {
        ctx.push_str(&format!("- Background shader: `{name}`"));
        if config.shader.custom_shader_enabled {
            ctx.push_str(" (enabled)\n");
        } else {
            ctx.push_str(" (disabled)\n");
        }
        if config.shader.custom_shader_enabled {
            ctx.push_str(&format!(
                "  - animation_speed: {}\n",
                config.shader.custom_shader_animation_speed
            ));
            ctx.push_str(&format!(
                "  - brightness: {}\n",
                config.shader.custom_shader_brightness
            ));
            ctx.push_str(&format!(
                "  - text_opacity: {}\n",
                config.shader.custom_shader_text_opacity
            ));
        }
    } else {
        ctx.push_str("- Background shader: none\n");
    }

    // Cursor shader
    if let Some(ref name) = config.shader.cursor_shader {
        ctx.push_str(&format!("- Cursor shader: `{name}`"));
        if config.shader.cursor_shader_enabled {
            ctx.push_str(" (enabled)\n");
        } else {
            ctx.push_str(" (disabled)\n");
        }
        if config.shader.cursor_shader_enabled {
            ctx.push_str(&format!(
                "  - animation_speed: {}\n",
                config.shader.cursor_shader_animation_speed
            ));
            ctx.push_str(&format!(
                "  - glow_radius: {}\n",
                config.shader.cursor_shader_glow_radius
            ));
            ctx.push_str(&format!(
                "  - glow_intensity: {}\n",
                config.shader.cursor_shader_glow_intensity
            ));
        }
    } else {
        ctx.push_str("- Cursor shader: none\n");
    }

    ctx.push('\n');

    // ---- Available Shaders ----
    ctx.push_str("## [Observation] Available Shaders\n");

    if bg_shaders.is_empty() && cursor_shaders.is_empty() {
        ctx.push_str("No shaders found in the shaders directory.\n");
    } else {
        if !bg_shaders.is_empty() {
            ctx.push_str("Background shaders:\n");
            for s in &bg_shaders {
                ctx.push_str(&format!("  - {s}\n"));
            }
        }
        if !cursor_shaders.is_empty() {
            ctx.push_str("Cursor shaders:\n");
            for s in &cursor_shaders {
                ctx.push_str(&format!("  - {s}\n"));
            }
        }
    }

    ctx.push('\n');

    // ---- Debug Files ----
    ctx.push_str("## [Observation] Debug Files\n");

    if let Some(ref name) = config.shader.custom_shader {
        let stem = Path::new(name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| name.clone());
        ctx.push_str(&format!(
            "- Transpiled WGSL: `/tmp/par_term_{stem}_shader.wgsl`\n"
        ));
    }
    if let Some(ref name) = config.shader.cursor_shader {
        let stem = Path::new(name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| name.clone());
        ctx.push_str(&format!(
            "- Cursor transpiled WGSL: `/tmp/par_term_{stem}_shader.wgsl`\n"
        ));
    }
    ctx.push_str("- Wrapped GLSL (last shader): `/tmp/par_term_debug_wrapped.glsl`\n");
    ctx.push_str(
        "- Live diagnostics: call `shader_diagnostics` for active shader state, last compile/reload errors, and debug paths.\n",
    );

    ctx.push('\n');

    // ---- Available Uniforms ----
    ctx.push_str("## [Observation] Available Uniforms\n");
    ctx.push_str("Common (all shaders):\n");
    ctx.push_str("  - `iTime` (float) - elapsed time in seconds\n");
    ctx.push_str(
        "  - `iResolution` (vec3) - viewport resolution in pixels (xy) and aspect ratio (z)\n",
    );
    ctx.push_str(
        "  - `iMouse` (vec4) - mouse position: xy=current, zw=click (Shadertoy-compatible)\n",
    );
    ctx.push_str("  - `iChannel0`..`iChannel3` (sampler2D) - user texture channels\n");
    ctx.push_str("  - `iChannel4` (sampler2D) - terminal content texture (par-term specific)\n");
    ctx.push_str("  - `iChannelResolution[0..4]` (vec3) - per-channel texture sizes\n");
    ctx.push_str(
        "  - `iProgress` (vec4) - progress state [state, percent, isActive, activeCount]\n",
    );
    ctx.push_str("  - `iCommand` (vec4) - command state [state, exitCode, eventTime, running]\n");
    ctx.push_str("  - `iFocusedPane` (vec4) - focused pane bounds [bottomLeftX, bottomLeftY, width, height]\n");
    ctx.push_str("  - `iScroll` (vec4) - scrollback [offset, visibleLines, scrollbackLines, normalizedDepth]\n");
    ctx.push_str("Cursor shader extras:\n");
    ctx.push_str("  - `iCurrentCursor` (vec4) - current cursor (xy=top-left px, zw=size px)\n");
    ctx.push_str("  - `iPreviousCursor` (vec4) - previous cursor (xy=top-left px, zw=size px)\n");
    ctx.push_str(
        "  - `iCurrentCursorColor` (vec4) - current cursor RGBA (alpha includes blink opacity)\n",
    );
    ctx.push_str("  - `iPreviousCursorColor` (vec4) - previous cursor RGBA\n");
    ctx.push_str("  - `iTimeCursorChange` (float) - time since last cursor move\n");
    ctx.push_str("  - `iCursorTrailDuration` (float), `iCursorGlowRadius` (float), `iCursorGlowIntensity` (float)\n");

    ctx.push('\n');

    // ---- Shader Uniform Controls ----
    ctx.push_str("## [Instruction] Shader Uniform Controls\n");
    ctx.push_str(r##"Background shaders can expose editable controls in Settings with `// control` comments immediately attached to explicit uniforms.
Full metadata defaults example (Float, Int, Vec2, Bool, Color):
```glsl
/*! par-term shader metadata
name: "Controlled Shader"
defaults:
  uniforms:
    iGlow: 0.5
    iFrequency: 1.0
    iOctaves: 4
    iBlendMode: 0
    iFlow: [0.1, -0.2]
    iOrigin: [0.5, 0.5]
    iBand: [0.25, 0.75]
    iRotation: 45.0
    iSourceChannel: 4
    iEnabled: true
    iTint: "#66ccff"
    iOverlay: "#ff8800cc"
*/
// control slider min=0 max=1 step=0.01
uniform float iGlow;
// control slider min=0.01 max=100 step=0.01 scale=log label="Frequency"
uniform float iFrequency;
// control checkbox
uniform bool iEnabled;
// control color label="Tint"
uniform vec3 iTint;
// control color label="Overlay"
uniform vec4 iOverlay;
// control int min=1 max=12 step=1
uniform int iOctaves;
// control select options="soft,hard,screen,add"
uniform int iBlendMode;
// control vec2 min=-1 max=1 step=0.01
uniform vec2 iFlow;
// control point label="Origin"
uniform vec2 iOrigin;
// control range min=0 max=1 step=0.01
uniform vec2 iBand;
// control angle unit=degrees
uniform float iRotation;
// control channel options="0,1,2,3,4"
uniform int iSourceChannel;
```
- Do not put default= in the control comment. Defaults live in shader metadata under `defaults.uniforms`; user edits persist as per-shader overrides and take precedence.
- Labels must be quoted (`label="Name"`).
- Prefer hex color defaults (`#rrggbb` for RGB, `#rrggbbaa` for RGBA). Normalized arrays are also accepted: `[r, g, b]` or `[r, g, b, a]`.
- Value types: `slider` and `angle` attach to `uniform float`; `checkbox` to `uniform bool`; `color` to `uniform vec3` or `uniform vec4`; `int`, `select`, and `channel` to `uniform int`; `vec2`, `point`, and `range` to `uniform vec2`.
- Color controls attach to `uniform vec3` (RGB picker) or `uniform vec4` (RGBA picker). `vec3` defaults to RGB / `alpha=false`; `alpha=true` is invalid for `vec3`. `vec4` defaults to RGBA / `alpha=true`; use `alpha=false` on `vec4` only to force an RGB picker with opaque alpha. `label="Name"` customizes the Settings label.
- Sliders require `min`, `max`, and `step`; add `scale=log` only when `0 < min < max`; fallback is the slider `min` when no override/default exists.
- Checkboxes fall back to `false`; colors fall back to opaque white; ints fall back to `min`; selects fall back to option index `0`; vec2 falls back to `[min, min]`; point falls back to `[0.5, 0.5]`; range falls back to `[min, max]`; angle falls back to `0`; channel falls back to its first option.
- Limits: 16 float slider controls/angle controls, 16 bool checkbox controls, 16 color controls, 16 int controls/select controls/channel controls, and 16 vec2 controls/point controls/range controls per shader. Extra or malformed controls produce non-fatal warnings; unsupported uniform types are not controls.
- Angle defaults and edits are authored in the declared UI unit (`unit=degrees` by default, or `unit=radians`); shaders always receive radians.
- Channel selector values choose among existing `iChannel0`..`iChannel4` sources only; a channel control does not create texture bindings or configure textures.
When to use controls:
- Use `slider` for continuous linear amounts.
- Use `slider scale=log` for frequency/exposure/gain/radius values spanning orders of magnitude.
- Use `int` for counts, iterations, samples, octaves, and quantization levels.
- Use `select` for discrete shader modes; shader receives zero-based option index.
- Use `vec2` for directions, offsets, scales, and velocities.
- Use `point` for normalized origins/focal points in 0..1 UV space.
- Use `range` for min/max thresholds and bands; shader receives vec2(low, high).
- Use `angle` for rotation/direction; defaults authored in declared unit and shaders receive radians.
- Use `channel` only to choose among existing `iChannel0`..`iChannel4` sources; it does not create texture bindings.
"##);
    ctx.push('\n');

    // ---- GLSL Compatibility Rules ----
    ctx.push_str("## [Constraint] GLSL Compatibility Rules\n");
    ctx.push_str("- Avoid passing sampler uniforms (e.g. `sampler2D`) as function parameters.\n");
    ctx.push_str("  Some GLSL versions / toolchains reject sampler-typed function arguments.\n");
    ctx.push_str("- Prefer sampling global uniforms like `iChannel0`..`iChannel4` directly.\n");
    ctx.push_str("Safe helper pattern:\n");
    ctx.push_str("```glsl\n");
    ctx.push_str("vec4 sampleTerminal(vec2 uv) {\n");
    ctx.push_str("    return texture(iChannel4, uv);\n");
    ctx.push_str("}\n");
    ctx.push_str("```\n");
    ctx.push_str("- UV/channel sampling rules:\n");
    ctx.push_str("  - `texture()` expects normalized UVs in `[0,1]`.\n");
    ctx.push_str("  - Start from `uv = fragCoord / iResolution.xy` for screen-space sampling.\n");
    ctx.push_str("  - After transforms (rotation/scale/offset), clamp before sampling:\n");
    ctx.push_str("    `vec2 suv = clamp(transformedUv, vec2(0.0), vec2(1.0));`\n");
    ctx.push_str("  - Do not mix pixel-space and UV-space in one variable.\n");
    ctx.push_str("    Convert pixel coords with `/ iResolution.xy` before `texture()`.\n");
    ctx.push_str(
        "  - Avoid arbitrary `+0.5` UV offsets unless intentionally correcting a known sampling artifact.\n",
    );
    ctx.push_str(
        "    Random `+0.5` shifts usually move sampling into the wrong coordinate space.\n",
    );
    ctx.push_str("- Coordinate-space contract:\n");
    ctx.push_str("  - `fragCoord` and cursor uniforms are pixel-space values.\n");
    ctx.push_str(
        "  - Texture sampling is UV-space. Convert once with `uv = fragCoord / iResolution.xy`.\n",
    );
    ctx.push_str("  - Keep pixel and UV vars separate (`cursorPx`, `cursorUv`, `distPx`, etc.).\n");
    ctx.push_str(
        "  - If mixing cursor data with UV math, convert explicitly: `cursorUv = (iCurrentCursor.xy + 0.5 * iCurrentCursor.zw) / iResolution.xy`.\n",
    );
    ctx.push_str(
        "  - Avoid implicit/double Y-flips. Use one coordinate convention per calculation path.\n",
    );
    ctx.push_str("- Optional channel textures:\n");
    ctx.push_str("  - Unset iChannel0-3 default to transparent 1x1 placeholders.\n");
    ctx.push_str("  - Detect a real configured texture with resolution > 1px, not `> 0.0`.\n");
    ctx.push_str("    Example: `bool hasTex0 = iChannelResolution[0].x > 1.0 && iChannelResolution[0].y > 1.0;`\n");

    ctx.push('\n');

    // ---- Minimal Shader Template ----
    ctx.push_str("## [Instruction] Minimal Shader Template\n");
    ctx.push_str("```glsl\n");
    ctx.push_str("void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n");
    ctx.push_str("    vec2 uv = fragCoord / iResolution.xy;\n");
    ctx.push_str("    vec4 tex = texture(iChannel4, uv);\n");
    ctx.push_str("    fragColor = tex;\n");
    ctx.push_str("}\n");
    ctx.push_str("```\n");

    ctx.push('\n');

    // ---- How to Apply Changes ----
    ctx.push_str("## [Instruction] How to Apply Changes\n");
    ctx.push_str(&format!(
        "1. Write shader GLSL files to: `{}`\n",
        shaders_dir.display()
    ));
    ctx.push_str("2. Use the `config_update` MCP tool to activate the shader:\n");
    ctx.push_str("   ```json\n");
    ctx.push_str(
        "   config_update({\"updates\": {\"custom_shader\": \"filename.glsl\", \"custom_shader_enabled\": true}})\n",
    );
    ctx.push_str("   ```\n");
    ctx.push_str("   For cursor shaders use `cursor_shader` and `cursor_shader_enabled` keys.\n");
    ctx.push_str("3. Changes apply immediately — no restart or manual config edit needed.\n");
    ctx.push_str("4. Call `shader_diagnostics` after activation to inspect active shader state,\n");
    ctx.push_str(
        "   last compile/reload errors, and generated debug paths before declaring success.\n",
    );
    ctx.push_str("   If the shader appears unchanged, black, white, or broken, call `shader_diagnostics` first.\n");
    ctx.push_str("5. For visual debugging/verification, use the `terminal_screenshot` MCP tool\n");
    ctx.push_str("   to capture the current terminal output (including shader rendering).\n");
    ctx.push_str("   This may require user permission before the screenshot is returned.\n");
    ctx.push_str(
        "6. Do not stop after writing the file if the user also asked to activate/set it.\n",
    );
    ctx.push_str(
        "   Completion requires a `config_update` call that sets the shader key and enable flag.\n",
    );
    ctx.push_str("7. If reading/listing the shader directory fails, do NOT loop on `Read` for the directory.\n");
    ctx.push_str(
        "   You can write a new file directly to the shader directory path (for example `vortex_checker.glsl`) and then activate it.\n",
    );
    ctx.push_str(
        "8. Custom shader-only changes do not require `make checkall`; use `shader_diagnostics` and `terminal_screenshot` instead.\n",
    );
    ctx.push_str(
        "9. Do not use Codex `apply_patch` for shader files under extra roots; if needed, use a direct Write/Edit tool or a small shell/python exact-replacement script, then read the file back.\n",
    );

    ctx.push('\n');

    // ---- Available Config Keys ----
    ctx.push_str("## [Constraint] Available Config Keys\n");
    ctx.push_str("Background shader: custom_shader (string|null), custom_shader_enabled (bool),\n");
    ctx.push_str("  custom_shader_animation (bool), custom_shader_animation_speed (float),\n");
    ctx.push_str("  custom_shader_brightness (float), custom_shader_text_opacity (float)\n");
    ctx.push_str("Cursor shader: cursor_shader (string|null), cursor_shader_enabled (bool),\n");
    ctx.push_str("  cursor_shader_animation (bool), cursor_shader_animation_speed (float),\n");
    ctx.push_str("  cursor_shader_glow_radius (float), cursor_shader_glow_intensity (float)\n");
    ctx.push('\n');
    ctx.push_str(
        "[Constraint] Do NOT edit config.yaml directly — always use the config_update tool.\n",
    );

    ctx
}
