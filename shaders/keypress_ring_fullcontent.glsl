// Key Press Ring Shader
// Full-content keypress effect for opaque alternate-screen apps.

/*! par-term shader metadata
name: "Key Press Ring Full Content"
defaults:
  full_content: true
  uniforms:
    iPulseDecay: 6.0
    iPulseDuration: 1.0
    iFlashAmount: 0.02
    iRingSpeed: 0.8
    iRingWidth: 0.035
    iRingStrength: 1.6
    iRingTint: "#33ccff"
    iKeyCursorGlowRadius: 72.0
    iKeyCursorGlowStrength: 0.85
*/
// control slider min=1 max=16 step=0.1 label="Pulse Decay"
uniform float iPulseDecay;
// control slider min=0.1 max=3 step=0.05 label="Pulse Duration"
uniform float iPulseDuration;
// control slider min=0 max=0.6 step=0.01 label="Flash Amount"
uniform float iFlashAmount;
// control slider min=0.1 max=2.5 step=0.05 label="Ring Speed"
uniform float iRingSpeed;
// control slider min=0.005 max=0.2 step=0.005 label="Ring Width"
uniform float iRingWidth;
// control slider min=0 max=2 step=0.05 label="Ring Strength"
uniform float iRingStrength;
// control color label="Ring Tint"
uniform vec3 iRingTint;
// control slider min=8 max=160 step=1 label="Cursor Glow Radius"
uniform float iKeyCursorGlowRadius;
// control slider min=0 max=2 step=0.05 label="Cursor Glow Strength"
uniform float iKeyCursorGlowStrength;

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord / iResolution.xy;
    vec4 terminal = texture(iChannel4, uv);

    float timeSinceKey = iTime - iTimeKeyPress;
    float pulse = exp(-timeSinceKey * iPulseDecay);
    pulse *= step(timeSinceKey, iPulseDuration);

    float flashIntensity = pulse * iFlashAmount;
    float cursorHasColor = step(0.001, iCurrentCursorColor.a + dot(iCurrentCursorColor.rgb, vec3(1.0)));
    float effectAlpha = max(iCurrentCursorColor.a, 0.35) * cursorHasColor;

    vec2 cursorPx = iCurrentCursor.xy + 0.5 * iCurrentCursor.zw;
    vec2 cursorUv = cursorPx / iResolution.xy;

    vec2 ringDelta = (uv - cursorUv) * vec2(iResolution.x / iResolution.y, 1.0);
    float dist = length(ringDelta);
    float ringRadius = timeSinceKey * iRingSpeed;
    float ring = smoothstep(iRingWidth, 0.0, abs(dist - ringRadius)) * pulse * effectAlpha;
    vec3 ringColor = iRingTint * ring * iRingStrength;

    float cursorDist = distance(fragCoord, cursorPx);
    float cursorGlowBoost = mix(0.28, 1.0, pulse);
    float cursorGlow = exp(-cursorDist / max(iKeyCursorGlowRadius, 1.0))
        * cursorGlowBoost
        * iKeyCursorGlowStrength
        * effectAlpha;

    vec3 color = terminal.rgb * (1.0 + flashIntensity);
    color += ringColor;
    color += iCurrentCursorColor.rgb * cursorGlow;

    fragColor = vec4(clamp(color, 0.0, 1.0), terminal.a);
}
