#!/usr/bin/env bash
# with-local-core.sh — run a cargo command against the LOCAL par-term-emu-core-rust
# checkout, auto-reverting every manifest it touched on exit (success or failure).
#
# Use it while developing against unpublished core changes. Commits must
# still build against the published pin, so the par-term side of such work
# lands only after the core release it needs is on crates.io.
#
#   1. Resolves the local core checkout: $PAR_TERM_LOCAL_CORE_DIR, else
#      ../par-term-emu-core-rust relative to this repo — and, from a linked
#      worktree (which lives under .claude/worktrees/ and has no such sibling),
#      relative to the MAIN checkout.
#   2. Raises the workspace version pin when the local core is beyond it
#      (prerelease builds are pinned exactly, since a caret pin does not match
#      a prerelease).
#   3. Adds [patch.crates-io] par-term-emu-core-rust = { path = <core> } to the
#      root Cargo.toml (skipped if already present).
#   4. Stages the core's par-mux daemon binary next to every built par-term
#      binary (the core's spawn looks next to current_exe), refreshing the
#      copy every run.
#   5. Runs the command, then restores root Cargo.toml and Cargo.lock from
#      pre-run backups. NEVER commit
#      the patched state: CI checks out only this repo, so a committed
#      patch fails every build leg.
#
# Usage:
#   scripts/with-local-core.sh                    # cargo check --workspace
#   scripts/with-local-core.sh cargo check --workspace
#   make with-local-core                          # same as the first form
#   make with-local-core CMD="cargo check --workspace"
set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
ROOT_MANIFEST="$REPO_ROOT/Cargo.toml"
LOCKFILE="$REPO_ROOT/Cargo.lock"
PATCH_LINE_PREFIX='par-term-emu-core-rust = { path ='
die() { echo "with-local-core: $*" >&2; exit 1; }

# --- resolve the local core checkout -----------------------------------------
if [ -n "${PAR_TERM_LOCAL_CORE_DIR:-}" ]; then
  CORE_DIR=$PAR_TERM_LOCAL_CORE_DIR
elif [ -d "$REPO_ROOT/../par-term-emu-core-rust" ]; then
  CORE_DIR="$REPO_ROOT/../par-term-emu-core-rust"
else
  main_root=$(git -C "$REPO_ROOT" worktree list --porcelain | head -1 | cut -d' ' -f2)
  CORE_DIR="$main_root/../par-term-emu-core-rust"
fi
[ -f "$CORE_DIR/Cargo.toml" ] ||
  die "no Cargo.toml under $CORE_DIR — clone par-term-emu-core-rust next to this repo (or set PAR_TERM_LOCAL_CORE_DIR)"

# --- read the two versions ----------------------------------------------------
local_ver=$(grep -m1 '^version = ' "$CORE_DIR/Cargo.toml" | sed 's/^version = "\(.*\)"$/\1/')
[ -n "$local_ver" ] || die "could not read the version from $CORE_DIR/Cargo.toml"
pin_line=$(grep -m1 '^par-term-emu-core-rust = { version = "' "$ROOT_MANIFEST") ||
  die "could not find the par-term-emu-core-rust workspace pin in $ROOT_MANIFEST"
pin=$(printf '%s' "$pin_line" | sed 's/.*version = "\([^"]*\)".*/\1/')

# A caret pin "0.50" means >=0.50.0 <0.51.0, which a prerelease local build does
# NOT satisfy — pin prereleases exactly so the patched path resolves.
case "$local_ver" in
  *-*) wanted_pin="=$local_ver" ;;
  *) wanted_pin="${local_ver%%.*}.$(printf '%s' "$local_ver" | cut -d. -f2)" ;;
esac

# --- backups + restore trap -----------------------------------------------------
BACKUP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/with-local-core.XXXXXX")
cp "$ROOT_MANIFEST" "$BACKUP_DIR/root.toml"
had_lock=no
if [ -f "$LOCKFILE" ]; then cp "$LOCKFILE" "$BACKUP_DIR/Cargo.lock"; had_lock=yes; fi

restore() {
  cp "$BACKUP_DIR/root.toml" "$ROOT_MANIFEST"
  if [ "$had_lock" = yes ]; then cp "$BACKUP_DIR/Cargo.lock" "$LOCKFILE"; fi
  rm -rf "$BACKUP_DIR"
  echo "with-local-core: restored root Cargo.toml and Cargo.lock"
}
trap restore EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

if ! git -C "$REPO_ROOT" diff --quiet -- Cargo.toml Cargo.lock 2>/dev/null; then
  echo "with-local-core: note — manifests were already dirty before the run; the pre-run state (not HEAD) is restored"
fi

