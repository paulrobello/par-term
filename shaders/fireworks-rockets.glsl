/*! par-term shader metadata
name: fireworks-rockets
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.33
  text_opacity: null
  full_content: null
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: ''
  cubemap_enabled: false
*/

// This Ghostty shader is a lightly modified port of https://www.shadertoy.com/view/4dBGRw

#define BLACK_BLEND_THRESHOLD .4

//Creates a diagonal red-and-white striped pattern.
vec3 barberpole(vec2 pos, vec2 rocketpos) {
    float d = (pos.x - rocketpos.x) + (pos.y - rocketpos.y);
    vec3 col = vec3(1.0);

    d = mod(d * 20., 2.0);
    if (d > 1.0) {
        col = vec3(1.0, 0.0, 0.0);
    }

    return col;
}

vec3 rocketTrail(vec2 pos, vec2 rocketpos, float time) {
    vec3 col = vec3(0.0);
    float dx = pos.x - rocketpos.x;
    float dy = rocketpos.y - pos.y;  // Below the rocket

    if (dy > 0.0 && dy < 1.2) {
        // Trail width narrows as it goes down
        float width = 0.05 * (1.0 - dy * 0.7);
        if (abs(dx) < width) {
            // Flicker effect
            float flicker = 0.8 + 0.2 * sin(time * 40.0 + dy * 20.0);
            // Color gradient: bright white/yellow at top, orange at bottom
            float t = dy * 0.8;
            vec3 trailCol = mix(vec3(1.5, 1.4, 1.0), vec3(1.2, 0.5, 0.1), t);
            // Fade out at edges and bottom - less aggressive fade
            float fade = (1.0 - abs(dx) / width * 0.5) * (1.0 - dy * 0.5);
            col = trailCol * fade * flicker * 2.0;
        }
    }
    return col;
}

vec3 rocket(vec2 pos, vec2 rocketpos, float time) {
    vec3 col = vec3(0.0);
    float f = 0.;
    float absx = abs(rocketpos.x - pos.x);
    float absy = abs(rocketpos.y - pos.y);

    // Rocket trail (flame exhaust)
    col += rocketTrail(pos, rocketpos, time);

    // Wooden stick
    if (absx < 0.01 && absy < 0.22) {
        col = vec3(1.0, 0.5, 0.5);
    }

    // Barberpole
    if (absx < 0.05 && absy < 0.15) {
        col = barberpole(pos, rocketpos);
    }

    // Rocket Point (flipped to point upward)
    float pointw = (pos.y - rocketpos.y - 0.25) * -0.7;
    if ((pos.y - rocketpos.y) > 0.1) {
        f = smoothstep(pointw - 0.001, pointw + 0.001, absx);

        col = mix(vec3(1.0, 0.0, 0.0), col, f);
    }

    // Shadow
    f = -.5 + smoothstep(-0.05, 0.05, (rocketpos.x - pos.x));
    col *= 0.7 + f;

    return col;
}

float rand(float val, float seed) {
    return cos(val * sin(val * seed) * seed);
}

float distance2(in vec2 a, in vec2 b) {
    return dot(a - b, a - b);
}

// Precomputed rotation matrix for 1 radian
const mat2 rr = mat2(0.5403, -0.8415, 0.8415, 0.5403);

vec3 drawParticles(vec2 pos, vec3 particolor, float time, vec2 cpos, float gravity, float seed, float timelength) {
    vec3 col = vec3(0.0);
    vec2 pp = vec2(1.0, 0.0);
    for (float i = 1.0; i <= 64.0; i++) {
        float d = rand(i, seed);
        float fade = (i / 64.0) * time;
        vec2 particpos = cpos + time * pp * d;
        pp = rr * pp;
        // Shimmer effect - simplified
        float shimmer = sin(time * 50.0 + i * 7.0 + d * 20.0);
        vec3 shimmerCol = particolor + vec3(0.3, 0.3, 0.5) * shimmer;
        shimmerCol *= (1.0 + 0.8 * shimmer);
        col = mix(shimmerCol / fade, col, smoothstep(0.0, 0.0001, distance2(particpos, pos)));
    }
    col *= smoothstep(0.0, 1.0, (timelength - time) / timelength);

    return col;
}
vec3 drawFireworks(float time, vec2 uv, vec3 particolor, float seed) {
    float timeoffset = 1.0;
    vec3 col = vec3(0.0);
    if (time <= 0.) {
        return col;
    }
    if (mod(time, 6.0) > timeoffset) {
        col = drawParticles(uv, particolor, mod(time, 6.0) - timeoffset, vec2(rand(ceil(time / 6.0), seed), 0.0), 0.5, ceil(time / 6.0), seed);
    } else {
        col = rocket(uv * 3., vec2(3. * rand(ceil(time / 6.0), seed), 3. * (-1.0 + mod(time, 6.0) / timeoffset)), time);
    }
    return col;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = 1.0 - 2.0 * fragCoord.xy / iResolution.xy;
    uv.x *= iResolution.x / iResolution.y;
    vec3 col = vec3(0.1, 0.1, 0.2);

    col += 0.1 * uv.y;

    col += drawFireworks(iTime, uv, vec3(1.0, 0.1, 0.1), 1.);
    col += drawFireworks(iTime - 2.0, uv, vec3(0.0, 1.0, 0.5), 2.);
    col += drawFireworks(iTime - 4.0, uv, vec3(1.0, 1.0, 0.1), 3.);

    vec2 termUV = fragCoord.xy / iResolution.xy;
    vec4 terminalColor = texture(iChannel4, termUV);

    float alpha = step(length(terminalColor.rgb), BLACK_BLEND_THRESHOLD);
    vec3 blendedColor = mix(terminalColor.rgb * 1.0, col.rgb * 0.3, alpha);

    fragColor = vec4(blendedColor, terminalColor.a);
}
