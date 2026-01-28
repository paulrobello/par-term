/*! par-term shader metadata
name: cubemap-test
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
  cubemap_enabled: null
*/

// Cubemap Test Shader
// Simple skybox with rotating view to test cubemap functionality

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;

    // Convert screen coords to normalized device coords (-1 to 1)
    vec2 ndc = (fragCoord - 0.5 * iResolution.xy) / min(iResolution.x, iResolution.y);

    // Create ray direction from screen position
    // Looking into -Z with slight perspective
    vec3 rayDir = normalize(vec3(ndc.x, ndc.y, -1.0));

    // Rotate the view over time
    float angle = iTime * 0.3;
    float c = cos(angle);
    float s = sin(angle);

    // Rotate around Y axis
    rayDir = vec3(
        rayDir.x * c - rayDir.z * s,
        rayDir.y,
        rayDir.x * s + rayDir.z * c
    );

    // Pitch controlled by mouse Y position (inverted so up looks up)
    float pitch = (0.5 - iMouse.y / iResolution.y) * 1.5;
    float cp = cos(pitch);
    float sp = sin(pitch);
    rayDir = vec3(
        rayDir.x,
        rayDir.y * cp - rayDir.z * sp,
        rayDir.y * sp + rayDir.z * cp
    );

    // Sample the cubemap
    vec4 skyColor = texture(iCubemap, rayDir);

    // Simple tone mapping for HDR cubemaps
    vec3 mapped = skyColor.rgb / (skyColor.rgb + vec3(1.0));

    fragColor = vec4(mapped, 1.0);
}
