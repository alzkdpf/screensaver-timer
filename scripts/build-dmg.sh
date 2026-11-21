#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP="$ROOT/dist/Screen Saver Timer.app"
[ -d "$APP" ] || "$ROOT/scripts/build-app.sh"
mkdir -p "$ROOT/work"
STAGING=$(mktemp -d "$ROOT/work/dmg-stage.XXXXXX")
trap 'rm -rf "$STAGING"' EXIT
chmod 755 "$STAGING"
/usr/bin/ditto --noextattr --noqtn --norsrc "$APP" "$STAGING/Screen Saver Timer.app"
ln -s /Applications "$STAGING/Applications"
cp "$ROOT/INSTALL.txt" "$STAGING/INSTALL.txt"
DMG="$ROOT/dist/Screen-Saver-Timer-1.4.0-arm64.dmg"
/usr/bin/hdiutil create -volname 'Screen Saver Timer 1.4.0' -srcfolder "$STAGING" -fs HFS+ -format UDZO -ov "$DMG"
/usr/bin/hdiutil verify "$DMG"
cd "$ROOT/dist"
/usr/bin/shasum -a 256 Screen-Saver-Timer-1.4.0-arm64.dmg > SHA256SUMS.txt
