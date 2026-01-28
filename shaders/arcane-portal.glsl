/*! par-term shader metadata
name: arcane-portal
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.75
  text_opacity: null
  full_content: null
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: ''
  cubemap_enabled: false
*/

// Arcane Portal - Portal background shader
// Original by chronos: https://www.shadertoy.com/view/wf3BWM
// Adapted for par-term terminal background (optimized)

const float H = 1.8;
const vec3 DOT_VEC = vec3(0.2);

// Smooth abs with fixed smoothing factor
float sabs(float x) {
    return sqrt(x*x + 0.09) - 0.3;
}

// Optimized SDF - unrolled loop, reduced iterations
float f(vec3 p) {
    float pz = p.z * 0.1;
    float sdf = p.y;
    // Unrolled: j = 0.04, 0.08, 0.16, 0.32, 0.64, 1.28, 2.56 (skip smallest for perf)
    sdf += (abs(dot(sin(pz + p * 6.25), DOT_VEC)) - 0.1) * 0.16;
    sdf += (abs(dot(sin(pz + p * 3.125), DOT_VEC)) - 0.1) * 0.32;
    sdf += (abs(dot(sin(pz + p * 1.5625), DOT_VEC)) - 0.1) * 0.64;
    sdf += (abs(dot(sin(pz + p * 0.78125), DOT_VEC)) - 0.1) * 1.28;
    sdf += (abs(dot(sin(pz + p * 0.390625), DOT_VEC)) - 0.1) * 2.56;
    return sdf;
}

// Simplified f2 - only needs 2 iterations per original
float f2(vec3 p) {
    float pz = p.z * 0.1;
    float sdf = p.y;
    sdf += (sabs(dot(sin(pz + p * 0.390625), DOT_VEC)) - 0.1) * 2.56;
    return min(sdf, p.y + H);
}

vec4 portal_target(float time, in vec3 ro, in vec3 rd) {
    vec3 col = vec3(0);
    float t2 = time * 0.2;
    ro -= vec3(0, 0.8, 4.0 * time);
    ro.x -= sin(t2) * 10.0;

    float ct2 = cos(t2) * 0.25;
    vec4 m = cos(ct2 + vec4(0, -11, 11, 0));
    rd.xy *= mat2(m.x, m.y, m.z, m.w);

    float t = 0.0;
    float f2_ro = f2(ro);
    ro.y += 0.2 - 1.5 * f2_ro;
    ro.y = 0.5 * (ro.y + H) + 0.5 * sabs(ro.y + H) - H;

    // Pre-compute angle (simplified gradient estimation)
    float angle = 0.2 * ((f2(ro) - f2(ro + vec3(0, 0, 0.5))) * 1.5 + 0.3);
    float C = cos(angle), S = sin(angle);
    rd.yz *= mat2(C, S, -S, C);

    float T = 1.0;
    float sdf;

    // Reduced iterations: 40 instead of 60
    for(int i = 0; i < 40 && t < 80.0; i++) {
        vec3 p = rd * t + ro;

        if(p.y < -H) {
            float fresnel = pow(clamp(1.0 + rd.y, 0.0, 1.0), 5.0);
            p.y = abs(p.y + H) - H;
            T = fresnel;
        }

        sdf = f(p);
        t += sdf * 0.65 + 0.001;

        if(abs(sdf) < 0.001) {
            vec2 e = vec2(0.05, 0);
            vec3 n = normalize(vec3(f(p + e.xyy), f(p + e.yxy), f(p + e.yyx)) - sdf);
            col += pow(clamp(1.0 + dot(n, rd), 0.0, 1.0), 5.0);
            break;
        }
        col += (0.75 + 0.25 * sin(vec3(-1.75, 2.5, 1.3) + 0.72 * sdf * vec3(1, 2, 3.33)))
               * 0.1 * sdf * exp2(-0.5 * sdf - 0.1 * t) * T;
    }

    return vec4(col, 0);
}

const float PI_INV2 = 0.15915494; // 1/(2*pi)

// Optimized triwave - inline constant
vec3 triwave(vec3 x) {
    return abs(fract(x * PI_INV2 - 0.25) - 0.5) * 4.0 - 1.0;
}

