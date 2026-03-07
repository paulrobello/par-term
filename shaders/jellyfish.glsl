/*! par-term shader metadata
name: jellyfish
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: null
  brightness: 0.15
  text_opacity: null
  full_content: null
  channel0: textures/wallpaper/MagicMushrooms.png
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: false
  use_background_as_channel0: null
*/

// Animated procedural jellyfish — dark water, neon blue/purple
// Depth layers, variable tentacle count/length, wide size variation

float hash(float n) { return fract(sin(n) * 43758.5453); }

float hash2(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = hash2(i), b = hash2(i + vec2(1,0));
    float c = hash2(i + vec2(0,1)), d = hash2(i + vec2(1,1));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

// Smooth minimum — negative inside both shapes, smooth junction
float smin(float a, float b, float k) {
    float hh = clamp(0.5 + 0.5*(b-a)/k, 0.0, 1.0);
    return mix(b, a, hh) - k*hh*(1.0-hh);
}

// Jellyfish bell SDF — upper dome open at bottom
// Uses smooth intersection (smax = -smin(-a,-b,k)) so the opening edge is
// rounded rather than a flat hard clip, eliminating the visible seam.
float bellSDF(vec2 p, float r, float h) {
    vec2 q = vec2(p.x / r, p.y / h);
    float d    = (length(q) - 1.0) * min(r, h);
    float clip = -(p.y + h * 0.08) * 0.5;
    float k    = h * 0.45; // smoothness — larger = rounder opening
    return -smin(-d, -clip, k); // smooth max = smooth intersection
}

// Single wavy tentacle — returns vec2(sdf, signed_x_from_axis)
// signed_x < 0 = left of axis, > 0 = right of axis (used for 3D horizontal lighting)
vec2 tentacleInfo(vec2 p, float bx, float s1, float s2, float maxLen) {
    float y = -p.y;
    if (y < 0.0) return vec2(1e6, 0.0);
    float tLen = maxLen * (0.70 + hash(s1 * 0.7) * 0.55);
    if (y > tLen) return vec2(1e6, 0.0);
    float t = y / tLen;
    float wave = sin(y * 22.0 + iTime * 2.8 + s1) * 0.010 * t
               + sin(y * 14.0 - iTime * 1.9 + s2)  * 0.007 * t
               + cos(y *  8.0 + iTime * 1.2 + s1 * 0.5) * 0.005 * t;
    float thick = mix(0.006, 0.0007, t * t);
    float dx = p.x - bx - wave;
    return vec2(abs(dx) - thick, dx);
}

// Returns additive (premultiplied) color for one jellyfish.
// depth: 0.0 = foreground (full size/brightness), 1.0 = background (small/dark)
vec3 drawJelly(vec2 p, float seed, float depth) {
    float depthScale = mix(1.0, 0.38, depth);

    // Wider size variation before depth scaling
    float r  = (0.040 + hash(seed * 3.1) * 0.090) * depthScale;
    float h  = r * 0.60;

    // Breathing pulse — nonlinear: quick snap contraction, slow relaxation
    float pulseFreq = 1.8 + hash(seed * 5.7) * 1.2;
    float rawPulse = 0.5 + 0.5 * sin(iTime * pulseFreq + seed * 6.28);
    float pulse = pow(rawPulse, 0.35); // spends most time relaxed, contracts sharply
    float br = r * 0.9 * (1.0 - 0.28 * (1.0 - pulse)); // narrows on contraction
    float bh = h * (1.0 + 0.30 * (1.0 - pulse));        // elongates on contraction

    // Neon blue / purple palette
    float hue = hash(seed * 7.3);
    vec3 colA, colB;
    if (hue < 0.33) {
        colA = vec3(0.05, 0.30, 1.00); // neon blue
        colB = vec3(0.50, 0.05, 0.95); // electric purple
    } else if (hue < 0.66) {
        colA = vec3(0.50, 0.05, 0.95); // electric purple
        colB = vec3(0.00, 0.70, 1.00); // cyan-blue
    } else {
        colA = vec3(0.00, 0.70, 1.00); // cyan-blue
        colB = vec3(0.15, 0.15, 0.90); // deep blue
    }
    vec3 col      = mix(colA, colB, hash(seed * 2.9));
    vec3 colBright = col + vec3(0.28, 0.22, 0.12);

    // Bell SDF
    float bd = bellSDF(p, br, bh);

    // Glowing inner core
    float coreDist = length(p - vec2(0.0, bh * 0.25)) / (r * 0.40);
    float core = exp(-coreDist * coreDist * 2.5) * 0.85;

    // Variable tentacle count: 3–9 per jellyfish
    int   nTent   = 3 + int(hash(seed * 9.3) * 7.0);
    // Per-jellyfish base tentacle length with wide variation, scaled by depth
    float tentLen = (0.10 + hash(seed * 6.3) * 0.25) * depthScale;

    float tentD = 1e6;
    float tentXSigned = 0.0; // signed x-offset from nearest tentacle axis
    vec2  tp = p + vec2(0.0, bh * 0.08);
    for (int k = 0; k < 9; k++) {
        if (k >= nTent) break;
        float fk  = float(k);
        float fnT = float(nTent);
        float bx  = (fk / max(fnT - 1.0, 1.0) - 0.5) * 2.0 * br * 0.88;
        vec2 info = tentacleInfo(tp, bx, fk + seed * 7.1, fk * seed * 1.3 + 0.5, tentLen);
        if (info.x < tentD) {
            tentD = info.x;
            tentXSigned = info.y;
        }
    }

    // Tentacle gradient: bright/white at root → deep saturated at tips
    float tentDepth = clamp(-(p.y + bh * 0.08) / max(tentLen, 0.01), 0.0, 1.0);
    vec3 tentColRoot = colBright;
    vec3 tentColTip  = col * vec3(0.5, 0.4, 0.7);
    vec3 tentCol = mix(tentColRoot, tentColTip, tentDepth * tentDepth);

    // --- Accumulate additive contributions ---
    vec3 result = vec3(0.0);

    // Outer glow halo
    float glow = exp(-max(0.0, bd) / (r * 0.65));
    result += col * glow * 0.22;

    // Bell translucent fill
    float bellFill = smoothstep(0.005, -0.001, bd);
    result += col * bellFill * 0.42;

    // Bright bioluminescent core
    result += colBright * bellFill * core;

    // Rim highlight — masked off at the bell opening so no hard edge there
    float rimMask = smoothstep(-bh * 0.05, bh * 0.30, p.y);
    float rim = smoothstep(0.010, 0.001, abs(bd)) * rimMask;
    result += (col + vec3(0.4)) * rim * 0.45;

    // Skirt — smooth fan bridging bell to tentacle roots
    float skirtDepth = -(p.y + bh * 0.08);
    float skirtMaxD  = br * 0.55;
    float skirtFade  = clamp(1.0 - skirtDepth / skirtMaxD, 0.0, 1.0);
    float skirtWidth = br * (0.95 - 0.6 * (skirtDepth / skirtMaxD));
    float skirtShape = smoothstep(1.0, 0.3, abs(p.x) / max(skirtWidth, 0.001));
    float skirt = skirtShape * skirtFade * skirtFade
                * smoothstep(-bh * 0.02, bh * 0.025, skirtDepth);
    result += colBright * skirt * 0.42; // boosted to fill the junction seam

    // Tentacle glow + fill
    float tentGlow = exp(-max(0.0, tentD) / 0.009);
    result += tentCol * tentGlow * 0.14;

    // 3D horizontal lighting gradient: cylindrical cross-section lit from upper-left
    // normalX: -1 = left edge, 0 = center axis, +1 = right edge
    float tentHalfW = 0.0045;
    float normalX = clamp(tentXSigned / tentHalfW, -1.0, 1.0);
    // Roundness from unsigned distance: center bright, edges dark
    float u = clamp(-tentD / tentHalfW, 0.0, 1.0);
    float roundness = sqrt(max(0.0, 2.0*u - u*u));
    // Diffuse: left face fully illuminated, right face in shadow
    float diffuse = max(0.0, -normalX);
    // Specular highlight: bright spot on the lit (left) side
    float specPeak = normalX + 0.50;
    float specular = exp(-specPeak * specPeak * 8.0) * 0.50;
    float lighting = 0.25 + 0.55 * diffuse + 0.40 * roundness + specular;
    vec3 tentColRound = tentCol * clamp(lighting, 0.1, 1.8);

    float tentFill = smoothstep(0.0022, -0.001, tentD);
    result += tentColRound * tentFill * 0.88;

    // Depth darkening: background jellyfish are much dimmer
    return result * mix(0.80, 0.18, depth);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv  = fragCoord / iResolution.xy;
    float asp = iResolution.x / iResolution.y;
    // Flip Y: uv.y=0 is top in par-term
    vec2 p   = vec2((uv.x - 0.5) * asp, 0.5 - uv.y);

    float t = iTime;

    // ── Background ─────────────────────────────────────────────────────────
    // Use iChannel0 image if configured, otherwise procedural dark water.
    bool hasBgTex = iChannelResolution[0].x > 1.0 && iChannelResolution[0].y > 1.0;

    vec3 bg;
    if (hasBgTex) {
        bg = texture(iChannel0, uv).rgb;
        // Darken it so jellyfish remain the visual focus
        bg *= 0.55;
    } else {
        // Procedural deep water
        vec3 deep    = vec3(0.003, 0.007, 0.045);
        vec3 surface = vec3(0.008, 0.022, 0.090);
        // uv.y=0 is top (surface), uv.y=1 is bottom (deep)
        bg = mix(surface, deep, pow(uv.y, 0.70));

        // Caustic light shimmer from above (water-specific, skip with image bg)
        float caust = 0.0;
        for (int i = 0; i < 3; i++) {
            float fi = float(i);
            vec2 cn = uv * (2.5 + fi * 1.3)
                    + vec2(sin(t * 0.18 + fi * 2.3), cos(t * 0.13 + fi * 1.7)) * 0.35;
            caust += abs(noise(cn) - 0.5);
        }
        // Caustic strongest near surface (top = uv.y near 0)
        bg += vec3(0.003, 0.007, 0.026) * (caust / 3.0) * (1.0 - uv.y) * 1.6;
    }

    // Bioluminescent floating particles rising slowly
    for (int i = 0; i < 20; i++) {
        float fi  = float(i);
        float px  = (hash(fi * 3.71) - 0.5) * asp;
        float spd = 0.012 + hash(fi * 9.37) * 0.022;
        float py  = fract(hash(fi * 5.13) + t * spd) - 0.5;
        float pd  = length(p - vec2(px, py));
        float ps  = 0.003 + hash(fi * 2.91) * 0.003;
        float tw  = 0.3 + 0.7 * sin(t * 2.1 + fi * 1.73);
        vec3  pc  = mix(vec3(0.10, 0.38, 0.90), vec3(0.50, 0.10, 0.90), hash(fi * 7.33));
        bg += pc * smoothstep(ps * 2.5, 0.0, pd) * tw * 0.30;
    }

    // ── Jellyfish — background layer first, then foreground ───────────────
    vec3 jellies = vec3(0.0);

    // Background: 6 jellyfish, smaller, darker, slower — appear further away
    for (int i = 0; i < 6; i++) {
        float fi   = float(i);
        float seed = fi + 20.0;                          // distinct seed range
        float depth = 0.60 + hash(fi * 3.37) * 0.35;   // 0.60–0.95

        float ySpeed = 0.010 + hash(fi * 6.83) * 0.014;
        float ySign  = (hash(fi * 13.71) > 0.35) ? 1.0 : -1.0; // 65% up, 35% down
        float xSpeed = (hash(fi * 19.31) - 0.5) * ySpeed * 0.55; // independent sideways drift
        float phase  = hash(fi * 11.17);
        float xPhase = hash(fi * 5.51);
        float py     = fract(phase  + t * ySpeed * ySign) * 1.60 - 0.75;
        float xPos   = (fract(xPhase + t * xSpeed) - 0.5) * asp * 1.84;
        float xWobble = sin(t * 0.17 + fi * 2.71) * 0.025 * asp;

        jellies += drawJelly(p - vec2(xPos + xWobble, py), seed, depth);
    }

    // Foreground: 7 jellyfish, full brightness
    for (int i = 0; i < 7; i++) {
        float fi   = float(i);
        float seed = fi;
        float depth = 0.0;

        float ySpeed = 0.018 + hash(fi * 4.37) * 0.025;
        float ySign  = (hash(fi * 23.71) > 0.35) ? 1.0 : -1.0; // 65% up, 35% down
        float xSpeed = (hash(fi * 31.13) - 0.5) * ySpeed * 0.55;
        float phase  = hash(fi * 8.11);
        float xPhase = hash(fi * 2.31);
        float py     = fract(phase  + t * ySpeed * ySign) * 1.60 - 0.75;
        float xPos   = (fract(xPhase + t * xSpeed) - 0.5) * asp * 1.60;
        float xWobble = sin(t * 0.22 + fi * 1.91) * 0.05 * asp;

        jellies += drawJelly(p - vec2(xPos + xWobble, py), seed, depth);
    }

    // ── Composite ──────────────────────────────────────────────────────────
    vec3 scene = bg + jellies;

    // Radial vignette
    float vd = dot(p / vec2(asp, 1.0), p / vec2(asp, 1.0));
    scene *= 1.0 - clamp(vd * 0.75, 0.0, 0.65);

    // Terminal text overlay
    vec4 term = texture(iChannel4, uv);
    scene = mix(scene, term.rgb, term.a);

    fragColor = vec4(clamp(scene, 0.0, 1.0), 1.0);
}
