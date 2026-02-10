#!/bin/bash
# Test script for progress bar rendering (OSC 9;4 and OSC 934)
#
# Run this script inside par-term to test progress bar rendering.
# Progress bars appear as thin overlays at the top (or bottom) of the window.
#
# Protocol: OSC 9;4;state[;progress] ST
#   state 0 = hidden, 1 = normal, 2 = error, 3 = indeterminate, 4 = warning/paused
#   progress = 0-100 (for states 1, 2, 4)

# Helper function to set OSC 9;4 progress bar
progress() {
    local state="$1"
    local percent="${2:-}"
    if [ -n "$percent" ]; then
        printf '\033]9;4;%s;%s\033\\' "$state" "$percent"
    else
        printf '\033]9;4;%s\033\\' "$state"
    fi
}

# Helper function to set OSC 934 named progress bar
# Usage: named_progress set <id> [key=value ...]
#        named_progress remove <id>
#        named_progress remove_all
named_progress() {
    local action="$1"
    if [ "$action" = "remove_all" ]; then
        printf '\033]934;remove_all\033\\'
        return
    fi
    local id="$2"
    local params=""
    local i
    for i in "${@:3}"; do
        params="${params};${i}"
    done
    printf '\033]934;%s;%s%s\033\\' "$action" "$id" "$params"
}

# Ensure all progress bars are cleared on exit
cleanup() {
    progress 0
    printf '\033]934;remove_all\033\\'
}
trap cleanup EXIT

echo "=== par-term Progress Bar Test ==="
echo ""

# --- Test 1: Simple progress bar (OSC 9;4) ---
echo "Test 1: Normal progress bar (0% -> 100%)"
for i in $(seq 0 5 100); do
    progress 1 "$i"
    sleep 0.05
done
sleep 1

echo "Test 2: Error state at 100%"
progress 2 100
sleep 2

echo "Test 3: Indeterminate (animated)"
progress 3
sleep 3

echo "Test 4: Warning/paused state at 75%"
progress 4 75
sleep 2

echo "Test 5: Hide progress bar"
progress 0
sleep 1

# --- Test 2: Named progress bars (OSC 934) ---
echo ""
echo "Test 6: Named progress bars (concurrent)"
named_progress set "download" "percent=0" "label=Downloading file.tar.gz"
named_progress set "build" "state=indeterminate" "label=Compiling"
sleep 1

for i in $(seq 10 10 100); do
    named_progress set "download" "percent=$i" "label=Downloading file.tar.gz"
    sleep 0.2
done
sleep 0.5

named_progress set "build" "state=normal" "percent=50" "label=Compiling (50%)"
sleep 1
named_progress set "build" "percent=100" "label=Build complete"
sleep 1

echo "Test 7: Remove named progress bars"
named_progress remove "download"
sleep 1
named_progress remove_all
sleep 1

echo ""
echo "=== All tests complete ==="
