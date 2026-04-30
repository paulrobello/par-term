/*! par-term shader metadata
name: just-snow
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.2
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: ''
  cubemap_enabled: false
  use_background_as_channel0: null
  uniforms:
    iDepthStep: 0.5
    iDitherAmount: 1.0
    iFallSpeed: 0.59999996
    iFlakeSize: 1.0
    iSnowDensity: 1.0
    iSnowTint: '#ffffff'
    iWindWidth: 0.84999996
*/

// Copyright (c) 2013 Andrew Baldwin (twitter: baldand, www: http://thndl.com)
// License = Attribution-NonCommercial-ShareAlike (http://creativecommons.org/licenses/by-nc-sa/3.0/deed.en_US)

// "Just snow"
// Simple (but not cheap) snow made from multiple parallax layers with randomly positioned 
// flakes and directions. Also includes a DoF effect. Pan around with mouse.

#define LIGHT_SNOW // Comment this out for a blizzard

#ifdef LIGHT_SNOW
	#define LAYERS 50
#else // BLIZZARD
	#define LAYERS 200
#endif

// control slider min=0.05 max=1 step=0.01 label="Density"
uniform float iSnowDensity;
// control slider min=0 max=2 step=0.01 label="Fall Speed"
uniform float iFallSpeed;
// control slider min=0 max=1.5 step=0.01 label="Wind Width"
uniform float iWindWidth;
// control slider min=0.05 max=1.25 step=0.01 label="Depth Spacing"
uniform float iDepthStep;
// control slider min=0.4 max=2.5 step=0.01 label="Flake Size"
uniform float iFlakeSize;
// control color label="Snow Tint"
uniform vec3 iSnowTint;
// control slider min=0 max=2 step=0.01 label="Dither"
uniform float iDitherAmount;

// Simple hash for dithering
float hash12(vec2 p) {
	vec3 p3 = fract(vec3(p.xyx) * 0.1031);
	p3 += dot(p3, p3.yzx + 33.33);
	return fract((p3.x + p3.y) * p3.z);
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
	const mat3 p = mat3(13.323122,23.5112,21.71123,21.1212,28.7312,11.9312,21.8112,14.7212,61.3934);
	vec2 uv = fragCoord.xy / iResolution.xy;
	uv.y = 1.0 - uv.y; // flip Y to correct orientation
	float aspect = iResolution.x / max(iResolution.y, 1.0);
	vec2 snowUv = vec2((uv.x - 0.5) * aspect + 0.5, uv.y);

	float density = clamp(iSnowDensity, 0.05, 1.0);
	float fallSpeed = max(iFallSpeed, 0.0);
	float windWidth = max(iWindWidth, 0.0);
	float depthStep = max(iDepthStep, 0.05);
	float flakeSize = max(iFlakeSize, 0.4);

	float acc = 0.0;
	float dof = 5.0 * sin(iTime * 0.1);
	for (int i = 0; i < LAYERS; i++) {
		float fi = float(i);
		float fiDepth = fi * depthStep;
		float depthScale = 1.0 + fiDepth;
		vec2 q = -snowUv * depthScale;
		q += vec2(q.y * (windWidth * mod(fi * 7.238917, 1.0) - windWidth * 0.5), -fallSpeed * iTime / (1.0 + fiDepth * 0.03));
		vec3 n = vec3(floor(q), 31.189 + fi);
		vec3 m = floor(n) * 0.00001 + fract(n);
		vec3 mp = (31415.9 + m) / fract(p * m);
		vec3 r = fract(mp);
		vec2 s = abs(mod(q, 1.0) - 0.5 + 0.9 * r.xy - 0.45);
		s += 0.01 * abs(2.0 * fract(10.0 * q.yx) - 1.0);
		float d = (0.6 * max(s.x - s.y, s.x + s.y) + max(s.x, s.y)) / flakeSize - 0.01;
		float edge = 0.005 + 0.05 * min(0.5 * abs(fi - 5.0 - dof), 1.0);
		acc += step(1.0 - density, r.z) * smoothstep(edge, -edge, d) * (r.x / (1.0 + 0.02 * fiDepth));
	}

	// Add dithering to reduce banding
	float dither = iDitherAmount * (hash12(fragCoord + fract(iTime)) - 0.5) / 255.0;
	acc += dither;

	fragColor = vec4(max(iSnowTint, vec3(0.0)) * acc, 1.0);
}
