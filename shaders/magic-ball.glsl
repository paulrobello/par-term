/*! par-term shader metadata
name: Magic Ball
author: sabosugi / adapted for par-term
description: Raymarched glowing magic sphere converted from the magic-ball index.html shader.
version: 1.1.4
defaults:
  animation_speed: 0.8
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: false
  use_background_as_channel0: null
  uniforms:
    iAutoRotateSpeed: 0.099999964
    iBallSpeed: 0.4
    iBaseRotationX: 0.0
    iBaseRotationY: 0.0
    iFadeBand:
    - 2.8999999
    - 3.0
    iFractalScale: 0.9
    iGlow: 1.0
    iMouseControl: false
    iMouseRotationStrength: 1.4
    iRaySteps: 60
    iSmoothingFactor: 0.1
*/

// High-quality default is 90. Values from 72-90 are usually reasonable.
#define MAX_TRACE_DISTANCE 10.0
#define START_NOISE_STRENGTH 0.015
#define STEP_BIAS 0.012
#define COLOR_PHASES vec3(5.8, 4.1, 2.8)

const float PI = 3.14159265359;

// control slider min=0.25 max=3.0 step=0.01
uniform float iBallSpeed;

// control slider min=0.2 max=2.0 step=0.01
uniform float iGlow;

// control int min=24 max=120 step=1 label="Ray Steps"
uniform int iRaySteps;

// control slider min=0.4 max=2.0 step=0.01
uniform float iFractalScale;

// control range min=0.5 max=8.0 step=0.01 label="Fade Band"
uniform vec2 iFadeBand;

// control slider min=0.1 max=4.0 step=0.01
uniform float iSmoothingFactor;

// control slider min=-1.5 max=1.5 step=0.01 label="Auto Rotate Speed"
uniform float iAutoRotateSpeed;

// control angle unit=degrees label="Base Rotation X"
uniform float iBaseRotationX;

// control angle unit=degrees label="Base Rotation Y"
uniform float iBaseRotationY;

// control slider min=0.0 max=3.0 step=0.01 label="Mouse Strength"
uniform float iMouseRotationStrength;

// control checkbox
uniform bool iMouseControl;

vec3 calculatePalette(float depth) {
    return 1.0 + cos(depth + COLOR_PHASES);
}

float smin(float a, float b, float k) {
    float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

float evaluateScene(vec3 rayPosition, float currentTime) {
    float fractalScale = max(iFractalScale, 0.01);
    vec3 primaryPos = rayPosition * fractalScale;
    vec3 secondaryPos = rayPosition * fractalScale;

    for (float iteration = 2.3; iteration <= 6.0; iteration += 1.1) {
        float secondaryScale = iteration * 0.3;
        secondaryPos += sin(0.6 * currentTime + primaryPos.zxy * secondaryScale) * 0.4;
        primaryPos += sin(currentTime + primaryPos.yzx * iteration) * 0.25;
    }

    float sphereA = length(primaryPos + 1.0) - 2.0;
    float sphereB = length(secondaryPos - 1.3) - 2.9;
    float smoothingFactor = max(iSmoothingFactor, 0.001);
    float blendedSpheres = smin(sphereA, sphereB, smoothingFactor);

    float fractalVolume = abs(blendedSpheres) * 0.1;
    return fractalVolume / fractalScale;
}

float randomNoise(vec2 p) {
    return fract(sin(dot(p, vec2(12.8998, 78.233))) * 43758.5453);
}

vec3 customTanh(vec3 x) {
    vec3 e2x = exp(2.0 * clamp(x, -20.0, 20.0));
    return (e2x - 1.0) / (e2x + 1.0);
}

mat3 rotateX(float angle) {
    float s = sin(angle);
    float c = cos(angle);
    return mat3(
        1.0, 0.0, 0.0,
        0.0, c, -s,
        0.0, s, c
    );
}

mat3 rotateY(float angle) {
    float s = sin(angle);
    float c = cos(angle);
    return mat3(
        c, 0.0, s,
        0.0, 1.0, 0.0,
        -s, 0.0, c
    );
}

mat3 getRotation(float currentTime) {
    vec2 mouseCentered = vec2(0.0);

    if (iMouseControl) {
        vec2 mouseUv = iMouse.xy / max(iResolution.xy, vec2(1.0));
        float hasMouse = step(0.001, dot(iMouse.xy, iMouse.xy));
        mouseCentered = (mouseUv - 0.5) * hasMouse;
    }

    float rotationX = iBaseRotationX - mouseCentered.y * iMouseRotationStrength;
    float rotationY = iBaseRotationY + currentTime * iAutoRotateSpeed + mouseCentered.x * iMouseRotationStrength;

    rotationX = clamp(rotationX, -0.5 * PI, 0.5 * PI);
    return rotateY(rotationY) * rotateX(rotationX);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 resolution = max(iResolution.xy, vec2(1.0));
    vec2 uv = (fragCoord * 2.0 - resolution) / resolution.y;
    float currentTime = iTime * iBallSpeed;

    float totalDistance = -START_NOISE_STRENGTH * randomNoise(fragCoord);
    vec3 colorAccumulator = vec3(0.0);

    vec3 rayOrigin = vec3(0.0, 0.0, -4.5);
    vec3 rayDir = normalize(vec3(uv, 1.0));
    mat3 rotation = getRotation(currentTime);
    int rayStepCount = clamp(iRaySteps, 24, 120);
    float fadeInner = clamp(min(iFadeBand.x, iFadeBand.y), 0.5, 8.0);
    float fadeOuter = clamp(max(iFadeBand.x, iFadeBand.y), 0.5, 8.0);
    fadeInner = min(fadeInner, fadeOuter - 0.01);

    for (int stepIndex = 0; stepIndex < rayStepCount; stepIndex++) {
        vec3 currentPos = rayOrigin + rayDir * totalDistance;

        if (totalDistance > MAX_TRACE_DISTANCE) {
            break;
        }

        float distFromCenter = length(currentPos);

        if (distFromCenter > fadeOuter + 0.01) {
            totalDistance += distFromCenter - fadeOuter;
            continue;
        }

        vec3 rotatedPos = rotation * currentPos;
        float sceneDistance = evaluateScene(rotatedPos, currentTime);

        float stepSize = STEP_BIAS + sceneDistance;
        totalDistance += stepSize;

        float radialFade = smoothstep(fadeOuter, fadeInner, distFromCenter);
        vec3 baseColor = calculatePalette(rotatedPos.z);

        colorAccumulator += (baseColor / stepSize) * radialFade;

        if (
            colorAccumulator.x > 25000.0 &&
            colorAccumulator.y > 25000.0 &&
            colorAccumulator.z > 25000.0
        ) {
            break;
        }
    }

    float vignetteEffect = max(length(uv), 0.2);
    colorAccumulator = colorAccumulator * iGlow / (8000.0 * vignetteEffect);

    vec3 color = customTanh(colorAccumulator);
    color = max(color, vec3(0.0));

    fragColor = vec4(color, 1.0);
}
