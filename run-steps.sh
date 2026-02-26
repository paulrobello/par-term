#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

# ── Configuration (all overridable via env vars) ─────────────────────
SPEC_DIR="feature-spec"
STEP_TIMEOUT="${STEP_TIMEOUT:-1800}"    # 30 min per step
STEP_BUDGET="${STEP_BUDGET:-5}"         # $5 max per step
MAX_FAILURES="${MAX_FAILURES:-1}"       # stop on first failure (steps must run in order)
MAX_STEPS="${MAX_STEPS:-0}"             # 0 = unlimited
LOG_FILE="run-steps.log"
STEP_LOG_DIR="step-logs"
mkdir -p "$STEP_LOG_DIR"

# Allow nested Claude Code sessions
unset CLAUDECODE 2>/dev/null || true

# ── Portable timeout wrapper (macOS lacks GNU timeout) ─────────────
if command -v timeout &>/dev/null; then
    run_with_timeout() { timeout "$@"; }
elif command -v gtimeout &>/dev/null; then
    run_with_timeout() { gtimeout "$@"; }
else
    # Perl-based fallback (perl ships with macOS and most Linux)
    run_with_timeout() {
        local secs="$1"; shift
        perl -e '
            alarm(shift @ARGV);
            $SIG{ALRM} = sub { kill "TERM", $pid; exit 124 };
            $pid = fork // die "fork: $!";
            if ($pid == 0) { exec @ARGV or die "exec: $!" }
            waitpid($pid, 0);
            exit($? >> 8);
        ' "$secs" "$@"
    }
fi

# ── Stale lock check ────────────────────────────────────────────────
for lock in "$SPEC_DIR"/.step*.lock; do
    [[ -e "$lock" ]] || continue
    echo "WARNING: Found stale lock $lock from interrupted run ($(cat "$lock"))"
done

# ── Collect pending steps ────────────────────────────────────────────
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
echo "=== Config: STEP_TIMEOUT=${STEP_TIMEOUT}s  STEP_BUDGET=\$${STEP_BUDGET}  MAX_FAILURES=${MAX_FAILURES}  MAX_STEPS=${MAX_STEPS} ==="
echo ""

# ── Main loop ────────────────────────────────────────────────────────
fail_count=0
step_count=0
summary=()

for num in "${sorted[@]}"; do
    # Max steps guard
    if (( MAX_STEPS > 0 && step_count >= MAX_STEPS )); then
        echo ">>> Reached MAX_STEPS=${MAX_STEPS} — stopping"
        break
    fi

    stepfile="$SPEC_DIR/step${num}.md"
    donefile="$SPEC_DIR/step${num}-done.md"
    lockfile="$SPEC_DIR/.step${num}.lock"
    steplog="$STEP_LOG_DIR/step${num}.log"

    echo "=========================================="
    echo "  Running step ${num}: ${stepfile}"
    echo "  Log: ${steplog}"
    echo "=========================================="

    # Start fresh log for this step
    echo "=== Step ${num} started at $(date -Iseconds) ===" > "$steplog"

    # Write lock file
    echo "$(date -Iseconds)" > "$lockfile"

    step_start=$(date +%s)
    step_ok=false

    # Run Claude with timeout and budget cap; tee output to step log in real-time
    if run_with_timeout "$STEP_TIMEOUT" claude \
        --dangerously-skip-permissions \
        --max-budget-usd "$STEP_BUDGET" \
        -p "Read @feature-spec/step${num}.md and implement it. When done, run 'cargo fmt', 'cargo clippy -- -D warnings', and 'cargo build' to verify it compiles, is formatted, and passes lints. Fix any issues before finishing. Do NOT run 'make pre-commit' — the outer script handles that." \
        2>&1 | tee -a "$steplog"; then

        # Claude succeeded — auto-format then run pre-commit externally
        echo ">>> Claude exited OK for step ${num}. Running cargo fmt + make pre-commit..." | tee -a "$steplog"
        cargo fmt 2>&1 | tee -a "$steplog"
        if make pre-commit 2>&1 | tee -a "$steplog"; then
            mv "$stepfile" "$donefile"
            echo ">>> Renamed ${stepfile} -> ${donefile}" | tee -a "$steplog"
            step_ok=true
        else
            echo ">>> pre-commit FAILED for step ${num} — NOT marking done" | tee -a "$steplog"
        fi
    else
        ec=$?
        if (( ec == 124 )); then
            echo ">>> Step ${num} TIMED OUT after ${STEP_TIMEOUT}s — NOT marking done" | tee -a "$steplog"
        else
            echo ">>> Step ${num} FAILED (exit $ec) — NOT marking done" | tee -a "$steplog"
        fi
    fi

    # Timing
    elapsed=$(( $(date +%s) - step_start ))
    status="FAIL"
    if $step_ok; then
        status="OK"
        fail_count=0
    else
        fail_count=$((fail_count + 1))
    fi

    line="Step ${num}: ${status} (${elapsed}s)"
    summary+=("$line")
    echo ">>> $line" | tee -a "$LOG_FILE"

    # Remove lock on success
    if $step_ok; then
        rm -f "$lockfile"
    fi

    step_count=$((step_count + 1))
    echo ""

    # Max consecutive failures guard
    if (( fail_count >= MAX_FAILURES )); then
        echo ">>> ${MAX_FAILURES} consecutive failure(s) — stopping"
        break
    fi
done

# ── Summary ──────────────────────────────────────────────────────────
echo ""
echo "=========== Run Summary ==========="
for line in "${summary[@]+"${summary[@]}"}"; do
    echo "  $line"
done
echo "==================================="
echo "(also logged to $LOG_FILE)"
echo "=== Done ==="
