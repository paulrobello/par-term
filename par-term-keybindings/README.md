# par-term-keybindings

Runtime-configurable keybinding system for the par-term terminal emulator.

This crate provides parsing, matching, and registration of user-defined keyboard shortcuts.
Users define keybindings in `config.yaml`; this crate parses them into `KeyCombo` values
and matches incoming winit events against the registry at runtime.

## What This Crate Provides

- `KeybindingRegistry` — registry mapping `KeyCombo` values to action name strings
- `KeyCombo` — a parsed key combination (modifiers + key)
- `KeybindingMatcher` — matches a live winit event against a `KeyCombo`, with optional
  modifier remapping and physical-key preference
- `parse_key_combo` / `parse_key_sequence` — parse keybinding strings like `"Ctrl+Shift+B"`
- `key_combo_to_bytes` — converts a combo back to the byte sequence it would send
- `ParseError` — error type for invalid keybinding strings
- Platform utilities for cross-platform modifier key handling

## Supported Key Formats

```yaml
keybindings:
  - key: "CmdOrCtrl+Shift+B"   # macOS: Cmd, other: Ctrl
    action: "toggle_background_shader"
  - key: "Ctrl+Alt+Left"        # cross-platform
    action: "navigate_pane_left"
  - key: "F12"                  # function key (no modifiers)
    action: "open_inspector"
```

Supported modifier names: `Ctrl`, `Shift`, `Alt`, `Cmd`, `Super`, `CmdOrCtrl`, `Meta`.
Physical key matching is available for language-agnostic bindings.

## Key Types

| Type | Purpose |
|------|---------|
| `KeybindingRegistry` | Holds all parsed bindings, provides `lookup()` |
| `KeyCombo` | A parsed key combination with modifier bitfield and key identity |
| `KeybindingMatcher` | Created per-event to check if a combo matches |

## Workspace Position

Layer 2 in the dependency graph. Depends on `par-term-config` for `KeyBinding` and
`ModifierRemapping` types. Used directly by the root `par-term` crate for input handling.

## Related Documentation

- [Keyboard Shortcuts](../docs/KEYBOARD_SHORTCUTS.md) — default shortcut reference
- [Config Reference](../docs/CONFIG_REFERENCE.md) — keybinding configuration
- [Snippets & Actions](../docs/SNIPPETS.md) — custom action keybindings
- [Crate Structure](../docs/CRATE_STRUCTURE.md) — dependency layers
