/*! par-term shader metadata
name: singularity
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.25
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: textures/cubemaps/env-outside
  cubemap_enabled: false
  use_background_as_channel0: null
  uniforms:
    iCenterDarkness: 1.0
    iCenterOffset: 0.19999996
    iColorGradientB: -1.0
    iColorGradientRG:
    - 0.6
    - -0.4
    iDiskBrightness: 1.0
    iDiskScale: 1.0
    iExposure: 1.0
    iGravityStrength: 0.19999999
    iPerspectiveBase: 0.099999994
    iRimLight: 0.030000001
    iSpinSpeed: 0.19999996
    iSpiralScale: 5.0
    iSpiralTwist: 0.49999994
    iTurbulence: 0.7
    iWaveCount: 9
    iWaveDrift: 0.5
    iZoom: 0.7
*/

/*
    "Singularity" by @XorDev
    A whirling blackhole.
    https://www.shadertoy.com/view/3csSWB

    Adapted for par-term background shader.
*/

#define MAX_WAVE_COUNT 16

// control slider min=-0.6 max=0.6 step=0.01 label="Center Offset"
uniform float iCenterOffset;
// control slider min=0.35 max=1.5 step=0.01 label="Zoom"
uniform float iZoom;
// control slider min=0.01 max=1 step=0.01 scale=log label="Perspective Base"
uniform float iPerspectiveBase;
// control slider min=0.01 max=1.5 step=0.01 scale=log label="Gravity Strength"
uniform float iGravityStrength;
// control slider min=-2 max=2 step=0.01 label="Spin Speed"
uniform float iSpinSpeed;
// control slider min=-2 max=2 step=0.01 label="Spiral Twist"
uniform float iSpiralTwist;
// control slider min=1 max=12 step=0.05 label="Spiral Scale"
uniform float iSpiralScale;
// control slider min=0 max=2 step=0.01 label="Turbulence"
uniform float iTurbulence;
// control slider min=0 max=2 step=0.01 label="Wave Drift"
uniform float iWaveDrift;
// control int min=1 max=16 step=1 label="Wave Count"
uniform int iWaveCount;
// control slider min=0.25 max=3 step=0.01 label="Disk Scale"
uniform float iDiskScale;
// control slider min=0.1 max=3 step=0.01 label="Disk Brightness"
uniform float iDiskBrightness;
// control slider min=0.1 max=3 step=0.01 label="Center Darkness"
uniform float iCenterDarkness;
// control slider min=0.001 max=0.2 step=0.001 scale=log label="Rim Light"
uniform float iRimLight;
// control slider min=0.1 max=4 step=0.01 scale=log label="Exposure"
uniform float iExposure;
// control vec2 min=-2 max=2 step=0.01 label="Color Gradient RG"
uniform vec2 iColorGradientRG;
// control slider min=-2 max=2 step=0.01 label="Color Gradient B"
uniform float iColorGradientB;

float activeWaveMask(int index, int count) {
    return step(float(index) + 0.5, float(count));
}

void mainImage(out vec4 O, in vec2 fragCoord)
{
    // Resolution and centered coordinates
    vec2 r = iResolution.xy;
    vec2 p = (fragCoord * 2.0 - r) / (r.y * max(iZoom, 0.001));

    // Diagonal vector and blackhole center
    vec2 d = vec2(-1.0, 1.0);
    vec2 b = p - iCenterOffset * d;

    // Perspective transformation
    float perspective = iPerspectiveBase + iGravityStrength / max(dot(b, b), 0.0001);
    vec2 c = p * mat2(1.0, 1.0, -1.0 / perspective, 1.0 / perspective);

    // Spiral rotation
    float a = max(dot(c, c), 0.0001);
    float angle = iSpiralTwist * log(a) + iTime * iSpinSpeed;
    float cosA = cos(angle);
    mat2 spiralMat = mat2(cosA, cos(angle + 33.0), cos(angle + 11.0), cosA);

    // Rotate into spiraling coordinates
    vec2 v = (c * spiralMat) * iSpiralScale;

    // Wave accumulation loop
    vec4 w = vec4(0.0);
    int waveCount = clamp(iWaveCount, 1, MAX_WAVE_COUNT);
    for (int i = 0; i < MAX_WAVE_COUNT; i++) {
        float mask = activeWaveMask(i, waveCount);
        float j = float(i + 1);
        float invJ = 1.0 / j;
        v += (iTurbulence * sin(v.yx * j + iTime) * invJ + iWaveDrift) * mask;
        vec2 sv = sin(v);
        w += (1.0 + sv.xyxy) * mask;
    }

    // Accretion disk
    float diskRadius = length(sin(v * 3.333) * 0.4 + c * (3.0 + d) * iDiskScale);
    float diskBright = (2.0 + diskRadius * (diskRadius * 0.25 - 1.0)) * iDiskBrightness;

    // Lighting factors
    float centerDark = 0.5 + iCenterDarkness / a;
    float rimLight = iRimLight + abs(length(p) - 0.7);

    // Combine: gradient / (waveColor * diskBright * centerDark * rimLight)
    vec3 gradientRGB = vec3(iColorGradientRG, iColorGradientB);
    vec4 gradient = exp(c.x * vec4(gradientRGB, 0.0));
    float combined = max(diskBright * centerDark * rimLight, 0.0001);
    O = 1.0 - exp(-gradient * iExposure / max(w.xyyx * combined, vec4(0.0001)));
}
