/*! par-term shader metadata
name: retro-terminal
author: null
description: null
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
  cubemap: textures/cubemaps/env-outside
  cubemap_enabled: false
*/

// Original shader collected from: https://www.shadertoy.com/view/WsVSzV
// Licensed under Shadertoy's default since the original creator didn't provide any license. (CC BY NC SA 3.0)
// Slight modifications were made to give a green-ish effect.

const float WARP = 0.25;          // CRT curvature amount
const float WARP_X = 0.3 * WARP;  // horizontal warp factor
const float WARP_Y = 0.4 * WARP;  // vertical warp factor
const float SCAN_INTENSITY = 0.25; // scanline darkness (0.5 * 0.5)
const vec3 TEAL_TINT = vec3(0.0, 0.8, 0.6);

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord / iResolution.xy;

    // squared distance from center for barrel distortion
    vec2 dc = uv - 0.5;
    vec2 dc2 = dc * dc;

    // apply barrel distortion (CRT curvature)
    uv = 0.5 + dc * (1.0 + vec2(dc2.y * WARP_X, dc2.x * WARP_Y));

    // black outside bounds, render inside
    if (any(lessThan(uv, vec2(0.0))) || any(greaterThan(uv, vec2(1.0)))) {
        fragColor = vec4(0.0, 0.0, 0.0, 1.0);
    } else {
        // scanline: cheap triangle wave, darker on even rows
        float scanline = abs(fract(fragCoord.y * 0.5) - 0.5) * 2.0 * SCAN_INTENSITY;

        // sample terminal and apply teal tint with scanline darkening
        vec3 color = texture(iChannel4, uv).rgb * TEAL_TINT * (1.0 - scanline);
        fragColor = vec4(color, 1.0);
    }
}
