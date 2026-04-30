/*! par-term shader metadata
name: Scrollback Parallax
author: par-term
description: Subtle depth fog and timeline bands driven by iScroll scrollback depth.
version: 1.0.0
defaults:
  animation_speed: 0.2
  brightness: 0.2
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: null
  use_background_as_channel0: null
  uniforms:
    iBandDensity: 18.0
    iDeepColor: '#26314f'
    iFogStrength: 0.45
    iNearColor: '#101827'
    iTimelineColor: '#66d9ef'
*/

// control color label="Near"
uniform vec3 iNearColor;
// control color label="Deep"
uniform vec3 iDeepColor;
// control color label="Timeline"
uniform vec3 iTimelineColor;
// control slider min=0 max=1 step=0.01 label="Fog Strength"
uniform float iFogStrength;
// control slider min=4 max=64 step=1 label="Band Density"
uniform float iBandDensity;

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    float depth = clamp(iScroll.w, 0.0, 1.0);
    float linePhase = (uv.y * iScroll.y + iScroll.x) / max(1.0, iBandDensity);
    float band = 1.0 - smoothstep(0.0, 0.10, abs(fract(linePhase) - 0.5));
    float drift = 0.5 + 0.5 * sin(iTime * 0.7 + depth * 9.0 + uv.x * 3.0);
    vec3 color = mix(iNearColor, iDeepColor, depth * iFogStrength);
    color += iTimelineColor * band * (0.025 + depth * 0.075);
    color += iDeepColor * drift * depth * 0.12;
    color *= 0.75 + 0.25 * (1.0 - length(uv - 0.5));
    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
