/*! par-term shader metadata
name: Progress Reactive Theme
author: par-term
description: Calm ambient terminal background that changes to amber warning pulses, red error edge bloom, and indeterminate animated stripes from iProgress.
version: 1.0.0
defaults:
  animation_speed: 0.55
  brightness: 0.30
  text_opacity: 1.0
  full_content: false
  uniforms:
    iAmbientColor: "#15345a"
    iNormalColor: "#42d392"
    iWarningColor: "#ffb347"
    iErrorColor: "#ff4d5e"
    iIntensity: 0.55
    iStripeScale: 34.0
*/

// control color label="Ambient"
uniform vec3 iAmbientColor;
// control color label="Normal Progress"
uniform vec3 iNormalColor;
// control color label="Warning"
uniform vec3 iWarningColor;
// control color label="Error"
uniform vec3 iErrorColor;
// control slider min=0 max=1 step=0.01 label="Intensity"
uniform float iIntensity;
// control slider min=8 max=96 step=1 label="Stripe Scale"
uniform float iStripeScale;

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
               mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec2 p = (fragCoord - 0.5 * iResolution.xy) / min(iResolution.x, iResolution.y);

    float n = noise(uv * 4.0 + vec2(iTime * 0.025, -iTime * 0.015));
    float radial = 1.0 - smoothstep(0.0, 0.95, length(p));
    vec3 base = iAmbientColor * (0.22 + 0.20 * n) + iAmbientColor * radial * 0.35;

    float active = step(0.5, iProgress.z);
    float state = iProgress.x;
    float pct = clamp(iProgress.y, 0.0, 1.0);
    vec3 progressColor = iNormalColor;
    progressColor = mix(progressColor, iErrorColor, step(1.5, state) * (1.0 - step(2.5, state)));
    progressColor = mix(progressColor, iWarningColor, step(3.5, state));

    float topEdge = 1.0 - smoothstep(0.0, 0.30, 1.0 - uv.y);
    float sideEdge = max(1.0 - smoothstep(0.0, 0.08, uv.x), 1.0 - smoothstep(0.0, 0.08, 1.0 - uv.x));
    float pulse = 0.65 + 0.35 * sin(iTime * 5.0);
    float err = step(1.5, state) * (1.0 - step(2.5, state));
    float warn = step(3.5, state);

    base += progressColor * active * topEdge * (0.15 + pct * 0.55) * iIntensity;
    base += iErrorColor * active * err * sideEdge * pulse * 0.55 * iIntensity;
    base += iWarningColor * active * warn * topEdge * pulse * 0.25 * iIntensity;

    float indeterminate = step(2.5, state) * (1.0 - step(3.5, state));
    float stripes = smoothstep(0.46, 0.50, fract((fragCoord.x + fragCoord.y + iTime * 240.0) / iStripeScale));
    base += progressColor * active * indeterminate * stripes * 0.18 * iIntensity;

    fragColor = vec4(clamp(base, 0.0, 1.0), 1.0);
}
