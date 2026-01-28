/*! par-term shader metadata
name: gradient-background
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.25
  text_opacity: null
  full_content: null
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: textures/cubemaps/env-outside
  cubemap_enabled: false
*/

// credits: https://github.com/unkn0wncode
void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord.xy / iResolution.xy;

    // Diagonal gradient from bottom-right to top-left
    float gradientFactor = (uv.x + uv.y) * 0.5;

    vec3 gradientStartColor = vec3(0.1, 0.1, 0.5); // dark blue
    vec3 gradientEndColor = vec3(0.5, 0.1, 0.1);   // dark red

    fragColor = vec4(mix(gradientStartColor, gradientEndColor, gradientFactor), 1.0);
}