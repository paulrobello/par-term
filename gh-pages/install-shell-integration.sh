#!/bin/sh
# install-shell-integration.sh - Install par-term shell integration
#
# This script downloads and installs shell integration for par-term.
# It provides directory tracking, command notifications, and CWD sync.
#
# Usage: curl -sSL https://paulrobello.github.io/par-term/install-shell-integration.sh | sh
#
# Cross-platform compatible: macOS, Linux, Windows (Git Bash/WSL)

set -e

REPO="paulrobello/par-term"
BRANCH="main"
BASE_URL="https://raw.githubusercontent.com/$REPO/$BRANCH/shell_integration"

# Markers for RC file updates
MARKER_START="# >>> par-term shell integration >>>"
MARKER_END="# <<< par-term shell integration <<<"

# Colors for output (if terminal supports it)
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    CYAN='\033[0;36m'
    NC='\033[0m' # No Color
else
    RED=''
    GREEN=''
    YELLOW=''
    CYAN=''
    NC=''
fi

# Detect OS and set config directory
detect_config_dir() {
    case "$(uname -s)" in
        Darwin)
            echo "$HOME/.config/par-term"
            ;;
        Linux)
            echo "${XDG_CONFIG_HOME:-$HOME/.config}/par-term"
            ;;
        MINGW*|MSYS*|CYGWIN*)
            echo "$APPDATA/par-term"
            ;;
        *)
            echo "$HOME/.config/par-term"
            ;;
    esac
}

# Detect shell from $SHELL environment variable
detect_shell() {
    case "$(basename "$SHELL")" in
        bash) echo "bash" ;;
        zsh)  echo "zsh" ;;
        fish) echo "fish" ;;
        *)    echo "unknown" ;;
    esac
}

# Get the RC file for a shell
get_rc_file() {
    shell="$1"
    case "$shell" in
        bash)
            # Prefer .bashrc for interactive shells, but check for .bash_profile on macOS
            if [ -f "$HOME/.bashrc" ]; then
                echo "$HOME/.bashrc"
            elif [ -f "$HOME/.bash_profile" ]; then
                echo "$HOME/.bash_profile"
            else
                echo "$HOME/.bashrc"
            fi
            ;;
        zsh)
            echo "$HOME/.zshrc"
            ;;
        fish)
            echo "$HOME/.config/fish/config.fish"
            ;;
        *)
            echo ""
            ;;
    esac
}

# Get the script filename for a shell
get_script_name() {
    shell="$1"
    echo "par_term_shell_integration.$shell"
}

# Download a file using curl or wget
download_file() {
    url="$1"
    dest="$2"

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL -o "$dest" "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$dest" "$url"
    else
        printf "${RED}Error: curl or wget is required but not installed.${NC}\n"
        exit 1
    fi
}

# Generate the source block content for an RC file
get_source_line() {
    shell="$1"
    script_path="$2"
    bin_dir="$3"

    case "$shell" in
        fish)
            printf 'if test -d "%s"\n    set -gx PATH "%s" $PATH\nend\nsource "%s"' "$bin_dir" "$bin_dir" "$script_path"
            ;;
        *)
            printf 'if [ -d "%s" ]; then\n    export PATH="%s:$PATH"\nfi\n[ -f "%s" ] && source "%s"' "$bin_dir" "$bin_dir" "$script_path" "$script_path"
            ;;
    esac
}

# Remove existing integration block from RC file
remove_existing_block() {
    rc_file="$1"

    if [ ! -f "$rc_file" ]; then
        return
    fi

    if grep -q "$MARKER_START" "$rc_file" 2>/dev/null; then
        # Create a temp file and remove the block
        temp_file=$(mktemp)
        awk -v start="$MARKER_START" -v end="$MARKER_END" '
            $0 ~ start { skip=1; next }
            $0 ~ end { skip=0; next }
            !skip { print }
        ' "$rc_file" > "$temp_file"
        mv "$temp_file" "$rc_file"
    fi
}

# Add integration block to RC file
add_integration_block() {
    rc_file="$1"
    source_line="$2"

    # Ensure RC file exists
    touch "$rc_file"

    # Add the block
    printf "\n%s\n%s\n%s\n" "$MARKER_START" "$source_line" "$MARKER_END" >> "$rc_file"
}

# Main installation
main() {
    CONFIG_DIR=$(detect_config_dir)
    SHELL_TYPE=$(detect_shell)

    echo "============================================="
    echo "  par-term Shell Integration Installer"
    echo "============================================="
    echo ""

    if [ "$SHELL_TYPE" = "unknown" ]; then
        printf "${RED}Error: Unsupported shell: %s${NC}\n" "$(basename "$SHELL")"
        echo "Supported shells: bash, zsh, fish"
        exit 1
    fi

    SCRIPT_NAME=$(get_script_name "$SHELL_TYPE")
    RC_FILE=$(get_rc_file "$SHELL_TYPE")
    SCRIPT_PATH="$CONFIG_DIR/$SCRIPT_NAME"

    printf "Detected shell: ${CYAN}%s${NC}\n" "$SHELL_TYPE"
    printf "Config directory: ${CYAN}%s${NC}\n" "$CONFIG_DIR"
    printf "RC file: ${CYAN}%s${NC}\n" "$RC_FILE"
    echo ""

    # Create config directory
    mkdir -p "$CONFIG_DIR"

    # For fish, ensure config directory exists
    if [ "$SHELL_TYPE" = "fish" ]; then
        mkdir -p "$(dirname "$RC_FILE")"
    fi

    # Download the shell integration script
    printf "Downloading shell integration script...\n"
    DOWNLOAD_URL="$BASE_URL/$SCRIPT_NAME"
    download_file "$DOWNLOAD_URL" "$SCRIPT_PATH"
    chmod +x "$SCRIPT_PATH"
    printf "${GREEN}Downloaded:${NC} %s\n" "$SCRIPT_PATH"
    echo ""

    # Download file transfer utilities
    BIN_DIR="$CONFIG_DIR/bin"
    mkdir -p "$BIN_DIR"
    printf "Downloading file transfer utilities...\n"
    for util in pt-dl pt-ul pt-imgcat; do
        UTIL_URL="$BASE_URL/$util"
        download_file "$UTIL_URL" "$BIN_DIR/$util"
        chmod +x "$BIN_DIR/$util"
        printf "${GREEN}Downloaded:${NC} %s\n" "$BIN_DIR/$util"
    done
    echo ""

    # Update RC file
    printf "Updating RC file...\n"
    SOURCE_LINE=$(get_source_line "$SHELL_TYPE" "$SCRIPT_PATH" "$BIN_DIR")

    # Remove any existing block first
    remove_existing_block "$RC_FILE"

    # Add new block
    add_integration_block "$RC_FILE" "$SOURCE_LINE"
    printf "${GREEN}Updated:${NC} %s\n" "$RC_FILE"

    echo ""
    echo "============================================="
    printf "  ${GREEN}Installation complete!${NC}\n"
    echo "============================================="
    echo ""
    echo "Shell integration provides:"
    echo "  - Directory tracking (OSC 7)"
    echo "  - Command notifications (OSC 777)"
    echo "  - Current working directory sync"
    echo "  - File transfer utilities (pt-dl, pt-ul, pt-imgcat)"
    echo ""
    printf "${YELLOW}Restart your shell or run:${NC}\n"
    case "$SHELL_TYPE" in
        fish)
            echo "  source $RC_FILE"
            ;;
        *)
            echo "  source $RC_FILE"
            ;;
    esac
    echo ""
}

main "$@"
