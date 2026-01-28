// Key Press Pulse Shader
// Demonstrates the iTimeKeyPress uniform with a visual pulse effect on each keystroke
//
// When you type, you'll see a radial pulse emanate from the cursor position,
// along with a subtle screen-wide flash effect.

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord / iResolution.xy;

    // Calculate time since last key press
    float timeSinceKey = iTime - iTimeKeyPress;

    // Exponential decay for smooth falloff (decays to ~5% in 0.5 seconds)
    float pulse = exp(-timeSinceKey * 6.0);

    // Only show effect for first 1 second after keypress
    pulse *= step(timeSinceKey, 1.0);

    // Sample terminal content
    vec4 terminal = texture(iChannel4, uv);

    // === Effect 1: Screen-wide brightness flash ===
    float flashIntensity = pulse * 0.15;

    // === Effect 2: Radial wave from cursor ===
    // Get cursor position in UV space
    vec2 cursorUV = iCurrentCursor.xy / iResolution.xy;

    // Distance from current pixel to cursor
    float dist = distance(uv, cursorUV);

    // Create expanding ring: radius grows over time, fades out
    float ringRadius = timeSinceKey * 0.8;  // Ring expands outward
    float ringWidth = 0.05;
    float ring = smoothstep(ringWidth, 0.0, abs(dist - ringRadius)) * pulse;

    // Ring color (cyan-ish glow)
    vec3 ringColor = vec3(0.2, 0.8, 1.0) * ring * 0.5;

    // === Effect 3: Cursor glow intensifies on keypress ===
    float cursorDist = distance(fragCoord, iCurrentCursor.xy);
    float cursorGlow = exp(-cursorDist / 40.0) * pulse * 0.3;
    vec3 cursorGlowColor = iCurrentCursorColor.rgb * cursorGlow;

    // === Combine effects ===
    // Base terminal color with brightness flash
    vec3 color = terminal.rgb * (1.0 + flashIntensity);

    // Add ring effect
    color += ringColor;

    // Add cursor glow
    color += cursorGlowColor;

    // Clamp to prevent oversaturation
    color = clamp(color, 0.0, 1.0);

    fragColor = vec4(color, terminal.a);
}
