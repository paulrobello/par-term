/*! par-term shader metadata
name: Blueprint Grid
author: par-term
description: Subtle CAD-style grid that brightens near cursor and active progress bars.
version: 1.0.0
defaults:
  animation_speed: 0.45
  brightness: 0.30
  text_opacity: 1.0
  full_content: false
  uniforms:
    iGridColor: "#3aa7ff"
    iCursorColor: "#72f1ff"
    iGridSpacing: 42.0
    iLineStrength: 0.42
    iCursorGlow: 0.55
*/

// control color label="Grid"
uniform vec3 iGridColor;
// control color label="Cursor Glow"
uniform vec3 iCursorColor;
// control slider min=16 max=96 step=1 label="Grid Spacing"
uniform float iGridSpacing;
// control slider min=0 max=1 step=0.01 label="Line Strength"
uniform float iLineStrength;
// control slider min=0 max=1 step=0.01 label="Cursor Glow"
uniform float iCursorGlow;

float gridLine(vec2 p, float spacing, float width) {
    vec2 g = abs(fract(p / spacing - 0.5) - 0.5) * spacing;
    return 1.0 - smoothstep(0.0, width, min(g.x, g.y));
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    float major = gridLine(fragCoord + vec2(iTime * 3.0, 0.0), iGridSpacing, 1.1);
    float minor = gridLine(fragCoord, iGridSpacing / 4.0, 0.55);
    vec2 cursorCenter = iCurrentCursor.xy + iCurrentCursor.zw * 0.5;
    cursorCenter.y = iResolution.y - cursorCenter.y;
    float cursorDist = length(fragCoord - cursorCenter);
    float cursorGlow = exp(-cursorDist / 100.0) * iCursorGlow;
    float progressGlow = iProgress.z * (1.0 - smoothstep(0.0, 0.22, 1.0 - uv.y)) * (0.15 + iProgress.y);

    vec3 color = vec3(0.012, 0.027, 0.048);
    color += iGridColor * (major * 0.12 + minor * 0.035) * iLineStrength;
    color += iCursorColor * cursorGlow * 0.30;
    color += iGridColor * progressGlow * 0.18;
    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
