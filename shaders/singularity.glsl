/*
    "Singularity" by @XorDev
    A whirling blackhole.
    https://www.shadertoy.com/view/3csSWB

    Adapted for par-term background shader.
*/

void mainImage(out vec4 O, vec2 F)
{
    // Iterator and attenuation (distance-squared)
    float i = 0.2;
    float a;
    // Resolution for scaling and centering
    vec2 r = iResolution.xy;
    // Centered ratio-corrected coordinates
    vec2 p = (F + F - r) / r.y / 0.7;
    // Diagonal vector for skewing
    vec2 d = vec2(-1.0, 1.0);
    // Blackhole center
    vec2 b = p - i * d;

    // Rotate and apply perspective - expanded mat2 construction
    float perspective = 0.1 + i / dot(b, b);
    vec2 dScaled = d / perspective;
    mat2 perspectiveMat = mat2(1.0, 1.0, dScaled.x, dScaled.y);
    vec2 c = p * perspectiveMat;

    // Compute spiral rotation angle
    a = dot(c, c);
    float angle = 0.5 * log(a) + iTime * i;

    // Build rotation matrix - cos(angle + offset) for each component
    float c0 = cos(angle);
    float c1 = cos(angle + 33.0);
    float c2 = cos(angle + 11.0);
    float c3 = cos(angle);
    mat2 spiralMat = mat2(c0, c1, c2, c3);

    // Rotate into spiraling coordinates
    vec2 v = (c * spiralMat) / i;

    // Waves cumulative total for coloring
    vec4 w = vec4(0.0);

    // Loop through waves
    for (float j = 0.0; j < 9.0; j += 1.0) {
        i = j + 1.0;
        // Distort coordinates
        v += 0.7 * sin(v.yx * i + iTime) / i + 0.5;
        w += 1.0 + vec4(sin(v.x), sin(v.y), sin(v.x), sin(v.y));
    }

    // Accretion disk radius
    float diskRadius = length(sin(v / 0.3) * 0.4 + c * (3.0 + d));

    // Red/blue gradient
    vec4 gradient = exp(c.x * vec4(0.6, -0.4, -1.0, 0.0));

    // Wave coloring
    vec4 waveColor = w.xyyx;

    // Accretion disk brightness
    float diskBright = 2.0 + diskRadius * diskRadius / 4.0 - diskRadius;

    // Center darkness
    float centerDark = 0.5 + 1.0 / a;

    // Rim highlight
    float rimLight = 0.03 + abs(length(p) - 0.7);

    // Combine all factors
    vec4 blackholeColor = 1.0 - exp(-gradient / waveColor / diskBright / centerDark / rimLight);

    // Terminal integration
    vec2 terminalUV = F / iResolution.xy;
    vec4 terminalColor = texture(iChannel0, terminalUV);

    float brightnessThreshold = 0.1;
    float terminalBrightness = dot(terminalColor.rgb, vec3(0.2126, 0.7152, 0.0722));

    if (terminalBrightness < brightnessThreshold) {
        O = mix(terminalColor, blackholeColor, 0.7);
    } else {
        O = terminalColor;
    }
}
