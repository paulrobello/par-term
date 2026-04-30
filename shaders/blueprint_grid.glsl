/*! par-term shader metadata
name: Blueprint Grid
author: par-term
description: Subtle CAD-style grid that brightens near cursor and active progress bars.
version: 1.0.0
defaults:
  animation_speed: 0.45
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: null
  use_background_as_channel0: null
  uniforms:
    iBlueprintBackground: '#102f4f'
    iBlueprintCursorRadius: 50.0
    iCursorColor: '#72f1ff'
    iCursorGlow: 0.55
    iGridColor: '#6bbcff'
    iGridSpacing: 64.0
    iLineStrength: 1.0
    iLineThickness: 2.0
*/

// control color label="Background"
uniform vec3 iBlueprintBackground;
// control color label="Grid"
uniform vec3 iGridColor;
// control color label="Cursor Glow"
uniform vec3 iCursorColor;
// control slider min=16 max=96 step=1 label="Grid Spacing"
uniform float iGridSpacing;
// control slider min=0 max=1 step=0.01 label="Line Strength"
uniform float iLineStrength;
// control slider min=0.25 max=4 step=0.05 label="Line Thickness"
uniform float iLineThickness;
// control slider min=0 max=1 step=0.01 label="Cursor Glow"
uniform float iCursorGlow;
// control slider min=20 max=320 step=1 label="Cursor Radius"
uniform float iBlueprintCursorRadius;

float gridLine(vec2 p, float spacing, float width) {
    vec2 g = abs(fract(p / spacing - 0.5) - 0.5) * spacing;
    return 1.0 - smoothstep(0.0, width, min(g.x, g.y));
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    float lineThickness = clamp(iLineThickness, 0.25, 4.0);
    float major = gridLine(fragCoord + vec2(iTime * 3.0, 0.0), iGridSpacing, 1.1 * lineThickness);
    float minor = gridLine(fragCoord, iGridSpacing / 4.0, 0.55 * lineThickness);
    vec2 cursorCenter = iCurrentCursor.xy + iCurrentCursor.zw * 0.5;
    float cursorDist = length(fragCoord - cursorCenter);
    float cursorGlow = exp(-cursorDist / max(iBlueprintCursorRadius, 1.0)) * iCursorGlow;
    float progressGlow = iProgress.z * (1.0 - smoothstep(0.0, 0.22, 1.0 - uv.y)) * (0.15 + iProgress.y);

    vec3 color = iBlueprintBackground;
    color += iGridColor * (major * 0.12 + minor * 0.035) * iLineStrength;
    color += iCursorColor * cursorGlow * 0.30;
    color += iGridColor * progressGlow * 0.18;
    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
