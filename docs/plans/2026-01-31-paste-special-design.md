# Paste Special Feature Design

**Date:** 2026-01-31
**Issue:** #41
**Status:** Approved

## Overview

Paste Special adds text transformations before pasting clipboard content. Users can transform text (case conversion, shell escaping, encoding, etc.) before it enters the terminal.

## User Flow

### Two Access Methods

1. **Quick shortcut** (`Cmd/Ctrl+Shift+V` default, configurable)
   - Opens command palette with current clipboard content
   - User selects transformation via fuzzy search
   - Text is transformed and pasted immediately

2. **Clipboard history integration** (`Shift+Enter` in history UI)
   - When viewing clipboard history (`Ctrl+Shift+H`)
   - `Shift+Enter` on selected entry opens transformation palette
   - Allows transforming historical clipboard entries

### Command Palette Behavior

- Centered overlay, dark themed (matches Settings UI)
- Search input at top, transformation list below
- Fuzzy search filters as you type ("b64" matches "Encode: Base64")
- Arrow keys navigate, Enter applies and pastes
- Escape cancels without pasting
- Shows preview of transformed text (first ~100 chars, truncated)

## Transformation List

Flat list with category prefixes for searchability:

### Shell Category
| Name | Description |
|------|-------------|
| `Shell: Single Quotes` | Wraps in `'...'`, escapes internal `'` as `'\''` |
| `Shell: Double Quotes` | Wraps in `"..."`, escapes `$`, `` ` ``, `\`, `"`, `!` |
| `Shell: Backslash Escape` | Escapes special chars with `\` |

### Case Category
| Name | Description |
|------|-------------|
| `Case: UPPERCASE` | All characters uppercase |
| `Case: lowercase` | All characters lowercase |
| `Case: Title Case` | First letter of each word uppercase |
| `Case: camelCase` | First word lower, subsequent capitalized, no separators |
| `Case: PascalCase` | All words capitalized, no separators |
| `Case: snake_case` | Lowercase with underscores |
| `Case: SCREAMING_SNAKE` | Uppercase with underscores |
| `Case: kebab-case` | Lowercase with hyphens |

### Whitespace Category
| Name | Description |
|------|-------------|
| `Whitespace: Trim` | Remove leading/trailing whitespace |
| `Whitespace: Trim Lines` | Trim each line individually |
| `Whitespace: Collapse Spaces` | Multiple spaces → single space |
| `Whitespace: Tabs to Spaces` | Convert tabs to 4 spaces |
| `Whitespace: Spaces to Tabs` | Convert 4 spaces to tabs |
| `Whitespace: Remove Empty Lines` | Delete blank lines |
| `Whitespace: Normalize Line Endings` | Convert to `\n` |

### Encode Category
| Name | Description |
|------|-------------|
| `Encode: Base64` | Base64 encode |
| `Decode: Base64` | Base64 decode |
| `Encode: URL` | URL/percent encode |
| `Decode: URL` | URL/percent decode |
| `Encode: Hex` | Hexadecimal encode |
| `Decode: Hex` | Hexadecimal decode |
| `Encode: JSON Escape` | Escape for JSON string |
| `Decode: JSON Unescape` | Unescape JSON string |

## Architecture

### New Files

**`src/paste_transform.rs`** - Transformation logic
- `PasteTransform` enum with all transformation types
- `fn transform(input: &str, transform: PasteTransform) -> Result<String, String>`
- Each transformation is a pure function, easy to test
- Returns `Result` to handle decode failures gracefully

**`src/paste_special_ui.rs`** - Command palette UI
- `PasteSpecialUI` struct (similar pattern to `ClipboardHistoryUI`)
- Fuzzy search using simple substring matching
- Renders via egui in the main render loop
- Shows transformation preview

### Modified Files

**`src/app/input_events.rs`**
- Add paste special shortcut handler
- Add `Shift+Enter` handling in clipboard history

**`src/config/types.rs`**
- Add `"paste_special"` action to keybinding system

**`src/app/handler.rs`**
- Add `PasteSpecialUI` to `AppState`
- Wire up rendering in main loop

### Data Flow

```
Shortcut pressed
    ↓
Open PasteSpecialUI with clipboard content
    ↓
User types to filter, selects transform
    ↓
transform() applied to content
    ↓
Result passed to existing paste_text()
    ↓
Text pasted with bracketed paste support
```

## Testing Strategy

### Unit Tests (`src/paste_transform.rs`)

- Each transformation has dedicated tests
- Edge cases: empty strings, unicode, very long text
- Round-trip tests for encode/decode pairs
- Error handling for invalid decode input

### Integration Tests

- Verify UI opens with shortcut
- Verify fuzzy search filters correctly
- Verify Shift+Enter works in clipboard history

## Implementation Order

1. Create `PasteTransform` enum and `transform()` function with tests
2. Create `PasteSpecialUI` struct with fuzzy search and preview
3. Wire up keybinding and integrate with `paste_text()`
4. Add `Shift+Enter` to clipboard history UI
5. Add default keybinding to config

## Dependencies

No external dependencies needed - all transformations use Rust stdlib:
- `base64` encoding: manual implementation or use existing base64 in deps
- URL encoding: percent-encode special chars
- Case transforms: char iteration with `to_uppercase()`/`to_lowercase()`

## Keybinding Configuration

Default keybinding added to `default_keybindings()`:
```rust
KeyBinding {
    key: "CmdOrCtrl+Shift+V".to_string(),
    action: "paste_special".to_string(),
}
```

Users can customize via Settings UI or config file.
