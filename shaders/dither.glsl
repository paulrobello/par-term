/*! par-term shader metadata
name: dither
author: null
description: Ordered dithering effect using 4x4 Bayer matrix
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: null
  text_opacity: null
  full_content: true
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: ''
  cubemap_enabled: false
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

#define LEVELS 4.0  // Color levels per channel (2=harsh, 4=moderate, 8=subtle)

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec3 color = texture(iChannel4, uv).rgb;

    // Get threshold from Bayer matrix
    int x = int(fragCoord.x) & 3;
    int y = int(fragCoord.y) & 3;
    float threshold = bayerMatrix[y][x];

    // Apply ordered dithering
    vec3 dithered = floor(color * LEVELS + threshold) / LEVELS;

    fragColor = vec4(dithered, 1.0);
}
