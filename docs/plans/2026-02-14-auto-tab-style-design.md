# Auto Tab Style Switching Based on System Theme

**Issue**: #141
**Date**: 2026-02-14
**Status**: Approved

## Overview

Auto-switch tab style (Dark/Light/Compact/Minimal/High Contrast) based on system light/dark appearance by adding an `Automatic` variant to the `TabStyle` enum with configurable light/dark style mapping.

## Approach

Add `Automatic` variant to `TabStyle` enum + `light_tab_style` / `dark_tab_style` config fields. When `Automatic` is selected, the tab bar appearance resolves to the appropriate sub-style based on the system theme. Hooks into the same detection points as the existing `auto_dark_mode` feature.

## Data Model Changes

### TabStyle enum (`src/config/types.rs`)

Add `Automatic` variant:

```rust
pub enum TabStyle {
    Dark, Light, Compact, Minimal, HighContrast, Automatic,
}
```

- `display_name()` returns `"Automatic"`
- `all()` includes `Automatic`
- New `all_concrete()` method returns all variants except `Automatic` (for sub-style dropdowns)

### Config fields (`src/config/mod.rs`)

```rust
light_tab_style: TabStyle  // default: TabStyle::Light
dark_tab_style: TabStyle   // default: TabStyle::Dark
```

### New method: `apply_system_tab_style(is_dark: bool) -> bool`

- Returns false if `tab_style != Automatic`
- Resolves to `dark_tab_style` or `light_tab_style` based on `is_dark`
- Temporarily sets `tab_style` to the resolved value, calls `apply_tab_style()`, restores `Automatic`
- Returns true if style was applied

## Runtime Integration

Same two detection points as `apply_system_theme()`:

1. **Startup** (`src/app/window_state.rs`) - after `apply_system_theme()`, call `apply_system_tab_style(is_dark)`
2. **ThemeChanged event** (`src/app/handler.rs`) - after applying system theme, call `apply_system_tab_style(is_dark)` and trigger redraw

## Settings UI

### Tab style dropdown (`src/settings_ui/window_tab.rs`)

- `Automatic` appears in the existing tab style dropdown
- When `Automatic` is selected, show two sub-dropdowns:
  - "Light tab style:" using `TabStyle::all_concrete()`
  - "Dark tab style:" using `TabStyle::all_concrete()`
- On selection of `Automatic`, immediately apply resolved style for current system theme

### Search keywords (`src/settings_ui/sidebar.rs`)

Add to Window tab: `"auto tab style"`, `"automatic tab"`, `"system tab style"`

## Testing

- `apply_system_tab_style()` applies correct style for light/dark
- No-op when `tab_style != Automatic`
- `all_concrete()` excludes `Automatic`
- YAML serialization/deserialization round-trip with `Automatic`
