// Pac-Man Cursor Shader
// Draws a Pac-Man at the cursor position that faces left or right
// based on cursor movement direction. Defaults to facing right.
//
// Usage: Set in config.yaml:
//   cursor_shader: "cursor_pacman.glsl"
//   cursor_shader_enabled: true
//   cursor_shader_animation: true

// Signed distance function for a circle
float sdCircle(vec2 p, float r) {
    return length(p) - r;
}

// Pac-Man SDF: circle with a wedge cut out for the mouth
float sdPacman(vec2 p, float r, float mouthAngle, bool facingRight) {
    // Flip horizontally if facing left
    if (!facingRight) {
        p.x = -p.x;
    }

    // Distance to the circle
    float dCircle = length(p) - r;

    // Check if point is in the mouth wedge angular region
    float angle = atan(p.y, p.x);
    bool inMouthAngle = abs(angle) < mouthAngle && p.x > 0.0;

    if (!inMouthAngle) {
        // Not in mouth region - just return circle distance
        return dCircle;
    }

    // We're in the angular region of the mouth
    // Calculate mouth geometry
    float c = cos(mouthAngle);
    float s = sin(mouthAngle);

    // Lip corner positions (where mouth meets circle)
    vec2 upperLip = vec2(r * c, r * s);
    vec2 lowerLip = vec2(r * c, -r * s);

    // Distance to lip corners
    float dUpperLip = length(p - upperLip);
    float dLowerLip = length(p - lowerLip);
    float dLipCorner = min(dUpperLip, dLowerLip);

    if (dCircle > 0.0) {
        // Outside the circle in the mouth area - completely outside Pac-Man
        // Return large distance to prevent any drawing
        return r * 2.0;
    }

    // Inside the circle AND in the mouth wedge
    // Distance to upper mouth line
    float dUpperLine = p.x * s - p.y * c;
    // Distance to lower mouth line
    float dLowerLine = p.x * s + p.y * c;
    // Nearest mouth edge
    float dMouthEdge = min(dUpperLine, dLowerLine);

    // Return distance to nearest mouth edge only (not the back of circle!)
    return dMouthEdge;
}

// Eye position helper
vec2 getEyePos(float r, bool facingRight) {
    float eyeX = r * 0.3;
    float eyeY = r * 0.4;
    if (!facingRight) {
        eyeX = -eyeX;
    }
    return vec2(eyeX, eyeY);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    // Sample the terminal content
    vec2 uv = fragCoord / iResolution.xy;
    vec4 terminal = texture(iChannel0, uv);

    // Get cursor info
    vec2 cursorPos = iCurrentCursor.xy;
    vec2 cursorSize = iCurrentCursor.zw;
    vec2 prevCursorPos = iPreviousCursor.xy;

    // Calculate cursor center (cursor pos is top-left corner)
    vec2 cursorCenter = cursorPos + cursorSize * 0.5;

    // Determine facing direction based on movement
    float dx = cursorPos.x - prevCursorPos.x;

    // Use movement direction if there's significant movement
    bool facingRight = true;
    float moveDist = length(cursorPos - prevCursorPos);
    if (moveDist > 1.0) {
        facingRight = (dx >= 0.0);
    }

    // Pac-Man parameters - fit within cell width
    float pacmanRadius = cursorSize.x * 0.45;

    // Animated mouth angle (chomping animation)
    float mouthSpeed = 8.0;
    float mouthAngle = 0.15 + 0.35 * abs(sin(iTime * mouthSpeed));

    // Position relative to cursor center
    vec2 p = fragCoord - cursorCenter;

    // Flip Y because screen coordinates have Y going down
    p.y = -p.y;

    // Stretch vertically to make Pac-Man taller (ellipse)
    float heightScale = 1.5;
    p.y /= heightScale;

    // Calculate distance to Pac-Man
    float d = sdPacman(p, pacmanRadius, mouthAngle, facingRight);

    // Eye
    vec2 eyePos = getEyePos(pacmanRadius, facingRight);
    float eyeRadius = pacmanRadius * 0.15;
    float eyeDist = length(p - eyePos) - eyeRadius;

    // Get cursor color from cursor uniform
    vec3 pacmanColor = iCurrentCursorColor.rgb;
    float cursorOpacity = iCurrentCursorColor.a;

    // Eye color
    vec3 eyeColor = vec3(0.0);

    // Check if we're in the cursor bounding box
    vec2 cursorMin = cursorPos;
    vec2 cursorMax = cursorPos + cursorSize;
    bool inCursorBox = fragCoord.x >= cursorMin.x && fragCoord.x <= cursorMax.x &&
                       fragCoord.y >= cursorMin.y && fragCoord.y <= cursorMax.y;

    // Sample background from outside cursor area
    vec2 bgSamplePos = vec2(cursorPos.x - cursorSize.x, cursorPos.y + cursorSize.y * 0.5);
    vec2 bgUv = bgSamplePos / iResolution.xy;
    bgUv = clamp(bgUv, vec2(0.0), vec2(1.0));
    vec4 background = texture(iChannel0, bgUv);

    // Start with terminal content
    vec4 result = terminal;

    // In cursor box, replace cursor-colored pixels with background (but keep text)
    if (inCursorBox && cursorOpacity > 0.01) {
        // Check if this pixel matches the cursor color (it's the cursor block, not text)
        float cursorMatch = 1.0 - distance(terminal.rgb, pacmanColor) * 2.0;
        cursorMatch = clamp(cursorMatch, 0.0, 1.0);
        // Replace cursor block pixels with background, preserve text
        result = mix(terminal, background, cursorMatch);
    }

    // Only draw if cursor is visible (has opacity)
    if (cursorOpacity > 0.01) {
        // Anti-aliased edge
        float aa = 2.0;

        // Draw Pac-Man body - solid color, semi-transparent to show text
        if (d < aa) {
            float bodyAlpha = 1.0 - smoothstep(-aa, aa, d);
            bodyAlpha *= cursorOpacity * 0.6;  // More transparent to show text
            result = mix(result, vec4(pacmanColor, 1.0), bodyAlpha);
        }

        // Draw eye - black
        if (d < 0.0 && eyeDist < aa) {
            float eyeAlpha = 1.0 - smoothstep(-aa, aa, eyeDist);
            eyeAlpha *= cursorOpacity;
            result = mix(result, vec4(0.0, 0.0, 0.0, 1.0), eyeAlpha);
        }
    }

    fragColor = result;
}
