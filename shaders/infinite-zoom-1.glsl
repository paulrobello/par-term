// Infinite Zoom 1 - Burning Ship Fractal
// Seamlessly looping infinite zoom using dual-layer crossfade
// to avoid floating-point precision loss at deep zoom levels.
//
// Technique: Two fractal layers render at different zoom depths,
// offset by half a cycle. As one layer zooms too deep (losing precision),
// it fades out while the other fades in at a shallower zoom.
// The crossfade makes the loop invisible.

/*! par-term shader metadata
name: Infinite Zoom 1
author: null
description: null
version: null
defaults:
  animation_speed: null
  brightness: null
  full_content: true
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: null
  use_background_as_channel0: null
  uniforms:
    iColorSpeed: 0.019999992
    iEdgeGlow: 1.0
    iFractalCenter:
    - -1.762
    - -0.028
    iGlowTint: '#0d264d'
    iTerminalBlend: 1.0
    iZoomRange: 1000.0
    iZoomSpeed: 0.04
*/
// control slider min=0.02 max=0.8 step=0.01 scale=log label="Zoom Speed"
uniform float iZoomSpeed;
// control slider min=10 max=10000 step=10 scale=log label="Zoom Range"
uniform float iZoomRange;
// control vec2 min=-2.5 max=1.5 step=0.001 label="Fractal Center"
uniform vec2 iFractalCenter;
// control slider min=-0.2 max=0.2 step=0.005 label="Color Cycle"
uniform float iColorSpeed;
// control slider min=0 max=3 step=0.01 label="Edge Glow"
uniform float iEdgeGlow;
// control color label="Glow Tint"
uniform vec3 iGlowTint;
// control slider min=0 max=1 step=0.01 label="Terminal Blend"
uniform float iTerminalBlend;

vec3 palette(float t) {
    vec3 a = vec3(0.02, 0.01, 0.08);
    vec3 b = vec3(0.4, 0.35, 0.5);
    vec3 c = vec3(1.0, 1.0, 1.0);
    vec3 d = vec3(0.0, 0.15, 0.45);
    return a + b * cos(6.28318 * (c * t + d));
}

// Render Burning Ship fractal at a given zoom level
vec3 burningShip(vec2 fragCoord, float zoom, float timeOffset) {
    vec2 p = (fragCoord - 0.5 * iResolution.xy) / min(iResolution.x, iResolution.y);

    vec2 c = iFractalCenter + p / zoom;

    // Burning Ship iteration
    vec2 z = vec2(0.0);
    float iter = 0.0;
    const float maxIter = 256.0;
    float zDot = 0.0;

    for (float i = 0.0; i < maxIter; i++) {
        z = vec2(abs(z.x), abs(z.y));
        float xNew = z.x * z.x - z.y * z.y + c.x;
        z.y = 2.0 * z.x * z.y + c.y;
        z.x = xNew;

        zDot = dot(z, z);
        if (zDot > 256.0) {
            iter = i;
            break;
        }
        iter = i;
    }

    // Smooth iteration count
    float smoothIter = iter;
    if (zDot > 256.0) {
        smoothIter = iter - log2(log2(zDot)) + 4.0;
    }

    // Color mapping
    vec3 col;
    if (zDot <= 256.0) {
        col = vec3(0.005, 0.005, 0.025);
    } else {
        float t = smoothIter / 40.0 + timeOffset * iColorSpeed;
        col = palette(t);

        float edgeGlow = 1.0 / (1.0 + smoothIter * 0.05);
        col += iGlowTint * edgeGlow * iEdgeGlow;

        float brightness = smoothstep(0.0, 8.0, smoothIter) * smoothstep(maxIter, 20.0, smoothIter);
        col *= 0.6 + 1.2 * brightness;
    }

    col = col / (1.0 + col);
    col = pow(col, vec3(0.85));
    return col;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec4 termTex = texture(iChannel4, uv);

    // Each cycle covers a fixed range of zoom before looping.
    // cycleDuration in seconds - how long before a single layer loops.
    // We use ln(zoomRange)/zoomSpeed so that exp(zoomSpeed * cycleDuration) = zoomRange.
    // zoomRange of ~1000x keeps us well within float precision.
    float zoomSpeed = max(iZoomSpeed, 0.001);
    float zoomRange = max(iZoomRange, 1.01);
    float cycleDuration = log(zoomRange) / zoomSpeed;  // ~46 seconds per cycle

    // Two layers offset by half a cycle
    float t1 = mod(iTime, cycleDuration);
    float t2 = mod(iTime + cycleDuration * 0.5, cycleDuration);

    float zoom1 = exp(t1 * zoomSpeed);
    float zoom2 = exp(t2 * zoomSpeed);

    // Crossfade: each layer fades in during first quarter, full during middle,
    // fades out during last quarter
    float fade1 = smoothstep(0.0, 0.15, t1 / cycleDuration)
                * smoothstep(1.0, 0.85, t1 / cycleDuration);
    float fade2 = smoothstep(0.0, 0.15, t2 / cycleDuration)
                * smoothstep(1.0, 0.85, t2 / cycleDuration);

    // Render both layers
    vec3 col1 = burningShip(fragCoord, zoom1, iTime);
    vec3 col2 = burningShip(fragCoord, zoom2, iTime);

    // Blend layers by their fade weights
    float totalWeight = fade1 + fade2;
    vec3 col = (col1 * fade1 + col2 * fade2) / max(totalWeight, 0.001);

    // Blend with terminal content
    float termAlpha = max(max(termTex.r, termTex.g), termTex.b);
    termAlpha = smoothstep(0.02, 0.15, termAlpha) * iTerminalBlend;
    fragColor = vec4(mix(col, termTex.rgb, termAlpha), 1.0);
}
