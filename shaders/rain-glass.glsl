/*! par-term shader metadata
name: rain-glass
author: null
description: Rain on glass with procedural dark nebula background - no texture needed
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.3
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: ''
  cubemap_enabled: false
  use_background_as_channel0: null
  uniforms:
    iColorShift: 1.0
    iDriftSpeed: 0.0
    iDropScale: 1.0
    iFineDetailAmount: 0.14
    iFineDetailScale: 4.0
    iFogAmount: 0.0
    iFogClearAmount: 0.79999995
    iFogColor: '#b3bfcc'
    iNebulaBrightness: 0.25
    iNebulaScale: 3.0
    iNebulaVignette: 0.0
    iRainAmount: 0.7
    iRainPulse: 0.29999998
    iRainSpeed: 1.0
    iRefractionStrength: 1.0
    iScreenVignette: 1.0
    iWarpScale: 10.0
*/

// Rain on Glass with Procedural Nebula Background
// Rain functions from Heartfelt by Martijn Steinrucken aka BigWings - 2017
// Nebula background using domain-warped FBM noise (clouds.glsl noise functions)

// ============== CONTROLS ==============
// control slider min=0 max=1 step=0.01 label="Rain Amount"
uniform float iRainAmount;
// control slider min=0 max=1 step=0.01 label="Rain Pulse"
uniform float iRainPulse;
// control slider min=0 max=3 step=0.01 label="Rain Speed"
uniform float iRainSpeed;
// control slider min=0.5 max=2 step=0.01 label="Drop Scale"
uniform float iDropScale;
// control slider min=0 max=2 step=0.01 label="Refraction"
uniform float iRefractionStrength;
// control slider min=0.5 max=8 step=0.01 label="Nebula Scale"
uniform float iNebulaScale;
// control slider min=0 max=20 step=0.1 label="Nebula Warp"
uniform float iWarpScale;
// control slider min=0 max=1 step=0.01 label="Nebula Drift"
uniform float iDriftSpeed;
// control slider min=0.5 max=12 step=0.01 label="Fine Detail Scale"
uniform float iFineDetailScale;
// control slider min=0 max=0.5 step=0.01 label="Fine Detail"
uniform float iFineDetailAmount;
// control slider min=0 max=2 step=0.01 label="Nebula Vignette"
uniform float iNebulaVignette;
// control slider min=0.02 max=1 step=0.01 label="Nebula Brightness"
uniform float iNebulaBrightness;
// control slider min=0 max=1 step=0.01 label="Fog Amount"
uniform float iFogAmount;
// control slider min=0 max=1 step=0.01 label="Fog Clearing"
uniform float iFogClearAmount;
// control color label="Fog Color"
uniform vec3 iFogColor;
// control slider min=0 max=1 step=0.01 label="Color Shift"
uniform float iColorShift;
// control slider min=0 max=1 step=0.01 label="Screen Vignette"
uniform float iScreenVignette;

#define NEBULA_OCTAVES 1
// ======================================

// ============== COLOR PALETTE ================
// Four nebula colors blended by noise value (RGB, keep values dark for readability)
#define COLOR_DEEP_SPACE vec3(0.02, 0.02, 0.04)
#define COLOR_DARK_TEAL  vec3(0.03, 0.08, 0.10)
#define COLOR_DARK_PURPLE vec3(0.08, 0.03, 0.12)
#define COLOR_DEEP_BLUE  vec3(0.04, 0.06, 0.15)
// Brightness highlight tint (added in bright warp regions)
#define COLOR_HIGHLIGHT  vec3(0.03, 0.05, 0.08)
// ==============================================

// Fog is runtime-controlled with iFogAmount. A default of 0.0 preserves
// the original compiled-off fog behavior.

#define S(a, b, t) smoothstep(a, b, t)
//#define CHEAP_NORMALS
#define USE_POST_PROCESSING

// --- Rotation matrix for FBM (from clouds.glsl) ---
const mat2 m = mat2( 1.6,  1.2, -1.2,  1.6 );

// --- Noise functions (from clouds.glsl, naga-compatible) ---

vec2 hash( vec2 p ) {
    p = vec2(dot(p,vec2(127.1,311.7)), dot(p,vec2(269.5,183.3)));
    return -1.0 + 2.0*fract(sin(p)*43758.5453123);
}

float noise( in vec2 p ) {
    const float K1 = 0.366025404; // (sqrt(3)-1)/2
    const float K2 = 0.211324865; // (3-sqrt(3))/6
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
    for (int i = 0; i < NEBULA_OCTAVES; i++) {
        total += noise(n) * amplitude;
        n = m * n;
        amplitude *= 0.4;
    }
    return total;
}

