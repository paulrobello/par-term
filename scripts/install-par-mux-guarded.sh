#!/usr/bin/env bash
#
# Copy a freshly built par-mux binary over the installed one only when the
# candidate is NEWER. The install target used to copy unconditionally, which
# silently downgraded a newer daemon installed from the core repo (card
# 01a19784ccc57052a5a034d3d5abc892).
#
# macOS: the installed daemon is Developer-ID-signed before it lands, never
# ad-hoc — TCC/Local Network grants bind to the code signature, and an ad-hoc
# signature re-mints per build, so every reinstall silently drops grants
# (incident 2026-11-20, core card 01a1faf7f2b9). An installed binary that is
# newer-but-ad-hoc is re-signed in place; with no identity in the keychain
# the newer ad-hoc binary is kept as-is.
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
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

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

properly_signed() {
  # 0 = signed with a real team identifier; 1 = ad-hoc, unsigned, or unknown.
  # Non-macOS has no codesign: report signed so resign logic never triggers.
  if [ "$(uname)" != "Darwin" ]; then return 0; fi
  codesign -dv "$1" 2>&1 >/dev/null | grep '^TeamIdentifier=' | grep -qv 'not set'
}

have_dev_id() {
  security find-identity -v -p codesigning 2>/dev/null | grep -q '"Developer ID Application:'
}

[ -x "$CANDIDATE" ] || { echo "::error::candidate $CANDIDATE is not executable" >&2; exit 1; }
CAND_VER=$("$CANDIDATE" --version 2>/dev/null || true)

if [ ! -e "$DEST" ]; then
  echo "par-mux: no installed binary found; installing candidate ${CAND_VER:-<unknown>}"
  ACTION=replace
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
  if properly_signed "$DEST"; then
    # Original version-guard behavior: only a strictly newer candidate replaces.
    if [ "$NEWER" -eq 0 ]; then
      echo "par-mux $DEST_VER already installed (>= candidate $CAND_VER); keeping installed binary"
      exit 0
    fi
    echo "par-mux: upgrading installed $DEST_VER -> candidate $CAND_VER"
    ACTION=replace
  elif [ "$NEWER" -eq 1 ]; then
    # Ad-hoc or unsigned install, but the candidate wins on version: replace
    # it, and the staging step Developer-ID-signs the replacement.
    echo "par-mux: upgrading installed $DEST_VER -> candidate $CAND_VER; replacement will be Developer ID signed"
    ACTION=replace
  elif have_dev_id; then
    # Installed >= candidate but ad-hoc-signed: re-sign it in place so
    # TCC/Local Network attribution sticks across restarts.
    echo "par-mux $DEST_VER (>= candidate $CAND_VER) is ad-hoc-signed; re-signing it in place"
    ACTION=resign
  else
    # No identity to sign with: keep the newer ad-hoc binary as-is.
    echo "par-mux $DEST_VER (>= candidate $CAND_VER) is ad-hoc-signed and no Developer ID identity is available; keeping the installed binary"
    exit 0
  fi
fi

# Stage to a sibling temp name and mv -f over the destination: writing onto
# a RUNNING binary's inode can SIGKILL it on macOS; mv replaces the directory
# entry instead.
TMP="$DEST.tmp.$$"
if [ "$ACTION" = resign ]; then
  cp "$DEST" "$TMP"
else
  cp "$CANDIDATE" "$TMP"
fi
chmod 755 "$TMP"
if [ "$(uname)" = "Darwin" ]; then
  # Exits 1 on codesign failure, which fails the install under set -e —
  # intended. Exits 0 with a loud warning when no Developer ID identity
  # exists, leaving the staged binary ad-hoc.
  "$SCRIPT_DIR/sign-installed-artifact.sh" par-mux "$TMP"
fi
mv -f "$TMP" "$DEST"
