/*! par-term shader metadata
name: singularity
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.28
  text_opacity: null
  full_content: null
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: textures/cubemaps/env-outside
  cubemap_enabled: false
*/

/*
    "Singularity" by @XorDev
    A whirling blackhole.
    https://www.shadertoy.com/view/3csSWB

    Adapted for par-term background shader.
*/

void mainImage(out vec4 O, in vec2 fragCoord)
{
    // Resolution and centered coordinates
    vec2 r = iResolution.xy;
    vec2 p = (fragCoord * 2.0 - r) / (r.y * 0.7);

    // Diagonal vector and blackhole center
    vec2 d = vec2(-1.0, 1.0);
    vec2 b = p - 0.2 * d;

    // Perspective transformation
    float perspective = 0.1 + 0.2 / dot(b, b);
    vec2 c = p * mat2(1.0, 1.0, -1.0 / perspective, 1.0 / perspective);

    // Spiral rotation
    float a = dot(c, c);
    float angle = 0.5 * log(a) + iTime * 0.2;
    float cosA = cos(angle);
    mat2 spiralMat = mat2(cosA, cos(angle + 33.0), cos(angle + 11.0), cosA);

    // Rotate into spiraling coordinates
    vec2 v = (c * spiralMat) * 5.0;  // * 5.0 = / 0.2

    // Wave accumulation loop
    vec4 w = vec4(0.0);
    for (float j = 1.0; j < 10.0; j += 1.0) {
        float invJ = 1.0 / j;
        v += 0.7 * sin(v.yx * j + iTime) * invJ + 0.5;
        vec2 sv = sin(v);
        w += 1.0 + sv.xyxy;
    }

    // Accretion disk
    float diskRadius = length(sin(v * 3.333) * 0.4 + c * (3.0 + d));
    float diskBright = 2.0 + diskRadius * (diskRadius * 0.25 - 1.0);

    // Lighting factors
    float centerDark = 0.5 + 1.0 / a;
    float rimLight = 0.03 + abs(length(p) - 0.7);

    // Combine: gradient / (waveColor * diskBright * centerDark * rimLight)
    vec4 gradient = exp(c.x * vec4(0.6, -0.4, -1.0, 0.0));
    float combined = diskBright * centerDark * rimLight;
    O = 1.0 - exp(-gradient / (w.xyyx * combined));
}
