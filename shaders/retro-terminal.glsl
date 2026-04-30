/*! par-term shader metadata
name: retro-terminal
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: null
  full_content: true
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: textures/cubemaps/env-outside
  cubemap_enabled: false
  use_background_as_channel0: null
  uniforms:
    iCurvature: 0.25
    iScanlineIntensity: 0.25
    iTint: '#00cc99'
    iTintStrength: 1.0
*/

// Original shader collected from: https://www.shadertoy.com/view/WsVSzV
// Licensed under Shadertoy's default since the original creator didn't provide any license. (CC BY NC SA 3.0)
// Slight modifications were made to give a green-ish effect.

// control slider min=0 max=0.8 step=0.01 label="Curvature"
uniform float iCurvature;
// control slider min=0 max=0.8 step=0.01 label="Scanlines"
uniform float iScanlineIntensity;
// control color label="Tint"
uniform vec3 iTint;
// control slider min=0 max=1 step=0.01 label="Tint Strength"
uniform float iTintStrength;

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord / iResolution.xy;

    // squared distance from center for barrel distortion
    vec2 dc = uv - 0.5;
    vec2 dc2 = dc * dc;

    // apply barrel distortion (CRT curvature)
    float warp = clamp(iCurvature, 0.0, 0.8);
    uv = 0.5 + dc * (1.0 + vec2(dc2.y * 0.3 * warp, dc2.x * 0.4 * warp));

    // black outside bounds, render inside
    if (any(lessThan(uv, vec2(0.0))) || any(greaterThan(uv, vec2(1.0)))) {
        fragColor = vec4(0.0, 0.0, 0.0, 1.0);
    } else {
        // scanline: cheap triangle wave, darker on even rows
        float scanline = abs(fract(fragCoord.y * 0.5) - 0.5) * 2.0 * clamp(iScanlineIntensity, 0.0, 0.8);

        // sample terminal and apply tint with scanline darkening
        vec3 tint = mix(vec3(1.0), iTint, clamp(iTintStrength, 0.0, 1.0));
        vec3 color = texture(iChannel4, uv).rgb * tint * (1.0 - scanline);
        fragColor = vec4(color, 1.0);
    }
}
