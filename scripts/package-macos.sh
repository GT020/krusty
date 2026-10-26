#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
ARCH="${ARCH:-$(uname -m)}"
BIN="${BIN:-$ROOT/target/release/krusty}"
DIST="$ROOT/dist"
APP="$DIST/krusty.app"
ICONSET="$DIST/krusty.iconset"
STAGE="$DIST/dmg"

rm -rf "$APP" "$ICONSET" "$STAGE"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$ICONSET" "$STAGE"

install -m755 "$BIN" "$APP/Contents/MacOS/krusty"

for size in 16 32 128 256 512; do
  sips -z "$size" "$size" assets/app-icon.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  sips -z "$((size * 2))" "$((size * 2))" assets/app-icon.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done

iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/krusty.icns"
sed "s/@VERSION@/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"

codesign --force --deep --sign - "$APP"

cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"

hdiutil create -volname "krusty" -srcfolder "$STAGE" -ov -format UDZO \
  "$DIST/krusty-${VERSION}-macos-${ARCH}.dmg"
