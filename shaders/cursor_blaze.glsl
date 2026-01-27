// Cursor Blaze Effect - Combined glow + trail for dramatic cursor effects
// CONFIGURATION
const float DURATION = 0.35;              // Trail duration in seconds
const float GLOW_RADIUS = 40.0;           // Glow radius in pixels  
const float INTENSITY = 1.0;              // Effect intensity (0.0 - 1.0)
// EFFECT_COLOR uses iCurrentCursorColor uniform - assigned in mainImage()

float easeOutQuad(float t) {
    return 1.0 - (1.0 - t) * (1.0 - t);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec4 terminalColor = texture(iChannel4, uv);

    // Get cursor positions
    vec2 currentPos = iCurrentCursor.xy + iCurrentCursor.zw * 0.5;
    vec2 previousPos = iPreviousCursor.xy + iPreviousCursor.zw * 0.5;
    vec2 cursorSize = iCurrentCursor.zw;

    // Time-based animation
    float timeSinceMove = iTime - iTimeCursorChange;
    float progress = clamp(timeSinceMove / DURATION, 0.0, 1.0);
    float trailFade = 1.0 - easeOutQuad(progress);

    // === Glow around current cursor ===
    float distToCursor = distance(fragCoord, currentPos);
    float glow = 1.0 - smoothstep(0.0, GLOW_RADIUS, distToCursor);
    glow = pow(glow, 2.5) * INTENSITY;

    // === Trail from previous to current ===
    float trailEffect = 0.0;
    vec2 lineDir = currentPos - previousPos;
    float lineLen = length(lineDir);

    if (lineLen > 2.0 && trailFade > 0.01) {
        vec2 lineNorm = lineDir / lineLen;
        vec2 toPoint = fragCoord - previousPos;
        float proj = clamp(dot(toPoint, lineNorm), 0.0, lineLen);

        vec2 closestPoint = previousPos + lineNorm * proj;
        float distFromLine = distance(fragCoord, closestPoint);

        float trailWidth = max(cursorSize.x, cursorSize.y) * 2.0;
        float lengthFade = pow(proj / lineLen, 0.5);

        trailEffect = (1.0 - smoothstep(0.0, trailWidth, distFromLine))
                    * lengthFade * trailFade * INTENSITY * 0.6;
    }

    // Combine effects
    float totalEffect = max(glow, trailEffect);

    // Additive blend with terminal content
    vec3 finalColor = terminalColor.rgb + iCurrentCursorColor.rgb * totalEffect * 0.5;

    fragColor = vec4(finalColor, terminalColor.a);
}
