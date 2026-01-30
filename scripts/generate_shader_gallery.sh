#!/bin/bash
# Generate a gallery of screenshots for all background shaders
# Usage: ./scripts/generate_shader_gallery.sh [-f|--force] [output_dir]
#
# Options:
#   -f, --force    Overwrite existing screenshots (default: skip existing)
#   output_dir     Output directory (default: shader-gallery)

set -e

# Parse arguments
FORCE=false
OUTPUT_DIR=""

while [[ $# -gt 0 ]]; do
    case $1 in
        -f|--force)
            FORCE=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [-f|--force] [output_dir]"
            echo ""
            echo "Options:"
            echo "  -f, --force    Overwrite existing screenshots (default: skip existing)"
            echo "  output_dir     Output directory (default: shader-gallery)"
            exit 0
            ;;
        *)
            OUTPUT_DIR="$1"
            shift
            ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SHADERS_DIR="$PROJECT_ROOT/shaders"
OUTPUT_DIR="${OUTPUT_DIR:-$PROJECT_ROOT/shader-gallery}"

# Build release version for better performance
echo "Building par-term in release mode..."
cargo build --release --manifest-path "$PROJECT_ROOT/Cargo.toml"

PAR_TERM="$PROJECT_ROOT/target/release/par-term"

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Get list of background shaders (exclude cursor_ prefix)
SHADERS=$(ls "$SHADERS_DIR"/*.glsl 2>/dev/null | xargs -n1 basename | grep -v "^cursor_" | sort)

if [ -z "$SHADERS" ]; then
    echo "No shaders found in $SHADERS_DIR"
    exit 1
fi

SHADER_COUNT=$(echo "$SHADERS" | wc -l | tr -d ' ')
echo "Found $SHADER_COUNT background shaders"
echo "Output directory: $OUTPUT_DIR"
echo ""

# Counter for progress
CURRENT=0
SKIPPED=0
GENERATED=0

for SHADER in $SHADERS; do
    CURRENT=$((CURRENT + 1))
    SHADER_NAME="${SHADER%.glsl}"
    OUTPUT_FILE="$OUTPUT_DIR/${SHADER_NAME}.png"

    # Skip if file exists and force is not enabled
    if [ -f "$OUTPUT_FILE" ] && [ "$FORCE" = false ]; then
        echo "[$CURRENT/$SHADER_COUNT] Skipping $SHADER_NAME (exists)"
        SKIPPED=$((SKIPPED + 1))
        continue
    fi

    echo "[$CURRENT/$SHADER_COUNT] Capturing $SHADER_NAME..."

    # Run par-term with the shader
    # - Command sent at 1 second
    # - Screenshot taken at exit_after - 1 = 4 seconds (gives time for shader + command output to render)
    # - Exit at 5 seconds
    # Enable info logging for screenshot debugging
    RUST_LOG=info "$PAR_TERM" \
        --shader "$SHADER" \
        --screenshot "$OUTPUT_FILE" \
        --exit-after 5 \
        --command-to-send "echo 'Shader: $SHADER_NAME'; neofetch 2>/dev/null || fastfetch 2>/dev/null || echo 'par-term shader demo'" \
        2>&1 | grep -iE "(screenshot|error|failed|warning)" || true

    if [ -f "$OUTPUT_FILE" ]; then
        echo "  Saved: $OUTPUT_FILE"
        GENERATED=$((GENERATED + 1))
    else
        echo "  Warning: Screenshot not created for $SHADER_NAME"
    fi
done

echo ""
echo "Gallery generation complete!"
echo "  Generated: $GENERATED"
echo "  Skipped:   $SKIPPED"
echo "  Total:     $SHADER_COUNT"
echo "Screenshots saved to: $OUTPUT_DIR"
if [ "$SKIPPED" -gt 0 ] && [ "$FORCE" = false ]; then
    echo ""
    echo "Tip: Use -f or --force to regenerate existing screenshots"
fi
echo ""

# Generate a simple HTML index if there are screenshots
if ls "$OUTPUT_DIR"/*.png &>/dev/null; then
    INDEX_FILE="$OUTPUT_DIR/index.html"
    echo "Generating HTML index..."

    cat > "$INDEX_FILE" << 'HTMLHEAD'
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>par-term Shader Gallery</title>
    <style>
        * { box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: #1a1a2e;
            color: #eee;
            margin: 0;
            padding: 20px;
        }
        h1 {
            text-align: center;
            color: #00d4ff;
            margin-bottom: 30px;
        }
        .gallery {
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(400px, 1fr));
            gap: 20px;
            max-width: 1600px;
            margin: 0 auto;
        }
        .shader-card {
            background: #16213e;
            border-radius: 12px;
            overflow: hidden;
            box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
            transition: transform 0.2s, box-shadow 0.2s;
        }
        .shader-card:hover {
            transform: translateY(-5px);
            box-shadow: 0 8px 30px rgba(0, 212, 255, 0.2);
        }
        .shader-card img {
            width: 100%;
            height: auto;
            display: block;
        }
        .shader-name {
            padding: 15px;
            text-align: center;
            font-weight: 500;
            color: #00d4ff;
            background: #0f0f23;
        }
    </style>
</head>
<body>
    <h1>par-term Shader Gallery</h1>
    <div class="gallery">
HTMLHEAD

    for PNG in "$OUTPUT_DIR"/*.png; do
        BASENAME=$(basename "$PNG")
        SHADER_NAME="${BASENAME%.png}"
        echo "        <div class=\"shader-card\">" >> "$INDEX_FILE"
        echo "            <img src=\"$BASENAME\" alt=\"$SHADER_NAME\" loading=\"lazy\">" >> "$INDEX_FILE"
        echo "            <div class=\"shader-name\">$SHADER_NAME</div>" >> "$INDEX_FILE"
        echo "        </div>" >> "$INDEX_FILE"
    done

    cat >> "$INDEX_FILE" << 'HTMLFOOT'
    </div>
</body>
</html>
HTMLFOOT

    echo "HTML index created: $INDEX_FILE"
fi
