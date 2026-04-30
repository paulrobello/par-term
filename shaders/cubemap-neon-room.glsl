/*! par-term shader metadata
name: Cubemap Neon Room
author: par-term
description: Subtle neon room cubemap wash with restrained motion for terminal backgrounds.
version: 1.0.0
defaults:
  animation_speed: 0.16
  brightness: 0.28
  text_opacity: 1.0
  full_content: false
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: textures/cubemaps/env-test
  cubemap_enabled: true
safety_badges:
  - uses_cubemap
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec2 p = uv * 2.0 - 1.0;
    p.x *= iResolution.x / max(iResolution.y, 1.0);

    float t = iTime * 0.02;
    vec3 dir = normalize(vec3(p.x * 0.42 + sin(t) * 0.04, p.y * 0.30 + cos(t * 0.7) * 0.03, 1.0));
    vec3 env = texture(iCubemap, dir).rgb;

    float horizon = smoothstep(-0.45, 0.85, p.y);
    float sideGlow = 1.0 - smoothstep(0.18, 1.25, abs(p.x));
    vec3 dusk = mix(vec3(0.012, 0.010, 0.022), vec3(0.025, 0.018, 0.040), horizon);
    vec3 neon = vec3(0.025, 0.075, 0.110) * sideGlow + vec3(0.060, 0.018, 0.075) * (1.0 - horizon);
    vec3 color = dusk + neon * 0.26 + env * vec3(0.08, 0.10, 0.13);
    color *= smoothstep(1.45, 0.15, length(p));

    fragColor = vec4(color, 1.0);
}