void mainImage(out vec4 o, in vec2 fragCoord) {
    vec2 r = iResolution.xy;
    vec2 uv = (fragCoord.xy * 2.0 - r) / r.y;
    uv.y = -uv.y;
    float t = iTime;

    o = vec4(0, 0, 0, 1);
    vec3 cam_pos = vec3(0, 1.5, 10.0);
    vec3 rd = normalize(vec3(uv, -1.4));

    // Camera animation
    float camTime = t * 0.25;
    float cosTime = cos(camTime);
    float sinTime = sin(camTime);
    cam_pos += vec3(1.5 * cosTime, 0, 2.0 * sinTime);
    float angle = cosTime * 0.25;
    float c = cos(angle), s = sin(angle);
    rd.xz *= mat2(c, s, -s, c);

    const vec3 P = vec3(0, 2.3, 2.5);  // Portal pos
    const float h = 1.0;               // ground height

    // Ground intersection
    float g_hit_t = (-h - cam_pos.y) / rd.y;
    vec3 g_hit = g_hit_t * rd + cam_pos;

    // Portal reflection setup
    vec3 portal_rd = rd;
    if(g_hit_t > 0.0 && g_hit.z > P.z) {
        vec3 A = vec3(-1, -h, P.z) - cam_pos;
        vec3 B = vec3(1, -h, P.z) - cam_pos;
        portal_rd = reflect(portal_rd, normalize(cross(A, B)));
    }

    // Check if ray hits portal area
    vec4 portal_target_color = vec4(0);
    vec3 P2 = vec3(0, -2.0 * h - 2.3, 2.5);
    float radius = smoothstep(0.0, 2.0, t) * 3.0;

    float dist1 = length(dot(P - cam_pos, rd) * rd + cam_pos - P);
    float dist2 = length(dot(P2 - cam_pos, rd) * rd + cam_pos - P2);
    if(min(dist1, dist2) < radius) {
        portal_target_color = portal_target(t, cam_pos, portal_rd);
        portal_target_color *= portal_target_color * 300.0;
    }

    // Pre-compute constants for main loop
    vec4 baseColor = cos(vec4(1, 2, 2.5, 0)) + 1.0;
    float t2 = t * 2.0;
    float tAnim = -4.5 * t;

    float d = 1.0, z = 0.0;
    vec3 p;
    float transmission = 1.0;

    // Main raymarch - reduced to 50 iterations
    for(int i = 0; i < 50 && z < 500.0; i++) {
        p = z * rd + cam_pos;

        float D = length(p - P) - radius;
        float D2 = length((p - vec3(P.x, -h, P.z)) * vec3(1.5, 10.0, 1.5)) - radius;

        // Ground reflection
        if(p.y < -h) {
            p.y = abs(p.y + h) - h;
            transmission = 0.8 * (0.15 + 0.85 * pow(clamp(1.0 + rd.y, 0.0, 1.0), 5.0));
        } else {
            transmission = 1.0;
        }

        p.y += 0.24 * sin(p.z * 2.0 + t2 - d * 12.0);

        // Distortion calculation
        float T_rot = 2.5 * t - d * 14.0;
        float cr = cos(T_rot), sr = sin(T_rot);
        vec3 q = p - P;
        q.xy *= mat2(cr, sr, -sr, cr);

        // Reduced triwave iterations: 6 instead of 8
        q += triwave(q * 2.0 + t2).yzx * 0.5;
        q += triwave(q * 3.0 + t2).yzx * 0.333;
        q += triwave(q * 4.0 + t2).yzx * 0.25;
        q += triwave(q * 5.0 + t2).yzx * 0.2;
        q += triwave(q * 6.0 + t2).yzx * 0.167;
        q += triwave(q * 7.0 + t2).yzx * 0.143;

        d = 0.1 * abs(length(p - P) - radius) + abs(q.z) * 0.1;

        // Accumulate color
        float dInv = 10.0 / d;  // Pre-compute 1/d * 10
        o += transmission * (
            mix(
                (cos(d * 10.0 + vec4(1, 2, 2.5, 0)) + 1.0) * dInv * 0.1 * z,
                portal_target_color,
                smoothstep(0.0, -0.2, max(D, p.z - P.z))
            )
            + 2.0 * (cos(tAnim + d * 10.0 + vec4(1, 2, 2.5, 0)) + 1.0) * exp2(-D * D) * z
            + 10.0 * baseColor * exp2(-abs(D2)) * z
        );

        z += min(abs(p.y + h) * 0.4 + 0.03, d);
    }

    o = o * 0.0001;
    o *= 1.0 - length(uv) * 0.2;
    o = sqrt(1.0 - exp(-1.5 * o * o));

    // Blend with terminal content
    vec2 terminalUV = fragCoord.xy / iResolution.xy;
    vec4 terminalColor = texture(iChannel4, terminalUV);

    float brightnessThreshold = 0.1;
    float terminalBrightness = dot(terminalColor.rgb, vec3(0.2126, 0.7152, 0.0722));

    if (terminalBrightness < brightnessThreshold) {
        o = mix(terminalColor, o, 0.6);
    } else {
        o = terminalColor;
    }
}
