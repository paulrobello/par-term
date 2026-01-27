// Cursor Glow Effect - Creates a soft glow around the cursor position
// CONFIGURATION
const float GLOW_RADIUS = 40.0;           // Glow radius in pixels
const float GLOW_INTENSITY = 1.0;         // Glow intensity (0.0 - 1.0)
// GLOW_COLOR uses iCurrentCursorColor uniform - assigned in mainImage()

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;

    // Sample the terminal content
    vec4 terminalColor = texture(iChannel4, uv);

    // Get cursor center position (add half cell size to center it)
    vec2 cursorCenter = iCurrentCursor.xy + iCurrentCursor.zw * 0.5;

    // Calculate distance from cursor
    float dist = distance(fragCoord, cursorCenter);

    // Create smooth falloff glow
    float glow = 1.0 - smoothstep(0.0, GLOW_RADIUS, dist);
    glow = pow(glow, 2.0) * GLOW_INTENSITY;

    // Blend glow with terminal content
    vec3 finalColor = terminalColor.rgb + iCurrentCursorColor.rgb * glow * 0.5;

    fragColor = vec4(finalColor, terminalColor.a);
}
