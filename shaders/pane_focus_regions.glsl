/*! par-term shader metadata
name: Pane Focus Regions
author: par-term
description: Uses iFocusedPane to subtly frame the active split pane while dimming inactive regions.
version: 1.0.0
defaults:
  animation_speed: 0.35
  brightness: 0.30
  text_opacity: 1.0
  full_content: false
  uniforms:
    iBaseColor: "#101623"
    iFocusColor: "#4aa3ff"
    iInactiveDim: 0.28
    iBorderStrength: 0.55
*/

// control color label="Base"
uniform vec3 iBaseColor;
// control color label="Focus"
uniform vec3 iFocusColor;
// control slider min=0 max=0.8 step=0.01 label="Inactive Dim"
uniform float iInactiveDim;
// control slider min=0 max=1 step=0.01 label="Border Strength"
uniform float iBorderStrength;

float insidePane(vec2 p, vec4 r) {
    vec2 a = step(r.xy, p);
    vec2 b = step(p, r.xy + r.zw);
    return a.x * a.y * b.x * b.y;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec2 p = fragCoord;
    vec4 pane = iFocusedPane;
    float inside = insidePane(p, pane);
    vec2 nearest = clamp(p, pane.xy, pane.xy + pane.zw);
    float dist = length(p - nearest);
    float innerDist = min(min(p.x - pane.x, pane.x + pane.z - p.x), min(p.y - pane.y, pane.y + pane.w - p.y));
    float border = (1.0 - smoothstep(0.0, 22.0, abs(innerDist))) * inside;
    float outerGlow = (1.0 - smoothstep(0.0, 50.0, dist)) * (1.0 - inside);
    float sweep = 0.5 + 0.5 * sin(iTime * 1.2 + uv.x * 5.0 + uv.y * 3.0);

    vec3 color = iBaseColor * (0.32 + 0.08 * uv.y);
    color *= mix(1.0 - iInactiveDim, 1.0, inside);
    color += iFocusColor * (border * (0.35 + 0.20 * sweep) + outerGlow * 0.16) * iBorderStrength;
    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
