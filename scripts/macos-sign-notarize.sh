#!/usr/bin/env bash
#
# Code-sign, notarize and staple a par-term.app bundle, then produce the
# release zip.
#
# Usage: scripts/macos-sign-notarize.sh <app-bundle> <output-zip>
#
# Required environment (all supplied by the release workflow):
#   MACOS_SIGNING_IDENTITY        plain value  Developer ID Application: ... (TEAMID)
#   APPLE_TEAM_ID                 plain value  the 10-character team identifier
#   MACOS_CERTIFICATE_P12_BASE64  secret       base64 of the Developer ID .p12
#   MACOS_CERTIFICATE_PASSWORD    secret       password protecting that .p12
#   MACOS_KEYCHAIN_PASSWORD       secret       unlock password for the temp keychain
#   APPLE_API_KEY_ID              secret       App Store Connect API key id
#   APPLE_API_ISSUER_ID           secret       App Store Connect issuer UUID
#   APPLE_API_PRIVATE_KEY_BASE64  secret       base64 of the AuthKey_*.p8
#
# There is no unsigned path out of this script: a missing variable, a failed
# codesign, a notarization verdict other than Accepted, or a staple that does
# not survive zipping all abort with a non-zero status.

set -euo pipefail

APP_PATH=${1:?usage: macos-sign-notarize.sh <app-bundle> <output-zip>}
OUTPUT_ZIP=${2:?usage: macos-sign-notarize.sh <app-bundle> <output-zip>}

require_var() {
  local name=$1
  if [ -z "${!name:-}" ]; then
    echo "::error::$name is unset or empty — refusing to produce an unsigned macOS bundle." >&2
    echo "Release signing is mandatory. Populate it before re-running the release." >&2
    exit 1
  fi
}

require_var MACOS_SIGNING_IDENTITY
require_var APPLE_TEAM_ID
require_var MACOS_CERTIFICATE_P12_BASE64
require_var MACOS_CERTIFICATE_PASSWORD
require_var MACOS_KEYCHAIN_PASSWORD
require_var APPLE_API_KEY_ID
require_var APPLE_API_ISSUER_ID
require_var APPLE_API_PRIVATE_KEY_BASE64

[ -d "$APP_PATH" ] || { echo "::error::app bundle not found: $APP_PATH" >&2; exit 1; }

APP_DIR=$(cd "$(dirname "$APP_PATH")" && pwd)
APP_NAME=$(basename "$APP_PATH")
APP_ABS="$APP_DIR/$APP_NAME"

mkdir -p "$(dirname "$OUTPUT_ZIP")"
OUTPUT_ZIP_ABS="$(cd "$(dirname "$OUTPUT_ZIP")" && pwd)/$(basename "$OUTPUT_ZIP")"

umask 077
WORK_DIR=$(mktemp -d)
KEYCHAIN_PATH="$WORK_DIR/par-term-signing.keychain-db"
CERT_PATH="$WORK_DIR/certificate.p12"
API_KEY_PATH="$WORK_DIR/AuthKey.p8"

# Preserve the real exit status: a clean teardown must never turn a signing
# failure into a green step.
cleanup() {
  local rc=$?
  security delete-keychain "$KEYCHAIN_PATH" >/dev/null 2>&1 || true
  rm -rf "$WORK_DIR"
  exit "$rc"
}
trap cleanup EXIT

printf '%s' "$MACOS_CERTIFICATE_P12_BASE64" | base64 --decode > "$CERT_PATH"
printf '%s' "$APPLE_API_PRIVATE_KEY_BASE64" | base64 --decode > "$API_KEY_PATH"

echo "==> Creating temporary signing keychain"
security create-keychain -p "$MACOS_KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
security set-keychain-settings -lut 21600 "$KEYCHAIN_PATH"
security unlock-keychain -p "$MACOS_KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
security import "$CERT_PATH" -k "$KEYCHAIN_PATH" -P "$MACOS_CERTIFICATE_PASSWORD" \
  -f pkcs12 -T /usr/bin/codesign -T /usr/bin/security

