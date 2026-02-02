#!/bin/sh
# install_shaders.sh - Install par-term shaders from the latest release
#
# This script downloads the shaders.zip from the latest par-term release
# and extracts it to your config directory.
#
# Usage: ./install_shaders.sh
#
# Cross-platform compatible: macOS, Linux, Windows (Git Bash/WSL)

set -e

REPO="paulrobello/par-term"

# Detect OS and set config directory
detect_config_dir() {
    case "$(uname -s)" in
        Darwin)
            echo "$HOME/.config/par-term/shaders"
            ;;
        Linux)
            echo "${XDG_CONFIG_HOME:-$HOME/.config}/par-term/shaders"
            ;;
        MINGW*|MSYS*|CYGWIN*)
            echo "$APPDATA/par-term/shaders"
            ;;
        *)
            echo "$HOME/.config/par-term/shaders"
            ;;
    esac
}

CONFIG_DIR=$(detect_config_dir)
TEMP_DIR=$(mktemp -d)

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

echo "============================================="
echo "  par-term Shader Installer"
echo "============================================="
echo ""
echo "Target directory: $CONFIG_DIR"
echo ""

# Check for required tools
if ! command -v curl >/dev/null 2>&1 && ! command -v wget >/dev/null 2>&1; then
    echo "Error: curl or wget is required but not installed."
    exit 1
fi

if ! command -v unzip >/dev/null 2>&1; then
    echo "Error: unzip is required but not installed."
    exit 1
fi

# Warning about overwriting
if [ -d "$CONFIG_DIR" ] && [ "$(ls -A "$CONFIG_DIR" 2>/dev/null)" ]; then
    echo "WARNING: This will overwrite existing shaders in:"
    echo "  $CONFIG_DIR"
    echo ""
    printf "Do you want to continue? [y/N] "
    read -r response
    case "$response" in
        [yY][eE][sS]|[yY])
            echo ""
            ;;
        *)
            echo "Installation cancelled."
            exit 0
            ;;
    esac
fi

# Get the latest release download URL
echo "Fetching latest release information..."

if command -v curl >/dev/null 2>&1; then
    DOWNLOAD_URL=$(curl -sL "https://api.github.com/repos/$REPO/releases/latest" | \
        grep -o '"browser_download_url": *"[^"]*shaders\.zip"' | \
        sed 's/"browser_download_url": *"//' | sed 's/"$//')
else
    DOWNLOAD_URL=$(wget -qO- "https://api.github.com/repos/$REPO/releases/latest" | \
        grep -o '"browser_download_url": *"[^"]*shaders\.zip"' | \
        sed 's/"browser_download_url": *"//' | sed 's/"$//')
fi

if [ -z "$DOWNLOAD_URL" ]; then
    echo "Error: Could not find shaders.zip in the latest release."
    echo "Please check https://github.com/$REPO/releases"
    exit 1
fi

echo "Downloading shaders from: $DOWNLOAD_URL"
echo ""

# Download the zip file
if command -v curl >/dev/null 2>&1; then
    curl -L -o "$TEMP_DIR/shaders.zip" "$DOWNLOAD_URL"
else
    wget -O "$TEMP_DIR/shaders.zip" "$DOWNLOAD_URL"
fi

# Create config directory if it doesn't exist
mkdir -p "$CONFIG_DIR"

# Extract shaders (zip contains shaders/ folder, so extract to parent and let it create the dir)
echo ""
echo "Extracting shaders to $CONFIG_DIR..."
PARENT_DIR=$(dirname "$CONFIG_DIR")
unzip -o "$TEMP_DIR/shaders.zip" -d "$PARENT_DIR"

# Count installed shaders
SHADER_COUNT=$(find "$CONFIG_DIR" -name "*.glsl" -type f 2>/dev/null | wc -l | tr -d ' ')

echo ""
echo "============================================="
echo "  Installation complete!"
echo "============================================="
echo ""
echo "Installed $SHADER_COUNT shaders to:"
echo "  $CONFIG_DIR"
echo ""
echo "To use a shader, add to your config.yaml:"
echo "  custom_shader: \"shader_name.glsl\""
echo "  custom_shader_enabled: true"
echo ""
echo "For cursor shaders:"
echo "  cursor_shader: \"cursor_glow.glsl\""
echo "  cursor_shader_enabled: true"
echo ""
echo "See docs/SHADERS.md for the full shader gallery."
