# par-term Configuration Examples

This directory contains example configuration files demonstrating various features of par-term.

## Configuration Files

### 1. `config-styled-fonts.yaml`
**Purpose:** Demonstrates how to use separate font families for bold and italic text.

**Use this when:**
- You want crisp, professionally-designed bold/italic fonts instead of synthetic rendering
- You're using a font family that has proper bold/italic variants
- You want maximum typographic quality

**Key features:**
```yaml
font_family: "JetBrains Mono"
font_family_bold: "JetBrains Mono Bold"
font_family_italic: "JetBrains Mono Italic"
font_family_bold_italic: "JetBrains Mono Bold Italic"
```

### 2. `config-text-shaping.yaml` ✨ NEW!
**Purpose:** Demonstrates HarfBuzz text shaping for ligatures and complex scripts.

**Use this when:**
- You want ligatures (fi → ﬁ, -> → →, >= → ≥)
- You're using fonts with programming ligatures (Fira Code, JetBrains Mono)
- You need proper emoji rendering (flags 🇺🇸, skin tones 👋🏽, ZWJ sequences)
- You work with complex scripts (Arabic, Devanagari, Thai)

**Key features:**
```yaml
enable_text_shaping: true   # Enable HarfBuzz
enable_ligatures: true       # Combine character sequences
enable_kerning: true         # Spacing adjustments
font_family: "Fira Code"    # Ligature-supporting font
```

**What you get:**
- Programming ligatures: `->` `=>` `>=` `<=` `!=` `==`
- Text ligatures: `fi` `fl` `ffi` `ffl`
- Emoji flags: 🇺🇸 🇬🇧 🇯🇵 (Regional Indicators)
- Emoji with skin tones: 👋🏽 👍🏿
- Complex emoji: 👨‍👩‍👧‍👦 (family with ZWJ)
- Proper kerning between all characters

### 3. `config-font-ranges.yaml`
**Purpose:** Demonstrates Unicode range-based font mapping.

**Use this when:**
- You work with multiple languages (CJK, Arabic, etc.)
- You want specific fonts for emoji or symbols
- You need specialized fonts for mathematical notation
- You want optimal glyph rendering for different scripts

**Key features:**
```yaml
font_ranges:
  - start: 0x4E00  # CJK start
    end: 0x9FFF    # CJK end
    font_family: "Noto Sans CJK SC"
```

### 4. `config-complete.yaml`
**Purpose:** Comprehensive configuration showing all features together.

**Use this when:**
- You want to see all available options
- You need both styled fonts AND Unicode ranges
- You're setting up a production configuration

**Features:**
- Styled fonts (bold/italic/bold-italic)
- Multiple Unicode ranges (CJK, emoji, symbols, math)
- Full terminal configuration
- Detailed comments explaining each option

## Font Selection Priority

Understanding font priority is crucial for getting the rendering you want:

```
1. Styled Fonts (if text is bold/italic)
   ├─ Bold text → font_family_bold
   ├─ Italic text → font_family_italic
   └─ Bold+Italic → font_family_bold_italic

2. Unicode Range Fonts (if character in defined range)
   └─ Checked in order, first match wins

3. General Fallback Fonts (automatic)
   └─ System fonts with good Unicode coverage

4. Primary Font (default)
   └─ font_family
```

### Example Scenarios

**Scenario 1: Bold CJK Text**
```
Character: 你 (U+4F60, bold)
Priority check:
1. ✗ Bold font (doesn't have CJK glyphs)
2. ✓ CJK range font (0x4E00-0x9FFF) → Uses "Noto Sans CJK SC"
```

**Scenario 2: Bold ASCII Text**
```
Character: A (U+0041, bold)
Priority check:
1. ✓ Bold font → Uses "JetBrains Mono Bold"
```

**Scenario 3: Emoji**
```
Character: 😀 (U+1F600)
Priority check:
1. ✗ Regular font (no emoji glyphs)
2. ✓ Emoji range font (0x1F600-0x1F64F) → Uses "Apple Color Emoji"
```

