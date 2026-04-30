/*! par-term shader metadata
name: debug-coords
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.8
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: null
  use_background_as_channel0: null
*/

// Debug Y-axis - tests Shadertoy compatibility
// fragCoord should be flipped automatically by the transpiler
// Expected: BRIGHT at top (high Y), DARK at bottom (low Y)

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    // Simple brightness based on Y coordinate
    float brightness = fragCoord.y / 1000.0;
    fragColor = vec4(brightness, brightness, brightness, 1.0);
}
