/*! par-term shader metadata
name: cursor_water_tank
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

// Water Tank Cursor Effect - Simulates water sloshing in a tank following the cursor
// The water tilts and sloshes in the direction of cursor movement

// CONFIGURATION
const float WALL_THICKNESS = 1.0;        // Tank wall thickness in pixels
const float TANK_HEIGHT = 0.75;           // Tank height as fraction of cursor height (0.0-1.0)
const float WATER_LEVEL = 0.5;           // Base water fill level (0.0-1.0)
const float SLOSH_INTENSITY = 0.75;       // How much the water tilts when moving
const float SLOSH_DAMPING = 3.0;         // How fast the sloshing settles
const float WAVE_FREQUENCY = 3.0;        // Frequency of surface waves
const float WAVE_AMPLITUDE = 0.06;       // Amplitude of surface ripples
const float WAVE_SPEED = 6.0;            // Speed of wave propagation

// Colors
const vec3 WATER_COLOR = vec3(0.2, 0.5, 0.9);      // Water base color
const vec3 WATER_SURFACE_COLOR = vec3(0.4, 0.7, 1.0); // Water surface highlight
const vec3 TANK_COLOR = vec3(0.3, 0.3, 0.35);      // Tank wall color
const vec3 TANK_HIGHLIGHT = vec3(0.5, 0.5, 0.55);  // Tank edge highlight
const float WATER_OPACITY = 0.7;
const float TANK_OPACITY = 0.85;


void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    // Sample terminal content
    vec4 terminalColor = texture(iChannel4, fragCoord / iResolution.xy);

    // Get cursor info
    vec2 cursorPos = iCurrentCursor.xy;
    vec2 cursorSize = iCurrentCursor.zw;
    vec2 prevPos = iPreviousCursor.xy;

    // Tank matches cursor width, reduced height anchored at bottom
    vec2 tankSize = vec2(cursorSize.x, cursorSize.y * TANK_HEIGHT);
    vec2 tankPos = vec2(cursorPos.x, cursorPos.y + cursorSize.y * (1.0 - TANK_HEIGHT));

    // Local coordinates within tank (0-1 range, flip Y so 0 is at bottom)
    vec2 localPos = (fragCoord - tankPos) / tankSize;
    localPos.y = 1.0 - localPos.y;

    // Check if we're in the tank area (with some margin for walls)
    float margin = WALL_THICKNESS / min(tankSize.x, tankSize.y);
    bool inTankArea = localPos.x >= -margin && localPos.x <= 1.0 + margin &&
                      localPos.y >= -margin && localPos.y <= 1.0 + margin;

    if (!inTankArea) {
        fragColor = terminalColor;
        return;
    }

    // Calculate movement velocity and direction
    // velocity represents the movement that triggered iTimeCursorChange
    vec2 velocity = cursorPos - prevPos;
    float speed = length(velocity);

    // Time since last cursor change for damping
    float timeSinceMove = iTime - iTimeCursorChange;

    // Calculate slosh amount with damping (elastic settling)
    float dampFactor = exp(-timeSinceMove * SLOSH_DAMPING);
    float sloshMagnitude = clamp(speed / 30.0, 0.0, 1.0);

    // Direction of tilt - water piles opposite to movement (inertia)
    // Use velocity.x directly, not normalized, to preserve direction even when small
    float tiltDirection = velocity.x != 0.0 ? -sign(velocity.x) : 0.0;

    // Oscillation for settling effect - oscillates around the tilted position
    float oscillation = sin(timeSinceMove * 12.0);

    // Combined tilt: base tilt + oscillation, both decay over time
    float baseTilt = sloshMagnitude * tiltDirection * SLOSH_INTENSITY;
    float oscillatingTilt = oscillation * sloshMagnitude * tiltDirection * SLOSH_INTENSITY * 0.5;
    float tiltOffset = (baseTilt + oscillatingTilt) * dampFactor * (localPos.x - 0.5) * 2.0;

    // Add ripple waves on water surface
    float wavePhase = iTime * WAVE_SPEED;
    float wave1 = sin(localPos.x * WAVE_FREQUENCY * 2.0 + wavePhase) * WAVE_AMPLITUDE;
    float wave2 = sin(localPos.x * WAVE_FREQUENCY * 3.14 - wavePhase * 1.3) * WAVE_AMPLITUDE * 0.5;

    // Waves are more prominent during sloshing
    float waveIntensity = 0.3 + sloshMagnitude * 0.7 + dampFactor * 0.5;
    float surfaceWave = (wave1 + wave2) * waveIntensity;

    // Final water surface level
    float waterSurface = WATER_LEVEL + tiltOffset + surfaceWave;

    // Check if point is underwater (remember Y is flipped - 0 at bottom in tank coords)
    bool underwater = localPos.y < waterSurface;

    // Check if point is in tank walls
    bool inLeftWall = localPos.x >= -margin && localPos.x < 0.0;
    bool inRightWall = localPos.x > 1.0 && localPos.x <= 1.0 + margin;
    bool inBottomWall = localPos.y >= -margin && localPos.y < 0.0;
    bool inWall = inLeftWall || inRightWall || inBottomWall;

    // Check if on water surface (highlight line)
    float surfaceDist = abs(localPos.y - waterSurface);
    bool onSurface = surfaceDist < 0.08 && localPos.x >= 0.0 && localPos.x <= 1.0;

    vec4 effectColor = terminalColor;

    if (inWall) {
        // Draw tank walls
        float edgeDist = 0.0;
        if (inLeftWall) edgeDist = localPos.x + margin;
        else if (inRightWall) edgeDist = 1.0 + margin - localPos.x;
        else if (inBottomWall) edgeDist = localPos.y + margin;

        float edgeHighlight = smoothstep(0.0, margin, edgeDist);
        vec3 wallColor = mix(TANK_HIGHLIGHT, TANK_COLOR, edgeHighlight);
        effectColor = mix(terminalColor, vec4(wallColor, 1.0), TANK_OPACITY);
    }
    else if (localPos.x >= 0.0 && localPos.x <= 1.0 && localPos.y >= 0.0) {
        if (underwater && localPos.y < 1.0) {
            // Draw water
            float depth = waterSurface - localPos.y;
            float depthFactor = clamp(depth / WATER_LEVEL, 0.0, 1.0);

            // Darker at bottom, lighter at top
            vec3 waterCol = mix(WATER_SURFACE_COLOR, WATER_COLOR, depthFactor * 0.7);

            // Subtle shimmer effect
            float shimmer = sin(localPos.x * 6.0 + iTime * 3.0) * 0.05 * (1.0 - depthFactor);
            waterCol += shimmer;

            effectColor = mix(terminalColor, vec4(waterCol, 1.0), WATER_OPACITY * (0.6 + depthFactor * 0.4));
        }

        if (onSurface) {
            // Surface highlight/reflection
            float surfaceGlow = smoothstep(0.08, 0.0, surfaceDist);
            vec3 surfaceCol = WATER_SURFACE_COLOR + 0.3;
            effectColor = mix(effectColor, vec4(surfaceCol, 1.0), surfaceGlow * 0.7);
        }
    }

    fragColor = effectColor;
}
