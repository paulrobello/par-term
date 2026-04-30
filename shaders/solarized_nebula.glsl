/*! par-term shader metadata
name: Solarized Nebula
author: par-term
description: Palette-aware nebula inspired by Solarized tones and active terminal background color.
version: 1.0.0
defaults:
  animation_speed: 0.25
  brightness: 0.30
  text_opacity: 1.0
  full_content: false
  uniforms:
    iAccentA: "#268bd2"
    iAccentB: "#2aa198"
    iAccentC: "#b58900"
    iNebulaStrength: 0.45
*/

// control color label="Blue Accent"
uniform vec3 iAccentA;
// control color label="Cyan Accent"
uniform vec3 iAccentB;
// control color label="Gold Accent"
uniform vec3 iAccentC;
// control slider min=0 max=1 step=0.01 label="Nebula Strength"
uniform float iNebulaStrength;

float hash(vec2 p) { return fract(sin(dot(p, vec2(17.13, 91.7))) * 43758.5453); }
float noise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1,0)), u.x), mix(hash(i + vec2(0,1)), hash(i + vec2(1,1)), u.x), u.y);
}
float fbm(vec2 p) {
    float v = 0.0;
    float a = 0.5;
    for (int i = 0; i < 4; i++) {
        v += noise(p) * a;
        p = mat2(1.6, 1.2, -1.2, 1.6) * p + 4.3;
        a *= 0.5;
    }
    return v;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec2 p = (fragCoord - 0.5 * iResolution.xy) / min(iResolution.x, iResolution.y);
    vec3 themeBase = (iBackgroundColor.a > 0.0) ? iBackgroundColor.rgb : vec3(0.0, 0.17, 0.21);
    float n = fbm(p * 2.4 + vec2(iTime * 0.04, -iTime * 0.02));
    float swirl = fbm(p * 4.0 + n + iTime * 0.03);
    vec3 neb = mix(iAccentA, iAccentB, n);
    neb = mix(neb, iAccentC, smoothstep(0.65, 1.0, swirl));
    vec3 color = themeBase * 0.55 + neb * swirl * iNebulaStrength;
    color *= 0.75 + 0.25 * (1.0 - smoothstep(0.2, 1.1, length(uv - 0.5)));
    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
