/*! par-term shader metadata
name: Command State Backdrop
author: par-term
description: Briefly tints the terminal after shell-integration command start, success, or failure using iCommand.
version: 1.0.0
defaults:
  animation_speed: 0.8
  channel0: null
  channel1: null
  channel2: null
  channel3: null
  cubemap: null
  cubemap_enabled: null
  use_background_as_channel0: null
  uniforms:
    iBackdropStrength: 0.5
    iFailureColor: '#ff4d5e'
    iFlashSeconds: 1.0
    iIdleColor: '#101827'
    iRunningColor: '#3aa7ff'
    iSuccessColor: '#42d392'
*/

// control color label="Idle"
uniform vec3 iIdleColor;
// control color label="Running"
uniform vec3 iRunningColor;
// control color label="Success"
uniform vec3 iSuccessColor;
// control color label="Failure"
uniform vec3 iFailureColor;
// control slider min=0.2 max=4 step=0.1 label="Flash Seconds"
uniform float iFlashSeconds;
// control slider min=0 max=1 step=0.01 label="Strength"
uniform float iBackdropStrength;

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec2 p = (fragCoord - 0.5 * iResolution.xy) / min(iResolution.x, iResolution.y);
    float status = iCommand.x;
    vec3 statusColor = iIdleColor;
    statusColor = mix(statusColor, iRunningColor, step(0.5, status) * (1.0 - step(1.5, status)));
    statusColor = mix(statusColor, iSuccessColor, step(1.5, status) * (1.0 - step(2.5, status)));
    statusColor = mix(statusColor, iFailureColor, step(2.5, status));

    float age = max(0.0, iTime - iCommand.z);
    float flash = exp(-age / max(0.05, iFlashSeconds));
    flash *= step(0.5, status);
    float running = iCommand.w;
    float breathe = 0.55 + 0.45 * sin(iTime * 3.0);
    float vignette = 1.0 - smoothstep(0.15, 1.15, length(p));
    float edge = 1.0 - smoothstep(0.0, 0.28, min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y)));

    vec3 color = iIdleColor * (0.25 + 0.15 * vignette);
    color += statusColor * (flash * (0.20 + edge * 0.45) + running * breathe * 0.18) * iBackdropStrength;
    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
