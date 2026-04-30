/*! par-term shader metadata
name: galaxy
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
    iDriftAmount: 0.19999999
    iNebulaOpacity: 0.48
    iNebulaTint: '#ffffff'
    iSaturation: 0.9
    iSpinSpeed: 0.07999999
    iStarBrightness: 0.0008
    iStarTint: '#ffffff'
    iTravelSpeed: 0.00019999992
    iZoom: 0.099999994
*/

// control slider min=0.0001 max=0.006 step=0.0001 scale=log label="Star Brightness"
uniform float iStarBrightness;
// control slider min=0 max=1 step=0.01 label="Nebula Opacity"
uniform float iNebulaOpacity;
// control slider min=0 max=1.5 step=0.01 label="Color Saturation"
uniform float iSaturation;
// control slider min=0.04 max=0.3 step=0.005 label="Zoom"
uniform float iZoom;
// control slider min=-0.001 max=0.001 step=0.00005 label="Travel Speed"
uniform float iTravelSpeed;
// control slider min=-0.25 max=0.25 step=0.005 label="Spin Speed"
uniform float iSpinSpeed;
// control slider min=0 max=0.8 step=0.01 label="Camera Drift"
uniform float iDriftAmount;
// control color label="Star Tint"
uniform vec3 iStarTint;
// control color label="Nebula Tint"
uniform vec3 iNebulaTint;

