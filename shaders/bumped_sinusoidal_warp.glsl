/*! par-term shader metadata
name: Bumped Sinusoidal Warp
author: Shane (Shadertoy)
description: Metallic sinusoidal warp with bump-mapped lighting
version: 1.0.0
defaults:
  animation_speed: 0.5
  channel0: textures/metalic1.jpg
  channel1: null
  channel2: null
  channel3: null
  cubemap: textures/cubemaps/env-outside
  cubemap_enabled: false
  use_background_as_channel0: null
  uniforms:
    iBumpStrength: 0.05
    iCrossWaveAmount: 0.333
    iFeedbackGain: 1.3
    iFlowSpeed: 0.25
    iJitterAmount: 0.0
    iLightDepth: -1.0
    iLightOrbit:
    - 0.5
    - 0.2
    iReflectionStrength: 3.0
    iReflectionTint: '#4080ff'
    iSineWaveAmount: 0.5
    iSpecularPower: 12.0
    iSpecularStrength: 2.0
    iSpecularTint: '#ff9933'
    iTextureWarp: 0.125
    iWarpScale: 4.0
    iWaveFrequency: 3.0
*/

/*
    Bumped Sinusoidal Warp
    ----------------------
    Source: https://www.shadertoy.com/view/4l2XWK
    Author: Shane

    Sinusoidal planar deformation with point-lit bump mapping.

    Related shaders:
    - Fantomas - Plop: https://www.shadertoy.com/view/ltSSDV
    - Fabrice - Plop 2: https://www.shadertoy.com/view/MlSSDV
    - IQ - Sculpture III: https://www.shadertoy.com/view/XtjSDK
    - Shane - Lit Sine Warp: https://www.shadertoy.com/view/Ml2XDV
*/

// control slider min=0 max=2 step=0.01 label="Flow Speed"
uniform float iFlowSpeed;
// control slider min=1 max=8 step=0.01 label="Warp Scale"
uniform float iWarpScale;
// control slider min=0.25 max=8 step=0.01 label="Wave Frequency"
uniform float iWaveFrequency;
// control slider min=0 max=1 step=0.001 label="Cross Wave"
uniform float iCrossWaveAmount;
// control slider min=0 max=1.5 step=0.001 label="Sine Wave"
uniform float iSineWaveAmount;
// control slider min=0.75 max=1.8 step=0.01 label="Feedback Gain"
uniform float iFeedbackGain;
// control slider min=0 max=0.03 step=0.0005 label="Jitter"
uniform float iJitterAmount;
// control slider min=0 max=0.2 step=0.001 label="Bump Strength"
uniform float iBumpStrength;
// control slider min=0 max=0.35 step=0.001 label="Texture Warp"
uniform float iTextureWarp;
// control vec2 min=0 max=1 step=0.01 label="Light Orbit"
uniform vec2 iLightOrbit;
// control slider min=-3 max=0.5 step=0.01 label="Light Depth"
uniform float iLightDepth;
// control slider min=2 max=96 step=1 label="Specular Power"
uniform float iSpecularPower;
// control slider min=0 max=5 step=0.01 label="Specular Strength"
uniform float iSpecularStrength;
// control slider min=0 max=8 step=0.01 label="Reflection Strength"
uniform float iReflectionStrength;
// control color label="Specular Tint"
uniform vec3 iSpecularTint;
// control color label="Reflection Tint"
uniform vec3 iReflectionTint;

// Sinusoidal warp function
vec2 W(vec2 p){
    p = (p + 3.)*iWarpScale;
    float t = iTime*iFlowSpeed;
    vec2 tOff = vec2(t, 1.57);
    vec2 tOff2 = vec2(1.57, 0);

    // Layered sinusoidal feedback (unrolled for clarity)
    p += cos(p.yx*iWaveFrequency + tOff)*iCrossWaveAmount;
    p += sin(p.yx + t + tOff2)*iSineWaveAmount;
    p *= iFeedbackGain;

    p += cos(p.yx*iWaveFrequency + tOff)*iCrossWaveAmount;
    p += sin(p.yx + t + tOff2)*iSineWaveAmount;
    p *= iFeedbackGain;

    p += cos(p.yx*iWaveFrequency + tOff)*iCrossWaveAmount;
    p += sin(p.yx + t + tOff2)*iSineWaveAmount;
    p *= iFeedbackGain;

    // Cheaper jitter using fract-based hash
    p += (fract(p*127.1 + p.yx*311.7) - .5)*iJitterAmount;

    return mod(p, 2.) - 1.;
}

void mainImage( out vec4 fragColor, in vec2 fragCoord ){
    vec2 uv = (fragCoord - iResolution.xy*.5)/iResolution.y;

    // Surface, ray direction, light position, normal
    vec3 sp = vec3(uv, 0);
    vec3 rd = normalize(vec3(uv, 1));
    vec3 lp = vec3(cos(iTime)*iLightOrbit.x, sin(iTime)*iLightOrbit.y, iLightDepth);
    vec3 sn = vec3(0, 0, -1);

    // Cache warp at center point (used for bump height AND texture warp)
    vec2 warpCenter = W(sp.xy);
    float f = length(warpCenter)*.7071;

    // Bump mapping - compute derivatives at offset positions
    vec2 eps = vec2(4./iResolution.y, 0);
    float fx = length(W(sp.xy - eps.xy))*.7071;
    float fy = length(W(sp.xy - eps.yx))*.7071;

    fx = (fx - f)/eps.x;
    fy = (fy - f)/eps.x;
    sn = normalize(sn + vec3(fx, fy, 0)*iBumpStrength);

    // Lighting
    vec3 ld = lp - sp;
    float lDist = max(length(ld), .0001);
    ld /= lDist;

    float atten = 1./(1. + lDist*lDist*.15);
    atten *= f*.9 + .1;

    float diff = max(dot(sn, ld), 0.);
    diff = pow(diff, 4.)*.66 + pow(diff, 8.)*.34;
    float spec = pow(max(dot(reflect(-ld, sn), -rd), 0.), iSpecularPower);

    // Texture color with cached warp
    vec3 texCol = texture(iChannel0, sp.xy + warpCenter*iTextureWarp).xyz;
    texCol *= texCol; // sRGB to linear
    texCol = smoothstep(.05, .75, pow(texCol, vec3(.75, .8, .85)));

    // Final color with environment reflection
    vec3 col = (texCol*(diff*vec3(1, .97, .92)*2. + .5) + iSpecularTint*spec*iSpecularStrength)*atten;
    float ref = max(dot(reflect(rd, sn), vec3(1)), 0.);
    col += col*pow(ref, 4.)*iReflectionTint*iReflectionStrength;

    fragColor = vec4(sqrt(clamp(col, 0., 1.)), 1);
}
