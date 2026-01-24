// Water caustic effect - background only
// Use with custom_shader_full_content: false (default)
// The caustic pattern is rendered as background, text is composited on top

#define TAU 6.28318530718
#define MAX_ITER 6

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    // Base water color (dark blue tint)
    vec3 water_color = vec3(0.1, 0.15, 0.25);

    // Animated time
    float time = iTime * 0.5 + 23.0;

    // Normalized coordinates
    vec2 uv = fragCoord.xy / iResolution.xy;

    // Generate caustic pattern
    vec2 p = mod(uv * TAU, TAU) - 250.0;
    vec2 i = vec2(p);
    float c = 1.0;
    float inten = 0.005;

    for (int n = 0; n < MAX_ITER; n++)
    {
        float t = time * (1.0 - (3.5 / float(n + 1)));
        i = p + vec2(
            cos(t - i.x) + sin(t + i.y),
            sin(t - i.y) + cos(t + i.x)
        );
        c += 1.0 / length(vec2(
            p.x / (sin(i.x + t) / inten),
            p.y / (cos(i.y + t) / inten)
        ));
    }

    c /= float(MAX_ITER);
    c = 1.17 - pow(c, 1.4);

    // Create caustic highlight color
    vec3 caustic = vec3(pow(abs(c), 15.0));

    // Blend caustic with water base color
    vec3 color = water_color + caustic * 0.4;
    color = clamp(color, 0.0, 1.0);

    // Output background color with full opacity
    // The wrapper will composite terminal text on top
    fragColor = vec4(color, 1.0);
}
