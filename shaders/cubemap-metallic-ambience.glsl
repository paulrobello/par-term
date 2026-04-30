/*! par-term shader metadata
name: Cubemap Metallic Ambience
author: par-term
description: Low-distraction metallic cubemap reflections tuned for terminal readability.
version: 1.0.0
defaults:
  animation_speed: 0.18
  brightness: 0.30
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

    float t = iTime * 0.025;
    vec3 viewDir = normalize(vec3(p.x * 0.32 + sin(t) * 0.05, p.y * 0.24, 1.0));
    vec3 normal = normalize(vec3(p.x * 0.22, p.y * 0.16 + cos(t) * 0.03, 1.0));
    vec3 reflected = reflect(-viewDir, normal);
    vec3 env = texture(iCubemap, reflected).rgb;

    float vignette = smoothstep(1.35, 0.20, length(p));
    float brushed = 0.025 * sin((uv.y + sin(uv.x * 5.0) * 0.01) * 95.0);
    vec3 base = mix(vec3(0.018, 0.019, 0.022), vec3(0.055, 0.057, 0.062), vignette);
    vec3 metal = env * vec3(0.16, 0.17, 0.18) + brushed;
    vec3 color = mix(base, metal, 0.34) * vignette;

    fragColor = vec4(max(color, vec3(0.0)), 1.0);
}
