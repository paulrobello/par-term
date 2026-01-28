/*! par-term shader metadata
name: spotlight
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

// Created by Paul Robello

#define PI 3.14159265
#define SPOTLIGHT_RADIUS 0.25
#define SPOTLIGHT_SOFTNESS 0.05  // 1.0 / 20.0
#define AMBIENT_LIGHT 0.5
#define BACKGROUND_DIM 0.2  // Extra dimming for background outside spotlight

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord.xy / iResolution.xy;
    vec4 texColor = texture(iChannel4, uv);

    // Aspect ratio correction
    float aspect = iResolution.x / iResolution.y;

    // Spotlight center moving with smooth oscillation
    vec2 spotlightCenter = vec2(
        0.5 + 0.4 * sin(iTime),           // X motion
        0.5 + 0.4 * sin(iTime * 1.3 + PI) // Y motion with different frequency and phase
    );

    // Distance from spotlight center (aspect-corrected)
    float dist = distance(vec2(uv.x * aspect, uv.y), vec2(spotlightCenter.x * aspect, spotlightCenter.y));

    // Spotlight intensity with soft edge
    float intensity = smoothstep(SPOTLIGHT_RADIUS, SPOTLIGHT_RADIUS - SPOTLIGHT_SOFTNESS, dist);

    // Check if iChannel0 background is set
    vec3 background = vec3(0.0);
    if (iChannelResolution[0].x > 0.0) {
        background = texture(iChannel0, uv).rgb;
    }

    // Apply spotlight to terminal content
    vec3 litTerminal = texColor.rgb * mix(AMBIENT_LIGHT, 1.0, intensity);

    // Apply spotlight to background (with extra dimming outside spotlight)
    vec3 litBackground = background * mix(BACKGROUND_DIM, 1.0, intensity);

    // Composite: terminal over background (where terminal has content)
    vec3 result = mix(litBackground, litTerminal, texColor.a);

    fragColor = vec4(result, max(texColor.a, step(0.01, length(background))));
}