float field(in vec3 position) {
  float strength = 7.0 + 0.03 * log(1.0e-6 + fract(sin(iTime) * 373.11));
  float accumulated = 0.0;
  float previousMagnitude = 0.0;
  float totalWeight = 0.0;	

  for (int i = 0; i < 5; ++i) {
    float magnitude = dot(position, position);
    position = abs(position) / magnitude + vec3(-0.5, -0.8 + 0.1 * sin(-iTime * 0.1 + 2.0), -1.1 + 0.3 * cos(iTime * 0.3));
    float weight = exp(-float(i) / 6.0);
    accumulated += weight * exp(-strength * pow(abs(magnitude - previousMagnitude), 2.3));
    totalWeight += weight;
    previousMagnitude = magnitude;
  }

  return max(0.0, 5.0 * accumulated / totalWeight - 0.7);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  const float baseSpeed = 0.02;
  const int maxIterations = 14;
  const float formulaParameter = 0.79;
  const float volumeSteps = 6.0;
  const float stepSize = 0.24;
  const float tilingFactor = 0.85;
  const float darkMatter = 0.2;
  const float distanceFading = 0.56;

  vec2 normalizedCoordinates = 2.0 * fragCoord.xy / iResolution.xy - 1.0;
  vec2 scaledCoordinates = normalizedCoordinates;

  float timeElapsed = iTime;               
  float speedAdjustment = -baseSpeed;
  float formulaAdjustment = formulaParameter;

  speedAdjustment = iTravelSpeed * cos(iTime * 0.02 + 3.1415926 / 4.0);          

  vec2 uvCoordinates = scaledCoordinates;		       

  float rotationXZ = 0.9;
  float rotationYZ = -0.6;
  float rotationXY = 0.9 + iTime * iSpinSpeed;	

  mat2 rotationMatrixXZ = mat2(vec2(cos(rotationXZ), sin(rotationXZ)), vec2(-sin(rotationXZ), cos(rotationXZ)));	
  mat2 rotationMatrixYZ = mat2(vec2(cos(rotationYZ), sin(rotationYZ)), vec2(-sin(rotationYZ), cos(rotationYZ)));		
  mat2 rotationMatrixXY = mat2(vec2(cos(rotationXY), sin(rotationXY)), vec2(-sin(rotationXY), cos(rotationXY)));

  vec2 canvasCenter = vec2(0.5, 0.5);
  vec3 rayDirection = vec3(uvCoordinates * iZoom, 1.0); 
  vec3 cameraPosition = vec3(0.0, 0.0, 0.0);                               
  cameraPosition.x -= 2.0 * (canvasCenter.x - 0.5);
  cameraPosition.y -= 2.0 * (canvasCenter.y - 0.5);

  vec3 forwardVector = vec3(0.0, 0.0, 1.0);   
  cameraPosition.x += iDriftAmount * cos(0.01 * iTime) + 0.001 * iTime;
  cameraPosition.y += iDriftAmount * sin(0.01 * iTime) + 0.001 * iTime;
  cameraPosition.z += 0.003 * iTime;	

  rayDirection.xz *= rotationMatrixXZ;
  forwardVector.xz *= rotationMatrixXZ;	
  rayDirection.yz *= rotationMatrixYZ;
  forwardVector.yz *= rotationMatrixYZ;

  cameraPosition.xy *= -1.0 * rotationMatrixXY;
  cameraPosition.xz *= rotationMatrixXZ;
  cameraPosition.yz *= rotationMatrixYZ;

  float zoomOffset = (timeElapsed - 3311.0) * speedAdjustment;
  cameraPosition += forwardVector * zoomOffset;
  float sampleOffset = mod(zoomOffset, stepSize);
  float normalizedSampleOffset = sampleOffset / stepSize;

  float stepDistance = 0.24;
  float secondaryStepDistance = stepDistance + stepSize / 2.0;
  vec3 accumulatedColor = vec3(0.0);
  float fieldContribution = 0.0;	
  vec3 backgroundColor = vec3(0.0);

  for (float stepIndex = 0.0; stepIndex < volumeSteps; ++stepIndex) {
    vec3 primaryPosition = cameraPosition + (stepDistance + sampleOffset) * rayDirection;
    vec3 secondaryPosition = cameraPosition + (secondaryStepDistance + sampleOffset) * rayDirection;

    primaryPosition = abs(vec3(tilingFactor) - mod(primaryPosition, vec3(tilingFactor * 2.0)));
    secondaryPosition = abs(vec3(tilingFactor) - mod(secondaryPosition, vec3(tilingFactor * 2.0)));

    fieldContribution = field(secondaryPosition);

    float particleAccumulator = 0.0, particleDistance = 0.0;
    for (int i = 0; i < maxIterations; ++i) {
      primaryPosition = abs(primaryPosition) / dot(primaryPosition, primaryPosition) - formulaAdjustment;
      float distanceChange = abs(length(primaryPosition) - particleDistance);
      particleAccumulator += i > 2 ? min(12.0, distanceChange) : distanceChange;
      particleDistance = length(primaryPosition);
    }
    particleAccumulator *= particleAccumulator * particleAccumulator;

    float fadeFactor = pow(distanceFading, max(0.0, float(stepIndex) - normalizedSampleOffset));
    accumulatedColor += vec3(stepDistance, stepDistance * stepDistance, stepDistance * stepDistance * stepDistance * stepDistance) 
                        * particleAccumulator * iStarBrightness * fadeFactor;
    backgroundColor += mix(0.4, 1.0, iNebulaOpacity) * vec3(1.8 * fieldContribution * fieldContribution * fieldContribution, 
                                                          1.4 * fieldContribution * fieldContribution, fieldContribution) * fadeFactor;
    stepDistance += stepSize;
    secondaryStepDistance += stepSize;		
  }
  
  accumulatedColor = mix(vec3(length(accumulatedColor)), accumulatedColor, iSaturation);

  vec4 foregroundColor = vec4(accumulatedColor * iStarTint * 0.01, 1.0);	
  backgroundColor *= iNebulaOpacity;	
  backgroundColor.b *= 1.8;
  backgroundColor.r *= 0.05;

  backgroundColor.b = 0.5 * mix(backgroundColor.g, backgroundColor.b, 0.8);
  backgroundColor.g = 0.0;
  backgroundColor.bg = mix(backgroundColor.gb, backgroundColor.bg, 0.5 * (cos(iTime * 0.01) + 1.0));

  fragColor = vec4(foregroundColor.rgb + backgroundColor * iNebulaTint, 1.0);
}

