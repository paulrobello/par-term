/*! par-term shader metadata
name: Animated Gradient
author: unkn0wncode (GitHub)
description: Smooth animated color gradient background
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.36
  text_opacity: null
  full_content: null
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: null
*/

// Source: https://github.com/unkn0wncode
void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord.xy / iResolution.xy;

    // Diagonal gradient factor with smooth curve
    float gradientFactor = (uv.x + uv.y) * 0.5;
    gradientFactor = gradientFactor * gradientFactor * (3.0 - 2.0 * gradientFactor); // inline smoothstep

    // Animated phase - compute sin/cos once and derive others
    float angle = iTime * 0.2;
    float s0 = sin(angle);
    float c0 = cos(angle); // cos(x) = sin(x + pi/2), cheaper to compute once

    // Derive other phases using sin addition formula: sin(a+b) = sin(a)cos(b) + cos(a)sin(b)
    // Precomputed: sin(1)≈0.841, cos(1)≈0.540, sin(2)≈0.909, cos(2)≈-0.416, sin(3)≈0.141, cos(3)≈-0.990
    float s1 = s0 * 0.540 + c0 * 0.841; // sin(angle + 1)
    float s2 = s0 * -0.416 + c0 * 0.909; // sin(angle + 2)
    float s3 = s0 * -0.990 + c0 * 0.141; // sin(angle + 3)

    // Convert to [0,1] range
    float t0 = s0 * 0.5 + 0.5;
    float t1 = s1 * 0.5 + 0.5;
    float t2 = s2 * 0.5 + 0.5;
    float t3 = s3 * 0.5 + 0.5;

    // Base colors
    vec3 color1 = vec3(0.1, 0.1, 0.5);
    vec3 color2 = vec3(0.5, 0.1, 0.1);
    vec3 color3 = vec3(0.1, 0.5, 0.1);

    // Interpolate between colors
    vec3 gradientStartColor = mix(mix(color1, color2, t0), color3, t2);
    vec3 gradientEndColor = mix(mix(color2, color3, t1), color1, t3);
    vec3 gradientColor = mix(gradientStartColor, gradientEndColor, gradientFactor);

    // Blend with terminal content (show gradient where terminal is dark)
    vec4 terminalColor = texture(iChannel4, uv);
    float brightness = terminalColor.r + terminalColor.g + terminalColor.b;
    vec3 blendedColor = mix(gradientColor, terminalColor.rgb, step(0.5, brightness));

    fragColor = vec4(blendedColor, terminalColor.a);
}
