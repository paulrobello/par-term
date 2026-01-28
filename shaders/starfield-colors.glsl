/*! par-term shader metadata
name: starfield-colors
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.22
  text_opacity: null
  full_content: null
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: textures/cubemaps/env-outside
  cubemap_enabled: false
*/

// divisions of grid
const float repeats = 30.;

// number of layers
const float layers = 21.;

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
    float timeScale = -(iTime + offset) / layers;
    float trans = fract(timeScale);
    float newRnd = floor(timeScale);
    vec3 col = vec3(0.);

    // Translate uv then scale for center
    uv = (uv - 0.5) * trans + 0.5;

    // Create square aspect ratio
    uv.x *= iResolution.x / iResolution.y;

    // Create boxes
    uv *= repeats;

    // Get position
    vec2 ipos = floor(uv);

    // Return uv as 0 to 1
    uv = fract(uv);

    // Calculate random xy and size
    vec2 rndXY = N22(newRnd + ipos * (offset + 1.)) * 0.9 + 0.05;
    float rndSize = N21(ipos) * 100. + 200.;

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
    for (float i = 0.0; i < layers; i++) {
        col += stars(uv, i);
    }

    fragColor = vec4(col, 1.0);
}
