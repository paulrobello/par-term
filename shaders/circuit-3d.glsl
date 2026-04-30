/*! par-term shader metadata
name: Shadertoy X3KcWR Circuit 3D
author: null
description: Self-contained par-term port from a mirrored Shadertoy renderpass that cites X3KcWR.
version: 1.0.1
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
    iAASamples: 1
    iCircuitDetail: 5
    iGlowDetail: 6
    iGlowIntensity: 1.0
    iGlowSteps: 32
    iMarchSteps: 36
    iShadowSteps: 8
    iSurfaceGain: 50.0
*/

/*
    Source requested: https://www.shadertoy.com/view/X3KcWR

    Shadertoy was Cloudflare-gated from this environment and no local
    SHADERTOY_APP_KEY was available. This file was converted from the
    public GitHub mirror entry below, whose description cites X3KcWR:

    https://raw.githubusercontent.com/GabeRundlett/shadertoy-api-shaders/master/shaders/lXyyDh.json

    Mirrored shader info:
    - name: circuit 3d
    - author: nayk
    - description: combination of https://www.shadertoy.com/view/stsXDl#
      https://www.shadertoy.com/view/X3KcWR

    par-term notes:
    - No external Shadertoy input channels were used by the renderpass.
    - Initialized one loop counter for stricter GLSL/WGSL transpilation.
    - Forced final alpha to 1.0; Shadertoy ignores alpha, par-term does not.
    - Optimized for terminal use by removing a discarded render path and
      exposing loop-count controls for quality/performance tuning.
*/

#define R(p,a,r) mix(a * dot(p, a), p, cos(r)) + sin(r) * cross(p, a)
#define H(h) (cos((h) * 6.3 + vec3(0, 23, 21)) * .5 + .5)
#define PI 3.141592
#define pal(x) (cos(x * 2. * PI + vec3(0, 22, 14)) * .5 + .5)
#define SIN(x) (.5 + .5 * sin(x))
#define tt iTime * 0.1
#define S(a, b, x) smoothstep(a, b, x)
#define rot2(x) (mat2(cos(x), sin(x), -sin(x), cos(x)))

// control int min=1 max=3 step=1 label="AA Samples"
uniform int iAASamples;
// control int min=12 max=80 step=4 label="Surface Steps"
uniform int iMarchSteps;
// control int min=0 max=24 step=2 label="Shadow Steps"
uniform int iShadowSteps;
// control int min=2 max=7 step=1 label="Circuit Detail"
uniform int iCircuitDetail;
// control int min=8 max=70 step=2 label="Glow Steps"
uniform int iGlowSteps;
// control int min=2 max=8 step=1 label="Glow Detail"
uniform int iGlowDetail;
// control slider min=0.1 max=2.0 step=0.05 label="Glow Intensity"
uniform float iGlowIntensity;
// control slider min=10 max=70 step=1 label="Surface Gain"
uniform float iSurfaceGain;


mat2 rot(float a) {
    float c = cos(a);
    float s = sin(a);
    return mat2(c, -s, s, c);
}

float x, y;

float cir1(vec2 p) {
    x = 1000.;
    y = 1000.;
    for (int i = 0; i < 7; i++) {
        if (i >= iCircuitDetail) break;
        p = abs(p) / clamp(p.x * p.y, .3, 5.) - 1.;
        x = min(x, abs(p.x));
        y = min(y, abs(p.y));
    }
    x = exp(-.5 * x);
    y = exp(-1. * y);
    return x - y * .1;
}

float cir2(vec2 p) {
    p.x -= 1.5;
    float x = 1000.;
    float y = 1000.;
    for (int i = 0; i < 5; i++) {
        if (i >= iCircuitDetail) break;
        p = abs(p) / clamp(abs(p.x * p.y), .2, 2.) - 1.;
        x = min(x, abs(p.x));
        y = min(y, abs(p.y));
    }
    y = exp(-.7 * y) - x * .1;
    return y;
}

float struc(vec3 p) {
    float d = length(p + vec3(0., -3., 0.)) + .6;
    d = min(d, length(p.xy + vec2(0., -3.)) + 1.3);
    d = min(d, length(p.zy + vec2(0., -3.)) + 1.5);
    vec2 s = sign(p.xz);
    p.xz = abs(p.xz);
    d = min(d, max(length(p.xz - 5. - s * .5) + .5 + p.y * .2, p.y - 2.));
    return d;
}

float h;

float de(vec3 p) {
    vec2 pzy = p.zy * rot2(iTime * 0.1);
    p.zy = pzy;
    vec2 pyz = p.yz * rot(1.1);
    p.yz = pyz;
    vec2 pxz = p.xz * rot(.0);
    p.xz = pxz;

    pzy = p.zy * rot2(iTime * 0.01);
    p.zy = pzy;
    vec2 pxy = p.xy * rot2(iTime * 0.21);
    p.xy = pxy;
    float r1 = cir1(p.xz * .35);
    float r2 = cir2(p.yz * .15);
    float sup = p.y;
    float sph = struc(p);
    float d = min(sup, sph);
    h = r1 * .01 + r2 * 0.1;

    d -= h;
    return d * .5;
}

