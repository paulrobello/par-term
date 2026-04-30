/*! par-term shader metadata
name: Build Reactor
author: par-term
description: Progress-aware reactor core glow that charges with iProgress.y and vents on warning/error states.
version: 1.0.0
defaults:
  animation_speed: 0.7
  brightness: 0.30
  text_opacity: 1.0
  full_content: false
  uniforms:
    iCoreColor: "#54d6ff"
    iWarningColor: "#ffb347"
    iErrorColor: "#ff4d5e"
    iCoreSize: 0.30
    iVentStrength: 0.65
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
    float rings = sin(r * 80.0 - iTime * 5.0) * 0.5 + 0.5;
    rings *= exp(-r * 4.0) * active;
    float err = step(1.5, iProgress.x) * (1.0 - step(2.5, iProgress.x));
    float warn = step(3.5, iProgress.x);
    vec3 stateColor = mix(iCoreColor, iWarningColor, warn);
    stateColor = mix(stateColor, iErrorColor, err);
    float vent = max(0.0, sin(atan(p.y, p.x) * 8.0 + iTime * 7.0)) * (warn + err) * exp(-r * 1.7);
    vec3 color = vec3(0.01, 0.012, 0.020) + stateColor * (core + rings * 0.08 + vent * iVentStrength * 0.35);
    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
