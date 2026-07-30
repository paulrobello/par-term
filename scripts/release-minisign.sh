#!/usr/bin/env bash
#
# Sign every release asset in a directory with minisign and verify each
# signature against the public key pinned into the shipped binary.
#
# Usage: scripts/release-minisign.sh <asset-dir>
#
# Required environment:
#   MINISIGN_SECRET_KEY            secret  full text of the minisign .key file
#   MINISIGN_SECRET_KEY_PASSWORD   secret  password for that key; omit only if
#                                          the key was generated with `-G -W`
#
# The `.minisig` names produced here are the ones par-term-update's
# `get_signature_asset_name()` looks for — `<platform asset>.minisig`.
#
# The verification pass is the gate that matters: it uses
# UPDATE_SIGNING_PUBLIC_KEY from par-term-update/src/signature.rs, i.e. the key
# the shipped binary will actually trust. Signing with a key that does not match
# the pin fails here rather than producing a release nobody can install.
#
# Running this against a directory holding a single throwaway file is a complete
# preflight: it proves the secret is present, the password is right, and the
# keypair matches the pin — which is why the release workflow does exactly that
# before any build starts.

set -euo pipefail

ASSET_DIR=${1:?usage: release-minisign.sh <asset-dir>}

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(dirname "$SCRIPT_DIR")
SIGNATURE_RS="$REPO_ROOT/par-term-update/src/signature.rs"

if [ -z "${MINISIGN_SECRET_KEY:-}" ]; then
  echo "::error::MINISIGN_SECRET_KEY is unset or empty — refusing to publish unsigned release assets." >&2
  echo "Set the MINISIGN_SECRET_KEY repository secret to the contents of the minisign .key file." >&2
  exit 1
fi

[ -f "$SIGNATURE_RS" ] || { echo "::error::cannot find $SIGNATURE_RS" >&2; exit 1; }

PINNED_KEY=$(sed -n 's/^pub const UPDATE_SIGNING_PUBLIC_KEY: &str = "\(.*\)";$/\1/p' "$SIGNATURE_RS")
if [ -z "$PINNED_KEY" ]; then
  echo "::error::UPDATE_SIGNING_PUBLIC_KEY in par-term-update/src/signature.rs is empty." >&2
  echo "The shipped updater trusts no key, so every signature this release publishes would be rejected." >&2
  echo "Paste the base64 line of the minisign public key into that constant before releasing." >&2
  exit 1
fi

umask 077
WORK_DIR=$(mktemp -d)
cleanup() {
  local rc=$?
  rm -rf "$WORK_DIR"
  exit "$rc"
}
trap cleanup EXIT

SECRET_KEY_FILE="$WORK_DIR/par-term-release.key"
PASSWORD_FILE="$WORK_DIR/password"
printf '%s\n' "$MINISIGN_SECRET_KEY" > "$SECRET_KEY_FILE"
printf '%s\n' "${MINISIGN_SECRET_KEY_PASSWORD:-}" > "$PASSWORD_FILE"

cd "$ASSET_DIR"

ASSETS=()
for entry in *; do
  [ -f "$entry" ] || continue
  case "$entry" in
    *.sha256 | *.minisig) continue ;;
  esac
  ASSETS+=("$entry")
done

if [ "${#ASSETS[@]}" -eq 0 ]; then
  echo "::error::no release assets found in $ASSET_DIR — nothing to sign." >&2
  exit 1
fi

# One invocation per asset, rather than one batch invocation for all of them.
# Each run opens its own descriptor on the password file at offset 0, so the
# password re-reads correctly however many times minisign asks for it, and the
# one-asset preflight exercises exactly the code path the real release takes.
# stdin is redirected from a file rather than piped so an unencrypted key (which
# never reads stdin) cannot become a broken pipe under `set -o pipefail`.
echo "==> Signing ${#ASSETS[@]} asset(s)"
for asset in "${ASSETS[@]}"; do
  minisign -S -s "$SECRET_KEY_FILE" -m "$asset" < "$PASSWORD_FILE"
done

echo "==> Verifying each signature against the key pinned in par-term-update/src/signature.rs"
for asset in "${ASSETS[@]}"; do
  minisign -V -P "$PINNED_KEY" -m "$asset"
done

echo "==> ${#ASSETS[@]} asset(s) signed and verified against the pinned public key"
