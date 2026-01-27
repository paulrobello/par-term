/*! par-term shader metadata
name: "Bumped Sinusoidal Warp"
author: "Shane (Shadertoy)"
description: "Metallic sinusoidal warp with bump-mapped lighting"
version: "1.0.0"

defaults:
  animation_speed: 1.0
  brightness: 1.0
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
    float t = iTime/2.;

    // Layered sinusoidal feedback
    for (int i=0; i<3; i++){
        p += cos(p.yx*3. + vec2(t, 1.57))/3.;
        p += sin(p.yx + t + vec2(1.57, 0))/2.;
        p *= 1.3;
    }

    // Subtle jitter to soften edges
    p += fract(sin(p+vec2(13, 7))*5e5)*.005 - .0025;

    return mod(p, 2.) - 1.;
}

// Bump height from warp function
float bumpFunc(vec2 p){
    return length(W(p))*.7071;
}

void mainImage( out vec4 fragColor, in vec2 fragCoord ){
    vec2 uv = (fragCoord - iResolution.xy*.5)/iResolution.y;

    // Surface, ray direction, light position, normal
    vec3 sp = vec3(uv, 0);
    vec3 rd = normalize(vec3(uv, 1));
    vec3 lp = vec3(cos(iTime)*.5, sin(iTime)*.2, -1);
    vec3 sn = vec3(0, 0, -1);

    // Bump mapping
    vec2 eps = vec2(4./iResolution.y, 0);
    float f = bumpFunc(sp.xy);
    float fx = bumpFunc(sp.xy - eps.xy);
    float fy = bumpFunc(sp.xy - eps.yx);

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

    // Texture color with warp
    vec3 texCol = texture(iChannel0, sp.xy + W(sp.xy)/8.).xyz;
    texCol *= texCol; // sRGB to linear
    texCol = smoothstep(.05, .75, pow(texCol, vec3(.75, .8, .85)));

    // Final color with environment reflection
    vec3 col = (texCol*(diff*vec3(1, .97, .92)*2. + .5) + vec3(1, .6, .2)*spec*2.)*atten;
    float ref = max(dot(reflect(rd, sn), vec3(1)), 0.);
    col += col*pow(ref, 4.)*vec3(.25, .5, 1)*3.;

    fragColor = vec4(sqrt(clamp(col, 0., 1.)), 1);
}