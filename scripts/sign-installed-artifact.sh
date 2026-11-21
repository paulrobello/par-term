#!/usr/bin/env bash
# Sign a locally installed artifact with the keychain's Developer ID under a
# stable identifier, so macOS privacy attribution (TCC / Local Network) keeps
# matching it across reinstalls. Cargo/rustc leaves an ad-hoc signature whose
# identity is per-build: TCC sees a brand-new unknown app after every install
# and silently drops Local Network (the 2026-11-20 incident — core card
# 01a1faf7f2b9; vault: Debugging/adhoc-signing-resets-macos-local-network-grants).
#
# Ported from par-term-emu-core-rust scripts/sign-installed-artifact.sh
# (0.85.0) so both repos install identically-signed artifacts.
#
# Usage: sign-installed-artifact.sh <identifier> <artifact> [artifact...]
#   no Developer ID identity in the keychain: loud warning, exit 0 (an
#     unsigned install must not block machines without the identity)
#   missing artifact or codesign failure: exit 1 (the install failed)
#   non-macOS: no-op, exit 0
set -euo pipefail

if [ "$(uname)" != "Darwin" ]; then
	exit 0
fi

identifier="$1"
shift

identity=$(security find-identity -v -p codesigning 2>/dev/null \
	| grep '"Developer ID Application:' | head -1 \
	| sed -E 's/.*"(.*)".*/\1/' || true)
if [ -z "$identity" ]; then
	echo "WARNING: no Developer ID identity in the keychain; leaving $* with its build signature (ad-hoc)." >&2
	echo "         TCC/Local Network grants will NOT survive this install." >&2
	exit 0
fi

for artifact in "$@"; do
	if [ ! -f "$artifact" ]; then
		echo "ERROR: installed artifact not found: $artifact" >&2
		exit 1
	fi
	codesign --force --identifier "$identifier" --sign "$identity" "$artifact"
	echo "signed $artifact (identifier $identifier)"
done
