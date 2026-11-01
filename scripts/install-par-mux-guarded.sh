#!/usr/bin/env bash
#
# Copy a freshly built par-mux binary over the installed one only when the
# candidate is NEWER. The install target used to copy unconditionally, which
# silently downgraded a newer daemon installed from the core repo (card
# 01a19784ccc57052a5a034d3d5abc892).
#
# Usage: install-par-mux-guarded.sh <candidate-binary> <installed-binary>
#
# Semver-aware: compares major.minor.patch; build metadata and pre-release
# suffixes are stripped before comparing, so 0.76.0+abc == 0.76.0-rc.1 ==
# 0.76.0 for this purpose. Never mtime-based — build order does not imply
# version order. A version probe that fails or parses as nothing means
# "unknown": the installed binary is never clobbered on unknown.

set -euo pipefail

CANDIDATE=${1:?usage: install-par-mux-guarded.sh <candidate> <installed>}
DEST=${2:?usage: install-par-mux-guarded.sh <candidate> <installed>}

semver() {
  # Print "major minor patch" for inputs like "par-mux 0.76.0+abc",
  # "0.77.0-rc.1", or plain "0.76.0"; print nothing when unparseable.
  python3 - "$1" <<'PY'
import re, sys
m = re.search(r"(\d+)\.(\d+)\.(\d+)", sys.argv[1])
if m:
    print(m.group(1), m.group(2), m.group(3))
PY
}

[ -x "$CANDIDATE" ] || { echo "::error::candidate $CANDIDATE is not executable" >&2; exit 1; }
CAND_VER=$("$CANDIDATE" --version 2>/dev/null || true)

if [ ! -e "$DEST" ]; then
  echo "par-mux: no installed binary found; installing candidate ${CAND_VER:-<unknown>}"
else
  DEST_VER=$("$DEST" --version 2>/dev/null || true)
  if [ -z "$DEST_VER" ]; then
    echo "par-mux: installed binary reported no version; leaving it in place"
    exit 0
  fi
  # Empty output leaves the fields empty; read's EOF status is fine here.
  read -r CM CC CP _ <<EOF || true
$(semver "$CAND_VER")
EOF
  read -r DM DC DP _ <<EOF || true
$(semver "$DEST_VER")
EOF
  if [ -z "$CM" ] || [ -z "$DM" ]; then
    echo "par-mux: could not parse versions (candidate '${CAND_VER}', installed '${DEST_VER}'); leaving installed binary in place"
    exit 0
  fi
  NEWER=$(( CM > DM || (CM == DM && CC > DC) || (CM == DM && CC == DC && CP > DP) ))
  if [ "$NEWER" -eq 0 ]; then
    echo "par-mux $DEST_VER already installed (>= candidate $CAND_VER); keeping installed binary"
    exit 0
  fi
  echo "par-mux: upgrading installed $DEST_VER -> candidate $CAND_VER"
fi

# Stage to a sibling temp name and mv -f over the destination: writing onto
# a RUNNING binary's inode can SIGKILL it on macOS; mv replaces the directory
# entry instead.
TMP="$DEST.tmp.$$"
cp "$CANDIDATE" "$TMP"
chmod 755 "$TMP"
mv -f "$TMP" "$DEST"
