/*! par-term shader metadata
name: starfield
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: textures/cubemaps/env-outside
  cubemap_enabled: false
  use_background_as_channel0: null
  uniforms:
    iLayerCount: 21
    iStarBrightness: 1.0
    iStarDensity: 30.0
    iStarScale: 1.0
    iStarTint: '#ffffff'
    iWarpSpeed: 1.0
    iZoomOrigin:
    - 0.5
    - 0.5
*/

const int MAX_LAYERS = 32;

// control slider min=8 max=80 step=1 label="Star Density"
uniform float iStarDensity;
// control int min=1 max=32 step=1 label="Depth Layers"
uniform int iLayerCount;
// control slider min=0 max=3 step=0.01 label="Warp Speed"
uniform float iWarpSpeed;
// control slider min=0.25 max=3 step=0.01 label="Star Scale"
uniform float iStarScale;
// control slider min=0 max=4 step=0.01 label="Star Brightness"
uniform float iStarBrightness;
// control point label="Zoom Origin"
uniform vec2 iZoomOrigin;
// control color label="Star Tint"
uniform vec3 iStarTint;

float N21(vec2 p) {
    p = fract(p * vec2(233.34, 851.73));
    p += dot(p, p + 23.45);
    return fract(p.x * p.y);
}

vec2 N22(vec2 p) {
    float n = N21(p);
    return vec2(n, N21(p + n));
}

vec3 stars(vec2 uv, float offset, float layerCount) {
    float safeLayers = max(layerCount, 1.0);
    float timeScale = -(iTime * iWarpSpeed + offset) / safeLayers;
    float trans = fract(timeScale);
    float newRnd = floor(timeScale);
    vec3 col = vec3(0.0);

    // Translate uv then scale from the selected origin.
    uv = (uv - iZoomOrigin) * trans + iZoomOrigin;

    // Create square aspect ratio.
    uv.x *= iResolution.x / iResolution.y;

    // Create boxes.
    uv *= iStarDensity;

    // Get position.
    vec2 ipos = floor(uv);

    // Return uv as 0 to 1.
    uv = fract(uv);

    // Calculate random xy and size.
    vec2 rndXY = N22(newRnd + ipos * (offset + 1.0)) * 0.9 + 0.05;
    float rndSize = (N21(ipos) * 100.0 + 200.0) * max(iStarScale, 0.001);

    vec2 j = (rndXY - uv) * rndSize;
    float sparkle = 1.0 / dot(j, j);

    col += sparkle * smoothstep(1.0, 0.8, trans) * iStarBrightness * iStarTint;
    return col;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord / iResolution.xy;

    vec3 col = vec3(0.0);
    float layerCount = float(iLayerCount);
    for (int i = 0; i < MAX_LAYERS; i++) {
        float activeLayer = step(float(i), layerCount - 0.5);
        col += stars(uv, float(i), layerCount) * activeLayer;
    }

    fragColor = vec4(col, 1.0);
}
