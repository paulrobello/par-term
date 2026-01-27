// Cubemap Skybox Shader
// Demonstrates environment mapping with animated rotation
// Uses iCubemap for sampling a 6-face cubemap texture

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    // Normalized coordinates (-1 to 1)
    vec2 uv = (fragCoord - 0.5 * iResolution.xy) / min(iResolution.x, iResolution.y);

    // Create a ray direction from the camera
    // We're looking down -Z, with Y up
    vec3 rayDir = normalize(vec3(uv.x, uv.y, -1.0));

    // Rotate the view over time for animation
    float angle = iTime * 0.2; // Slow rotation
    float c = cos(angle);
    float s = sin(angle);

    // Rotate around Y axis
    rayDir = vec3(
        rayDir.x * c - rayDir.z * s,
        rayDir.y,
        rayDir.x * s + rayDir.z * c
    );

    // Sample the cubemap
    vec4 skyColor = texture(iCubemap, rayDir);

    // HDR tone mapping (Reinhard) for HDR cubemaps
    vec3 tonemapped = skyColor.rgb / (skyColor.rgb + vec3(1.0));

    // Optional: Apply gamma correction if needed
    // tonemapped = pow(tonemapped, vec3(1.0 / 2.2));

    // Blend with terminal content
    // Sample terminal from iChannel4
    vec2 termUV = fragCoord / iResolution.xy;
    vec4 terminal = texture(iChannel4, vec2(termUV.x, 1.0 - termUV.y));

    // If there's terminal content (alpha > 0), show it over the skybox
    if (terminal.a > 0.01) {
        fragColor = vec4(terminal.rgb, 1.0);
    } else {
        fragColor = vec4(tonemapped, 1.0);
    }
}
