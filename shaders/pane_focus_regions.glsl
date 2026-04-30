/*! par-term shader metadata
name: Pane Focus Regions
author: par-term
description: Uses iFocusedPane to frame the active split pane while dimming inactive terminal content.
version: 1.0.1
defaults:
  animation_speed: 0.35
  brightness: null
  full_content: true
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: null
  use_background_as_channel0: null
  uniforms:
    iBaseColor: '#101623'
    iBorderStrength: 0.96
    iFocusColor: '#4aa3ff'
    iInactiveDim: 0.44
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

    float dimFactor = mix(1.0 - iInactiveDim, 1.0, inside);
    vec3 background = iBaseColor * (0.32 + 0.08 * uv.y);
    background *= dimFactor;
    background += iFocusColor * (border * (0.35 + 0.20 * sweep) + outerGlow * 0.16) * iBorderStrength;

    vec4 terminal = texture(iChannel4, uv);
    vec3 terminalPremul = terminal.rgb * dimFactor;
    vec3 color = terminalPremul + background * (1.0 - terminal.a);
    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
