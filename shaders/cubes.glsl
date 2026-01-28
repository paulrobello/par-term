/*! par-term shader metadata
name: cubes
author: null
description: null
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 0.3
  text_opacity: null
  full_content: null
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: ''
  cubemap_enabled: false
*/

// credits: https://github.com/rymdlego
// Optimized version

const float speed = 0.2;
const float cube_size = 1.0;
const float cube_rotation_speed = 2.8;
const float camera_rotation_speed = 0.1;

mat3 rotationMatrix(vec3 m, float a) {
    m = normalize(m);
    float c = cos(a), s = sin(a);
    float oc = 1.0 - c;
    return mat3(
        c + oc * m.x * m.x,     oc * m.x * m.y - s * m.z, oc * m.x * m.z + s * m.y,
        oc * m.x * m.y + s * m.z, c + oc * m.y * m.y,     oc * m.y * m.z - s * m.x,
        oc * m.x * m.z - s * m.y, oc * m.y * m.z + s * m.x, c + oc * m.z * m.z
    );
}

// Precomputed rotation matrix passed in to avoid recomputing per iteration
float box(vec3 pos, vec3 size, mat3 rot) {
    pos = pos * 0.9 * rot;
    return length(max(abs(pos) - size, 0.0));
}

// Precomputed values passed in
float distfunc(vec3 pos, float size, mat3 rot) {
    vec3 q = mod(pos, 5.0) - 2.5;
    return box(q, vec3(size), rot);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    float t = iTime;
    float ts = t * speed;

    // Precompute time-based values once
    float size = 0.45 + 0.25 * abs(16.0 * sin(ts / 4.0));
    size = cube_size * 0.16 * clamp(size, 2.0, 4.0);

    vec3 rotAxis = vec3(sin(ts / 4.0) * 10.0, cos(ts / 4.0) * 12.0, 2.7);
    float rotAngle = ts * 2.4 / 4.0 * cube_rotation_speed;
    mat3 cubeRot = rotationMatrix(rotAxis, rotAngle);

    // Camera setup
    vec2 screenPos = -1.0 + 2.0 * fragCoord.xy / iResolution.xy;
    screenPos.x *= iResolution.x / iResolution.y;

    vec3 cameraOrigin = vec3(ts, 0.0, 0.0);
    vec3 cameraTarget = vec3(t * 20.0, 0.0, 0.0) * rotationMatrix(vec3(0.0, 0.0, 1.0), ts * camera_rotation_speed);

    vec3 upDirection = vec3(0.5, 1.0, 0.6);
    vec3 cameraDir = normalize(cameraTarget - cameraOrigin);
    vec3 cameraRight = normalize(cross(upDirection, cameraOrigin));
    vec3 cameraUp = cross(cameraDir, cameraRight);
    vec3 rayDir = normalize(cameraRight * screenPos.x + cameraUp * screenPos.y + cameraDir);

    const int MAX_ITER = 48;  // Reduced from 64
    const float MAX_DIST = 48.0;
    const float EPSILON = 0.001;

    float totalDist = 0.0;
    vec3 pos = cameraOrigin;
    float dist = EPSILON;

    for (int i = 0; i < MAX_ITER; i++) {
        if (dist < EPSILON || totalDist > MAX_DIST) break;
        dist = distfunc(pos, size, cubeRot);
        totalDist += dist;
        pos += dist * rayDir;
    }

    vec4 cubes;
    if (dist < EPSILON) {
        // Normal calculation
        vec2 eps = vec2(0.0, EPSILON);
        vec3 normal = normalize(vec3(
            distfunc(pos + eps.yxx, size, cubeRot) - distfunc(pos - eps.yxx, size, cubeRot),
            distfunc(pos + eps.xyx, size, cubeRot) - distfunc(pos - eps.xyx, size, cubeRot),
            distfunc(pos + eps.xxy, size, cubeRot) - distfunc(pos - eps.xxy, size, cubeRot)
        ));

        float diffuse = max(0.0, dot(-rayDir, normal));
        float specular = pow(diffuse, 32.0);
        vec3 color = vec3(diffuse + specular);
        float fade = 1.0 - (totalDist / MAX_DIST);
        cubes = vec4(color * fade * iBrightness, 0.1);
    } else {
        cubes = vec4(0.0);
    }

    fragColor = cubes;
}
