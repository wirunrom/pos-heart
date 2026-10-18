#!/bin/sh
# Builds "POS Heart.app" (universal: Apple silicon + Intel) and a .dmg in
# target/dist. Run from anywhere:   sh packaging/macos/build-dmg.sh
#
# Signing: with MACOS_SIGN_IDENTITY ("Developer ID Application: …") the app
# is signed with the hardened runtime; otherwise it is signed ad hoc and
# macOS asks users to confirm the first launch (right-click → Open).
# Notarization runs when APPLE_ID, APPLE_TEAM_ID and APPLE_APP_PASSWORD are
# also set. TARGETS overrides the architectures (e.g. only aarch64-apple-darwin).
set -eu
cd "$(dirname "$0")/../.."
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
TARGETS=${TARGETS:-"aarch64-apple-darwin x86_64-apple-darwin"}
BINS=""
for t in $TARGETS; do
  rustup target add "$t" >/dev/null
  cargo build --release --locked -p pos-desktop --bin pos-heart --target "$t"
  BINS="$BINS target/$t/release/pos-heart"
done
OUT=target/dist
APP="$OUT/POS Heart.app"
rm -rf "$OUT"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
# shellcheck disable=SC2086
lipo -create -output "$APP/Contents/MacOS/pos-heart" $BINS
cp packaging/icon/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
sed "s/__VERSION__/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" >/dev/null
if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
  codesign --force --options runtime --timestamp --sign "$MACOS_SIGN_IDENTITY" "$APP"
else
  codesign --force --sign - "$APP"
fi
codesign --verify --strict "$APP"
STAGE="$OUT/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
DMG="$OUT/POS-Heart-$VERSION-macOS.dmg"
hdiutil create -quiet -volname "POS Heart $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
rm -rf "$STAGE"
if [ -n "${MACOS_SIGN_IDENTITY:-}" ]; then
  codesign --force --timestamp --sign "$MACOS_SIGN_IDENTITY" "$DMG"
  if [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ] && [ -n "${APPLE_APP_PASSWORD:-}" ]; then
    xcrun notarytool submit "$DMG" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
    xcrun stapler staple "$DMG"
  fi
fi
echo "$DMG"
