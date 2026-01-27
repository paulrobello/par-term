/*! par-term shader metadata
name: Bumped Sinusoidal Warp
author: Shane (Shadertoy)
description: Metallic sinusoidal warp with bump-mapped lighting
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.25
  text_opacity: null
  full_content: null
  channel0: textures/metalic1.jpg
  channel1: null
  channel2: null
  channel3: null
  cubemap: textures/cubemaps/env-outside
  cubemap_enabled: false
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


// Sinusoidal warp function
vec2 W(vec2 p){
    p = (p + 3.)*4.;
    float t = iTime*.5;
    vec2 tOff = vec2(t, 1.57);
    vec2 tOff2 = vec2(1.57, 0);

    // Layered sinusoidal feedback (unrolled for clarity)
    p += cos(p.yx*3. + tOff)*.333;
    p += sin(p.yx + t + tOff2)*.5;
    p *= 1.3;

    p += cos(p.yx*3. + tOff)*.333;
    p += sin(p.yx + t + tOff2)*.5;
    p *= 1.3;

    p += cos(p.yx*3. + tOff)*.333;
    p += sin(p.yx + t + tOff2)*.5;
    p *= 1.3;

    // Cheaper jitter using fract-based hash
    p += fract(p*127.1 + p.yx*311.7)*.005 - .0025;

    return mod(p, 2.) - 1.;
}

void mainImage( out vec4 fragColor, in vec2 fragCoord ){
    vec2 uv = (fragCoord - iResolution.xy*.5)/iResolution.y;

    // Surface, ray direction, light position, normal
    vec3 sp = vec3(uv, 0);
    vec3 rd = normalize(vec3(uv, 1));
    vec3 lp = vec3(cos(iTime)*.5, sin(iTime)*.2, -1);
    vec3 sn = vec3(0, 0, -1);

    // Cache warp at center point (used for bump height AND texture warp)
    vec2 warpCenter = W(sp.xy);
    float f = length(warpCenter)*.7071;

    // Bump mapping - compute derivatives at offset positions
    vec2 eps = vec2(4./iResolution.y, 0);
    float fx = length(W(sp.xy - eps.xy))*.7071;
    float fy = length(W(sp.xy - eps.yx))*.7071;

    const float bumpFactor = .05;
    fx = (fx - f)/eps.x;
    fy = (fy - f)/eps.x;
    sn = normalize(sn + vec3(fx, fy, 0)*bumpFactor);

    // Lighting
    vec3 ld = lp - sp;
    float lDist = max(length(ld), .0001);
    ld /= lDist;

    float atten = 1./(1. + lDist*lDist*.15);
    atten *= f*.9 + .1;

    float diff = max(dot(sn, ld), 0.);
    diff = pow(diff, 4.)*.66 + pow(diff, 8.)*.34;
    float spec = pow(max(dot(reflect(-ld, sn), -rd), 0.), 12.);

    // Texture color with cached warp
    vec3 texCol = texture(iChannel0, sp.xy + warpCenter*.125).xyz;
    texCol *= texCol; // sRGB to linear
    texCol = smoothstep(.05, .75, pow(texCol, vec3(.75, .8, .85)));

    // Final color with environment reflection
    vec3 col = (texCol*(diff*vec3(1, .97, .92)*2. + .5) + vec3(1, .6, .2)*spec*2.)*atten;
    float ref = max(dot(reflect(rd, sn), vec3(1)), 0.);
    col += col*pow(ref, 4.)*vec3(.25, .5, 1)*3.;

    fragColor = vec4(sqrt(clamp(col, 0., 1.)), 1);
}