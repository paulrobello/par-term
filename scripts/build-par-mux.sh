#!/usr/bin/env bash
#
# Build the par-mux daemon pinned to the core version in this checkout's
# Cargo.lock, and copy it next to the par-term binary.
#
# Usage: scripts/build-par-mux.sh <output-file> [--target <triple>]
#
# The daemon lives in the external par-term-emu-core-rust crate (a separate
# repository), so it cannot be built with `cargo build -p` from this
# workspace. `cargo install --version <lock's version>` fetches exactly the
# crates.io release the client links — the version guarantee the stale-daemon
# check (`check_daemon_version`) relies on: a daemon and client built from
# different core versions would mismatch at attach.
#
# With --target, the daemon is built for that triple (the host must have the
# target installed and linkable — the release workflow's cross legs already
# build par-term the same way). The script always installs into a temp root
# and copies the binary out, so callers never see cargo's layout.

set -euo pipefail

OUT=${1:?usage: build-par-mux.sh <output-file> [--target <triple>]}
shift
TARGET=""
while [ $# -gt 0 ]; do
  case "$1" in
    --target) TARGET=$2; shift 2 ;;
    *) echo "::error::unknown argument: $1" >&2; exit 1 ;;
  esac
done

LOCK_CORE_VERSION=$(grep -A1 '^name = "par-term-emu-core-rust"$' Cargo.lock \
  | grep -m1 '^version' | sed 's/version = "\(.*\)"/\1/')
if [ -z "$LOCK_CORE_VERSION" ]; then
  echo "::error::could not read the par-term-emu-core-rust version from Cargo.lock" >&2
  exit 1
fi
echo "Building par-mux from par-term-emu-core-rust ${LOCK_CORE_VERSION}"

ROOT=$(mktemp -d)
trap 'rm -rf "$ROOT"' EXIT

INSTALL_ARGS=(
  --no-default-features --features mux --bin par-mux
  --version "$LOCK_CORE_VERSION"
  --root "$ROOT"
  par-term-emu-core-rust
)
if [ -n "$TARGET" ]; then
  cargo install --target "$TARGET" "${INSTALL_ARGS[@]}"
else
  cargo install "${INSTALL_ARGS[@]}"
fi

mkdir -p "$(dirname "$OUT")"
if [ "$(uname -s)" = "Msys" ] || [[ "$OUT" == *.exe ]]; then
  cp "$ROOT/bin/par-mux.exe" "$OUT"
else
  cp "$ROOT/bin/par-mux" "$OUT"
fi

echo "par-mux daemon written to $OUT"