# Without an explicit partition list codesign cannot use the imported key
# non-interactively and fails with errSecInternalComponent.
security set-key-partition-list -S apple-tool:,apple:,codesign: \
  -s -k "$MACOS_KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH" >/dev/null
security list-keychains -d user -s "$KEYCHAIN_PATH"

if ! security find-identity -v -p codesigning "$KEYCHAIN_PATH" | grep -qF "$MACOS_SIGNING_IDENTITY"; then
  echo "::error::'$MACOS_SIGNING_IDENTITY' is not present in the imported certificate." >&2
  security find-identity -v -p codesigning "$KEYCHAIN_PATH" >&2 || true
  exit 1
fi

echo "==> Signing $APP_NAME"
codesign --force --timestamp --options runtime \
  --keychain "$KEYCHAIN_PATH" --sign "$MACOS_SIGNING_IDENTITY" \
  "$APP_ABS/Contents/MacOS/par-term"
codesign --force --timestamp --options runtime \
  --keychain "$KEYCHAIN_PATH" --sign "$MACOS_SIGNING_IDENTITY" \
  "$APP_ABS"

codesign --verify --deep --strict --verbose=2 "$APP_ABS"
if ! codesign -dv --verbose=4 "$APP_ABS" 2>&1 | grep -qF "TeamIdentifier=$APPLE_TEAM_ID"; then
  echo "::error::signed bundle does not report TeamIdentifier=$APPLE_TEAM_ID (ad-hoc signature?)." >&2
  codesign -dv --verbose=4 "$APP_ABS" >&2 2>&1 || true
  exit 1
fi

echo "==> Submitting to the notary service"
# ditto is used only for the submission archive; the released zip below keeps
# the `zip -r` format the Homebrew cask and the in-app updater already consume.
SUBMIT_ZIP="$WORK_DIR/notarize.zip"
ditto -c -k --keepParent "$APP_ABS" "$SUBMIT_ZIP"

NOTARY_JSON="$WORK_DIR/notary.json"
xcrun notarytool submit "$SUBMIT_ZIP" \
  --key "$API_KEY_PATH" \
  --key-id "$APPLE_API_KEY_ID" \
  --issuer "$APPLE_API_ISSUER_ID" \
  --wait --timeout 30m \
  --no-progress \
  --output-format json > "$NOTARY_JSON"

SUBMISSION_ID=$(jq -r '.id // empty' "$NOTARY_JSON")
NOTARY_STATUS=$(jq -r '.status // empty' "$NOTARY_JSON")
echo "Notarization submission $SUBMISSION_ID finished with status: $NOTARY_STATUS"

# notarytool can exit 0 on a rejected submission, so the verdict is checked
# explicitly rather than inferred from the exit status.
if [ "$NOTARY_STATUS" != "Accepted" ]; then
  echo "::error::notarization did not succeed (status: ${NOTARY_STATUS:-unknown})." >&2
  if [ -n "$SUBMISSION_ID" ]; then
    xcrun notarytool log "$SUBMISSION_ID" \
      --key "$API_KEY_PATH" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER_ID" >&2 || true
  fi
  exit 1
fi

echo "==> Stapling the notarization ticket"
xcrun stapler staple "$APP_ABS"
xcrun stapler validate "$APP_ABS"

echo "==> Creating $OUTPUT_ZIP_ABS"
rm -f "$OUTPUT_ZIP_ABS"
(cd "$APP_DIR" && zip -qr "$OUTPUT_ZIP_ABS" "$APP_NAME")

# Verifying the bundle in place proves nothing about what ships. Round-trip the
# released archive so a staple that did not survive zipping fails here.
echo "==> Verifying the extracted release archive"
VERIFY_DIR="$WORK_DIR/verify"
mkdir -p "$VERIFY_DIR"
unzip -q "$OUTPUT_ZIP_ABS" -d "$VERIFY_DIR"
codesign --verify --deep --strict --verbose=2 "$VERIFY_DIR/$APP_NAME"
xcrun stapler validate "$VERIFY_DIR/$APP_NAME"
spctl -a -t exec -vvv "$VERIFY_DIR/$APP_NAME"

echo "==> $APP_NAME is signed, notarized and stapled"
