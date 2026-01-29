/*! par-term shader metadata
name: cursor_orbit
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: null
  brightness: null
  text_opacity: null
  full_content: null
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: null
  use_background_as_channel0: null
  hides_cursor: true
  glow_radius: null
  glow_intensity: null
  trail_duration: null
  cursor_color: null
*/

// Orbiting Ball Cursor Shader
// A ball that traces the inside edge of the cursor cell with a fading trail
//
// Usage: Set in config.yaml:
//   cursor_shader: "cursor_orbit.glsl"
//   cursor_shader_enabled: true
//   cursor_shader_animation: true
//   cursor_shader_hides_cursor: true  # Recommended: hides default cursor

// Adjustable parameters
const float ORBIT_SPEED = 2.0;        // Speed of orbit (cycles per second)
const float TRAIL_LENGTH = 0.8;       // Trail length (0.0 - 1.0, fraction of orbit)
const float BALL_RADIUS = 2.0;        // Ball radius in pixels
// Keep the orbit tight to the cell edge; inset just enough to avoid clipping with AA.
const float EDGE_INSET = BALL_RADIUS + 0.75; // Distance from cell edge in pixels

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    // Sample terminal content
    vec2 uv = fragCoord / iResolution.xy;
    vec4 terminal = texture(iChannel4, uv);

    // Get cursor info
    vec2 cursorPos = iCurrentCursor.xy;
    vec2 cursorSize = iCurrentCursor.zw;

    // Get cursor color and opacity (includes blink animation)
    vec3 ballColor = iCurrentCursorColor.rgb;
    float cursorOpacity = iCurrentCursorColor.a;

    // Start with terminal content
    vec4 result = terminal;

    // Only draw if cursor is visible
    if (cursorOpacity > 0.01) {
        // Calculate the inset rectangle for the orbit path
        vec2 innerMin = cursorPos + vec2(EDGE_INSET);
        vec2 innerMax = cursorPos + cursorSize - vec2(EDGE_INSET);
        vec2 innerSize = innerMax - innerMin;

        // Calculate perimeter of the orbit path
        float perimeter = 2.0 * (innerSize.x + innerSize.y);

        // Current position along perimeter (0 to 1)
        float t = fract(iTime * ORBIT_SPEED);

        // Convert t to distance along perimeter
        float dist = t * perimeter;

        // Calculate ball position along the rectangular path
        // Path goes: right edge down, bottom edge left, left edge up, top edge right
        vec2 ballPos;

        float rightEdge = innerSize.y;
        float bottomEdge = rightEdge + innerSize.x;
        float leftEdge = bottomEdge + innerSize.y;
        // topEdge = perimeter

        if (dist < rightEdge) {
            // Right edge, going down
            ballPos = vec2(innerMax.x, innerMin.y + dist);
        } else if (dist < bottomEdge) {
            // Bottom edge, going left
            ballPos = vec2(innerMax.x - (dist - rightEdge), innerMax.y);
        } else if (dist < leftEdge) {
            // Left edge, going up
            ballPos = vec2(innerMin.x, innerMax.y - (dist - bottomEdge));
        } else {
            // Top edge, going right
            ballPos = vec2(innerMin.x + (dist - leftEdge), innerMin.y);
        }

        // Function to get position along path for a given t value
        // (inline calculation for trail points)
        #define GET_PATH_POS(tVal) ( \
            (tVal * perimeter < rightEdge) ? vec2(innerMax.x, innerMin.y + tVal * perimeter) : \
            (tVal * perimeter < bottomEdge) ? vec2(innerMax.x - (tVal * perimeter - rightEdge), innerMax.y) : \
            (tVal * perimeter < leftEdge) ? vec2(innerMin.x, innerMax.y - (tVal * perimeter - bottomEdge)) : \
            vec2(innerMin.x + (tVal * perimeter - leftEdge), innerMin.y) \
        )

        // Calculate distance from current fragment to ball
        float distToBall = length(fragCoord - ballPos);

        // Draw ball with anti-aliasing
        float aa = 1.5;
        float ballAlpha = 1.0 - smoothstep(BALL_RADIUS - aa, BALL_RADIUS + aa, distToBall);

        // Calculate trail effect
        float trailAlpha = 0.0;

        // Check multiple points along the trail
        for (float i = 1.0; i <= 20.0; i += 1.0) {
            float trailT = t - (i / 20.0) * TRAIL_LENGTH;
            if (trailT < 0.0) trailT += 1.0;  // Wrap around

            // Calculate trail point position
            float trailDist = trailT * perimeter;
            vec2 trailPos;

            if (trailDist < rightEdge) {
                trailPos = vec2(innerMax.x, innerMin.y + trailDist);
            } else if (trailDist < bottomEdge) {
                trailPos = vec2(innerMax.x - (trailDist - rightEdge), innerMax.y);
            } else if (trailDist < leftEdge) {
                trailPos = vec2(innerMin.x, innerMax.y - (trailDist - bottomEdge));
            } else {
                trailPos = vec2(innerMin.x + (trailDist - leftEdge), innerMin.y);
            }

            // Distance to this trail point
            float distToTrail = length(fragCoord - trailPos);

            // Trail gets thinner and more transparent further back
            float trailFade = 1.0 - (i / 20.0);
            float trailRadius = BALL_RADIUS * trailFade;
            float pointAlpha = (1.0 - smoothstep(trailRadius - aa, trailRadius + aa, distToTrail)) * trailFade;

            trailAlpha = max(trailAlpha, pointAlpha);
        }

        // Combine ball and trail
        float totalAlpha = max(ballAlpha, trailAlpha * 0.6);
        totalAlpha *= cursorOpacity;

        // Apply color
        result = mix(result, vec4(ballColor, 1.0), totalAlpha);
    }

    fragColor = result;
}
