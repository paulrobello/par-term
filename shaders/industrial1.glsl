/*! par-term shader metadata
name: Industrial1
author: converted for par-term
description: Optimized industrial raymarched structure adapted from ShaderToy-style GLSL with audio and multi-sample AA paths removed.
version: 1.0.0
defaults:
  animation_speed: 0.8
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: null
  use_background_as_channel0: null
  uniforms:
    iCyanAccent: '#00e8ff'
    iGlow: 0.65
    iHeatAccent: '#ff4c18'
    iMovementSpeed: 0.049999997
    iRaymarchSteps: 65
    iRelief: 1.0
*/

// control slider min=0 max=2 step=0.01 label="Movement Speed"
uniform float iMovementSpeed;
// control int min=16 max=128 step=1 label="Raymarch Steps"
uniform int iRaymarchSteps;
// control slider min=0 max=1.5 step=0.01 label="Surface Relief"
uniform float iRelief;
// control slider min=0 max=1.5 step=0.01 label="Glow"
uniform float iGlow;
// control color label="Cyan Accent"
uniform vec3 iCyanAccent;
// control color label="Heat Accent"
uniform vec3 iHeatAccent;

const int MAX_MARCH_STEPS = 128;
const int SHADOW_STEPS = 16;
const float SURFACE_EPSILON = 0.006;
const float MAX_DISTANCE = 70.0;

mat2 rotate2d(float angle) {
    float c = cos(angle);
    float s = sin(angle);
    return mat2(c, -s, s, c);
}

vec3 circuitFoldA(vec2 p) {
    float minX = 1000.0;
    float minY = 1000.0;

    for (int i = 0; i < 7; i++) {
        p = abs(p) / clamp(p.x * p.y, 0.3, 5.0) - 1.0;
        minX = min(minX, abs(p.x));
        minY = min(minY, abs(p.y));
    }

    minX = exp(-0.5 * minX);
    minY = exp(-1.0 * minY);
    return vec3(minX - minY * 0.1, minX, minY);
}

float circuitFoldB(vec2 p) {
    p.x -= 1.5;
    float minX = 1000.0;
    float minY = 1000.0;

    for (int i = 0; i < 5; i++) {
        p = abs(p) / clamp(abs(p.x * p.y), 0.2, 2.0) - 1.0;
        minX = min(minX, abs(p.x));
        minY = min(minY, abs(p.y));
    }

    return exp(-0.7 * minY) - minX * 0.1;
}

float structuralDistance(vec3 p) {
    float d = length(p + vec3(0.0, -3.0, 0.0)) + 0.6;
    d = min(d, length(p.xy + vec2(0.0, -3.0)) + 1.3);
    d = min(d, length(p.zy + vec2(0.0, -3.0)) + 1.5);

    vec2 side = sign(p.xz);
    p.xz = abs(p.xz);
    float strut = max(length(p.xz - 5.0 - side * 0.5) + 0.5 + p.y * 0.2, p.y - 2.0);
    return min(d, strut);
}

vec4 sceneMap(vec3 p) {
    p.yz *= rotate2d(0.6);
    p.xz *= rotate2d(0.6);

    float t = iTime * iMovementSpeed;
    p.xy += vec2(15.0);
    p.xz += vec2(t * 1.0, t * 0.35);
    p.xz = mod(p.xz, 16.0) - 8.0;

    vec3 foldA = circuitFoldA(p.xz * 0.1);
    float foldB = circuitFoldB(p.yz * 0.02);
    float base = min(p.y, structuralDistance(p));
    float motionStabilizer = mix(1.0, 0.68, smoothstep(0.03, 0.8, abs(iMovementSpeed)));
    float relief = (foldA.x * 0.7 + foldB * 1.2) * iRelief * motionStabilizer;

    return vec4((base - relief) * 0.5, foldA.y, foldA.z, relief);
}

vec3 estimateNormal(vec3 p) {
    vec2 k = vec2(1.0, -1.0);
    return normalize(
        k.xyy * sceneMap(p + k.xyy * SURFACE_EPSILON).x +
        k.yyx * sceneMap(p + k.yyx * SURFACE_EPSILON).x +
        k.yxy * sceneMap(p + k.yxy * SURFACE_EPSILON).x +
        k.xxx * sceneMap(p + k.xxx * SURFACE_EPSILON).x
    );
}

float softShadow(vec3 origin, vec3 lightDir) {
    float shade = 1.0;
    float travel = 0.03;

    for (int i = 0; i < SHADOW_STEPS; i++) {
        float d = sceneMap(origin + lightDir * travel).x;
        shade = min(shade, 10.0 * d / travel);
        travel += clamp(d, 0.02, 1.2);

        if (shade < 0.05 || travel > 20.0) {
            break;
        }
    }

    return clamp(shade, 0.0, 1.0);
}

vec3 marchIndustrial(vec3 rayOrigin, vec3 rayDir) {
    rayOrigin.xy *= 5.0;

    vec3 p = rayOrigin;
    vec4 hit = vec4(0.0);
    float travel = 0.0;
    bool didHit = false;

    int marchSteps = clamp(iRaymarchSteps, 1, MAX_MARCH_STEPS);

    for (int i = 0; i < MAX_MARCH_STEPS; i++) {
        if (i >= marchSteps) {
            break;
        }

        p = rayOrigin + rayDir * travel;
        hit = sceneMap(p);

        if (hit.x < SURFACE_EPSILON) {
            didHit = true;
            break;
        }

        travel += hit.x;
        if (travel > MAX_DISTANCE) {
            break;
        }
    }

    if (!didHit) {
        return vec3(0.0);
    }

    vec3 normal = estimateNormal(p);
    vec3 lightDir = normalize(vec3(-1.0, 1.0, -1.0));
    float keyLight = smoothstep(0.25, 0.95, max(0.0, dot(lightDir, normal)));
    float rim = smoothstep(0.35, 0.85, max(0.0, dot(-rayDir, normal)));
    float shadow = softShadow(p + normal * 0.03, lightDir);
    float depthFade = exp(-0.018 * travel);

    vec3 metal = vec3(1.8, 1.0, 0.5) * (keyLight + rim * 0.35);
    vec3 color = metal * (0.28 + shadow * 0.72);

    float cyanMask = 1.0 - smoothstep(0.84, 0.91, hit.y);
    float heatStripe = abs(fract(hit.z * 2.0 + 0.5) - 0.5);
    float heatMask = 1.0 - smoothstep(0.18, 0.34, heatStripe);

    color += cyanMask * iCyanAccent * (0.36 * iGlow);
    color += heatMask * iHeatAccent * (0.62 * iGlow);

    return color * depthFade;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 flippedCoord = vec2(fragCoord.x, iResolution.y - fragCoord.y);
    vec2 uv = (flippedCoord - iResolution.xy * 0.5) / iResolution.y;
    vec2 screenUv = flippedCoord / iResolution.xy;

    vec3 rayOrigin = vec3(uv, -6.0);
    vec3 rayDir = vec3(0.0, 0.0, 1.0);
    vec3 color = marchIndustrial(rayOrigin, rayDir) + vec3(0.025, 0.020, 0.018);

    float vignette = exp(-3.0 * length(screenUv - 0.5)) * 1.3 + 0.2;
    color *= vignette;
    color = pow(max(color, vec3(0.0)), vec3(0.92));

    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
