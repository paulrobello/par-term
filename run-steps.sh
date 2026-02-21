#!/usr/bin/env bash
set -euo pipefail

SPEC_DIR="feature-spec"
cd "$(dirname "$0")"

# Allow nested Claude Code sessions
unset CLAUDECODE 2>/dev/null || true

# Collect step files that are NOT done (exclude filenames containing done/complete/completed)
pending_steps=()
for f in "$SPEC_DIR"/step*.md; do
    basename="$(basename "$f")"
    # Skip files with done/complete/completed in the name (case-insensitive)
    if echo "$basename" | grep -iqE '(done|complete)'; then
        continue
    fi
    # Extract step number
    num=$(echo "$basename" | sed -E 's/^step([0-9]+)\.md$/\1/')
    if [[ -n "$num" ]]; then
        pending_steps+=("$num")
    fi
done

# Sort numerically
IFS=$'\n' sorted=($(printf '%s\n' "${pending_steps[@]}" | sort -n)); unset IFS

echo "=== Pending steps: ${sorted[*]} ==="
echo "=== Total: ${#sorted[@]} steps ==="
echo ""

for num in "${sorted[@]}"; do
    stepfile="$SPEC_DIR/step${num}.md"
    donefile="$SPEC_DIR/step${num}-done.md"

    echo "=========================================="
    echo "  Running step ${num}: ${stepfile}"
    echo "=========================================="

    claude --dangerously-skip-permissions -p "read @feature-spec/step${num}.md and implement it. When done, run 'make pre-commit' to verify everything compiles and passes."

    # Rename to done
    mv "$stepfile" "$donefile"
    echo ">>> Renamed ${stepfile} -> ${donefile}"
    echo ""
done

echo "=== All steps complete ==="
