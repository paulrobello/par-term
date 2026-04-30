/*! par-term shader metadata
name: gradient-background
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
    angle_speed: 0.24999996
    animate_angle: false
    gradient_angle: 45.0
    gradient_center:
    - 0.5
    - 0.5
    gradient_end_color: '#c48a3a'
    gradient_mid_color: '#2b6f6f'
    gradient_start_color: '#0b1d33'
    gradient_type: 0
    wave_frequency: 3.0
*/

// credits: https://github.com/unkn0wncode

// control color label="Start Color"
uniform vec3 gradient_start_color;
// control color label="Middle Color"
uniform vec3 gradient_mid_color;
// control color label="End Color"
uniform vec3 gradient_end_color;
// control select options="linear,radial,conic,diamond,wave" label="Gradient Type"
uniform int gradient_type;
// control angle unit=degrees label="Angle"
uniform float gradient_angle;
// control checkbox label="Animate Angle"
uniform bool animate_angle;
// control slider min=-2 max=2 step=0.01 label="Angle Speed"
uniform float angle_speed;
// control point label="Gradient Center"
uniform vec2 gradient_center;
// control slider min=0.25 max=12 step=0.05 label="Wave Frequency"
uniform float wave_frequency;

vec3 gradientPalette(float t) {
    t = clamp(t, 0.0, 1.0);

    vec3 startColor = gradient_start_color;
    vec3 midColor = gradient_mid_color;
    vec3 endColor = gradient_end_color;
    float paletteSpan = distance(startColor, midColor) + distance(midColor, endColor) + distance(startColor, endColor);

    if (paletteSpan < 0.01) {
        startColor = vec3(0.043, 0.114, 0.200);
        midColor = vec3(0.169, 0.435, 0.435);
        endColor = vec3(0.769, 0.541, 0.227);
    }

    if (t < 0.5) {
        return mix(startColor, midColor, t * 2.0);
    }
    return mix(midColor, endColor, (t - 0.5) * 2.0);
}

float linearGradient(vec2 uv, float angle, vec2 center) {
    vec2 dir = vec2(cos(angle), sin(angle));
    vec2 p = uv - center;
    float span = abs(dir.x) + abs(dir.y);
    return dot(p, dir) / max(span, 0.0001) + 0.5;
}

vec2 rotatePoint(vec2 p, float angle) {
    float s = sin(angle);
    float c = cos(angle);
    return vec2(c * p.x - s * p.y, s * p.x + c * p.y);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord.xy / iResolution.xy;
    vec2 centered = uv - gradient_center;
    centered.x *= iResolution.x / iResolution.y;

    float gradientFactor;
    float effectiveAngle = gradient_angle;
    if (animate_angle) {
        effectiveAngle += iTime * angle_speed;
    }

    if (gradient_type == 0) {
        gradientFactor = linearGradient(uv, effectiveAngle, gradient_center);
    } else if (gradient_type == 1) {
        gradientFactor = length(centered) * 1.8;
    } else if (gradient_type == 2) {
        gradientFactor = fract(atan(centered.y, centered.x) / 6.2831853 + 0.5);
    } else if (gradient_type == 3) {
        vec2 rotated = rotatePoint(centered, effectiveAngle);
        gradientFactor = (abs(rotated.x) + abs(rotated.y)) * 1.4;
    } else {
        float linear = linearGradient(uv, effectiveAngle, gradient_center);
        gradientFactor = 0.5 + 0.5 * sin((linear * wave_frequency + iTime * 0.08) * 6.2831853);
    }

    fragColor = vec4(gradientPalette(gradientFactor), 1.0);
}
