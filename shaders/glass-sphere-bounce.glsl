// Glass Sphere Bouncing Shader
// Refracts a background image with customizable size, speed, and IOR
// Background only mode - terminal content passes through

/*! par-term shader metadata
name: Glass Sphere Bounce
author: par-term
description: Glass sphere bouncing and refracting background image
version: 1.0.0
defaults:
  animation_speed: 0.5
  brightness: 1.0
  text_opacity: 1.0
  full_content: false
  channel0: ''
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: false
*/

// Customizable parameters
float uSphereSize = 0.12;   // Sphere radius as fraction of screen
float uBounceSpeed = 0.4;   // Speed of bouncing motion
float uIOR = 1.75;           // Index of refraction (1.0 = none, 1.5 = glass, 2.4 = diamond)
float uChromaticAberration = 0.0;  // Chromatic aberration strength (0.0 = none, 1.0 = glass, 2.0 = soap bubble)
float uThinFilm = 0.0;      // Thin film interference (0.0 = none, 1.0 = soap bubble effect)

// Calculate sphere position with bouncing motion (in 0-1 UV space)
vec2 getSpherePosition(float time, float aspect) {
    float speed = uBounceSpeed;

    // Horizontal oscillation (accounting for aspect ratio and sphere size)
    float xRange = max(0.1, 0.5 - uSphereSize / aspect);
    float x = 0.5 + sin(time * speed * 0.7) * xRange;

    // Vertical bouncing with gravity feel
    float bouncePeriod = 2.5 / speed;
    float t = mod(time * speed, bouncePeriod);

    // Parabolic bounce motion
    float gravity = 0.0;
    if (t < bouncePeriod * 0.5) {
        float phase = t / (bouncePeriod * 0.5);
        gravity = 1.0 - phase * phase;
    } else {
        float phase = (t - bouncePeriod * 0.5) / (bouncePeriod * 0.5);
        gravity = phase * phase;
    }
    float y = 0.15 + gravity * 0.7;

    return vec2(x, y);
}

// Thin film interference - creates rainbow colors like soap bubbles
vec3 thinFilmInterference(float cosTheta, float thickness) {
    // Simulate thin film interference
    // Different wavelengths interfere constructively/destructively based on path length
    float d = thickness * 2.0;

    // Wavelengths for R, G, B (normalized)
    vec3 wavelengths = vec3(0.65, 0.55, 0.45);  // Red, Green, Blue

    // Phase shift for each wavelength
    vec3 phase = vec3(
        cos(d / wavelengths.r * 6.28318 + cosTheta * 3.14159),
        cos(d / wavelengths.g * 6.28318 + cosTheta * 3.14159),
        cos(d / wavelengths.b * 6.28318 + cosTheta * 3.14159)
    );

    // Normalize to 0-1 range and enhance colors
    return (phase * 0.5 + 0.5);
}

// HSV to RGB for rainbow effects
vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