// --- Domain warping for organic nebula swirls ---

float warpedFbm(vec2 p, float t) {
    float driftSpeed = clamp(iDriftSpeed, 0.0, 1.0);
    float warpScale = clamp(iWarpScale, 0.0, 20.0);

    // First warp pass
    vec2 q = vec2(
        fbm(p + vec2(0.0, 0.0) + t * driftSpeed),
        fbm(p + vec2(5.2, 1.3) + t * driftSpeed * 0.7)
    );
    // Second warp pass for extra organic shape
    vec2 r = vec2(
        fbm(p + warpScale * q + vec2(1.7, 9.2) + t * driftSpeed * 0.4),
        fbm(p + warpScale * q + vec2(8.3, 2.8) + t * driftSpeed * 0.5)
    );
    return fbm(p + warpScale * r);
}

// --- Dark nebula color mapping ---

vec3 nebulaColor(float val, float t) {
    // Slow hue drift
    float hueShift = t * clamp(iDriftSpeed, 0.0, 1.0) * 0.5;

    vec3 c0 = COLOR_DEEP_SPACE;
    vec3 c1 = COLOR_DARK_TEAL;
    vec3 c2 = COLOR_DARK_PURPLE;
    vec3 c3 = COLOR_DEEP_BLUE;

    // Rotate palette slightly over time
    float phase = val * 3.0 + hueShift;
    float w0 = max(0.0, 1.0 - abs(phase - 0.0));
    float w1 = max(0.0, 1.0 - abs(phase - 1.0));
    float w2 = max(0.0, 1.0 - abs(phase - 2.0));
    float w3 = max(0.0, 1.0 - abs(phase - 3.0));

    float wSum = w0 + w1 + w2 + w3 + 0.001;
    return (c0 * w0 + c1 * w1 + c2 * w2 + c3 * w3) / wSum;
}

// --- Procedural background ---

vec3 proceduralBackground(vec2 uv, float t) {
    vec2 safeUv = clamp(uv, vec2(0.0), vec2(1.0));

    // Aspect-corrected UV for nebula
    vec2 nuv = safeUv * clamp(iNebulaScale, 0.5, 8.0);

    // Domain-warped FBM for organic nebula shapes
    float warp = warpedFbm(nuv, t);

    // Map to color
    float val = warp * 3.0 + 0.5; // shift into useful range
    val = clamp(val, 0.0, 1.0);
    vec3 col = nebulaColor(val, t);

    // Brighten based on warp intensity for variation
    float brightness = smoothstep(-0.1, 0.15, warp) * 0.25;
    col += brightness * COLOR_HIGHLIGHT;

    // Fine detail layer for visible raindrop refraction at small UV offsets
    float detail = noise(safeUv * clamp(iFineDetailScale, 0.5, 12.0) + t * clamp(iDriftSpeed, 0.0, 1.0) * 2.0);
    col += detail * clamp(iFineDetailAmount, 0.0, 0.5);

    // Vignette
    vec2 center = safeUv - 0.5;
    float vig = 1.0 - dot(center, center) * clamp(iNebulaVignette, 0.0, 2.0);
    col *= clamp(vig, 0.0, 1.0);

    // Keep values in dark range
    col = clamp(col, 0.0, clamp(iNebulaBrightness, 0.02, 1.0));

    return col;
}

// --- Rain functions (verbatim from rain.glsl) ---

vec3 N13(float p) {
    //  from DAVE HOSKINS
   vec3 p3 = fract(vec3(p) * vec3(.1031,.11369,.13787));
   p3 += dot(p3, p3.yzx + 19.19);
   return fract(vec3((p3.x + p3.y)*p3.z, (p3.x+p3.z)*p3.y, (p3.y+p3.z)*p3.x));
}

vec4 N14(float t) {
    return fract(sin(t*vec4(123., 1024., 1456., 264.))*vec4(6547., 345., 8799., 1564.));
}

float N(float t) {
    return fract(sin(t*12345.564)*7658.76);
}

float Saw(float b, float t) {
    return S(0., b, t)*S(1., b, t);
}

