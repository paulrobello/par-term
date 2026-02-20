# Nerd Font Integration for Profile Icon Picker

**Date**: 2026-02-20
**Status**: Approved

## Problem

egui's glyph atlas is alpha-only, so color emoji cannot render. The profile icon picker uses emoji presets that display as blank boxes or fallback squares. Status indicators using basic Unicode shapes (●, ○) work fine, but the decorative emoji icons do not.

## Solution

Embed the Nerd Font Symbols-only subset (`SymbolsNerdFontMono-Regular.ttf`, ~2MB) into the binary and register it as a fallback font in egui's `Proportional` font family. Replace the emoji presets in the profile icon picker with ~120 curated Nerd Font icons.

## Architecture

### Font Loading

- Embed `SymbolsNerdFontMono-Regular.ttf` via `include_bytes!` in `par-term-settings-ui`
- Create `par-term-settings-ui/src/nerd_font.rs` with:
  - `configure_nerd_font(ctx: &egui::Context)` — adds font to `FontDefinitions` as Proportional fallback
  - Curated icon constant arrays organized by category
- Call from both egui context init sites:
  - `src/app/renderer_init.rs` (main window)
  - `src/settings_window.rs` (settings window)

### Icon Picker Update

- Replace `EMOJI_PRESETS` in `profile_modal_ui.rs` with `NERD_FONT_PRESETS`
- Same categories: Terminal, Dev & Tools, Files & Data, Network & Cloud, Security, Status & Alerts, Containers & Infra, People & Roles, Misc
- ~12-15 icons per category, all from the Nerd Font Symbols codepoint range

### Backward Compatibility

- Existing profiles with emoji icons continue to display (egui's built-in fonts cover basic emoji as monochrome glyphs)
- Users can still type any character in the icon text field
- Tab bar rendering (`ui.label(icon)`) needs no changes — it renders whatever string is stored

## Components Modified

1. `assets/SymbolsNerdFontMono-Regular.ttf` — new embedded font file
2. `par-term-settings-ui/src/nerd_font.rs` — new module: font setup + icon constants
3. `par-term-settings-ui/src/profile_modal_ui.rs` — swap emoji presets for Nerd Font presets
4. `par-term-settings-ui/src/lib.rs` — export new module
5. `src/app/renderer_init.rs` — call `configure_nerd_font()` after creating egui context
6. `src/settings_window.rs` — call `configure_nerd_font()` after creating egui context
