// Infinite Zoom 2 - Multibrot z^3 Fractal
// Seamlessly looping infinite zoom using dual-layer crossfade.
// Uses z = z^3 + c producing 3-fold symmetric fern-like spirals.
//
// Same crossfade architecture as infinite-zoom-1 but with a completely
// different fractal and warm amber/magenta color palette.

/*! par-term shader metadata
name: Infinite Zoom 2
author: null
description: null
version: null
defaults:
  animation_speed: 0.8
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
    - -0.117
    - 0.76
    iGlowTint: '#4d1a0d'
    iTerminalBlend: 1.0
    iZoomRange: 1000.0
    iZoomSpeed: 0.04
*/
// control slider min=0.02 max=0.8 step=0.01 scale=log label="Zoom Speed"
uniform float iZoomSpeed;
// control slider min=10 max=10000 step=10 scale=log label="Zoom Range"
uniform float iZoomRange;
// control vec2 min=-2 max=2 step=0.001 label="Fractal Center"
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
    vec3 a = vec3(0.08, 0.02, 0.03);
    vec3 b = vec3(0.5, 0.3, 0.4);
    vec3 c = vec3(1.0, 0.8, 0.6);
    vec3 d = vec3(0.0, 0.25, 0.55);
    return a + b * cos(6.28318 * (c * t + d));
}

// Complex multiplication helper
vec2 cmul(vec2 a, vec2 b) {
    return vec2(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

// Render Multibrot z^3 fractal at a given zoom level
vec3 multibrot3(vec2 fragCoord, float zoom, float timeOffset) {
    vec2 p = (fragCoord - 0.5 * iResolution.xy) / min(iResolution.x, iResolution.y);

    vec2 c = iFractalCenter + p / zoom;

    // z^3 + c iteration
    vec2 z = vec2(0.0);
    float iter = 0.0;
    const float maxIter = 256.0;
    float zDot = 0.0;

    for (float i = 0.0; i < maxIter; i++) {
        // z = z^3 + c = z * z * z + c
        vec2 z2 = cmul(z, z);
        vec2 z3 = cmul(z2, z);
        z = z3 + c;

        zDot = dot(z, z);
        if (zDot > 256.0) {
            iter = i;
            break;
        }
        iter = i;
    }

    // Smooth iteration count (adjusted for power 3)
    float smoothIter = iter;
    if (zDot > 256.0) {
        smoothIter = iter - log(log(sqrt(zDot))) / log(3.0) + 1.0;
    }

    // Color mapping - warm amber/magenta tones
    vec3 col;
    if (zDot <= 256.0) {
        // Interior - deep dark wine
        col = vec3(0.02, 0.005, 0.015);
    } else {
        float t = smoothIter / 35.0 + timeOffset * iColorSpeed;
        col = palette(t);

        // Edge glow in warm tones
        float edgeGlow = 1.0 / (1.0 + smoothIter * 0.04);
        col += iGlowTint * edgeGlow * iEdgeGlow;

        // Brightness modulation
        float brightness = smoothstep(0.0, 8.0, smoothIter) * smoothstep(maxIter, 20.0, smoothIter);
        col *= 0.5 + 1.3 * brightness;
    }

    // Tone mapping
    col = col / (1.0 + col);
    col = pow(col, vec3(0.9));
    return col;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec4 termTex = texture(iChannel4, uv);

    // Cycle duration: ln(zoomRange) / zoomSpeed
    // zoomRange of 1000x keeps us within float precision
    float zoomSpeed = max(iZoomSpeed, 0.001);
    float zoomRange = max(iZoomRange, 1.01);
    float cycleDuration = log(zoomRange) / zoomSpeed;

    // Two layers offset by half a cycle
    float t1 = mod(iTime, cycleDuration);
    float t2 = mod(iTime + cycleDuration * 0.5, cycleDuration);

    float zoom1 = exp(t1 * zoomSpeed);
    float zoom2 = exp(t2 * zoomSpeed);

    // Crossfade weights
    float fade1 = smoothstep(0.0, 0.15, t1 / cycleDuration)
                * smoothstep(1.0, 0.85, t1 / cycleDuration);
    float fade2 = smoothstep(0.0, 0.15, t2 / cycleDuration)
                * smoothstep(1.0, 0.85, t2 / cycleDuration);

    // Render both layers
    vec3 col1 = multibrot3(fragCoord, zoom1, iTime);
    vec3 col2 = multibrot3(fragCoord, zoom2, iTime);

    // Blend layers by fade weights
    float totalWeight = fade1 + fade2;
    vec3 col = (col1 * fade1 + col2 * fade2) / max(totalWeight, 0.001);

    // Blend with terminal content
    float termAlpha = max(max(termTex.r, termTex.g), termTex.b);
    termAlpha = smoothstep(0.02, 0.15, termAlpha) * iTerminalBlend;
    fragColor = vec4(mix(col, termTex.rgb, termAlpha), 1.0);
}