vec2 DropLayer2(vec2 uv, float t) {
    vec2 UV = uv;

    uv.y -= t*0.75;
    vec2 a = vec2(6., 1.);
    vec2 grid = a*2.;
    vec2 id = floor(uv*grid);

    float colShift = N(id.x);
    uv.y += colShift;

    id = floor(uv*grid);
    vec3 n = N13(id.x*35.2+id.y*2376.1);
    vec2 st = fract(uv*grid)-vec2(.5, 0);

    float x = n.x-.5;

    float y = UV.y*20.;
    float wiggle = sin(y+sin(y));
    x += wiggle*(.5-abs(x))*(n.z-.5);
    x *= .7;
    float ti = fract(t+n.z);
    y = 1.0 - ((Saw(.85, ti)-.5)*.9+.5);
    vec2 p = vec2(x, y);

    float d = length((st-p)*a.yx);

    float mainDrop = S(.4, .0, d);

    float r = sqrt(S(0., y, st.y));
    float cd = abs(st.x-x);
    float trail = S(.23*r, .15*r*r, cd);
    float trailFront = S(-.02, .02, y-st.y);
    trail *= trailFront*r*r;

    y = UV.y;
    float trail2 = S(.2*r, .0, cd);
    float droplets = max(0., (sin(y*(1.-y)*120.)-st.y))*trail2*trailFront*n.z;
    y = fract(y*10.)+(st.y-.5);
    float dd = length(st-vec2(x, y));
    droplets = S(.3, 0., dd);
    float m2 = mainDrop+droplets*r*trailFront;

    return vec2(m2, trail);
}

float StaticDrops(vec2 uv, float t) {
    uv *= 40.;

    vec2 id = floor(uv);
    uv = fract(uv)-.5;
    vec3 n = N13(id.x*107.45+id.y*3543.654);
    vec2 p = (n.xy-.5)*.7;
    float d = length(uv-p);

    float fade = Saw(.025, fract(t+n.z));
    float c = S(.3, 0., d)*fract(n.z*10.)*fade;
    return c;
}

vec2 Drops(vec2 uv, float t, float l0, float l1, float l2) {
    float s = StaticDrops(uv, t)*l0;
    vec2 m1 = DropLayer2(uv, t)*l1;
    vec2 m2 = DropLayer2(uv*1.85, t)*l2;

    float c = s+m1.x+m2.x;
    c = S(.3, 1., c);

    return vec2(c, max(m1.y*l0, m2.y*l1));
}

// --- Main ---

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 uv = (fragCoord.xy-.5*iResolution.xy) / iResolution.y;
    vec2 UV = fragCoord.xy/iResolution.xy;
    float T = iTime * max(iRainSpeed, 0.0);

    float t = T*.2;

    float rainBase = clamp(iRainAmount, 0.0, 1.0);
    float rainPulse = clamp(iRainPulse, 0.0, 1.0);
    float rainAmount = clamp(sin(T*.05) * rainPulse + rainBase, 0.0, 1.0);

    uv *= .7 * clamp(iDropScale, 0.5, 2.0);
    UV = (UV-.5)*.9+.5;

    float staticDrops = S(-.5, 1., rainAmount)*2.;
    float layer1 = S(.25, .75, rainAmount);
    float layer2 = S(.0, .5, rainAmount);

    vec2 c = Drops(uv, t, staticDrops, layer1, layer2);

    #ifdef CHEAP_NORMALS
        vec2 n = vec2(dFdx(c.x), dFdy(c.x));
    #else
        vec2 e = vec2(.001, 0.);
        float cx = Drops(uv+e, t, staticDrops, layer1, layer2).x;
        float cy = Drops(uv+e.yx, t, staticDrops, layer1, layer2).x;
        vec2 n = vec2(cx-c.x, cy-c.x);
    #endif

    // Sample procedural background instead of texture
    vec2 backgroundUV = UV + n * clamp(iRefractionStrength, 0.0, 2.0);
    vec3 col = proceduralBackground(backgroundUV, T);

    // Apply fog effect - drops and trails clear the fog
    float fogClearAmount = clamp(iFogClearAmount, 0.0, 1.0);
    float fogClear = c.y * fogClearAmount;
    fogClear = max(fogClear, S(.1, .3, c.x) * fogClearAmount);
    float fogLevel = clamp(iFogAmount, 0.0, 1.0) * (1.0 - fogClear);
    col = mix(col, iFogColor, fogLevel);

    #ifdef USE_POST_PROCESSING
    float colFade = (sin(t*.2)*.5+.5) * clamp(iColorShift, 0.0, 1.0);
    col *= mix(vec3(1.), vec3(.8, .9, 1.3), colFade);    // subtle color shift
    vec2 vignetteUV = UV - .5;
    col *= mix(1.0, 1.0 - dot(vignetteUV, vignetteUV), clamp(iScreenVignette, 0.0, 1.0));
    #endif

    fragColor = vec4(col, 1.);
}
