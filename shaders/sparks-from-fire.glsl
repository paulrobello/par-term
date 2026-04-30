/*! par-term shader metadata
name: sparks-from-fire
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.4
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: textures/cubemaps/env-outside
  cubemap_enabled: false
  use_background_as_channel0: null
  uniforms:
    iBloomBrightness: 0.8
    iBloomColor: '#ff660d'
    iEmberGlow: 0.02
    iFireSpeed: 1.0
    iFlameScale: 2.5
    iLayerCount: 8
    iLayerScale: 1.05
    iMovementDirection:
    - 0.7
    - 1.0
    iMovementSpeed: 0.32999998
    iParticleFade: 0.9
    iParticleSize: 0.0025
    iSmokeBrightness: 0.8
    iSmokeColor: '#ff6e1a'
    iSmokeIntensity: 0.90000004
    iSmokeOpacity: 0.5
    iSparkBrightness: 1.5
    iSparkColor: '#ff660d'
    iVignette: 1.0
*/

// adapted by Alex Sherwin for Ghstty from https://www.shadertoy.com/view/wl2Gzc

//Shader License: CC BY 3.0
//Author: Jan Mróz (jaszunio15)

#define MAX_LAYERS_COUNT 12

// control slider min=0 max=2 step=0.05 label="Smoke Intensity"
uniform float iSmokeIntensity;
// control slider min=0.2 max=0.98 step=0.01 label="Particle Fade"
uniform float iParticleFade;
// control slider min=0 max=1 step=0.01 label="Smoke Opacity"
uniform float iSmokeOpacity;
// control int min=1 max=12 step=1 label="Particle Layers"
uniform int iLayerCount;
// control slider min=0.05 max=4 step=0.05 scale=log label="Fire Speed"
uniform float iFireSpeed;
// control slider min=0 max=1 step=0.01 label="Movement Speed"
uniform float iMovementSpeed;
// control vec2 min=-2 max=2 step=0.05 label="Movement Direction"
uniform vec2 iMovementDirection;
// control slider min=0.0005 max=0.01 step=0.0001 scale=log label="Particle Size"
uniform float iParticleSize;
// control slider min=0.8 max=1.5 step=0.01 label="Layer Scale"
uniform float iLayerScale;
// control slider min=0.75 max=5 step=0.05 scale=log label="Flame Scale"
uniform float iFlameScale;
// control slider min=0 max=2 step=0.05 label="Vignette"
uniform float iVignette;
// control slider min=0 max=0.2 step=0.005 label="Ember Glow"
uniform float iEmberGlow;
// control slider min=0 max=4 step=0.05 label="Spark Brightness"
uniform float iSparkBrightness;
// control slider min=0 max=3 step=0.05 label="Bloom Brightness"
uniform float iBloomBrightness;
// control slider min=0 max=3 step=0.05 label="Smoke Brightness"
uniform float iSmokeBrightness;
// control color label="Spark Color"
uniform vec3 iSparkColor;
// control color label="Bloom Color"
uniform vec3 iBloomColor;
// control color label="Smoke Color"
uniform vec3 iSmokeColor;

vec2 movementVec(void) {
    return iMovementDirection * iMovementSpeed;
}

float activeLayerMask(int index, int layers) {
    return step(float(index) + 0.5, float(layers));
}

// Precomputed sin/cos for rotation angle 0.7
#define SIN_07 0.644218
#define COS_07 0.764842

#define PARTICLE_SCALE (vec2(0.5, 1.6))
#define PARTICLE_SCALE_VAR (vec2(0.25, 0.2))

#define PARTICLE_BLOOM_SCALE (vec2(0.5, 0.8))
#define PARTICLE_BLOOM_SCALE_VAR (vec2(0.3, 0.1))


float hash1_2(in vec2 x)
{
 	return fract(sin(dot(x, vec2(52.127, 61.2871))) * 521.582);   
}

vec2 hash2_2(in vec2 x)
{
    return fract(sin(x * mat2x2(20.52, 24.1994, 70.291, 80.171)) * 492.194);
}

