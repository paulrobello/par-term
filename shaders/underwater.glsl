/*! par-term shader metadata
name: underwater
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.3
  text_opacity: null
  full_content: null
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: textures/cubemaps/env-outside
  cubemap_enabled: false
*/

// adapted by Alex Sherwin for Ghostty from https://www.shadertoy.com/view/lljGDt

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
    float dither = hash21(coord) * 0.015 - 0.0075;
    
    float ray = clamp(
        (0.45 + 0.15 * sin(cosAngle * seedA + iTime * speed)) +
        (0.3 + 0.2 * cos(-cosAngle * seedB + iTime * speed)) + dither,
        0.0, 1.0);
        
    // Smoothstep the distance falloff
    float distFade = smoothstep(0.0, iResolution.x, iResolution.x - length(sourceToCoord));
    return ray * mix(0.5, 1.0, distFade);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 coord = fragCoord.xy;

    // Sun ray parameters
    vec2 rayPos1 = iResolution.xy * vec2(0.7, 1.1);
    vec2 rayPos2 = iResolution.xy * vec2(0.8, 1.2);
    vec2 rayRefDir1 = normalize(vec2(1.0, 0.116));
    vec2 rayRefDir2 = normalize(vec2(1.0, -0.241));

    // Calculate sun rays
    float rays1 = rayStrength(rayPos1, rayRefDir1, coord, 36.2214, 21.11349, 1.1);
    float rays2 = rayStrength(rayPos2, rayRefDir2, coord, 22.3991, 18.0234, 0.9);
    float rays = rays1 * 0.5 + rays2 * 0.4;

    // Attenuate brightness towards bottom, add blue-green tinge
    float brightness = 1.0 - coord.y / iResolution.y;
    vec3 col = rays * vec3(
        0.05 + brightness * 0.8,
        0.15 + brightness * 0.6,
        0.3 + brightness * 0.5
    );

    fragColor = vec4(col, 1.0);
}
