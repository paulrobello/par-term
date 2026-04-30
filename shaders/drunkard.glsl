/*! par-term shader metadata
name: drunkard
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: null
  full_content: true
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: ''
  cubemap_enabled: false
  use_background_as_channel0: null
  uniforms:
    aberration: true
    aberration_delta: 0.099999994
    animate: true
    noise_intensity: 0.049999997
    noise_scale: 1.0
    speed: 0.39999998
*/

// Drunken stupor effect using fractal Brownian motion and Perlin noise
// (c) moni-dz (https://github.com/moni-dz) 
// CC BY-NC-SA 4.0 (https://creativecommons.org/licenses/by-nc-sa/4.0/)

vec2 hash2(vec2 p) {
    uvec2 q = uvec2(floatBitsToUint(p.x), floatBitsToUint(p.y));
    q = (q * uvec2(1597334673U, 3812015801U)) ^ (q.yx * uvec2(2798796415U, 1979697793U));
    return vec2(q) * (1.0/float(0xffffffffU)) * 2.0 - 1.0;
}

float perlin2d(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f*f*(3.0-2.0*f);

    return mix(mix(dot(hash2(i), f),
                   dot(hash2(i + vec2(1,0)), f - vec2(1,0)), u.x),
               mix(dot(hash2(i + vec2(0,1)), f - vec2(0,1)),
                   dot(hash2(i + vec2(1,1)), f - vec2(1,1)), u.x), u.y);
}

#define OCTAVES 5      // How many passes of fractal Brownian motion to perform
#define GAIN 0.5       // How much should each pixel move
#define LACUNARITY 2.0 // How fast should each ripple be per pass

float fbm(vec2 p) {
    float sum = 0.0;
    float amp = 0.5;
    float freq = 1.0;
    
    for(int i = 0; i < OCTAVES; i++) {
        sum += amp * perlin2d(p * freq);
        freq *= LACUNARITY;
        amp *= GAIN;
    }
    
    return sum;
}


// How distorted the image you want to be.
// control slider min=0.1 max=10 step=0.01 scale=log label="Noise Scale"
uniform float noise_scale;

// How strong the noise effect is.
// control slider min=0 max=0.5 step=0.005 label="Noise Intensity"
uniform float noise_intensity;

// Chromatic aberration.
// control checkbox label="Chromatic Aberration"
uniform bool aberration;

// How strong the chromatic aberration effect is.
// control slider min=0 max=1 step=0.01 label="Aberration Delta"
uniform float aberration_delta;

// control checkbox label="Animate"
uniform bool animate;

// Animation speed.
// control slider min=0 max=3 step=0.01 label="Speed"
uniform float speed;

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = fragCoord/iResolution.xy;
    float time = animate ? iTime * speed : 0.0;
 
    vec2 noisePos = uv * noise_scale + vec2(time);
    float noise = fbm(noisePos) * noise_intensity;

    vec3 col;

    if (aberration) {
        col.r = texture(iChannel4, uv + vec2(noise * (1.0 + aberration_delta))).r;
        col.g = texture(iChannel4, uv + vec2(noise)).g;
        col.b = texture(iChannel4, uv + vec2(noise * (1.0 - aberration_delta))).b;
    } else {
        vec2 distortedUV = uv + vec2(noise);
        col = texture(iChannel4, distortedUV).rgb;
    }

    fragColor = vec4(col, 1.0);
}
