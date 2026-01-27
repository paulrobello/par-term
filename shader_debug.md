# Shader Y-Axis Debug Log

## Goal
Make Shadertoy shaders work with par-term without modification.
- Shadertoy convention: fragCoord.y = 0 at BOTTOM, height at TOP
- wgpu convention: @builtin(position).y = 0 at TOP, height at BOTTOM

## Confirmed Facts
1. **wgpu @builtin(position).y** = 0 at top of screen, height at bottom
2. **Raw gl_FragCoord_st read in mainImage** - WORKS (shows DARK at top = y=0)
3. **Parameter passing from main() to mainImage** - BROKEN (values incorrect)
4. **Global variable writes in main_1, reads in mainImage** - BROKEN (sees old value)
5. **Separate global variable approach** - BROKEN (same issue)
6. **WGSL post-processing injection** - BROKEN (same issue)
7. **GLSL preprocessing with parameter rename** - BROKEN (same issue)

## Approaches Tried

### 1. Flip in fs_main only (WGSL post-processing)
- fs_main: `gl_FragCoord_st = vec2(frag_pos.x, iResolution.y - frag_pos.y)`
- main_1: reads gl_FragCoord_st, passes to mainImage
- mainImage: uses fragCoord parameter
- **Result: INVERTED** (BLUE at top)

### 2. Flip in fs_main + workaround (mainImage reads global)
- fs_main: applies flip to gl_FragCoord_st
- mainImage: reads from gl_FragCoord_st global (workaround)
- **Result: INVERTED** (BLUE at top)

### 3. Double flip (fs_main + main_1)
- Both places apply flip
- **Result: Same as no flip** (expected - flips cancel)

### 4. Flip in GLSL main() only
- fs_main: raw frag_pos
- main_1 (GLSL): `gl_FragCoord_st.y = iResolution.y - gl_FragCoord_st.y;`
- **Result: INVERTED** (BLUE at top)

### 5. Flip in GLSL main() with local variable
- fs_main: raw frag_pos
- main_1: `st_fragCoord = vec2(gl_FragCoord_st.x, iResolution.y - gl_FragCoord_st.y);`
- mainImage: uses fragCoord parameter
- **Result: INVERTED** (BLUE at top)

### 6. mainImage does its own flip (CLAIMED TO WORK - needs reverification)
- fs_main: raw frag_pos
- main_1: passes raw gl_FragCoord_st
- mainImage: `vec2 flippedCoord = vec2(gl_FragCoord_st.x, iResolution.y - gl_FragCoord_st.y);`
- **Result: CLAIMED CORRECT** (RED at top) - but subsequent tests show this may not be reproducible

### 7. Flip in main_1 + write back to gl_FragCoord_st + workaround
- fs_main: raw frag_pos to gl_FragCoord_st
- main_1: `gl_FragCoord_st = vec2(x, iRes.y - y)` (writes flipped value)
- mainImage: reads from gl_FragCoord_st global (workaround)
- **Result: INVERTED** (DARK at top) - mainImage sees OLD value despite write!

### 8. Separate global variable st_fragCoord_flipped
- New var<private> st_fragCoord_flipped
- main_1 writes flipped value to st_fragCoord_flipped
- mainImage reads from st_fragCoord_flipped (workaround)
- **Result: INVERTED** (DARK at top) - STILL doesn't work!

### 9. Inject flip INTO mainImage via WGSL post-processing
- Replace `fragCoord_1 = fragCoord` with flip calculation inside mainImage
- **Result: INVERTED** (DARK at top) - WGSL injection doesn't work!

### 10. GLSL preprocessing - rename parameter + inject flip
- Rename `fragCoord` to `_fragCoord_raw` in function signature
- Inject `vec2 fragCoord = vec2(_fragCoord_raw.x, iResolution.y - _fragCoord_raw.y);`
- **Result: INVERTED** - parameter value is wrong anyway

### 11. GLSL preprocessing - use gl_FragCoord_st instead of parameter
- Rename parameter to `_fragCoord_unused`
- Inject `vec2 fragCoord = vec2(gl_FragCoord_st.x, iResolution.y - gl_FragCoord_st.y);`
- **Result: INVERTED** (DARK at top) - flip still not working!

### 12. Raw gl_FragCoord_st read test (NO flip, preprocessing disabled)
- mainImage: `brightness = gl_FragCoord_st.y / 1000.0;`
- **Result: DARK at top, BRIGHT at bottom** = CORRECT for raw read (y=0 at top)
- **CONFIRMS: mainImage CAN read gl_FragCoord_st correctly**

### 13. Test iResolution.y value
- mainImage: `brightness = iResolution.y / 1000.0;`
- **Result: UNIFORM WHITE** = iResolution.y is correct (~500-800 depending on window)
- **CONFIRMS: iResolution.y is accessible and has correct value**

