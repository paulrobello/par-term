# Quick Start: Font Features

Get up and running with par-term's advanced font features in 5 minutes.

## Table of Contents

- [Styled Fonts](#quick-setup-styled-fonts)
- [CJK Support](#quick-setup-cjk-support)
- [Emoji](#quick-setup-emoji)
- [Math Symbols](#quick-setup-math-symbols)
- [Complete Setup](#complete-setup-everything-together)
- [Test Script](#test-script)
- [Troubleshooting](#troubleshooting)
- [Next Steps](#next-steps)
- [Pro Tips](#pro-tips)

---

## Quick Setup: Styled Fonts

**Goal:** Use proper bold/italic fonts instead of synthetic rendering.

### Step 1: Check Font Availability
```bash
# macOS/Linux - check if font is installed
fc-list | grep -i "JetBrains Mono"

# You should see variations like:
# JetBrains Mono Bold
# JetBrains Mono Italic
# JetBrains Mono Bold Italic
```

### Step 2: Update Config
Edit `~/.config/par-term/config.yaml`:
```yaml
font_family: "JetBrains Mono"
font_family_bold: "JetBrains Mono Bold"
font_family_italic: "JetBrains Mono Italic"
font_family_bold_italic: "JetBrains Mono Bold Italic"
```

### Step 3: Test It
```bash
# Start par-term
par-term

# In the terminal, try these commands:
echo -e "\e[1mBold Text\e[0m"           # Bold
echo -e "\e[3mItalic Text\e[0m"         # Italic
echo -e "\e[1;3mBold+Italic\e[0m"      # Both
```

**Expected result:** Crisp, professionally-designed bold/italic text!

---

## Quick Setup: CJK Support

**Goal:** Use proper fonts for Chinese, Japanese, Korean characters.

### Step 1: Install CJK Fonts
```bash
# macOS - check if already installed
fc-list :lang=zh | grep -i "Noto Sans CJK"

# Linux - install if needed
sudo apt install fonts-noto-cjk  # Debian/Ubuntu
sudo dnf install google-noto-cjk-fonts  # Fedora
```

### Step 2: Update Config
Edit `~/.config/par-term/config.yaml`:
```yaml
font_family: "JetBrains Mono"

font_ranges:
  # Chinese characters
  - start: 0x4E00
    end: 0x9FFF
    font_family: "Noto Sans CJK SC"
```

### Step 3: Test It
```bash
# Start par-term
par-term

# Test CJK rendering
echo "Chinese: 你好世界"
echo "Japanese: こんにちは 世界"
echo "Korean: 안녕하세요 세계"
```

**Expected result:** Beautiful CJK characters with proper font!

---

## Quick Setup: Emoji

**Goal:** Colorful emoji rendering.

### Step 1: Update Config
```yaml
font_family: "JetBrains Mono"

font_ranges:
  - start: 0x1F600  # Emoticons
    end: 0x1F64F
    font_family: "Apple Color Emoji"

  - start: 0x1F300  # Symbols
    end: 0x1F5FF
    font_family: "Apple Color Emoji"
```

### Step 2: Test It
```bash
par-term

echo "😀 🚀 ⭐ 🎉 ❤️ 👍"
echo "🌟 🔥 💻 📱 ⚡"
```

**Expected result:** Colorful emoji!

---

## Quick Setup: Math Symbols

**Goal:** Proper rendering for mathematical notation.

### Step 1: Install Math Font
```bash
# Check for STIX fonts
fc-list | grep -i "STIX"

# macOS usually has these built-in
# Linux: sudo apt install fonts-stix
```

### Step 2: Update Config
```yaml
font_family: "JetBrains Mono"

font_ranges:
  - start: 0x2200  # Mathematical Operators
    end: 0x22FF
    font_family: "STIX Two Math"
```

### Step 3: Test It
```bash
par-term

echo "∀x∈ℝ: x²≥0"
echo "∫₀^∞ e^(-x²)dx = √π/2"
echo "∑ᵢ₌₁^n i = n(n+1)/2"
```

**Expected result:** Beautiful math symbols!

---

## Complete Setup: Everything Together

For the ultimate configuration, use all features:

```yaml
# Primary font
font_family: "JetBrains Mono"
font_size: 14.0

# Styled fonts
font_family_bold: "JetBrains Mono Bold"
font_family_italic: "JetBrains Mono Italic"
font_family_bold_italic: "JetBrains Mono Bold Italic"

# Unicode ranges
font_ranges:
  # CJK
  - start: 0x4E00
    end: 0x9FFF
    font_family: "Noto Sans CJK SC"

  # Emoji
  - start: 0x1F600
    end: 0x1F64F
    font_family: "Apple Color Emoji"

  # Math
  - start: 0x2200
    end: 0x22FF
    font_family: "STIX Two Math"

  # Box Drawing
  - start: 0x2500
    end: 0x257F
    font_family: "DejaVu Sans Mono"
```

Test everything:
```bash
par-term

# Test all features
cat << 'EOF'
Regular, Bold, and Italic:
Regular text
Bold text (uses styled font)
Italic text (uses styled font)

CJK Characters:
中文 (Chinese)
日本語 (Japanese)
한국어 (Korean)

Emoji:
😀 🚀 ⭐ 🎉

Math:
∀x∈ℝ: x²≥0

Box Drawing:
┌─────┐
│ Box │
└─────┘
EOF
```

---

## Test Script

Save this as `test_fonts.sh`:

```bash
#!/bin/bash
# Test script for par-term font features

echo "=== Testing par-term Font Features ==="
echo ""

echo "1. Regular ASCII:"
echo "   ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
echo ""

echo "2. Bold text (uses font_family_bold):"
echo -e "   \e[1mBOLD TEXT - Should use styled font\e[0m"
echo ""

echo "3. Italic text (uses font_family_italic):"
echo -e "   \e[3mItalic Text - Should use styled font\e[0m"
echo ""

echo "4. Bold+Italic (uses font_family_bold_italic):"
echo -e "   \e[1;3mBold and Italic - Should use styled font\e[0m"
echo ""

echo "5. CJK Characters (uses font_ranges):"
echo "   Chinese: 你好世界 (Nǐ hǎo shìjiè)"
echo "   Japanese: こんにちは世界 (Konnichiwa sekai)"
echo "   Korean: 안녕하세요 세계 (Annyeonghaseyo segye)"
echo ""

echo "6. Emoji (uses font_ranges):"
echo "   😀 😃 😄 😁 😊 🙂 🙃 😉"
echo "   🚀 ⭐ 🌟 🔥 💻 📱 ⚡ 🎉"
echo ""

echo "7. Mathematical Symbols (uses font_ranges):"
echo "   ∀x∈ℝ: x²≥0"
echo "   ∫₀^∞ e^(-x²)dx = √π/2"
echo "   ∑ᵢ₌₁^n i = n(n+1)/2"
echo ""

echo "8. Box Drawing (uses font_ranges):"
echo "   ┌─┬─┐"
echo "   │ │ │"
echo "   ├─┼─┤"
echo "   │ │ │"
echo "   └─┴─┘"
echo ""

echo "9. Arrows and Symbols:"
echo "   ← → ↑ ↓ ⇐ ⇒ ⇑ ⇓"
echo "   ✓ ✗ ★ ☆ ♠ ♣ ♥ ♦"
echo ""

echo "10. OSC 8 Hyperlink (click with Ctrl+Click):"
printf "    Visit \e]8;;https://github.com/paulrobello/par-term\e\\GitHub\e]8;;\e\\ for more info\n"
echo ""

echo "=== Test Complete ==="
echo "All features above should render with appropriate fonts!"
```

Make it executable and run:
```bash
chmod +x test_fonts.sh
./test_fonts.sh
```

---

## Troubleshooting

### Problem: Styled fonts not working
```bash
# Check if fonts are installed
fc-list | grep -i "YourFontName"

# Verify font name spelling in config matches exactly
# Common mistake: "JetBrains Mono Bold" vs "JetBrainsMono-Bold"
```

### Problem: CJK characters showing boxes
```bash
# Install CJK fonts
sudo apt install fonts-noto-cjk  # Linux
# macOS usually has these built-in

# Verify font name
fc-list :lang=zh
fc-list :lang=ja
fc-list :lang=ko
```

### Problem: Emoji showing as text
```bash
# macOS: Should have "Apple Color Emoji" built-in
fc-list | grep -i emoji

# Linux: Install emoji font
sudo apt install fonts-noto-color-emoji

# Update font name in config to match installed font
```

### Problem: Not sure what's working
```bash
# Enable debug logging
RUST_LOG=debug par-term

# Look for font loading messages:
# "Successfully loaded primary font: ..."
# "Successfully loaded bold font: ..."
# "Loading range font for U+4E00-U+9FFF: ..."
```

---

## Next Steps

1. **See examples/** directory for complete configs
2. **Read examples/README.md** for comprehensive guide
3. **Check [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** for technical details
4. **Experiment!** Try different font combinations

---

## Pro Tips

1. **Start Simple:** Get styled fonts working first, then add ranges
2. **Test Each Range:** Add one font_range at a time and test
3. **Check Logs:** Use `RUST_LOG=info par-term` to see what fonts load
4. **Font Names:** Use exact names from `fc-list` output
5. **Mix Fonts:** You can use different fonts for italic (e.g., Victor Mono Italic with Fira Code regular)

---

Enjoy your beautifully rendered terminal! 🎉
