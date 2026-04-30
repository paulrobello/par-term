# Ideas: Custom Background Shaders

**Important** Remove completed items from this list to save context for future runs

These ideas focus on enhancing par-term's existing custom background shader system: GLSL/Shadertoy-compatible background shaders, texture channels, cubemaps, `iChannel4` terminal-content sampling, cursor/progress/key-press uniforms, metadata defaults, Settings UI controls, and the bundled shader gallery.

## Shader authoring and discovery

- **Shader preset browser**: Expand the Effects tab into a gallery with thumbnails, categories, favorites, and “safe for readability” labels for bundled and user-installed background shaders.
- **One-click Shadertoy import**: Provide an import flow that accepts pasted Shadertoy GLSL, maps common uniforms/channels to par-term equivalents, warns about unsupported constructs, and creates a metadata block scaffold.

## Texture and asset workflows

- **Texture pack installer**: Extend `install-shaders` with optional texture packs for noise, gradients, paper, metal, starfields, and cubemap environments tuned for terminal readability.
- **Per-shader asset bundle format**: Allow a shader directory/package containing `.glsl`, textures, cubemaps, screenshots, license info, and manifest metadata.
- **Generated noise channels**: Provide built-in procedural noise textures as selectable `iChannel0-3` sources so common shaders do not require external PNG files.
- **Background image blending modes**: Expand `custom_shader_use_background_as_channel0` with blend modes like overlay, screen, multiply, blur-behind, and luminance-mask.
- **Cubemap showcase shaders**: Add more low-distraction cubemap-based backgrounds, such as slow metallic reflections, neon room ambience, or atmospheric sky gradients.

## Developer and ecosystem features

- **Gallery metadata generation**: Generate `docs/SHADERS.md`, website gallery entries, thumbnails, and `shaders/manifest.json` from a single shader metadata source.
- **Community shader submission checklist**: Document readability, license, performance, metadata, and screenshot requirements for contributed background shaders.
- **Performance budget hints**: Surface approximate frame cost per shader and recommend default animation speeds or low-power behavior.
- **Uniform debug overlay**: Add a developer overlay that displays live values for `iResolution`, `iTime`, `iMouse`, `iProgress`, cursor uniforms, channel resolutions, and current resolved shader config.
