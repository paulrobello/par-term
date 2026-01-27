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

    // Also add slight pitch based on mouse Y (if available)
    float pitch = iMouse.z > 0.0 ? (iMouse.y / iResolution.y - 0.5) * 1.5 : sin(iTime * 0.1) * 0.3;
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

    // Sample terminal content
    vec4 terminal = texture(iChannel4, vec2(uv.x, 1.0 - uv.y));

    // Composite: show terminal where there's content, skybox elsewhere
    if (terminal.a > 0.01) {
        // Blend terminal with slight reflection of sky
        vec3 finalColor = mix(mapped * 0.3, terminal.rgb, 0.85);
        fragColor = vec4(finalColor, 1.0);
    } else {
        fragColor = vec4(mapped, 1.0);
    }
}