//Simple interpolated noise
vec2 noise2_2(vec2 uv)
{
    //vec2 f = fract(uv);
    vec2 f = smoothstep(0.0, 1.0, fract(uv));
    
 	vec2 uv00 = floor(uv);
    vec2 uv01 = uv00 + vec2(0,1);
    vec2 uv10 = uv00 + vec2(1,0);
    vec2 uv11 = uv00 + 1.0;
    vec2 v00 = hash2_2(uv00);
    vec2 v01 = hash2_2(uv01);
    vec2 v10 = hash2_2(uv10);
    vec2 v11 = hash2_2(uv11);
    
    vec2 v0 = mix(v00, v01, f.y);
    vec2 v1 = mix(v10, v11, f.y);
    vec2 v = mix(v0, v1, f.x);
    
    return v;
}

//Simple interpolated noise
float noise1_2(in vec2 uv)
{
    // vec2 f = fract(uv);
    vec2 f = smoothstep(0.0, 1.0, fract(uv));
    
 	vec2 uv00 = floor(uv);
    vec2 uv01 = uv00 + vec2(0,1);
    vec2 uv10 = uv00 + vec2(1,0);
    vec2 uv11 = uv00 + 1.0;
    
    float v00 = hash1_2(uv00);
    float v01 = hash1_2(uv01);
    float v10 = hash1_2(uv10);
    float v11 = hash1_2(uv11);
    
    float v0 = mix(v00, v01, f.y);
    float v1 = mix(v10, v11, f.y);
    float v = mix(v0, v1, f.x);
    
    return v;
}


float layeredNoise1_2(in vec2 uv, in float sizeMod, in float alphaMod, in int layers, in float animation)
{
 	  float noise = 0.0;
    float alpha = 1.0;
    float size = 1.0;
    vec2 offset = vec2(0.0);
    for (int i = 0; i < MAX_LAYERS_COUNT; i++)
    {
        float mask = activeLayerMask(i, layers);
        offset += hash2_2(vec2(alpha, size)) * 10.0 * mask;
        
        //Adding noise with movement
     	  noise += noise1_2(uv * size + iTime * animation * 8.0 * movementVec() + offset) * alpha * mask;
        alpha *= mix(1.0, alphaMod, mask);
        size *= mix(1.0, sizeMod, mask);
    }
    
    float safeAlphaMod = clamp(alphaMod, 0.001, 0.999);
    noise *= (1.0 - safeAlphaMod)/(1.0 - pow(safeAlphaMod, float(max(layers, 1))));
    return noise;
}

//Cell center from point on the grid
vec2 voronoiPointFromRoot(in vec2 root, in float deg)
{
  	vec2 point = hash2_2(root) - 0.5;
    float s = sin(deg);
    float c = cos(deg);
    point = mat2x2(s, c, -c, s) * point * 0.66;
    point += root + 0.5;
    return point;
}

//Voronoi cell point rotation degrees
float degFromRootUV(in vec2 uv)
{
 	return iTime * iFireSpeed * (hash1_2(uv) - 0.5) * 2.0;   
}

vec2 randomAround2_2(in vec2 point, in vec2 range, in vec2 uv)
{
 	return point + (hash2_2(uv) - 0.5) * range;
}


