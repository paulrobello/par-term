/*! par-term shader metadata
name: Diff Heatmap Glow
author: par-term
description: Full-content shader that adds subtle edge highlights to bright or changed-looking text regions without blurring glyphs.
version: 1.0.0
defaults:
  animation_speed: 0.2
  brightness: 0.30
  text_opacity: 1.0
  full_content: true
  uniforms:
    iAddColor: "#42d392"
    iDeleteColor: "#ff4d5e"
    iHighlightStrength: 0.35
    iThreshold: 0.22
*/

// control color label="Add Glow"
uniform vec3 iAddColor;
// control color label="Delete Glow"
uniform vec3 iDeleteColor;
// control slider min=0 max=1 step=0.01 label="Highlight Strength"
uniform float iHighlightStrength;
// control slider min=0.02 max=0.8 step=0.01 label="Threshold"
uniform float iThreshold;

float lum(vec3 c) { return dot(c, vec3(0.299, 0.587, 0.114)); }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec2 px = 1.0 / iResolution.xy;
    vec4 center = texture(iChannel4, uv);
    float l = lum(center.rgb);
    float edge = 0.0;
    edge += abs(l - lum(texture(iChannel4, uv + vec2(px.x, 0.0)).rgb));
    edge += abs(l - lum(texture(iChannel4, uv - vec2(px.x, 0.0)).rgb));
    edge += abs(l - lum(texture(iChannel4, uv + vec2(0.0, px.y)).rgb));
    edge += abs(l - lum(texture(iChannel4, uv - vec2(0.0, px.y)).rgb));
    edge = smoothstep(iThreshold, iThreshold + 0.35, edge);
    float greenish = smoothstep(0.02, 0.20, center.g - max(center.r, center.b) * 0.75);
    float reddish = smoothstep(0.02, 0.20, center.r - max(center.g, center.b) * 0.75);
    vec3 heat = mix(vec3(0.0), iAddColor, greenish);
    heat = mix(heat, iDeleteColor, reddish);
    heat += mix(iAddColor, iDeleteColor, 0.5 + 0.5 * sin(iTime)) * (1.0 - max(greenish, reddish)) * 0.18;
    vec3 color = center.rgb + heat * edge * iHighlightStrength;
    fragColor = vec4(clamp(color, 0.0, 1.0), center.a);
}
