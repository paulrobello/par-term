/*! par-term shader metadata
name: Cubemap Atmospheric Sky
author: par-term
description: Low-distraction atmospheric cubemap gradient tuned for terminal readability.
version: 1.0.0
defaults:
  animation_speed: 0.2
  brightness: 0.35
  text_opacity: 1.0
  full_content: false
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: textures/cubemaps/env-outside
  cubemap_enabled: true
safety_badges:
  - uses_cubemap
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec2 p = uv * 2.0 - 1.0;
    p.x *= iResolution.x / max(iResolution.y, 1.0);

    float t = iTime * 0.03;
    vec3 dir = normalize(vec3(p.x * 0.35 + sin(t) * 0.08, p.y * 0.25, 1.0));
    vec3 env = texture(iCubemap, dir).rgb;

    float sky = smoothstep(-0.8, 1.0, p.y);
    float vignette = smoothstep(1.40, 0.10, length(p));
    vec3 base = mix(vec3(0.015, 0.018, 0.032), vec3(0.060, 0.080, 0.120), sky);
    vec3 color = mix(base, env * 0.18, 0.35) * vignette;

    fragColor = vec4(color, 1.0);
}
