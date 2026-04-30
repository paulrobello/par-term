/*! par-term shader metadata
name: clouds
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
    iCloudColor: '#ffffd9'
    iCloudCover: 0.19999999
    iCloudDensity: 8.0
    iCloudHighlight: 0.29999998
    iCloudScale: 1.1
    iCloudShadow: 0.5
    iCloudSpeed: 0.030000001
    iSkyHighColor: '#66b3ff'
    iSkyLowColor: '#336699'
    iSkyTint: 0.5
*/

// Animated clouds background shader
// Based on Shadertoy cloud shader
// Usage: Set custom_shader: "clouds.glsl" in config

// control slider min=0.3 max=3.0 step=0.01 label="Cloud Scale"
uniform float iCloudScale;
// control slider min=0.0 max=0.15 step=0.001 label="Drift Speed"
uniform float iCloudSpeed;
// control slider min=0.0 max=1.0 step=0.01 label="Cloud Shadow"
uniform float iCloudShadow;
// control slider min=0.0 max=1.0 step=0.01 label="Cloud Highlight"
uniform float iCloudHighlight;
// control slider min=0.0 max=0.8 step=0.01 label="Cloud Cover"
uniform float iCloudCover;
// control slider min=1.0 max=14.0 step=0.1 label="Cloud Density"
uniform float iCloudDensity;
// control slider min=0.0 max=1.5 step=0.01 label="Sky Tint"
uniform float iSkyTint;
// control color label="Lower Sky"
uniform vec3 iSkyLowColor;
// control color label="Upper Sky"
uniform vec3 iSkyHighColor;
// control color label="Cloud Tint"
uniform vec3 iCloudColor;

const mat2 m = mat2( 1.6,  1.2, -1.2,  1.6 );

vec2 hash( vec2 p ) {
    p = vec2(dot(p,vec2(127.1,311.7)), dot(p,vec2(269.5,183.3)));
    return -1.0 + 2.0*fract(sin(p)*43758.5453123);
}

float noise( in vec2 p ) {
    const float K1 = 0.366025404; // (sqrt(3)-1)/2;
    const float K2 = 0.211324865; // (3-sqrt(3))/6;
    vec2 i = floor(p + (p.x+p.y)*K1);
    vec2 a = p - i + (i.x+i.y)*K2;
    vec2 o = (a.x>a.y) ? vec2(1.0,0.0) : vec2(0.0,1.0);
    vec2 b = a - o + K2;
    vec2 c = a - 1.0 + 2.0*K2;
    vec3 h = max(0.5-vec3(dot(a,a), dot(b,b), dot(c,c) ), 0.0 );
    vec3 n = h*h*h*h*vec3( dot(a,hash(i+0.0)), dot(b,hash(i+o)), dot(c,hash(i+1.0)));
    return dot(n, vec3(70.0));
}

float fbm(vec2 n) {
    float total = 0.0, amplitude = 0.1;
    for (int i = 0; i < 5; i++) {
        total += noise(n) * amplitude;
        n = m * n;
        amplitude *= 0.4;
    }
    return total;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 p = fragCoord.xy / iResolution.xy;
    vec2 aspect = vec2(iResolution.x / iResolution.y, 1.0);
    vec2 uv = p * aspect;
    float time = iTime * iCloudSpeed;
    float q = fbm(uv * iCloudScale * 0.5);

    // ridged noise shape
    float r = 0.0;
    uv *= iCloudScale;
    uv -= q - time;
    float weight = 0.8;
    for (int i = 0; i < 6; i++) {
        r += abs(weight * noise(uv));
        uv = m * uv + time;
        weight *= 0.7;
    }

    // noise shape
    float f = 0.0;
    uv = p * aspect * iCloudScale;
    uv -= q - time;
    weight = 0.7;
    for (int i = 0; i < 6; i++) {
        f += weight * noise(uv);
        uv = m * uv + time;
        weight *= 0.6;
    }

    f *= r + f;

    // noise colour
    float c = 0.0;
    time = iTime * iCloudSpeed * 2.0;
    uv = p * aspect * iCloudScale * 2.0;
    uv -= q - time;
    weight = 0.4;
    for (int i = 0; i < 5; i++) {
        c += weight * noise(uv);
        uv = m * uv + time;
        weight *= 0.6;
    }

    // noise ridge colour
    float c1 = 0.0;
    time = iTime * iCloudSpeed * 3.0;
    uv = p * aspect * iCloudScale * 3.0;
    uv -= q - time;
    weight = 0.4;
    for (int i = 0; i < 5; i++) {
        c1 += abs(weight * noise(uv));
        uv = m * uv + time;
        weight *= 0.6;
    }

    c += c1;

    vec3 skycolour = mix(iSkyHighColor, iSkyLowColor, p.y);
    vec3 cloudcolour = iCloudColor * clamp(iCloudShadow + iCloudHighlight * c, 0.0, 1.0);

    f = iCloudCover + iCloudDensity * f * r;

    vec3 result = mix(skycolour, clamp(iSkyTint * skycolour + cloudcolour, 0.0, 1.0), clamp(f + c, 0.0, 1.0));

    fragColor = vec4(result, 1.0);
}
