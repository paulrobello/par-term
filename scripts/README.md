# par-term Test Scripts

This directory contains test scripts for verifying and benchmarking various features of par-term.

## Text Shaping & Font Tests

### `test_fonts.sh`
Comprehensive visual test suite for font features and text shaping.

**Usage:**
```bash
# Inside par-term terminal
./scripts/test_fonts.sh

# Or use Makefile
make test-fonts
```

**Tests 21 comprehensive features:**
1. Regular ASCII characters
2. Bold text (styled fonts)
3. Italic text (styled fonts)
4. Bold+Italic text (styled fonts)
5. CJK characters (font ranges)
6. Basic emoji
7. Flag emoji (Regional Indicators) ✅ Text Shaping
8. Mathematical symbols
9. Box drawing characters
10. Arrows and symbols
11. OSC 8 hyperlinks
12. Mixed content (all features)
13. **Emoji skin tones** (Fitzpatrick modifiers) ✅ Text Shaping
14. **ZWJ sequences** (complex emoji) ✅ Text Shaping
15. **Complex scripts** (Arabic, Devanagari, Thai, Bengali, Tamil) ✅ Text Shaping
16. **Bidirectional text** (LTR + RTL mix) ✅ Text Shaping
17. **Combining diacritics** (precomposed and combining characters) ✅ Text Shaping
18. **Programming ligatures** (liga, clig features) ✅ Text Shaping
19. **Wide character rendering** (CJK, emoji cell width) ✅ Text Shaping
20. **Variation selectors** (text vs emoji style) ✅ Text Shaping
21. Performance test (shaped glyph caching)

**Verification Checklist:**
- Styled fonts: Bold/italic should use configured font families
- Emoji skin tones: Should render with correct colors (👍🏻-👍🏿)
- Flag emoji: Should combine into flags (🇺🇸 🇬🇧 🇯🇵)
- Complex emoji: ZWJ sequences should form single glyphs (👨‍👩‍👧‍👦)
- Arabic/RTL: Letters should connect contextually
- Devanagari/Thai: Complex ligatures should form
- BiDi text: Mixed LTR/RTL should reorder correctly
- Diacritics: Combining marks should overlay properly
- Wide chars: CJK/emoji should occupy 2 cells
- Performance: Should render smoothly without lag

### `benchmark_text_shaping.sh`
Performance benchmark for text shaping vs non-shaped rendering.

**Usage:**
```bash
# Inside par-term terminal
./scripts/benchmark_text_shaping.sh

# Or use Makefile
make benchmark-shaping
```

**Benchmarks 14 content types:**
1. Pure ASCII (baseline)
2. ASCII with symbols
3. CJK characters
4. Simple emoji
5. Emoji with skin tones (complex graphemes)
6. Flag emoji (Regional Indicators)
7. ZWJ sequences (complex emoji)
8. Arabic text (RTL + contextual shaping)
9. Devanagari text (complex ligatures)
10. Thai text (non-spacing marks)
11. Mixed content (stress test)
12. Heavy emoji load
13. Combining diacritics
14. Wide character mix

**Comparison workflow:**
1. Run benchmark with `enable_text_shaping: true` (default)
2. Edit `~/.config/par-term/config.yaml` and set `enable_text_shaping: false`
3. Reload config (F5) or restart par-term
4. Run benchmark again
5. Compare throughput (lines/sec) for each test

**Expected Results:**
- ASCII: Minimal difference (simple caching either way)
- CJK: Similar performance (direct glyph lookup)
- Complex scripts: Shaped rendering may be slightly slower
- Multi-component emoji: Shaped rendering much better quality
- Overall: Text shaping overhead should be minimal (<10%)
- Cache warmup: Second run should be faster (LRU cache)

**Debug Analysis:**
```bash
# Monitor performance logs
RUST_LOG=debug ./target/release/par-term 2>&1 | grep "PERF:"

# Analyze cache hit rates
RUST_LOG=debug ./target/release/par-term 2>&1 | grep "shaped" | grep "cache"
```

## Graphics Tests

### `test_sixel.sh`
Tests Sixel graphics rendering (if available).

**Usage:**
```bash
./scripts/test_sixel.sh
```

### `test_bells.sh`
Tests bell notification system (visual, audio, desktop notifications).

**Usage:**
```bash
./scripts/test_bells.sh
```

## Configuration

All tests assume default par-term configuration with text shaping enabled:

```yaml
# Text shaping (enabled by default)
enable_text_shaping: true
enable_ligatures: true
enable_kerning: true

# Font configuration examples
font_family: "JetBrains Mono"
font_family_bold: "JetBrains Mono Bold"
font_family_italic: "JetBrains Mono Italic"
font_family_bold_italic: "JetBrains Mono Bold Italic"

# Font ranges for specific scripts
font_ranges:
  - start: 0x4E00
    end: 0x9FFF
    font_family: "Noto Sans CJK"
  - start: 0x1F600
    end: 0x1F64F
    font_family: "Noto Color Emoji"
```

See `examples/` directory for complete configuration templates.

## Requirements

### Font Requirements

For best results with comprehensive tests, install appropriate fonts:

**Monospace (programming):**
- JetBrains Mono (recommended)
- Fira Code
- Cascadia Code
- Iosevka

**CJK (Chinese, Japanese, Korean):**
- Noto Sans CJK
- Source Han Sans
- WenQuanYi Micro Hei

**Emoji:**
- Noto Color Emoji (Linux)
- Apple Color Emoji (macOS)
- Segoe UI Emoji (Windows)

**Arabic/RTL:**
- Noto Sans Arabic
- Amiri
- Scheherazade

**Devanagari:**
- Noto Sans Devanagari
- Lohit Devanagari

**Thai:**
- Noto Sans Thai
- Sarabun

### System Requirements

- par-term built in release mode (`make release`)
- Terminal with color support
- Sufficient scrollback buffer for viewing results

## Troubleshooting

### Emoji not rendering
- Check that emoji font is installed
- Verify `font_ranges` includes emoji codepoint range
- Ensure color emoji support in system

### Complex scripts not shaping correctly
- Verify `enable_text_shaping: true` in config
- Check that appropriate font is installed
- Try `RUST_LOG=debug` to see shaping details

### Performance issues
- Run benchmark to identify slow content types
- Check cache hit rates in debug logs
- Consider adjusting `font_size` or reducing content complexity

### Programming ligatures not displaying
- This is a known limitation
- Terminal cell width constraints prevent multi-char ligatures
- OpenType features are activated but glyphs don't fit in cells
- Future enhancement pending

## Contributing

To add new tests:

1. Add test to appropriate script (`test_fonts.sh` or create new script)
2. Update this README with test description
3. Add Makefile target if needed
4. Test on all supported platforms (macOS, Linux, Windows)

## See Also

- `examples/` - Configuration file examples
- `QUICK_START_FONTS.md` - 5-minute font setup guide
- `IMPLEMENTATION_SUMMARY.md` - Technical architecture reference
- `README.md` - Main project documentation
