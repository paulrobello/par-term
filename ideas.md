# Ideas: Custom Background Shaders

**Important** Remove completed items from this list to save context for future runs

These ideas focus on enhancing par-term's existing custom background shader system: GLSL/Shadertoy-compatible background shaders, texture channels, cubemaps, `iChannel4` terminal-content sampling, cursor/progress/key-press uniforms, metadata defaults, Settings UI controls, and the bundled shader gallery.

## Shader authoring and discovery

- **Shader preset browser**: Expand the Effects tab into a gallery with thumbnails, categories, favorites, and “safe for readability” labels for bundled and user-installed background shaders.

## Developer and ecosystem features

- **Gallery metadata generation**: Generate `docs/SHADERS.md`, website gallery entries, thumbnails, and `shaders/manifest.json` from a single shader metadata source.
- **Community shader submission checklist**: Document readability, license, performance, metadata, and screenshot requirements for contributed background shaders.
- **Performance budget hints**: Surface approximate frame cost per shader and recommend default animation speeds or low-power behavior.
- **Uniform debug overlay**: Add a developer overlay that displays live values for `iResolution`, `iTime`, `iMouse`, `iProgress`, cursor uniforms, channel resolutions, and current resolved shader config.
