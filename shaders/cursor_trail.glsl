// Cursor Trail Effect - Creates a fading trail from previous to current cursor position
// CONFIGURATION - adjust these values to customize the effect
const float DURATION = 0.3;               // Trail duration in seconds
const float TRAIL_WIDTH_MULT = 1.5;       // Trail width multiplier (relative to cursor size)
const float INTENSITY = 0.7;              // Trail intensity (0.0 - 1.0)
// TRAIL_COLOR uses iCurrentCursorColor uniform - assigned in mainImage()

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;

    // Sample the terminal content
    vec4 terminalColor = texture(iChannel4, uv);

    // Get cursor positions
    vec2 currentPos = iCurrentCursor.xy + iCurrentCursor.zw * 0.5;
    vec2 previousPos = iPreviousCursor.xy + iPreviousCursor.zw * 0.5;
    vec2 cursorSize = iCurrentCursor.zw;

    // Calculate time since cursor moved
    float timeSinceMove = iTime - iTimeCursorChange;

    // Trail fades out over duration
    float trailFade = 1.0 - clamp(timeSinceMove / DURATION, 0.0, 1.0);

    // Calculate distance from the line between previous and current cursor
    vec2 lineDir = currentPos - previousPos;
    float lineLen = length(lineDir);

    float trailEffect = 0.0;

    if (lineLen > 1.0 && trailFade > 0.0) {
        vec2 lineNorm = lineDir / lineLen;
        vec2 toPoint = fragCoord - previousPos;

        // Project point onto line
        float proj = dot(toPoint, lineNorm);
        proj = clamp(proj, 0.0, lineLen);

        // Distance from line
        vec2 closestPoint = previousPos + lineNorm * proj;
        float distFromLine = distance(fragCoord, closestPoint);

        // Trail width based on cursor size
        float trailWidth = max(cursorSize.x, cursorSize.y) * TRAIL_WIDTH_MULT;

        // Fade along trail length (stronger near current position)
        float lengthFade = proj / lineLen;

        // Create trail effect
        trailEffect = (1.0 - smoothstep(0.0, trailWidth, distFromLine)) * lengthFade * trailFade;
    }

    // Blend trail with terminal content
    vec3 finalColor = terminalColor.rgb + iCurrentCursorColor.rgb * trailEffect * INTENSITY;

    fragColor = vec4(finalColor, terminalColor.a);
}