## Common Unicode Ranges

Here are commonly used Unicode ranges for font mapping:

| Range | Characters | Example Font |
|-------|-----------|--------------|
| `0x0000-0x007F` | Basic Latin (ASCII) | Any monospace |
| `0x0370-0x03FF` | Greek and Coptic | DejaVu Sans Mono |
| `0x0400-0x04FF` | Cyrillic | DejaVu Sans Mono |
| `0x0600-0x06FF` | Arabic | Noto Sans Arabic |
| `0x3040-0x309F` | Hiragana | Noto Sans CJK JP |
| `0x30A0-0x30FF` | Katakana | Noto Sans CJK JP |
| `0x4E00-0x9FFF` | CJK Unified Ideographs | Noto Sans CJK SC/JP/KR |
| `0xAC00-0xD7AF` | Hangul Syllables | Noto Sans CJK KR |
| `0x1F300-0x1F5FF` | Symbols & Pictographs | Apple Color Emoji |
| `0x1F600-0x1F64F` | Emoticons | Apple Color Emoji |
| `0x1F680-0x1F6FF` | Transport & Map | Apple Color Emoji |
| `0x2190-0x21FF` | Arrows | DejaVu Sans Mono |
| `0x2200-0x22FF` | Mathematical Operators | STIX Two Math |
| `0x2500-0x257F` | Box Drawing | DejaVu Sans Mono |
| `0x2580-0x259F` | Block Elements | DejaVu Sans Mono |

## Tips and Best Practices

### 1. Font Availability
Before configuring a font, verify it's installed on your system:
```bash
# macOS
fc-list | grep -i "font name"

# Linux
fc-list | grep -i "font name"

# Check specifically for CJK fonts
fc-list :lang=zh
fc-list :lang=ja
fc-list :lang=ko
```

### 2. Testing Your Configuration
Create a test file with various Unicode characters:
```bash
echo "ASCII: Hello World" > test.txt
echo "Bold: **Bold Text**" >> test.txt
echo "CJK: 你好世界 こんにちは 안녕하세요" >> test.txt
echo "Emoji: 😀 🚀 ⭐" >> test.txt
echo "Math: ∀x∈ℝ: x²≥0" >> test.txt
echo "Box: ┌─┬─┐" >> test.txt
```

Then view it in par-term to verify font rendering.

### 3. Performance Considerations
- More font ranges = slightly more lookup time (negligible in practice)
- Glyph cache ensures subsequent renders are fast
- Font ranges are only checked if character not in styled font

### 4. Combining with OSC 8 Hyperlinks
Font ranges work seamlessly with OSC 8 hyperlinks:
```bash
# CJK hyperlink example
printf '\e]8;;https://example.com\e\\中文链接\e]8;;\e\\\n'
```

The CJK characters will use your configured range font, while still being clickable.

## Troubleshooting

### Problem: Font not being used
**Check:**
1. Is the font installed? (`fc-list | grep "Font Name"`)
2. Is the font name spelled correctly in config?
3. Does the font have glyphs for your characters?
4. Is the Unicode range correct? (Use a Unicode chart)

### Problem: Bold/Italic not working
**Check:**
1. Are the styled font families installed?
2. Do the font names match exactly?
3. Try removing styled fonts temporarily to see if regular font works

### Problem: Some characters missing
**Check:**
1. Add a broader fallback font range
2. Check if character is in your defined ranges
3. Verify font has the glyph (use a font viewer)

## Configuration Location

Copy your chosen configuration to:
- **macOS/Linux:** `~/.config/par-term/config.yaml`
- **Windows:** `%APPDATA%\par-term\config.yaml`

Or use `--config` flag:
```bash
par-term --config path/to/config.yaml
```

## Further Reading

- [Unicode Character Ranges](https://en.wikipedia.org/wiki/Unicode_block)
- [Font Quick Start Guide](../QUICK_START_FONTS.md)
- [Main README](../README.md)

## Contributing Examples

Have a great configuration? Submit a PR with your example configuration and add it to this README!