// Apply glass sphere refraction at given position
vec3 applyGlassSphere(vec2 uv, vec2 spherePos, float radius, float aspect, bool hasTexture) {
    vec2 toSphere = uv - spherePos;
    // Correct for aspect ratio so sphere appears circular
    vec2 toSphereAspect = vec2(toSphere.x * aspect, toSphere.y);
    float dist = length(toSphereAspect);

    if (dist < radius) {
        // Inside sphere - calculate 3D sphere normal (use aspect-corrected coords)
        float z = sqrt(radius * radius - dist * dist);
        vec3 normal = normalize(vec3(toSphereAspect.x, toSphereAspect.y, z));

        // View direction (looking down Z)
        vec3 viewDir = vec3(0.0, 0.0, 1.0);

        // Cosine of angle between view and normal
        float cosTheta = abs(dot(viewDir, normal));

        // Depth through sphere (thicker at center)
        float depth = z / radius;

        // Edge factor (1 at edge, 0 at center)
        float edgeFactor = 1.0 - depth;

        // Refraction strength based on IOR
        float refractBase = (uIOR - 1.0) * 0.25;
        float refractStrength = edgeFactor * refractBase;
        vec2 refractUV = uv + toSphere * refractStrength;

        vec3 refracted;
        if (hasTexture) {
            // Base chromatic aberration from IOR
            float baseAberration = edgeFactor * (uIOR - 1.0) * 0.02;

            // Enhanced chromatic aberration from user setting
            float enhancedAberration = baseAberration * (1.0 + uChromaticAberration * 2.0);

            // Sample with different offsets for R, G, B channels
            vec3 r = texture(iChannel0, refractUV + vec2(enhancedAberration, enhancedAberration * 0.5)).rgb;
            vec3 g = texture(iChannel0, refractUV).rgb;
            vec3 b = texture(iChannel0, refractUV - vec2(enhancedAberration * 0.7, enhancedAberration)).rgb;
            refracted = vec3(r.r, g.g, b.b);
        } else {
            // Fallback: refract the gradient
            vec3 color1 = vec3(0.08, 0.1, 0.18);
            vec3 color2 = vec3(0.15, 0.08, 0.2);
            float gradient = refractUV.y + sin(refractUV.x * 3.0 + iTime * 0.2) * 0.1;
            refracted = mix(color1, color2, gradient);
        }

        // Apply thin film interference if enabled (soap bubble effect)
        if (uThinFilm > 0.0) {
            // Film thickness varies across sphere (thinner at edges)
            float filmThickness = depth * 2.0 + iTime * 0.5;

            // Get interference colors
            vec3 iridescence = thinFilmInterference(cosTheta, filmThickness);

            // Alternative: rainbow based on position and time
            float hue = fract(edgeFactor * 0.5 + atan(toSphere.y, toSphere.x) / 6.28318 + iTime * 0.1);
            vec3 rainbow = hsv2rgb(vec3(hue, 0.8, 1.0));

            // Blend interference and rainbow
            vec3 filmColor = mix(iridescence, rainbow, 0.5);

            // Apply to refracted color
            refracted = mix(refracted, refracted * filmColor + filmColor * 0.3, uThinFilm * edgeFactor);
        }

        // Slight glass tint (reduced when thin film is active)
        vec3 glassTint = mix(vec3(0.92, 0.96, 1.0), vec3(1.0), uThinFilm * 0.5);
        refracted *= glassTint;

        // Fresnel - more reflective at edges
        float fresnelBase = (uIOR - 1.0) * 0.3 + 0.2;
        float fresnel = pow(edgeFactor, 3.0);

        // Fresnel color can have iridescence too
        vec3 fresnelColor = mix(vec3(0.8, 0.9, 1.0), vec3(1.0), uThinFilm * edgeFactor);
        refracted = mix(refracted, fresnelColor, fresnel * fresnelBase);

        // Specular highlight (top-left light source)
        vec3 lightDir = normalize(vec3(-0.5, 0.5, 0.8));
        float spec = pow(max(0.0, dot(normal, lightDir)), 64.0);
        refracted += spec * 0.6;

        // Edge rim glow
        float rim = pow(edgeFactor, 4.0);

        // Rim can also have rainbow effect
        vec3 rimColor = vec3(0.6, 0.8, 1.0);
        if (uThinFilm > 0.0) {
            float rimHue = fract(edgeFactor + iTime * 0.2);
            rimColor = mix(rimColor, hsv2rgb(vec3(rimHue, 0.7, 1.0)), uThinFilm);
        }
        refracted += rimColor * rim * 0.3;

        return refracted;
    }

    // Outside sphere - return background or fallback
    if (hasTexture) {
        return texture(iChannel0, uv).rgb;
    }

    // Fallback gradient
    vec3 color1 = vec3(0.08, 0.1, 0.18);
    vec3 color2 = vec3(0.15, 0.08, 0.2);
    float gradient = uv.y + sin(uv.x * 3.0 + iTime * 0.2) * 0.1;
    return mix(color1, color2, gradient);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    // Normalized UV coordinates (0-1)
    vec2 uv = fragCoord / iResolution.xy;

    // Aspect ratio for proper sphere shape
    float aspect = iResolution.x / iResolution.y;

    // Check if iChannel0 has a real background image (unset channels are 1x1 placeholders)
    bool hasTexture = iChannelResolution[0].x > 1.0 && iChannelResolution[0].y > 1.0;

    // Get sphere position in UV space (0-1)
    vec2 spherePos = getSpherePosition(iTime, aspect);

    // Radius in UV space (same visual size regardless of aspect)
    float radius = uSphereSize;

    // Apply glass sphere refraction
    vec3 bgColor = applyGlassSphere(uv, spherePos, radius, aspect, hasTexture);

    // Get terminal content
    vec4 tex = texture(iChannel4, uv);

    // Background only mode - composite terminal on top
    float textAlpha = tex.a;
    vec3 finalColor = mix(bgColor, tex.rgb, textAlpha);

    fragColor = vec4(finalColor, 1.0);
}
