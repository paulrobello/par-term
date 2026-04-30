// Infinite Zoom 3 - Julia Set Fractal
// Seamlessly looping infinite zoom using dual-layer crossfade.
// Zooms into the fractal boundary where detail persists at all scales.

/*! par-term shader metadata
name: Infinite Zoom 3
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
    iBaseZoom: 1.0
    iColorSpeed: 0.0100000035
    iEdgeGlow: 2.5
    iFractalCenter:
    - 0.0
    - 0.65
    iGlowTint: '#1a4059'
    iJuliaC:
    - -0.123
    - 0.745
    iZoomRange: 500.0
    iZoomSpeed: 0.04
*/
// control slider min=0.01 max=0.5 step=0.005 scale=log label="Zoom Speed"
uniform float iZoomSpeed;
// control slider min=0.1 max=10 step=0.1 scale=log label="Base Zoom"
uniform float iBaseZoom;
// control slider min=10 max=5000 step=10 scale=log label="Zoom Range"
uniform float iZoomRange;
// control vec2 min=-1 max=1 step=0.001 label="Julia Constant"
uniform vec2 iJuliaC;
// control vec2 min=-2 max=2 step=0.001 label="Fractal Center"
uniform vec2 iFractalCenter;
// control slider min=-0.1 max=0.1 step=0.002 label="Color Cycle"
uniform float iColorSpeed;
// control slider min=0 max=6 step=0.05 label="Edge Glow"
uniform float iEdgeGlow;
// control color label="Glow Tint"
uniform vec3 iGlowTint;

vec3 palette(float t) {
    vec3 a = vec3(0.5, 0.5, 0.5);
    vec3 b = vec3(0.5, 0.5, 0.5);
    vec3 c = vec3(1.0, 1.0, 1.0);
    vec3 d = vec3(0.0, 0.1, 0.2);
    return a + b * cos(6.28318 * (c * t + d));
}

vec2 cmul(vec2 a, vec2 b) {
    return vec2(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

// Render Julia set at a given zoom level
vec3 julia(vec2 fragCoord, float zoom, float timeOffset) {
    vec2 p = (fragCoord - 0.5 * iResolution.xy) / min(iResolution.x, iResolution.y);

    vec2 z = iFractalCenter + p / zoom;

    float iter = 0.0;
    const float maxIter = 400.0;
    float zDot = 0.0;

    for (float i = 0.0; i < maxIter; i++) {
        z = cmul(z, z) + iJuliaC;
        zDot = dot(z, z);
        if (zDot > 4.0) {
            iter = i;
            break;
        }
        iter = i;
    }

    float smoothIter = iter;
    if (zDot > 4.0) {
        smoothIter = iter - log(log(zDot) * 0.5) / log(2.0);
    }

    vec3 col;
    if (zDot <= 4.0) {
        // Interior - very dark
        col = vec3(0.02, 0.02, 0.04);
    } else {
        // Exterior - color based on escape time
        float t = smoothIter / 25.0 + timeOffset * iColorSpeed;
        col = palette(t);

        // Edge glow
        float edgeGlow = 1.0 / (1.0 + smoothIter * 0.06);
        col += iGlowTint * edgeGlow * iEdgeGlow;

        float brightness = smoothstep(0.0, 12.0, smoothIter) * smoothstep(maxIter, 35.0, smoothIter);
        col *= 0.5 + 1.3 * brightness;
    }

    col = col / (1.0 + col);
    col = pow(col, vec3(0.9));
    return col;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    float zoomSpeed = max(iZoomSpeed, 0.001);
    float baseZoom = max(iBaseZoom, 0.001);
    float zoomRange = max(iZoomRange, 1.01);
    float cycleDuration = log(zoomRange) / zoomSpeed;

    float t1 = mod(iTime, cycleDuration);
    float t2 = mod(iTime + cycleDuration * 0.5, cycleDuration);

    float zoom1 = baseZoom * exp(t1 * zoomSpeed);
    float zoom2 = baseZoom * exp(t2 * zoomSpeed);

    float fade1 = smoothstep(0.0, 0.2, t1 / cycleDuration)
                * smoothstep(1.0, 0.8, t1 / cycleDuration);
    float fade2 = smoothstep(0.0, 0.2, t2 / cycleDuration)
                * smoothstep(1.0, 0.8, t2 / cycleDuration);

    vec3 col1 = julia(fragCoord, zoom1, iTime);
    vec3 col2 = julia(fragCoord, zoom2, iTime);

    float totalWeight = fade1 + fade2;
    vec3 col = (col1 * fade1 + col2 * fade2) / max(totalWeight, 0.001);

    fragColor = vec4(col, 1.0);
}
