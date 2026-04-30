/*! par-term shader metadata
name: underwater
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
    iDeepTint: '#0d3266'
    iDepthFade: 1.0
    iDitherAmount: 0.015000001
    iLightOrigin:
    - 0.75
    - 1.15
    iRayIntensity: 1.0
    iRaySpeed: 1.0
    iShallowTint: '#d9fff0'
*/

// adapted by Alex Sherwin for Ghostty from https://www.shadertoy.com/view/lljGDt

// control slider min=0 max=2 step=0.01 label="Ray Intensity"
uniform float iRayIntensity;
// control slider min=0 max=3 step=0.01 label="Ray Speed"
uniform float iRaySpeed;
// control slider min=0 max=2 step=0.01 label="Depth Fade"
uniform float iDepthFade;
// control slider min=0 max=0.05 step=0.001 label="Dither"
uniform float iDitherAmount;
// control vec2 min=-0.5 max=1.5 step=0.01 label="Light Origin"
uniform vec2 iLightOrigin;
// control color label="Shallow Tint"
uniform vec3 iShallowTint;
// control color label="Deep Tint"
uniform vec3 iDeepTint;

float hash21(vec2 p) {
    p = fract(p * vec2(233.34, 851.73));
    p += dot(p, p + 23.45);
    return fract(p.x * p.y);
}

float rayStrength(vec2 raySource, vec2 rayRefDirection, vec2 coord, float seedA, float seedB, float speed)
{
    vec2 sourceToCoord = coord - raySource;
    float cosAngle = dot(normalize(sourceToCoord), rayRefDirection);

    // Add subtle dithering based on screen coordinates
    float dither = hash21(coord) * iDitherAmount - iDitherAmount * 0.5;
    float rayTime = iTime * speed * iRaySpeed;

    float ray = clamp(
        (0.45 + 0.15 * sin(cosAngle * seedA + rayTime)) +
        (0.3 + 0.2 * cos(-cosAngle * seedB + rayTime)) + dither,
        0.0, 1.0);

    // Smoothstep the distance falloff
    float fadeWidth = max(iResolution.x * max(iDepthFade, 0.001), 1.0);
    float distFade = smoothstep(0.0, fadeWidth, iResolution.x - length(sourceToCoord));
    return ray * mix(0.5, 1.0, distFade);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 coord = fragCoord.xy;

    // Sun ray parameters
    vec2 rayPos1 = iResolution.xy * iLightOrigin;
    vec2 rayPos2 = iResolution.xy * (iLightOrigin + vec2(0.1, 0.1));
    vec2 rayRefDir1 = normalize(vec2(1.0, 0.116));
    vec2 rayRefDir2 = normalize(vec2(1.0, -0.241));

    // Calculate sun rays
    float rays1 = rayStrength(rayPos1, rayRefDir1, coord, 36.2214, 21.11349, 1.1);
    float rays2 = rayStrength(rayPos2, rayRefDir2, coord, 22.3991, 18.0234, 0.9);
    float rays = (rays1 * 0.5 + rays2 * 0.4) * iRayIntensity;

    // Attenuate brightness towards bottom, add blue-green tinge
    float brightness = 1.0 - coord.y / iResolution.y;
    vec3 waterTint = mix(iDeepTint, iShallowTint, brightness);
    vec3 col = rays * waterTint;

    fragColor = vec4(col, 1.0);
}
