/*! par-term shader metadata
name: Matrix Rain 2.0
author: par-term
description: Less distracting matrix rain that samples terminal content and avoids dense text while reacting to typing bursts.
version: 1.0.1
defaults:
  animation_speed: 0.55
  brightness: 0.30
  text_opacity: 1.0
  full_content: true
  uniforms:
    iRainColor: "#44ff88"
    iBackgroundTint: "#021006"
    iRainDensity: 0.55
    iTypingBoost: 0.45
*/

// control color label="Rain"
uniform vec3 iRainColor;
// control color label="Background Tint"
uniform vec3 iBackgroundTint;
// control slider min=0 max=1 step=0.01 label="Rain Density"
uniform float iRainDensity;
// control slider min=0 max=1 step=0.01 label="Typing Boost"
uniform float iTypingBoost;

float hash(vec2 p) { return fract(sin(dot(p, vec2(12.9898,78.233))) * 43758.5453); }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec4 terminal = texture(iChannel4, uv);
    vec2 cell = floor(fragCoord / vec2(12.0, 18.0));
    float colHash = hash(vec2(cell.x, 3.7));
    float speed = mix(0.45, 1.4, colHash);
    float y = cell.y - iTime * speed * 5.0;
    float glyph = hash(vec2(cell.x, floor(y)));
    float trail = smoothstep(0.98 - iRainDensity * 0.18, 1.0, glyph);
    float head = smoothstep(0.985, 1.0, hash(vec2(cell.x, floor(y * 0.43))));
    float textDensity = smoothstep(0.03, 0.18, terminal.a + dot(terminal.rgb, vec3(0.299, 0.587, 0.114)));
    float keyPulse = exp(-max(0.0, iTime - iTimeKeyPress) * 7.0) * iTypingBoost;
    vec3 rain = iRainColor * (trail * 0.18 + head * 0.35 + keyPulse * trail * 0.30) * (1.0 - textDensity * 0.85);
    vec3 bg = iBackgroundTint * (0.55 + 0.15 * uv.y);
    vec3 color = max(terminal.rgb, bg + rain);
    fragColor = vec4(clamp(color, 0.0, 1.0), max(terminal.a, 1.0));
}
