/*! par-term shader metadata
name: cineShader-Lava
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.32
  text_opacity: null
  full_content: null
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: ''
  cubemap_enabled: false
*/

// INFO: This shader is a port of https://www.shadertoy.com/view/3sySRK
// Optimized for par-term

#define NUM_SPHERES 12
#define MARCH_STEPS 48

float opSmoothUnion(float d1, float d2, float k) {
    float h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}

float map(vec3 p) {
    float d = 2.0;
    for (int i = 0; i < NUM_SPHERES; i++) {
        float fi = float(i);
        float time = iTime * (fract(fi * 412.531 + 0.513) - 0.5) * 2.0;
        vec3 offset = sin(time + fi * vec3(52.5126, 64.62744, 632.25)) * vec3(2.0, 2.0, 0.8);
        float radius = mix(0.5, 1.0, fract(fi * 412.531 + 0.5124));
        d = opSmoothUnion(length(p + offset) - radius, d, 0.4);
    }
    return d;
}

vec3 calcNormal(vec3 p) {
    const float h = 1e-5;
    const vec2 k = vec2(1, -1);
    return normalize(
        k.xyy * map(p + k.xyy * h) +
        k.yyx * map(p + k.yyx * h) +
        k.yxy * map(p + k.yxy * h) +
        k.xxx * map(p + k.xxx * h)
    );
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;

    float aspect = iResolution.x / iResolution.y;
    vec3 rayOri = vec3((uv - 0.5) * vec2(aspect, 1.0) * 6.0, 3.0);
    vec3 rayDir = vec3(0.0, 0.0, -1.0);

    float depth = 0.0;
    vec3 p;

    for (int i = 0; i < MARCH_STEPS; i++) {
        p = rayOri + rayDir * depth;
        float dist = map(p);
        depth += dist;
        if (dist < 1e-6 || depth > 6.0) break;
    }

    depth = min(6.0, depth);
    vec3 n = calcNormal(p);
    float b = max(0.0, dot(n, vec3(0.577)));

    // Original brightness
    vec3 col = (0.5 + 0.5 * cos((b + iTime * 3.0) + uv.xyx * 2.0 + vec3(0, 2, 4))) * (0.85 + b * 0.35);
    col *= exp(-depth * 0.15);

    // Fade out background where rays miss blobs (fixes banding)
    col *= smoothstep(5.5, 4.0, depth);

    fragColor = vec4(col, 1.0);
}
