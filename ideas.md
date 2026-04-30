# Ideas: Custom Background Shaders

**Important** Remove completed items from this list to save context for future runs

These ideas focus on enhancing par-term's existing custom background shader system: GLSL/Shadertoy-compatible background shaders, texture channels, cubemaps, `iChannel4` terminal-content sampling, cursor/progress/key-press uniforms, metadata defaults, Settings UI controls, and the bundled shader gallery.

## Shader authoring and discovery

- **Shader preset browser**: Expand the Effects tab into a gallery with thumbnails, categories, favorites, and “safe for readability” labels for bundled and user-installed background shaders.
- **One-click Shadertoy import**: Provide an import flow that accepts pasted Shadertoy GLSL, maps common uniforms/channels to par-term equivalents, warns about unsupported constructs, and creates a metadata block scaffold.

## More terminal-aware background effects

- **Progress-reactive themes**: Add bundled background shaders that react to `iProgress`: calm ambient glow during normal progress, amber pulse for warnings, red edge bloom for errors, and indeterminate animated stripes.
- **Command-state backdrops**: Introduce hooks/uniforms for last command status so background shaders can briefly tint or animate after command success/failure.
- **Pane-aware shader regions**: Expose split-pane bounds to shaders so backgrounds can subtly differentiate active/inactive panes without requiring the renderer to draw separate effects manually.
- **Scrollback depth parallax**: Feed scroll offset or viewport position into custom shaders so long scrollback can create subtle depth, fog, or timeline effects.

## Texture and asset workflows

- **Texture pack installer**: Extend `install-shaders` with optional texture packs for noise, gradients, paper, metal, starfields, and cubemap environments tuned for terminal readability.
- **Per-shader asset bundle format**: Allow a shader directory/package containing `.glsl`, textures, cubemaps, screenshots, license info, and manifest metadata.
- **Generated noise channels**: Provide built-in procedural noise textures as selectable `iChannel0-3` sources so common shaders do not require external PNG files.
- **Background image blending modes**: Expand `custom_shader_use_background_as_channel0` with blend modes like overlay, screen, multiply, blur-behind, and luminance-mask.
- **Cubemap showcase shaders**: Add more low-distraction cubemap-based backgrounds, such as slow metallic reflections, neon room ambience, or atmospheric sky gradients.

## Settings UI improvements

- **Uniform control groups**: Let shader metadata group controls into sections like Palette, Motion, Distortion, Readability, and Performance in the Settings UI.
- **Per-profile shader overrides**: Allow each terminal profile/theme to select a different background shader, brightness, text opacity, texture set, and animation speed.
- **Temporary shader toggle palette**: Add a quick command or keybinding palette to cycle background shaders, pause animation, or switch to a low-power/readability mode.
- **Adaptive brightness slider**: Add an “auto-dim under text” option that samples text density and reduces shader intensity only where terminal content exists.
- **Shader safety badges**: Show badges for “full-content”, “distorts text”, “uses textures”, “uses cubemap”, “high GPU cost”, and “works well on battery”.

## New bundled background shader concepts

- **Aurora terminal**: Soft northern-light ribbons with color controls, slow motion, and strong readability defaults.
- **Blueprint grid**: Subtle animated CAD/grid background that brightens around the cursor and active progress bars.
- **Ink wash**: Low-contrast paper/ink diffusion shader using generated noise channels for a calm writing environment.
- **Solarized nebula**: A palette-aware nebula that derives colors from the active terminal theme.
- **Matrix rain 2.0**: A less distracting matrix shader that avoids drawing behind dense terminal text and reacts to typing bursts.
- **Build reactor**: A progress-aware reactor/core glow that charges as `iProgress.y` advances and vents on error states.
- **Diff heatmap glow**: Full-content mode shader that adds subtle edge highlights to changed/bright text regions without blurring glyphs.
- **Low-power ambience pack**: Static or ultra-slow shaders designed to look polished while rendering at reduced frame cadence.

## Developer and ecosystem features

- **Gallery metadata generation**: Generate `docs/SHADERS.md`, website gallery entries, thumbnails, and `shaders/manifest.json` from a single shader metadata source.
- **Community shader submission checklist**: Document readability, license, performance, metadata, and screenshot requirements for contributed background shaders.
- **Performance budget hints**: Surface approximate frame cost per shader and recommend default animation speeds or low-power behavior.
- **Uniform debug overlay**: Add a developer overlay that displays live values for `iResolution`, `iTime`, `iMouse`, `iProgress`, cursor uniforms, channel resolutions, and current resolved shader config.
