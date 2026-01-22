#!/bin/bash
# Performance benchmark script for par-term text shaping
# Tests rendering performance with various content types

set -euo pipefail

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Configuration
LINES_PER_TEST=1000
WARMUP_LINES=100

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  par-term Text Shaping Performance Benchmark                ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo -e "${YELLOW}NOTE: This benchmark should be run inside par-term${NC}"
echo -e "${YELLOW}      Compare results with enable_text_shaping: true vs false${NC}"
echo ""

# Function to print a separator
separator() {
    echo "────────────────────────────────────────────────────────"
}

# Function to run a benchmark
run_benchmark() {
    local test_name="$1"
    local content="$2"
    local lines="$3"

    echo -e "${BLUE}Running: $test_name${NC}"
    separator

    # Warmup
    for ((i=1; i<=WARMUP_LINES; i++)); do
        echo "$content" > /dev/null
    done

    # Actual test
    local start_time=$(date +%s%N)
    for ((i=1; i<=lines; i++)); do
        echo "$content"
    done
    local end_time=$(date +%s%N)

    # Calculate elapsed time
    local elapsed_ns=$((end_time - start_time))
    local elapsed_ms=$((elapsed_ns / 1000000))
    local lines_per_sec=$((lines * 1000000000 / elapsed_ns))

    echo ""
    echo -e "${GREEN}Results:${NC}"
    echo "  Lines rendered: $lines"
    echo "  Time elapsed: ${elapsed_ms}ms"
    echo "  Throughput: ${lines_per_sec} lines/sec"
    echo ""
}

# Test 1: Pure ASCII
echo ""
echo -e "${BLUE}═══ Test 1: Pure ASCII (Baseline) ═══${NC}"
run_benchmark "ASCII text" \
    "The quick brown fox jumps over the lazy dog. ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789" \
    "$LINES_PER_TEST"

# Test 2: Mixed ASCII with symbols
echo -e "${BLUE}═══ Test 2: ASCII with Symbols ═══${NC}"
run_benchmark "ASCII + symbols" \
    "foo->bar, x=>y, if (x != y && z >= 0) { return a + b; }" \
    "$LINES_PER_TEST"

# Test 3: CJK Characters
echo -e "${BLUE}═══ Test 3: CJK Characters ═══${NC}"
run_benchmark "CJK text" \
    "中文字体 日本語フォント 한국어 글꼴 混合内容 Mixed CJK and English 漢字" \
    "$LINES_PER_TEST"

# Test 4: Emoji (simple)
echo -e "${BLUE}═══ Test 4: Simple Emoji ═══${NC}"
run_benchmark "Simple emoji" \
    "😀 😃 😄 😁 😊 🙂 🚀 ⭐ 🌟 🔥 💻 📱 ⚡ 🎉 ❤️ 👍" \
    "$LINES_PER_TEST"

# Test 5: Emoji with skin tones (complex graphemes)
echo -e "${BLUE}═══ Test 5: Emoji with Skin Tones ═══${NC}"
run_benchmark "Emoji + skin tones" \
    "👋🏻 👋🏼 👋🏽 👋🏾 👋🏿 👍🏻 👍🏼 👍🏽 👍🏾 👍🏿 ✊🏻 ✊🏼 ✊🏽 ✊🏾 ✊🏿" \
    "$LINES_PER_TEST"

# Test 6: Flag emoji (Regional Indicators)
echo -e "${BLUE}═══ Test 6: Flag Emoji (Regional Indicators) ═══${NC}"
run_benchmark "Flag emoji" \
    "🇺🇸 🇬🇧 🇯🇵 🇩🇪 🇫🇷 🇨🇦 🇦🇺 🇧🇷 🇮🇳 🇨🇳 🇰🇷 🇮🇹 🇪🇸 🇷🇺" \
    "$LINES_PER_TEST"

# Test 7: ZWJ Sequences
echo -e "${BLUE}═══ Test 7: ZWJ Sequences ═══${NC}"
run_benchmark "ZWJ emoji" \
    "👨‍👩‍👧‍👦 👨‍👩‍👧 👨‍💻 👩‍💻 👨‍🚀 👩‍🚀 👁️‍🗨️ 🏴‍☠️ 👨🏽‍💻 👩🏾‍🚀" \
    "$LINES_PER_TEST"

