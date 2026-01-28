/*! par-term shader metadata
name: gyroid
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.28
  text_opacity: null
  full_content: null
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: ''
  cubemap_enabled: false
*/

// https://www.shadertoy.com/view/tXtyW8
// Optimized version - reduced iterations and removed reflection bounce
#define FAR 30.
#define PI 3.1415

int m = 0;

mat2 rot(float a) { float c = cos(a), s = sin(a); return mat2(c, -s, s, c); }
mat3 lookAt(vec3 dir) {
    vec3 up = vec3(0., 1., 0.);
    vec3 rt = normalize(cross(dir, up));
    return mat3(rt, cross(rt, dir), dir);
}

float gyroid(vec3 p) { return dot(cos(p), sin(p.zxy)) + 1.; }

float map(vec3 p) {
    float d1 = gyroid(p);
    float d2 = gyroid(p - vec3(0, 0, PI));
    m = d1 < d2 ? 1 : 2;
    return min(d1, d2);
}

float raymarch(vec3 ro, vec3 rd) {
    float t = 0.;
    for (int i = 0; i < 80; i++) {
        float d = map(ro + rd * t);
        if (abs(d) < .001 || t > FAR) break;
        t += d;
    }
    return t;
}

float getAO(vec3 p, vec3 sn) {
    float occ = 0.;
    float t = .08;
    occ += t - map(p + sn * t); t += .08;
    occ += t - map(p + sn * t); t += .08;
    occ += t - map(p + sn * t);
    return clamp(1. - occ * .5, 0., 1.);
}

vec3 getNormal(vec3 p) {
    vec2 e = vec2(0.5773, -0.5773) * 0.001;
    return normalize(e.xyy * map(p + e.xyy) + e.yyx * map(p + e.yyx) +
                     e.yxy * map(p + e.yxy) + e.xxx * map(p + e.xxx));
}

vec3 trace(vec3 ro, vec3 rd) {
    float d = raymarch(ro, rd);
    if (d > FAR) return vec3(0);

    // fog attenuation
    float fog = exp(-.008 * d * d);

    vec3 p = ro + rd * d;
    vec3 sn = getNormal(p);
    // Simplified normal perturbation - use lower frequency
    sn = normalize(sn + cos(p * 16.) * .05);

    // lighting
    vec3 lp = vec3(10., -10., -10. + ro.z);
    vec3 ld = normalize(lp - p);
    float diff = max(0., .5 + 2. * dot(sn, ld));
    float diff2 = pow(length(sin(sn * 2.) * .5 + .5), 2.);
    float diff3 = max(0., .5 + .5 * dot(sn, vec3(0, 1, 0)));

    float spec = max(0., dot(reflect(-ld, sn), -rd));
    vec3 col = vec3(0);

    col += vec3(.4, .6, .9) * diff;
    col += vec3(.5, .1, .1) * diff2;
    col += vec3(.9, .1, .4) * diff3;
    col += vec3(.3, .25, .25) * pow(spec, 4.) * 8.;

    // material colors
    float freck = dot(cos(p * 23.), vec3(1));
    vec3 alb = m == 1
        ? vec3(.2, .1, .9) * max(.6, step(2.5, freck))
        : vec3(.6, .3, .1) * max(.8, step(-2.5, freck));
    col *= alb;

    col *= getAO(p, sn);
    return col * fog;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord.xy - iResolution.xy * .5) / iResolution.y;

    vec3 ro = vec3(PI / 2., 0, -iTime * .5);
    vec3 rd = normalize(vec3(uv, -.5));

    rd.xy = rot(sin(iTime * .2)) * rd.xy;
    vec3 ta = vec3(cos(iTime * .4), sin(iTime * .4), 4.);
    rd = lookAt(normalize(ta)) * rd;

    vec3 col = trace(ro, rd);

    col *= smoothstep(0., 1., 1.2 - length(uv * .9));
    col = pow(col, vec3(0.4545));
    fragColor = vec4(col, 1.0);
}
