/*! par-term shader metadata
name: Build Reactor
author: par-term
description: Progress-aware reactor core glow that charges with iProgress.y and vents on warning/error states.
version: 1.0.0
defaults:
  animation_speed: 0.7
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: null
  use_background_as_channel0: null
  uniforms:
    iCoreColor: '#54d6ff'
    iCoreSize: 0.3
    iErrorColor: '#ff4d5e'
    iVentStrength: 0.65
    iWarningColor: '#ffb347'
*/

// control color label="Core"
uniform vec3 iCoreColor;
// control color label="Warning"
uniform vec3 iWarningColor;
// control color label="Error"
uniform vec3 iErrorColor;
// control slider min=0.1 max=0.7 step=0.01 label="Core Size"
uniform float iCoreSize;
// control slider min=0 max=1 step=0.01 label="Vent Strength"
uniform float iVentStrength;

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec2 p = (fragCoord - 0.5 * iResolution.xy) / min(iResolution.x, iResolution.y);
    float active = step(0.5, iProgress.z);
    float charge = mix(0.18, clamp(iProgress.y, 0.0, 1.0), active);
    float r = length(p);
    float core = exp(-r / max(0.04, iCoreSize)) * (0.25 + charge * 0.85);
    float angle = atan(p.y, p.x);
    float rings = sin(r * 92.0 - iTime * mix(1.4, 5.0, active)) * 0.5 + 0.5;
    rings *= exp(-r * 4.2) * mix(0.22, 1.0, active);
    float ringMask = smoothstep(0.008, 0.0, abs(r - 0.23))
        + smoothstep(0.006, 0.0, abs(r - 0.38))
        + smoothstep(0.005, 0.0, abs(r - 0.53));
    float spokeWave = cos(angle * 12.0 + iTime * 0.75);
    float spokeMask = smoothstep(0.965, 1.0, spokeWave) * smoothstep(0.12, 0.18, r) * (1.0 - smoothstep(0.64, 0.78, r));
    vec2 gridUv = uv * iResolution.xy / 42.0;
    vec2 gridCell = abs(fract(gridUv) - 0.5);
    float grid = (1.0 - smoothstep(0.470, 0.500, max(gridCell.x, gridCell.y))) * 0.045;
    grid *= 1.0 - smoothstep(0.25, 0.95, r);
    float err = step(1.5, iProgress.x) * (1.0 - step(2.5, iProgress.x));
    float warn = step(3.5, iProgress.x);
    vec3 stateColor = mix(iCoreColor, iWarningColor, warn);
    stateColor = mix(stateColor, iErrorColor, err);
    float vent = max(0.0, sin(angle * 8.0 + iTime * 7.0)) * (warn + err) * exp(-r * 1.7);
    float chassis = ringMask * 0.18 + spokeMask * 0.12 + grid;
    vec3 color = vec3(0.01, 0.012, 0.020) + stateColor * (core + rings * 0.11 + chassis + vent * iVentStrength * 0.35);
    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
