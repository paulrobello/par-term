/*! par-term shader metadata
name: spotlight
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
    iAmbientLight: 0.5
    iBackgroundDim: 0.19999999
    iEdgeSoftness: 0.049999997
    iMotionAmount: 0.39999998
    iMotionSpeed: 1.0
    iSpotlightCenter:
    - 0.5
    - 0.5
    iSpotlightRadius: 0.25
    iSpotlightTint: '#ffffff'
    iUseMouse: false
*/

// Created by Paul Robello

#define PI 3.14159265

// control slider min=0.05 max=0.85 step=0.01 label="Spotlight Radius"
uniform float iSpotlightRadius;
// control slider min=0.005 max=0.35 step=0.005 scale=log label="Edge Softness"
uniform float iEdgeSoftness;
// control slider min=0 max=1 step=0.01 label="Ambient Light"
uniform float iAmbientLight;
// control slider min=0 max=1 step=0.01 label="Background Dim"
uniform float iBackgroundDim;
// control slider min=0 max=0.5 step=0.01 label="Motion Amount"
uniform float iMotionAmount;
// control slider min=0.05 max=4 step=0.05 scale=log label="Motion Speed"
uniform float iMotionSpeed;
// control point label="Resting Center"
uniform vec2 iSpotlightCenter;
// control checkbox label="Follow Mouse"
uniform bool iUseMouse;
// control color label="Spotlight Tint"
uniform vec3 iSpotlightTint;

vec2 animatedCenter(void) {
    float t = iTime * iMotionSpeed;
    return iSpotlightCenter + iMotionAmount * vec2(
        sin(t),
        sin(t * 1.3 + PI)
    );
}

vec2 mouseCenter(vec2 fallbackCenter) {
    bool hasMouse = iMouse.x > 0.0 || iMouse.y > 0.0;
    if (iUseMouse && hasMouse) {
        return clamp(iMouse.xy / iResolution.xy, vec2(0.0), vec2(1.0));
    }
    return fallbackCenter;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord.xy / iResolution.xy;
    vec4 texColor = texture(iChannel4, uv);

    // Aspect ratio correction
    float aspect = iResolution.x / iResolution.y;

    vec2 spotlightCenter = mouseCenter(animatedCenter());

    // Distance from spotlight center (aspect-corrected)
    float dist = distance(vec2(uv.x * aspect, uv.y), vec2(spotlightCenter.x * aspect, spotlightCenter.y));

    // Spotlight intensity with soft edge
    float radius = max(iSpotlightRadius, 0.001);
    float softness = min(iEdgeSoftness, radius);
    float intensity = smoothstep(radius, radius - softness, dist);

    // Check if iChannel0 background is set
    vec3 background = vec3(0.0);
    bool hasBackground = iChannelResolution[0].x > 1.0 && iChannelResolution[0].y > 1.0;
    if (hasBackground) {
        background = texture(iChannel0, uv).rgb;
    }

    // Apply spotlight to terminal content
    vec3 litTerminal = texColor.rgb * mix(iAmbientLight, 1.0, intensity) * mix(vec3(1.0), iSpotlightTint, intensity);

    // Apply spotlight to background (with extra dimming outside spotlight)
    vec3 litBackground = background * mix(iBackgroundDim, 1.0, intensity) * mix(vec3(1.0), iSpotlightTint, intensity);

    // Composite: terminal over background (where terminal has content)
    vec3 result = mix(litBackground, litTerminal, texColor.a);

    fragColor = vec4(result, max(texColor.a, step(0.01, length(background))));
}
