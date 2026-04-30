/*! par-term shader metadata
name: Ink Wash
author: par-term
description: Low-contrast paper and ink diffusion using procedural noise for calm writing.
version: 1.0.0
defaults:
  animation_speed: 0.08
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: null
  use_background_as_channel0: null
  uniforms:
    iGrain: 0.39999998
    iInkColor: '#293242'
    iPaperColor: '#d8d0bd'
    iWashStrength: 0.32
*/

// control color label="Paper"
uniform vec3 iPaperColor;
// control color label="Ink"
uniform vec3 iInkColor;
// control slider min=0 max=1 step=0.01 label="Wash Strength"
uniform float iWashStrength;
// control slider min=0 max=1 step=0.01 label="Grain"
uniform float iGrain;

float hash(vec2 p) { return fract(sin(dot(p, vec2(41.0, 289.0))) * 143758.5453); }
float noise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1,0)), u.x), mix(hash(i + vec2(0,1)), hash(i + vec2(1,1)), u.x), u.y);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    float n1 = noise(uv * 5.0 + iTime * 0.02);
    float n2 = noise(uv * 18.0 - iTime * 0.01);
    float wash = smoothstep(0.28, 0.82, n1 + 0.35 * n2);
    vec3 color = mix(iPaperColor, iInkColor, wash * iWashStrength);
    float fiber = hash(fragCoord) - 0.5;
    color += fiber * iGrain * 0.08;
    color *= 0.78;
    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
