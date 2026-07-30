#!/usr/bin/env bash
#
# Write a `<asset>.sha256` next to every release asset in a directory.
#
# Usage: scripts/release-checksums.sh <asset-dir>
#
# The names produced here are the ones par-term-update's
# `get_checksum_asset_name()` looks for — `<platform asset>.sha256` — and the
# bare-hash body is what its `parse_checksum_file()` reads. A release without
# these files makes every self-update abort with "refusing to install an
# unverified binary", so an empty directory is a failure, not a no-op.

set -euo pipefail

ASSET_DIR=${1:?usage: release-checksums.sh <asset-dir>}
cd "$ASSET_DIR"

# Snapshot the list before writing anything, so the .sha256 files this loop
# creates cannot become inputs to it.
ASSETS=()
for entry in *; do
  [ -f "$entry" ] || continue
  case "$entry" in
    *.sha256 | *.minisig) continue ;;
  esac
  ASSETS+=("$entry")
done

if [ "${#ASSETS[@]}" -eq 0 ]; then
  echo "::error::no release assets found in $ASSET_DIR — nothing to checksum." >&2
  exit 1
fi

for asset in "${ASSETS[@]}"; do
  shasum -a 256 "$asset" | awk '{print $1}' > "$asset.sha256"
  echo "$asset.sha256  $(cat "$asset.sha256")"
done
