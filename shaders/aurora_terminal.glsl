/*! par-term shader metadata
name: Aurora Terminal
author: par-term
description: Soft northern-light ribbons with slow motion and readability-first defaults.
version: 1.0.0
defaults:
  animation_speed: 0.35
  brightness: 0.30
  text_opacity: 1.0
  full_content: false
  uniforms:
    iColorA: "#4fffd2"
    iColorB: "#8a6cff"
    iColorC: "#2f7cff"
    iRibbonStrength: 0.55
    iRibbonScale: 3.0
*/

// control color label="Ribbon A"
uniform vec3 iColorA;
// control color label="Ribbon B"
uniform vec3 iColorB;
// control color label="Ribbon C"
uniform vec3 iColorC;
// control slider min=0 max=1 step=0.01 label="Ribbon Strength"
uniform float iRibbonStrength;
// control slider min=1 max=8 step=0.1 label="Ribbon Scale"
uniform float iRibbonScale;

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    float aspect = iResolution.x / max(1.0, iResolution.y);
    vec2 p = vec2((uv.x - 0.5) * aspect, uv.y);
    float t = iTime * 0.12;
    float ribbon1 = sin((p.x * iRibbonScale + sin(p.y * 2.6 + t) * 0.5 + t) * 4.0);
    float ribbon2 = sin((p.x * (iRibbonScale * 0.7) - p.y * 1.8 - t * 1.6) * 5.0);
    float mask = smoothstep(0.15, 0.95, uv.y) * (1.0 - smoothstep(0.90, 1.0, uv.y));
    float glow1 = pow(max(0.0, ribbon1 * 0.5 + 0.5), 6.0) * mask;
    float glow2 = pow(max(0.0, ribbon2 * 0.5 + 0.5), 8.0) * mask;
    vec3 color = vec3(0.015, 0.025, 0.055) + vec3(0.02, 0.04, 0.08) * uv.y;
    color += (iColorA * glow1 + iColorB * glow2 + iColorC * glow1 * glow2) * iRibbonStrength;
    color *= 0.8 + 0.2 * (1.0 - length(uv - 0.5));
    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
