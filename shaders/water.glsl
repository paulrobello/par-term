/*! par-term shader metadata
name: water
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: textures/cubemaps/env-outside
  cubemap_enabled: false
  use_background_as_channel0: null
  uniforms:
    iCausticColor: '#ffffff'
    iCausticStrength: 0.39999998
    iContrast: 1.4
    iFlowSpeed: 0.5
    iHighlightSharpness: 15.0
    iLineIntensity: 0.0050000004
    iPatternScale: 1.0
    iWaterColor: '#1a2640'
*/

// Water caustic effect - background only
// Use with custom_shader_full_content: false (default)
// The caustic pattern is rendered as background, text is composited on top

// control color label="Water Color"
uniform vec3 iWaterColor;
// control color label="Caustic Color"
uniform vec3 iCausticColor;
// control slider min=0.05 max=2 step=0.01 label="Flow Speed"
uniform float iFlowSpeed;
// control slider min=0.25 max=4 step=0.01 label="Pattern Scale"
uniform float iPatternScale;
// control slider min=0.001 max=0.02 step=0.001 scale=log label="Line Intensity"
uniform float iLineIntensity;
// control slider min=0.6 max=3 step=0.01 label="Contrast"
uniform float iContrast;
// control slider min=2 max=24 step=0.1 label="Highlight Sharpness"
uniform float iHighlightSharpness;
// control slider min=0 max=1.5 step=0.01 label="Caustic Strength"
uniform float iCausticStrength;

#define TAU 6.28318530718
#define MAX_ITER 6

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    // Animated time
    float time = iTime * iFlowSpeed + 23.0;

    // Normalized coordinates
    vec2 uv = fragCoord.xy / iResolution.xy;

    // Generate caustic pattern
    vec2 p = mod(uv * TAU * iPatternScale, TAU) - 250.0;
    vec2 i = vec2(p);
    float c = 1.0;
    float inten = iLineIntensity;

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
    c = 1.17 - pow(c, iContrast);

    // Create caustic highlight color
    vec3 caustic = iCausticColor * pow(abs(c), iHighlightSharpness);

    // Blend caustic with water base color
    vec3 color = iWaterColor + caustic * iCausticStrength;
    color = clamp(color, 0.0, 1.0);

    // Output background color with full opacity
    // The wrapper will composite terminal text on top
    fragColor = vec4(color, 1.0);
}
