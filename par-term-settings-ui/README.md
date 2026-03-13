# par-term-settings-ui

Egui-based settings interface for the par-term terminal emulator.

This crate provides the full settings window UI for configuring terminal options at
runtime. It is decoupled from the main terminal implementation through trait interfaces
(`SettingsContext`), allowing the settings UI to be compiled and tested independently.

## What This Crate Provides

- `SettingsUI` — the main settings window struct; renders all settings tabs via egui
- `SettingsWindowAction` — enum of actions returned to the main application after UI events
  (apply config, save config, apply shader, open profile, start/stop coprocess, etc.)
- `SettingsTab` — enum identifying the active settings tab (used for search and navigation)
- Tab modules for every settings area: appearance, terminal, input, effects, automation,
  snippets/actions, profiles, SSH, status bar, AI inspector, integrations, advanced, etc.
- `ProfileModalUI` — inline profile picker modal
- `ArrangementManager` — window arrangement data types and persistence
- `nerd_font` — Nerd Font icon loading and preset definitions for the icon picker
- `sidebar` — settings sidebar with tab search functionality
- `section` — reusable section heading components
- `shader_utils` — utilities for shader editor panels

## Key Types

| Type | Purpose |
|------|---------|
| `SettingsUI` | Main settings window; call `show()` each frame |
| `SettingsWindowAction` | Return value from `show()` for the main app to process |
| `SettingsTab` | Identifies the currently visible tab |
| `ArrangementManager` | Manages saved window arrangements |
| `ProfileModalUI` | Profile selection modal for tab bar and settings |

## Decoupling Pattern

Settings UI communicates with the main application through `SettingsWindowAction` return
values rather than direct callbacks. The main application processes each action after the
UI frame completes. This keeps the settings crate free of main-crate dependencies.

## Workspace Position

Layer 2 in the dependency graph. Depends on `par-term-config` for all configuration types.
Used directly by the root `par-term` crate and re-exported as `par_term::settings_ui`.

## Related Documentation

- [Config Reference](../docs/CONFIG_REFERENCE.md) — all configurable options
- [Profiles](../docs/PROFILES.md) — profile management
- [Arrangements](../docs/ARRANGEMENTS.md) — window arrangements
- [Custom Shaders](../docs/CUSTOM_SHADERS.md) — shader editor documentation
- [Crate Structure](../docs/CRATE_STRUCTURE.md) — dependency layers