# Replace the first exact-line match of $2 with $3 in $1 (awk string compare,
# so version/regex metacharacters in the line cannot misfire).
replace_line() {
  local out
  out=$(mktemp "${TMPDIR:-/tmp}/with-local-core.XXXXXX")
  awk -v old="$2" -v new="$3" '!done && $0 == old { print new; done = 1; next } { print }' "$1" > "$out"
  mv "$out" "$1"
}

# --- apply the vendoring edits ---------------------------------------------------
if [ "$pin" != "$wanted_pin" ]; then
  replace_line "$ROOT_MANIFEST" "$pin_line" "$(printf '%s' "$pin_line" | sed "s/version = \"$pin\"/version = \"$wanted_pin\"/")"
  echo "with-local-core: local core $local_ver is beyond workspace pin \"$pin\" — raised pin to \"$wanted_pin\" for this run (restored afterwards; to make it permanent, edit the par-term-emu-core-rust line in [workspace.dependencies])"
fi

patch_line="$PATCH_LINE_PREFIX \"$CORE_DIR\" }"
if grep -qF "$PATCH_LINE_PREFIX" "$ROOT_MANIFEST"; then
  echo "with-local-core: [patch.crates-io] already points par-term-emu-core-rust at a local path — leaving it in place"
else
  if grep -q '^\[patch\.crates-io\]' "$ROOT_MANIFEST"; then
    out=$(mktemp "${TMPDIR:-/tmp}/with-local-core.XXXXXX")
    awk -v ins="$patch_line" '/^\[patch\.crates-io\]/ && !done { print; print ins; done = 1; next } { print }' "$ROOT_MANIFEST" > "$out"
    mv "$out" "$ROOT_MANIFEST"
  else
    printf '\n[patch.crates-io]\n%s\n' "$patch_line" >> "$ROOT_MANIFEST"
  fi
  echo "with-local-core: patched $ROOT_MANIFEST -> $patch_line"
fi

# --- stage the par-mux daemon next to the app binary -----------------------------
# The core's spawn looks for `par-mux` next to the RUNNING executable (walking
# out of deps/), so a vendored run must stage the daemon from the core's own
# target dir or every mux attach fails with the daemon binary missing. The
# copy refreshes every run; a missing core binary is a warning, not a failure
# — the attach surfaces its own error.
#
# A daemon ALREADY RUNNING owns its sockets regardless of what is staged
# here: attaches go to the socket owner, so a daemon left over from an
# older build silently serves this run's daemon-side fixes as absent (the
# stale-daemon trap). Surface it — the app's attach now toasts a version
# mismatch too, but the operator should know before launching.
# ps + grep with the [p] bracket trick rather than pgrep: pgrep
# false-negatives on macOS for path-shaped patterns.
if ps -ax -o command= 2>/dev/null | grep -q '[/]par-mux --socket'; then
  echo "with-local-core: WARNING — a par-mux daemon is already running; it owns the mux sockets, so the freshly staged daemon will NOT be used (daemon-side changes will read as unfixed). Kill it first: pkill -f par-mux" >&2
fi
DAEMON_SRC=""
if [ -x "$CORE_DIR/target/release/par-mux" ] && { [ ! -x "$CORE_DIR/target/debug/par-mux" ] || [ "$CORE_DIR/target/release/par-mux" -nt "$CORE_DIR/target/debug/par-mux" ]; }; then
  DAEMON_SRC="$CORE_DIR/target/release/par-mux"
elif [ -x "$CORE_DIR/target/debug/par-mux" ]; then
  DAEMON_SRC="$CORE_DIR/target/debug/par-mux"
fi
if [ -z "$DAEMON_SRC" ]; then
  echo "with-local-core: WARNING — no par-mux daemon binary in $CORE_DIR/target (release or debug); build it (cargo build --no-default-features --features mux --bin par-mux) or mux attach will fail" >&2
else
  if [ -n "$(find "$CORE_DIR/src" -type f -newer "$DAEMON_SRC" -print -quit 2>/dev/null)" ]; then
    echo "with-local-core: WARNING — staged daemon $DAEMON_SRC is older than the core's newest source; rebuild it or daemon-side fixes will read as absent" >&2
  fi
  staged=no
  for dir in "$REPO_ROOT"/target/*/; do
    [ -x "${dir}par-term" ] || continue
    cp -f "$DAEMON_SRC" "${dir}par-mux"
    staged=yes
  done
  if [ "$staged" = no ]; then
    mkdir -p "$REPO_ROOT/target/debug"
    cp -f "$DAEMON_SRC" "$REPO_ROOT/target/debug/par-mux"
  fi
  echo "with-local-core: staged par-mux daemon from $DAEMON_SRC next to the app binary"
fi

# --- run ------------------------------------------------------------------------
cd "$REPO_ROOT"
if [ "$#" -gt 0 ]; then
  "$@"
else
  cargo check --workspace
fi