### 14. Test flip calculation directly
- mainImage: `flippedY = iResolution.y - gl_FragCoord_st.y; brightness = flippedY / 1000.0;`
- Expected: BRIGHT at top, DARK at bottom (opposite of test #12)
- **Result: BRIGHT at top, DARK at bottom** ✅ CORRECT!
- **CONFIRMS: Flip calculation works when done directly in mainImage!**

### 15. Test flip with vec2 (like preprocessing does)
- mainImage: `vec2 flippedCoord = vec2(gl_FragCoord_st.x, iResolution.y - gl_FragCoord_st.y);`
- **Result: BRIGHT at top, DARK at bottom** ✅ CORRECT!
- **CONFIRMS: vec2 flip also works!**

### 16. Exactly mimic preprocessing output
- Parameter: `in vec2 _fragCoord_unused`
- Local: `vec2 fragCoord = vec2(gl_FragCoord_st.x, iResolution.y - gl_FragCoord_st.y);`
- **Result: BRIGHT at top, DARK at bottom** ✅ CORRECT!
- **CONFIRMS: Manual preprocessing pattern works!**

### 17-20. Various preprocessing approaches
- Tried GLSL preprocessing with #define - didn't work (naga may not support #define)
- Tried WGSL-level flip in fs_main - mainImage doesn't see flipped value
- **Result: All INVERTED** - preprocessing/injection approaches fail

### 21. WGSL-level flip with fragCoord parameter
- Flip in fs_main when setting gl_FragCoord_st
- mainImage uses fragCoord parameter
- **Result: INVERTED** - parameter passing still broken

### 22. Read gl_FragCoord_st directly with fs_main flip
- fs_main flips gl_FragCoord_st
- mainImage reads gl_FragCoord_st directly
- **Result: INVERTED** - mainImage sees OLD (unflipped) value!

### 23. Debug with solid color split
- mainImage outputs BLUE if gl_FragCoord_st.y > halfHeight, RED otherwise
- **Result: TOP=RED, BOTTOM=BLUE** - confirms mainImage sees unflipped value
- This confirms var<private> writes in main_1/fs_main are NOT visible to mainImage

### 24. GLSL main() flip with fresh local variable
- Create st_fragCoord with flip directly, then assign to gl_FragCoord_st
- **Result: TOP=RED, BOTTOM=BLUE** - STILL inverted

## Key Discovery
The raw read (test #12) shows mainImage DOES read gl_FragCoord_st correctly when there's no flip calculation. The value IS y=0 at top.

But when we ADD the flip calculation `iResolution.y - gl_FragCoord_st.y`, the result is wrong. This suggests the issue is with the flip CALCULATION itself, not the global variable read.

## New iteration (2026-01-26 12:10 PST)
- Re-enabled flip in `mainImage` prologue only (preprocess injects `fragCoord = vec2(_fc_raw.x, iResolution.y - _fc_raw.y); gl_FragCoord_st = fragCoord;`).
- Wrapper `main()` now passes raw `gl_FragCoord_st` (no flip).
- fs_main seeds `gl_FragCoord_st` with raw @builtin(position).
- Expected: single flip; TOP should render BLUE, bottom RED.
- If still inverted, suspect Metal auto-flips `_fc_raw` already; then remove prologue flip and instead flip in wrapper main_1.

Next steps to try
1. Run `make build && DEBUG_LEVEL=3 cargo run --release` to refresh generated WGSL.
2. If top still RED, move flip to wrapper main_1 and keep mainImage raw.
3. If both fail, force inline flip everywhere (WGSL post-process) or test Vulkan backend.

## Cursor shader not applied (2026-01-26 12:18 PST)
- Observation: cursor shaders appear disabled; no “Cursor shader renderer initialized” logs.
- Config shows `cursor_shader_enabled: true`, `cursor_shader: cursor_pacman.glsl` in `~/.config/par-term/config.yaml`.
- Added debug logging in:
  - `renderer/shaders.rs::init_cursor_shader` (log config and errors)
  - `renderer/shaders.rs::set_cursor_shader_enabled` (log toggles/loads/failures)
  - `renderer/mod.rs::set_cursor_shader_disabled_for_alt_screen` (log alt-screen toggles)
- Hypothesis: either init is skipped (flag false) or load fails silently; new logs should expose which.
- Next: run `DEBUG_LEVEL=3 cargo run --release`, then grep `/tmp/par_term_debug.log` for “Cursor shader”.

## Current State
- GLSL preprocessing is DISABLED
- GLSL main() wrapper has flip code (but doesn't work)
- Using debug-coords-manual.glsl test shader (test #23 - color split)
- Config: custom_shader: debug-coords-manual.glsl

## Key Finding
**Manual flip inside mainImage WORKS** (test #14, #18), but ANY attempt to inject the flip via:
- WGSL post-processing
- GLSL preprocessing
- GLSL main() wrapper modification

...results in mainImage seeing the ORIGINAL (unflipped) value. This suggests a fundamental issue with how naga/wgpu handles var<private> writes across function call boundaries.
