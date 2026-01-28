/*! par-term shader metadata
name: convergence
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.15
  text_opacity: null
  full_content: null
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: ''
  cubemap_enabled: false
*/

// Convergence shader - Optimized
// Adapted from https://www.shadertoy.com/view/wXyyRD
// Reference: https://www.shadertoy.com/view/WXycRW

#define NOISE_SCALE 6.0
#define NOISE_SPEED 5.0
#define SPLIT_INTENSITY 0.01
#define GLOW_WIDTH 0.0005

const vec3 BOLT_COLOR = vec3(0.8, 0.9, 1.0);
const vec4 LEFT_COLOR = vec4(9.0, 15.0, 17.0, 0.0);
const vec4 RIGHT_COLOR = vec4(19.0, 11.0, 11.0, 1.0);

float hash(float n) {
    return fract(sin(n) * 43758.5453123);
}

float noise(vec2 x) {
    vec2 p = floor(x);
    vec2 f = fract(x);
    f = f * f * (3.0 - 2.0 * f);
    float n = p.x + p.y * 57.0;
    return mix(mix(hash(n), hash(n + 1.0), f.x),
               mix(hash(n + 57.0), hash(n + 58.0), f.x), f.y);
}

float fbm(vec2 p) {
    float f = 0.0;
    float w = 0.5;
    for (int i = 0; i < 5; i++) {
        f += w * noise(p);
        p *= 2.0;
        w *= 0.5;
    }
    return f;
}

// Original voronoise - kept intact for visual accuracy
float voronoise_proc(vec2 p) {
    float d = 1000.0;
    vec2 q = fract(p);
    vec2 f = floor(p);

    for (float x = -0.5; x <= 0.5; x++)
    for (float y = -0.5; y <= 0.5; y++) {
        vec2 h = vec2(x, y);
        vec2 noiseVal = vec2(
            hash(dot(f + h, vec2(127.1, 311.7))),
            hash(dot(f + h, vec2(269.5, 183.3)))
        );
        d = min(d, length(q - h - noiseVal));
    }
    return d;
}

mat2 rot(float x) {
    float c = cos(x), s = sin(x);
    return mat2(c, s, -s, c);
}

// BufferA effect - left side with inward time flow
vec4 calcBufferA(vec2 fragCoord, vec2 r) {
    vec2 p = (fragCoord + fragCoord - r) / sqrt(r.x * r.y);

    if (length(p) < 0.1) {
        return vec4(0.0);
    }

    p /= dot(p, p) * 2.1;
    p *= rot(pow(dot(p, p), 1.5) * 0.4);

    vec4 c = vec4(0.0);
    float t = -iTime * 7.0;

    for (float n = fract(t); n < 24.0; n++) {
        float noiseOffset = hash(floor(n - floor(t)) * 127.1) * 256.0;
        c += pow(voronoise_proc(p * n * 0.4 + noiseOffset) * 1000.0 + 10.0, -1.0)
             * clamp(1.0 - n / 22.0, 0.0, 1.0)
             * clamp(n, 0.0, 1.0);
    }

    c += dot(p, p) * 0.07;
    c *= LEFT_COLOR;
    c -= dot(p, p) * 0.6 * vec4(0.9, 1.2, 1.4, 0.0);
    c = max(c, 0.0);
    c *= smoothstep(1.0, 0.0, length(p * 0.6) - 0.5);
    c *= c;
    return tanh(c);
}

// BufferB effect - right side with outward time flow
vec4 calcBufferB(vec2 fragCoord, vec2 r) {
    vec2 p = (fragCoord + fragCoord - r) / sqrt(r.x * r.y);

    p /= dot(p, p) * 2.1;
    p *= rot(pow(dot(p, p), 1.5) * 0.4);

    vec4 c = vec4(0.0);
    float t = iTime * 7.0;

    for (float n = fract(t); n < 24.0; n++) {
        float noiseOffset = hash(floor(n - floor(t)) * 127.1) * 256.0;
        c += pow(voronoise_proc(p * n * 0.4 + noiseOffset) * 1000.0 + 10.0, -1.0)
             * clamp(1.0 - n / 22.0, 0.0, 1.0)
             * clamp(n, 0.0, 1.0);
    }

    c += dot(p, p) * 0.07;
    c *= RIGHT_COLOR;
    c -= sin(dot(p, p) * 0.5 * vec4(p.x - 1.0, length(p.y * 0.01), 0.5, 0.0));
    c += length(p * 0.01) - 0.5;
    c *= smoothstep(1.0, 0.0, length(p * 0.6) - 0.5);
    c *= c;
    return tanh(c);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 r = iResolution.xy;
    float splitPos = 0.5;

    // Lightning displacement
    vec2 noiseUV = (fragCoord - 0.5 * r) / r.y;
    float noiseTime = iTime * NOISE_SPEED;
    float displacement = fbm(noiseUV * NOISE_SCALE + vec2(noiseTime, noiseTime * 0.5));
    displacement = (displacement - 0.5) * 2.0;
    float dist = (fragCoord.x / r.x) - splitPos + displacement * SPLIT_INTENSITY;

    // KEY OPTIMIZATION: Only calculate the buffer we actually need
    vec4 color = (dist < 0.0) ? calcBufferA(fragCoord, r) : calcBufferB(fragCoord, r);

    // Lightning glow
    float glow = GLOW_WIDTH / abs(dist);
    glow = pow(clamp(glow, 0.0, 1.0), 1.2);
    glow *= (0.8 + 0.2 * sin(iTime * 20.0));
    color.rgb += BOLT_COLOR * glow * 4.0;

    fragColor = color;
}
