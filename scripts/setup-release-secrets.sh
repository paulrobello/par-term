#!/usr/bin/env bash
#
# Populate the par-term release signing secrets on GitHub.
#
# Every value is read locally with a silent prompt and piped straight into
# `gh secret set`. Nothing is echoed, nothing secret is written to disk, and no
# value passes through an agent transcript — which is the reason this exists as
# a script rather than as instructions to paste values into a chat.
#
# Run:  scripts/setup-release-secrets.sh
#
# Sets the seven secrets the release preflight requires beyond
# MINISIGN_SECRET_KEY. Safe to re-run: each `gh secret set` overwrites, so a
# run that fails partway is fixed by running it again.
#
# The eighth, MINISIGN_SECRET_KEY, is just the key file:
#   gh secret set MINISIGN_SECRET_KEY --repo <repo> < ~/.minisign/minisign.key

set -euo pipefail

REPO=paulrobello/par-term
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(dirname "$SCRIPT_DIR")
IDENTITY_SHA=910B448F82260AD75FCB7E621A02175D8770B287   # Developer ID Application: PAUL AARON ROBELLO

umask 077
WORK=$(mktemp -d)
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

set_secret() { printf '%s' "$2" | gh secret set "$1" --repo "$REPO" && echo "  ✓ $1"; }

echo "par-term release secrets → $REPO"
echo

# ---------------------------------------------------------------- minisign
echo "[1/4] minisign key password (for ~/.minisign/minisign.key)"
read -rsp "      passphrase: " MINISIGN_PW; echo
# Prove it before uploading: sign a probe and verify against the pinned key.
if command -v minisign >/dev/null 2>&1; then
  echo "probe" > "$WORK/probe"
  if printf '%s\n' "$MINISIGN_PW" \
       | minisign -S -s "$HOME/.minisign/minisign.key" -m "$WORK/probe" >/dev/null 2>&1; then
    PIN=$(tr -d '\n' < "$REPO_ROOT/par-term-update/src/signature.rs" \
      | sed -n 's/.*pub const UPDATE_SIGNING_PUBLIC_KEY: &str =[[:space:]]*"\([^"]*\)";.*/\1/p')
    if minisign -V -P "$PIN" -m "$WORK/probe" >/dev/null 2>&1; then
      echo "      passphrase correct, and the keypair matches the pinned public key"
    else
      echo "      ✗ signature does not verify against the pinned key — aborting" >&2; exit 1
    fi
  else
    echo "      ✗ wrong passphrase (minisign refused to sign) — aborting" >&2; exit 1
  fi
else
  echo "      (minisign CLI not installed; skipping local proof — the release"
  echo "       preflight will catch a wrong passphrase before any build runs)"
fi
set_secret MINISIGN_SECRET_KEY_PASSWORD "$MINISIGN_PW"
unset MINISIGN_PW
echo

# ------------------------------------------------------------ Developer ID
echo "[2/4] Developer ID certificate"
echo
echo "      Export it by hand first, so that exactly one identity leaves the"
echo "      keychain. \`security export -t identities\` takes ALL of them, which"
echo "      would push your two Apple Development private keys to GitHub as well."
echo
echo "        Keychain Access → login → My Certificates"
echo "        → \"Developer ID Application: PAUL AARON ROBELLO (QMLVG482FY)\""
echo "        → right-click → Export… → .p12 → set a password"
echo "        (SHA-1 $IDENTITY_SHA)"
echo
read -rp  "      Path to the exported .p12: " P12_PATH
P12_PATH="${P12_PATH/#\~/$HOME}"
[ -f "$P12_PATH" ] || { echo "      ✗ no such file: $P12_PATH" >&2; exit 1; }
read -rsp "      Password you set on that .p12: " P12_PW; echo

# Refuse to upload a bundle that cannot sign. This proves a Developer ID
# certificate is present; it does not prove nothing else is, so the manual
# single-identity export above is what keeps the other keys out.
if ! openssl pkcs12 -in "$P12_PATH" -passin "pass:$P12_PW" -nokeys -legacy 2>/dev/null \
     | grep -q 'BEGIN CERTIFICATE'; then
  echo "      ✗ could not read that .p12 — wrong password, or not a PKCS#12 file" >&2
  exit 1
fi
if ! openssl pkcs12 -in "$P12_PATH" -passin "pass:$P12_PW" -nokeys -legacy 2>/dev/null \
     | openssl x509 -noout -subject 2>/dev/null | grep -q "Developer ID Application"; then
  echo "      ✗ that .p12 holds no Developer ID Application certificate" >&2
  exit 1
fi
echo "      ✓ contains a Developer ID Application certificate"
set_secret MACOS_CERTIFICATE_P12_BASE64 "$(base64 < "$P12_PATH" | tr -d '\n')"
set_secret MACOS_CERTIFICATE_PASSWORD "$P12_PW"
unset P12_PW
echo

# ----------------------------------------------------------- temp keychain
echo "[3/4] Temporary CI keychain password"
echo "      Arbitrary — the keychain is created and destroyed inside one job."
read -rsp "      password: " KC_PW; echo
set_secret MACOS_KEYCHAIN_PASSWORD "$KC_PW"
unset KC_PW
echo

# ------------------------------------------------------ App Store Connect
echo "[4/4] App Store Connect API key (for notarytool)"
echo "      From appstoreconnect.apple.com → Users and Access → Integrations → App Store Connect API."
echo "      The key must be a TEAM key, not an Individual key — notarytool rejects Individual keys."
read -rp  "      Key ID: " ASC_KEY_ID
read -rp  "      Issuer ID (UUID): " ASC_ISSUER_ID
read -rp  "      Path to AuthKey_${ASC_KEY_ID}.p8: " ASC_P8
ASC_P8="${ASC_P8/#\~/$HOME}"
[ -f "$ASC_P8" ] || { echo "      ✗ no such file: $ASC_P8" >&2; exit 1; }
grep -q "BEGIN PRIVATE KEY" "$ASC_P8" || { echo "      ✗ not a PEM private key: $ASC_P8" >&2; exit 1; }
set_secret APPLE_API_KEY_ID "$ASC_KEY_ID"
set_secret APPLE_API_ISSUER_ID "$ASC_ISSUER_ID"
set_secret APPLE_API_PRIVATE_KEY_BASE64 "$(base64 < "$ASC_P8" | tr -d '\n')"
echo

echo "Done. Configured secrets:"
gh secret list --repo "$REPO" | cut -f1 | sed 's/^/  /'
