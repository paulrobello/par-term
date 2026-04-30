/*! par-term shader metadata
name: sin-interference
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
    iBluePhase: 3.9969997
    iCenter:
    - 0.5
    - 0.5
    iColorPhase:
    - 0.0
    - 2.0
    iColorSpeed: 0.09999995
    iColorSpread: 1.25
    iContrast: 0.25
    iIntensity: 1.0
    iMotionAmount: 0.5
    iMotionRatio: 1.25
    iMotionSpeed: 0.9999999
    iVignette: 0.71
    iWaveFrequency: 0.05
*/

// Based on https://www.shadertoy.com/view/ms3cWn
// Optimized: removed redundant calculations, simplified math

// control point label="Center"
uniform vec2 iCenter;
// control slider min=0.005 max=0.2 step=0.001 scale=log label="Wave Frequency"
uniform float iWaveFrequency;
// control slider min=0 max=5 step=0.01 label="Color Spread"
uniform float iColorSpread;
// control slider min=-2 max=2 step=0.01 label="Color Speed"
uniform float iColorSpeed;
// control slider min=-4 max=4 step=0.01 label="Motion Speed"
uniform float iMotionSpeed;
// control slider min=0.1 max=4 step=0.01 scale=log label="Motion Ratio"
uniform float iMotionRatio;
// control slider min=0 max=1 step=0.01 label="Motion Amount"
uniform float iMotionAmount;
// control slider min=0 max=1 step=0.01 label="Contrast"
uniform float iContrast;
// control slider min=0 max=2 step=0.01 label="Vignette"
uniform float iVignette;
// control slider min=0 max=4 step=0.01 label="Intensity"
uniform float iIntensity;
// control vec2 min=-6.283 max=6.283 step=0.01 label="Color Phase RG"
uniform vec2 iColorPhase;
// control slider min=-6.283 max=6.283 step=0.01 label="Color Phase B"
uniform float iBluePhase;

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 uv = fragCoord / iResolution.xy;
    vec2 centered = uv - iCenter;
    float d = length(centered) * 2.0;

    // Color gradient based on distance and time
    float t = d * d * iColorSpread - iTime * iColorSpeed;
    vec3 col = 0.5 + 0.5 * cos(t + uv.xyx + vec3(iColorPhase, iBluePhase));

    // Interference from center (reuse centered vector)
    float dCSin = sin(length(centered) * iResolution.x * iWaveFrequency);

    // Animated interference point - map [-1,1] to [0,1], then scale around center
    float motionTime = iTime * iMotionSpeed;
    vec2 orbit = vec2(sin(motionTime), sin(motionTime * iMotionRatio));
    vec2 animUV = iCenter + orbit * iMotionAmount;
    float dMSin = sin(length(fragCoord - animUV * iResolution.xy) * iWaveFrequency);

    // Combined interference with vignette falloff
    float greycol = (dMSin * dCSin + 1.0) * iContrast * max(0.0, 1.0 - d * iVignette);

    fragColor = vec4(greycol * col * iIntensity, 1.0);
}