float det = 0.005;

vec3 normal(vec3 p) {
    vec2 e = vec2(0.0, det);
    return normalize(vec3(de(p + e.yxx), de(p + e.xyx), de(p + e.xxy)) - de(p));
}

vec3 march(vec3 from, vec3 dir) {
    from.xy *= 5.;
    vec3 p = from;
    vec3 col = vec3(0.0);
    float d = 0.;
    for (int i = 0; i < 80; i++) {
        if (i >= iMarchSteps) break;
        p += d * dir;
        d = de(p);
        if (d < det) break;
    }
    if (d < det) {
        float cx = x;
        float cy = y;
        vec3 n = normal(p);
        vec3 ldir = normalize(vec3(-1., 1., -1.0));
        float dif = smoothstep(.8, 1., max(0., dot(ldir, n)));
        dif += smoothstep(.5, .8, max(0., dot(-dir, n))) * .35;
        col = vec3(1.8, 1., .5) * dif;
        float shadow = 1.0;
        vec3 shadowRay = p + n * 0.01;
        for (int i = 0; i < 24; i++) {
            if (i >= iShadowSteps) break;
            float d = de(shadowRay);
            if (d < 0.005) {
                shadow = 0.0;
                break;
            }
            shadowRay += ldir * d;
        }
        col += step(cx, .87) * vec3(0., 1., 1.) * .5;
        col *= .3 + shadow * .7;
        col += step(fract(cy * 3.), .25) * vec3(1., 0.3, 0.1);
    }
    return col;
}

vec3 antialiasedMarch(vec3 from, vec3 dir) {
    vec3 color = vec3(0.0);
    vec2 offsetScale = 2. / iResolution.xy;
    int aa = iAASamples;
    if (aa < 1) aa = 1;
    if (aa > 3) aa = 3;

    for (int i = 0; i < 3; i++) {
        if (i >= aa) break;
        for (int j = 0; j < 3; j++) {
            if (j >= aa) break;
            vec2 offset = vec2(float(i), float(j)) / float(aa) - 0.5;
            vec3 displacedFrom = from + vec3(offset * offsetScale, 0.0);
            color += march(displacedFrom, dir);
        }
    }

    return color / float(aa * aa);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec4 accumColor = vec4(0);

    vec2 uv = (fragCoord - .5 * iResolution.xy) / iResolution.y;
    vec2 uv3 = (fragCoord - iResolution.xy * 0.5) / iResolution.y;

    vec3 from = vec3(uv3, -6.);
    vec3 dir = normalize(vec3(0., 0., 1.));

    vec3 col3 = antialiasedMarch(from, dir) + .05;
    col3 *= exp(-3. * length(fragCoord / iResolution.xy - .5)) * 1.3 + .2;

    vec3 n1;
    vec2 r = iResolution.xy;
    vec3 d = normalize(vec3((fragCoord * 2. - r) / r.y, 1));
    float g = 0.0;
    float e = 1.0;
    for (int i = 1; i < 70; i++) {
        if (i >= iGlowSteps) break;
        n1 = g * d * col3;
        vec2 nxy = n1.xy * rot2(iTime * 0.1);
        n1 = vec3(nxy.x, nxy.y, n1.z);
        vec2 nzy = n1.zy * rot2(iTime * 0.1);
        n1 = vec3(n1.x, nzy.y, nzy.x);
        n1 = vec3(n1.x, n1.y, n1.z + iTime);

        float a = 30.;
        n1 = mod(n1 - a, a * 2.) - a;
        float s = 3.;

        for (int j = 0; j < 8; j++) {
            if (j >= iGlowDetail) break;
            n1 = .3 - abs(n1);
            if (n1.x < n1.z) {
                n1 = n1.zyx;
            }
            if (n1.z < n1.y) {
                n1 = n1.xzy;
            }
            e = 1.7 + sin(iTime * .021) * .01;
            s *= e;
            n1 = abs(n1) * e -
                vec3(
                    5. * 3.,
                    120,
                    8. * 5.
                 );
         }
         e = length(vec4(n1.y, n1.z, n1.z, n1.z)) / s;
         g += e;
         vec3 glow = accumColor.xyz + iGlowIntensity * mix(vec3(0.1, 0.2, 3.), H(g * .1), .8) * 1. / e / 8e3;
         accumColor = vec4(glow, accumColor.a);
    }
    accumColor = accumColor * vec4(col3 * iSurfaceGain, 1.);
    fragColor = vec4(accumColor.rgb, 1.0);
}
