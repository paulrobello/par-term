/*! par-term shader metadata
name: sin-interference
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

// Based on https://www.shadertoy.com/view/ms3cWn
// Optimized: removed redundant calculations, simplified math

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 uv = fragCoord / iResolution.xy;
    vec2 centered = uv - 0.5;
    float d = length(centered) * 2.0;

    // Color gradient based on distance and time
    float t = d * d * 1.25 - iTime * 0.1;  // Simplified: 25/20 = 1.25, 2/20 = 0.1
    vec3 col = 0.5 + 0.5 * cos(t + uv.xyx + vec3(0.0, 2.0, 4.0));

    // Interference from center (reuse centered vector)
    float dCSin = sin(length(centered) * iResolution.x * 0.05);

    // Animated interference point - map [-1,1] to [0,1] then scale
    vec2 animUV = (vec2(sin(iTime), sin(iTime * 1.25)) + 1.0) * 0.5;
    float dMSin = sin(length(fragCoord - animUV * iResolution.xy) * 0.05);

    // Combined interference with vignette falloff
    // Original: ((dMSin * dCSin + 1) * 0.5) * (0.5 - d * 0.3536)
    // where 0.3536 ≈ 0.5/sqrt(2)
    float greycol = (dMSin * dCSin + 1.0) * 0.25 * max(0.0, 1.0 - d * 0.707);

    fragColor = vec4(greycol * col, 1.0);
}
