/*! par-term shader metadata
name: Low Power Ambience
author: par-term
description: Static-to-ultra-slow polished ambience intended for reduced frame cadence and battery-friendly sessions.
version: 1.0.0
defaults:
  animation_speed: 0.08
  brightness: 0.2
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: null
  use_background_as_channel0: null
  uniforms:
    iAccent: '#7c5cff'
    iColorA: '#101827'
    iColorB: '#26314f'
    iGrain: 0.099999994
    iMotion: 0.14999999
*/

// control color label="Base A"
uniform vec3 iColorA;
// control color label="Base B"
uniform vec3 iColorB;
// control color label="Accent"
uniform vec3 iAccent;
// control slider min=0 max=1 step=0.01 label="Motion"
uniform float iMotion;
// control slider min=0 max=1 step=0.01 label="Grain"
uniform float iGrain;

float hash(vec2 p) { return fract(sin(dot(p, vec2(101.7, 47.3))) * 43758.5453); }

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec2 p = uv - 0.5;
    float drift = sin((p.x * 2.0 + p.y * 1.3) + iTime * iMotion) * 0.5 + 0.5;
    float blob = 1.0 - smoothstep(0.0, 0.8, length(p + vec2(0.12 * sin(iTime * iMotion), 0.08 * cos(iTime * iMotion * 0.7))));
    vec3 color = mix(iColorA, iColorB, uv.y * 0.65 + drift * 0.15);
    color += iAccent * blob * 0.12;
    color += (hash(fragCoord) - 0.5) * iGrain * 0.04;
    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