# Test 8: Arabic (RTL + contextual shaping)
echo -e "${BLUE}═══ Test 8: Arabic Text (RTL + Contextual) ═══${NC}"
run_benchmark "Arabic script" \
    "مرحبا السلام عليكم العربية مرحبا بك في العالم اللغة العربية" \
    "$LINES_PER_TEST"

# Test 9: Devanagari (complex ligatures)
echo -e "${BLUE}═══ Test 9: Devanagari Text (Complex Ligatures) ═══${NC}"
run_benchmark "Devanagari script" \
    "नमस्ते धन्यवाद स्वागत देवनागरी हिन्दी भाषा संस्कृत" \
    "$LINES_PER_TEST"

# Test 10: Thai (non-spacing marks)
echo -e "${BLUE}═══ Test 10: Thai Text (Non-spacing Marks) ═══${NC}"
run_benchmark "Thai script" \
    "สวัสดี ขอบคุณ ภาษาไทย ยินดีต้อนรับ สบายดีไหม" \
    "$LINES_PER_TEST"

# Test 11: Mixed content (stress test)
echo -e "${BLUE}═══ Test 11: Mixed Content (Stress Test) ═══${NC}"
run_benchmark "Mixed content" \
    "Hello مرحبا 你好 😀👍🏽 नमस्ते ∀x∈ℝ ┌─┐ -> => fi fl 🇺🇸🇬🇧🇯🇵" \
    "$LINES_PER_TEST"

# Test 12: Heavy emoji lines
echo -e "${BLUE}═══ Test 12: Heavy Emoji Load ═══${NC}"
run_benchmark "Heavy emoji" \
    "😀😃😄😁😊🙂🙃😉😍🥰😘😗😙😚😋😛😝😜🤪🤨🧐🤓😎🥸🤩🥳😏😒😞😔😟😕🙁☹️😣😖😫😩🥺😢😭😤😠😡🤬🤯😳🥵🥶" \
    "$LINES_PER_TEST"

# Test 13: Combining diacritics
echo -e "${BLUE}═══ Test 13: Combining Diacritics ═══${NC}"
run_benchmark "Combining diacritics" \
    "á é í ó ú à è ì ò ù â ê î ô û ã õ ñ ä ë ï ö ü Tiếng Việt Xin chào" \
    "$LINES_PER_TEST"

# Test 14: Wide character mix
echo -e "${BLUE}═══ Test 14: Wide Character Mix ═══${NC}"
run_benchmark "Wide chars" \
    "中文ABC日本語123한국어456漢字789😀🚀⭐" \
    "$LINES_PER_TEST"

# Summary
echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  Benchmark Complete                                          ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo -e "${GREEN}All benchmarks completed!${NC}"
echo ""
echo "Performance Analysis:"
echo "  1. ASCII baseline: Should be fastest (simple glyphs)"
echo "  2. CJK: May be slower (wide chars, fallback fonts)"
echo "  3. Simple emoji: Moderate (color glyphs)"
echo "  4. Emoji + skin tones: Tests multi-codepoint grapheme cache"
echo "  5. Flags: Tests Regional Indicator shaping"
echo "  6. ZWJ sequences: Most complex (multi-component graphemes)"
echo "  7. Arabic/RTL: Tests contextual shaping + BiDi"
echo "  8. Devanagari/Thai: Tests complex ligature formation"
echo "  9. Mixed content: Real-world stress test"
echo ""
echo "Comparison Instructions:"
echo "  1. Run benchmark with enable_text_shaping: true"
echo "  2. Edit ~/.config/par-term/config.yaml"
echo "  3. Set enable_text_shaping: false"
echo "  4. Reload config (F5) or restart par-term"
echo "  5. Run benchmark again"
echo "  6. Compare throughput (lines/sec) for each test"
echo ""
echo "Expected Results:"
echo "  - ASCII: Minimal difference (simple caching either way)"
echo "  - CJK: Similar performance (direct glyph lookup)"
echo "  - Complex scripts: Shaped rendering may be slightly slower"
echo "  - Multi-component emoji: Shaped rendering much better quality"
echo "  - Overall: Text shaping overhead should be minimal (<10%)"
echo "  - Cache warmup: Second run should be faster (LRU cache)"
echo ""
echo "Debug Info:"
echo "  - Check RUST_LOG=debug output for cache hit rates"
echo "  - Monitor 'PERF:' log entries for timing details"
echo "  - Use 'grep \"shaped\" | grep \"cache\"' to analyze caching"
echo ""
