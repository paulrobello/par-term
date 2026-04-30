/*! par-term shader metadata
name: cineShader-Lava
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: ''
  cubemap_enabled: false
  use_background_as_channel0: null
  uniforms:
    iHighlightTightness: 2.0
    iLavaCore: '#e10000'
    iLavaHighlight: '#ffffff'
    iLavaShadow: '#24040a'
    iLightAzimuth: 45.0
    iLightElevation: -33.0
    iMarchSteps: 25.0
    iNumSpheres: 12.0
*/

// INFO: This shader is a port of https://www.shadertoy.com/view/3sySRK
// Optimized for par-term

#define MAX_NUM_SPHERES 32
#define MAX_MARCH_STEPS 128

// control slider min=1 max=32 step=1 label="Num Spheres"
uniform float iNumSpheres;

// control slider min=8 max=128 step=1 label="March Steps"
uniform float iMarchSteps;

// control color label="Shadow Color"
uniform vec3 iLavaShadow;

// control color label="Core Color"
uniform vec3 iLavaCore;

// control color label="Highlight Color"
uniform vec3 iLavaHighlight;

// control slider min=0 max=2 step=0.01 label="Highlight Tightness"
uniform float iHighlightTightness;

// control angle unit=degrees label="Light Azimuth"
uniform float iLightAzimuth;

// control angle unit=degrees label="Light Elevation"
uniform float iLightElevation;

float opSmoothUnion(float d1, float d2, float k) {
    float h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}

float map(vec3 p) {
    float d = 2.0;
    int sphereCount = int(clamp(floor(iNumSpheres + 0.5), 1.0, float(MAX_NUM_SPHERES)));
    for (int i = 0; i < MAX_NUM_SPHERES; i++) {
        if (i >= sphereCount) {
            break;
        }
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

    int marchStepCount = int(clamp(floor(iMarchSteps + 0.5), 8.0, float(MAX_MARCH_STEPS)));
    for (int i = 0; i < MAX_MARCH_STEPS; i++) {
        if (i >= marchStepCount) {
            break;
        }
        p = rayOri + rayDir * depth;
        float dist = map(p);
        depth += dist;
        if (dist < 1e-6 || depth > 6.0) break;
    }

    depth = min(6.0, depth);
    vec3 n = calcNormal(p);
    vec3 lightDir = normalize(vec3(
        cos(iLightElevation) * cos(iLightAzimuth),
        sin(iLightElevation),
        cos(iLightElevation) * sin(iLightAzimuth)
    ));
    float b = max(0.0, dot(n, lightDir));

    float light = clamp(b, 0.0, 1.0);
    vec3 col = mix(iLavaShadow, iLavaCore, smoothstep(0.00, 0.65, light));
    float highlightStart = mix(0.45, 0.98, clamp(iHighlightTightness, 0.0, 2.0) * 0.5);
    col = mix(col, iLavaHighlight, smoothstep(highlightStart, 1.0, light));
    col *= 0.75 + light * 0.45;
    col *= exp(-depth * 0.15);

    // Fade out background where rays miss blobs (fixes banding)
    col *= smoothstep(5.5, 4.0, depth);

    fragColor = vec4(col, 1.0);
}
