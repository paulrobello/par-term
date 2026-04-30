/*! par-term shader metadata
name: dither
author: null
description: Ordered dithering effect using 4x4 Bayer matrix
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: null
  full_content: true
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: ''
  cubemap_enabled: false
  use_background_as_channel0: null
  uniforms:
    iColorMode: 0
    iDitherLevels: 4
    iDitherStrength: 1.0
    iPatternScale: 1
    iThresholdWeight: 1.0
*/

// Ordered dithering effect
// Original by moni-dz (https://github.com/moni-dz)
// CC BY-NC-SA 4.0 (https://creativecommons.org/licenses/by-nc-sa/4.0/)

// Standard 4x4 Bayer matrix (normalized to 0-1 range)
// Values: 0,8,2,10 / 12,4,14,6 / 3,11,1,9 / 15,7,13,5
const mat4 bayerMatrix = mat4(
     0.0/16.0,  8.0/16.0,  2.0/16.0, 10.0/16.0,
    12.0/16.0,  4.0/16.0, 14.0/16.0,  6.0/16.0,
     3.0/16.0, 11.0/16.0,  1.0/16.0,  9.0/16.0,
    15.0/16.0,  7.0/16.0, 13.0/16.0,  5.0/16.0
);

// control int min=2 max=16 step=1 label="Color Levels"
uniform int iDitherLevels;
// control slider min=0 max=1 step=0.01 label="Dither Strength"
uniform float iDitherStrength;
// control int min=1 max=8 step=1 label="Pattern Scale"
uniform int iPatternScale;
// control slider min=0 max=2 step=0.01 label="Threshold Weight"
uniform float iThresholdWeight;
// control select options="color,mono,amber,green" label="Color Mode"
uniform int iColorMode;

vec3 applyColorMode(vec3 color) {
    if (iColorMode == 1) {
        float luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
        return vec3(luma);
    }
    if (iColorMode == 2) {
        float luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
        return luma * vec3(1.0, 0.62, 0.22);
    }
    if (iColorMode == 3) {
        float luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
        return luma * vec3(0.34, 1.0, 0.42);
    }
    return color;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec3 color = applyColorMode(texture(iChannel4, uv).rgb);

    // Get threshold from Bayer matrix
    float patternScale = float(max(iPatternScale, 1));
    int x = int(floor(fragCoord.x / patternScale)) & 3;
    int y = int(floor(fragCoord.y / patternScale)) & 3;
    float threshold = (bayerMatrix[y][x] - 0.5) * iThresholdWeight + 0.5;

    // Apply ordered dithering
    float levels = float(max(iDitherLevels, 2));
    vec3 dithered = floor(color * levels + threshold) / levels;
    vec3 mixedColor = mix(color, dithered, clamp(iDitherStrength, 0.0, 1.0));

    fragColor = vec4(mixedColor, 1.0);
}
