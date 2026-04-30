/*! par-term shader metadata
name: starfield-colors
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
    iCenter:
    - 0.5
    - 0.5
    iIntensity: 1.0
    iLayerCount: 21
    iSaturation: 1.0
    iStarDensity: 30.0
    iStarScale: 1.0
    iWarpSpeed: 1.0
*/

const float maxLayers = 32.0;

// control slider min=8 max=80 step=1 label="Star Density"
uniform float iStarDensity;
// control int min=1 max=32 step=1 label="Depth Layers"
uniform int iLayerCount;
// control slider min=0.05 max=4 step=0.05 scale=log label="Warp Speed"
uniform float iWarpSpeed;
// control slider min=0.35 max=3 step=0.05 scale=log label="Star Scale"
uniform float iStarScale;
// control slider min=0 max=3 step=0.05 label="Intensity"
uniform float iIntensity;
// control slider min=0 max=1.5 step=0.05 label="Color Saturation"
uniform float iSaturation;
// control point label="Center"
uniform vec2 iCenter;

// star colours
const vec3 blue = vec3(0.2, 0.251, 0.765);
const vec3 cyan = vec3(0.459, 0.98, 0.996);
const vec3 yellow = vec3(0.984, 0.961, 0.173);
const vec3 red = vec3(0.969, 0.008, 0.078);

// spectrum function
vec3 spectrum(vec2 pos) {
    float x = pos.x * 4.0;
    float f = fract(x);
    vec3 outCol;
    if (x < 1.0) {
        outCol = mix(blue, cyan, f);
    } else if (x < 2.0) {
        outCol = mix(cyan, vec3(1.0), f);
    } else if (x < 3.0) {
        outCol = mix(vec3(1.0), yellow, f);
    } else {
        outCol = mix(yellow, red, f);
    }
    return 1.0 - pos.y * (1.0 - outCol);
}

vec3 saturateColor(vec3 color, float amount) {
    float luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
    return mix(vec3(luma), color, amount);
}

float N21(vec2 p) {
    p = fract(p * vec2(233.34, 851.73));
    p += dot(p, p + 23.45);
    return fract(p.x * p.y);
}

vec2 N22(vec2 p) {
    float n = N21(p);
    return vec2(n, N21(p + n));
}

vec3 stars(vec2 uv, float offset) {
    float layerCount = max(1.0, float(iLayerCount));
    float timeScale = -(iTime * iWarpSpeed + offset) / layerCount;
    float trans = fract(timeScale);
    float newRnd = floor(timeScale);
    vec3 col = vec3(0.);

    // Translate uv then scale for center
    uv = (uv - iCenter) * trans + iCenter;

    // Create square aspect ratio
    uv.x *= iResolution.x / iResolution.y;

    // Create boxes
    uv *= iStarDensity;

    // Get position
    vec2 ipos = floor(uv);

    // Return uv as 0 to 1
    uv = fract(uv);

    // Calculate random xy and size
    vec2 rndXY = N22(newRnd + ipos * (offset + 1.)) * 0.9 + 0.05;
    float rndSize = (N21(ipos) * 100. + 200.) * iStarScale;

    vec2 j = (rndXY - uv) * rndSize;
    float sparkle = 1. / dot(j, j);

    // Set stars to be pure white
    col += spectrum(fract(rndXY * newRnd * ipos)) * vec3(sparkle);

    col *= smoothstep(1., 0.8, trans);
    return col; // Return pure white stars only
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord / iResolution.xy;

    vec3 col = vec3(0.0);
    float layerCount = max(1.0, float(iLayerCount));
    for (float i = 0.0; i < maxLayers; i++) {
        float layerMask = step(i + 0.5, layerCount);
        col += stars(uv, i) * layerMask;
    }

    col = saturateColor(col, iSaturation) * iIntensity;

    fragColor = vec4(col, 1.0);
}