vec3 fireParticles(in vec2 uv, in vec2 originalUV)
{
    vec3 particles = vec3(0.0);
    vec2 rootUV = floor(uv);
    float deg = degFromRootUV(rootUV);
    vec2 pointUV = voronoiPointFromRoot(rootUV, deg);
    float dist = 2.0;
    float distBloom = 0.0;
   
   	//UV manipulation for the faster particle movement
    vec2 tempUV = uv + (noise2_2(uv * 2.0) - 0.5) * 0.1;
    tempUV += -(noise2_2(uv * 3.0 + iTime * iFireSpeed) - 0.5) * 0.07;

    //Rotated offset (precomputed sin/cos for 0.7)
    vec2 rotated = mat2(SIN_07, COS_07, -COS_07, SIN_07) * (tempUV - pointUV);

    //Sparks sdf
    dist = length(rotated * randomAround2_2(PARTICLE_SCALE, PARTICLE_SCALE_VAR, rootUV));

    //Bloom sdf
    distBloom = length(rotated * randomAround2_2(PARTICLE_BLOOM_SCALE, PARTICLE_BLOOM_SCALE_VAR, rootUV));

    //Add sparks
    particles += (1.0 - smoothstep(iParticleSize * 0.6, iParticleSize * 3.0, dist)) * iSparkColor * iSparkBrightness;
    
    //Add bloom
    float bloom = 1.0 - smoothstep(0.0, iParticleSize * 6.0, distBloom);
    particles += bloom * bloom * bloom * iBloomColor * iBloomBrightness;

    //Upper disappear curve randomization
    float border = (hash1_2(rootUV) - 0.5) * 2.0;
 	float disappear = 1.0 - smoothstep(border, border + 0.5, originalUV.y);
	
    //Lower appear curve randomization
    border = (hash1_2(rootUV + 0.214) - 1.8) * 0.7;
    float appear = smoothstep(border, border + 0.4, originalUV.y);
    
    return particles * disappear * appear;
}


//Layering particles to imitate 3D view
vec3 layeredParticles(in vec2 uv, in float sizeMod, in float alphaMod, in int layers, in float smoke) 
{ 
    vec3 particles = vec3(0);
    float size = 1.0;
    // float alpha = 1.0;
    float alpha = 1.0;
    vec2 offset = vec2(0.0);
    vec2 noiseOffset;
    vec2 bokehUV;
    
    for (int i = 0; i < MAX_LAYERS_COUNT; i++)
    {
        float mask = activeLayerMask(i, layers);
        //Particle noise movement
        noiseOffset = (noise2_2(uv * size * 2.0 + 0.5) - 0.5) * 0.15;
        
        //UV with applied movement
        bokehUV = (uv * size + iTime * movementVec()) + offset + noiseOffset; 
        
        //Adding particles								if there is more smoke, remove smaller particles
		    particles += fireParticles(bokehUV, uv) * alpha * (1.0 - smoothstep(0.0, 1.0, smoke) * (float(i) / float(max(layers, 1)))) * mask;
        
        //Moving uv origin to avoid generating the same particles
        offset += hash2_2(vec2(alpha, alpha)) * 10.0 * mask;
        
        alpha *= mix(1.0, alphaMod, mask);
        size *= mix(1.0, sizeMod, mask);
    }
    
    return particles;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (2.0 * fragCoord - iResolution.xy) / iResolution.x;
    
    // float vignette = 1.1 - smoothstep(0.4, 1.4, length(uv + vec2(0.0, 0.3)));
    float vignette = mix(1.0, 1.3 - smoothstep(0.4, 1.4, length(uv + vec2(0.0, 0.3))), iVignette);
    
    uv *= iFlameScale;
    
    int layers = clamp(iLayerCount, 1, MAX_LAYERS_COUNT);
    vec2 movement = movementVec();
    float smokeIntensity = layeredNoise1_2(uv * 10.0 + iTime * iFireSpeed * 4.0 * movement, 1.7, 0.7, 6, 0.2);
    float smokeY = smoothstep(-1.0, 1.6, uv.y);
    smokeIntensity *= smokeY * smokeY;
    vec3 smoke = smokeIntensity * iSmokeColor * iSmokeBrightness * vignette * iSmokeIntensity * iSmokeOpacity;

    //Cutting holes in smoke
    float holes = layeredNoise1_2(uv * 4.0 + iTime * iFireSpeed * 0.5 * movement, 1.8, 0.5, 3, 0.2);
    smoke *= holes * holes * 1.5;
    
    vec3 particles = layeredParticles(uv, iLayerScale, iParticleFade, layers, smokeIntensity);
    
    vec3 col = particles + smoke + iSmokeColor * iSmokeBrightness * iEmberGlow;
	  col *= vignette;
    
    fragColor = vec4(smoothstep(-0.08, 1.0, col), 1.0);
}
