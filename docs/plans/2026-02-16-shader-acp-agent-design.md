# Shader ACP Agent Integration Design

**Date:** 2026-02-16
**Issue:** #156
**Status:** Approved

## Summary

Add shader-aware context injection and config file watching to the AI inspector's ACP agent integration, enabling agents to create, edit, debug, and manage custom shaders (both background and cursor) in par-term.

## Approach

**Enhanced System Prompt + Config File Reload** - Leverage existing ACP agent file I/O capabilities rather than adding new RPC methods or creating a separate MCP server. The agent already has `fs/readTextFile` and file write permissions; we provide it with the context it needs to use those capabilities effectively for shader work.

## Architecture

Three components:

### 1. Shader Context Generator (`src/ai_inspector/shader_context.rs`)

Generates a dynamic context block containing:

- **Current shader state**: Which background/cursor shaders are active, their parameters (animation speed, brightness, text opacity, etc.)
- **Available shaders**: Scanned from `~/.config/par-term/shaders/` and bundled `shaders/` directory
- **Debug file paths**: Transpiled WGSL at `/tmp/par_term_<name>_shader.wgsl`, wrapped GLSL at `/tmp/par_term_debug_wrapped.glsl`
- **Uniforms reference**: Available Shadertoy-compatible uniforms (`iTime`, `iResolution`, `iMouse`, `iChannel0-4`) plus par-term extensions (cursor uniforms)
- **Shader template**: Minimal working GLSL shader for quick starts
- **Config file location**: Path to `config.yaml` and relevant field names

**Key functions:**
- `build_shader_context(config: &Config) -> String` - Generates full context block
- `should_inject_shader_context(message: &str, config: &Config) -> bool` - Determines if shader context should be included based on keywords or active shader state

**Trigger keywords:** `shader`, `glsl`, `wgsl`, `effect`, `crt`, `cursor effect`, `scanline`, `post-process`, `fragment`, `mainImage`, `iChannel`, `iTime`, `shadertoy`, `transpile`, `naga`

### 2. Context-Triggered Injection

Modifies the `SendPrompt` handler in `window_state.rs` to conditionally prepend shader context:

- On first prompt: system guidance + shader context (if applicable)
- On subsequent prompts: shader context only when triggered by keywords or when a shader compilation error is detected
- When shaders are active: always include minimal state info (which shader, enabled/disabled)

### 3. Config File Watcher (`src/config/watcher.rs`)

A `notify` crate file watcher on the config directory:

1. Watches `config.yaml` for modifications
2. On change: reads and parses new config
3. Diffs against current config for shader-related fields
4. Triggers shader reload when shader config changes
5. Applies other config changes as appropriate

Integrates into the main event loop via a channel that the watcher sends events through.

## Agent Workflow

### Shader Creation
1. User asks: "Create a CRT scanline effect shader"
2. Keyword "shader" triggers context injection
3. Agent receives shader context with template, uniforms reference, available shaders
4. Agent writes GLSL to `~/.config/par-term/shaders/crt_scanlines.glsl`
5. Agent updates `config.yaml` with `custom_shader: "crt_scanlines.glsl"` and `custom_shader_enabled: true`
6. Config watcher detects change, reloads config, recompiles shader
7. User sees effect immediately

### Shader Debugging
1. User reports: "My shader shows a black screen"
2. Agent receives context with debug file paths
3. Agent reads `/tmp/par_term_<name>_shader.wgsl` to see transpiled output
4. Agent reads `/tmp/par_term_debug_wrapped.glsl` to see wrapped input
5. Agent diagnoses issue (e.g., coordinate system, missing uniform)
6. Agent writes fixed shader file
7. Config watcher triggers reload

### Shader Management
1. User asks: "List my shaders" or "Switch to galaxy shader"
2. Agent receives available shader list in context
3. Agent updates `config.yaml` to switch shaders
4. Config watcher applies change

## Files Modified/Created

| File | Action | Description |
|------|--------|-------------|
| `src/ai_inspector/shader_context.rs` | Create | Shader context generator |
| `src/ai_inspector/mod.rs` | Modify | Add shader_context module |
| `src/config/watcher.rs` | Create | Config file watcher |
| `src/config/mod.rs` | Modify | Add watcher module, config diff |
| `src/app/window_state.rs` | Modify | Inject shader context, integrate watcher |
| `src/ai_inspector/chat.rs` | Modify | Extend guidance constant |

## Testing

- Unit tests for `build_shader_context()` output format
- Unit tests for `should_inject_shader_context()` keyword matching
- Unit tests for config diffing logic
- Integration test for config watcher (file write -> event received)

## Dependencies

- `notify` crate (file watching) - may already be indirect; add explicitly if needed
