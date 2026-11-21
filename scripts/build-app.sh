#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
"$ROOT/scripts/cargo.sh" build --release --locked
APP="$ROOT/dist/Screen Saver Timer.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
mkdir -p "$ROOT/work"
ICON_WORK=$(mktemp -d "$ROOT/work/app-icon.XXXXXX")
trap 'rm -rf "$ICON_WORK"' EXIT
mkdir "$ICON_WORK/AppIcon.iconset"
/usr/bin/swift "$ROOT/scripts/render-icon.swift" "$ICON_WORK/AppIcon.iconset"
/usr/bin/iconutil -c icns "$ICON_WORK/AppIcon.iconset" -o "$APP/Contents/Resources/AppIcon.icns"
cp "$ROOT/target/release/screensaver-timer" "$APP/Contents/MacOS/screensaver-timer"
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Screen Saver Timer</string>
<key>CFBundleDisplayName</key><string>Screen Saver Timer</string>
<key>CFBundleIdentifier</key><string>local.screensaver-timer.app</string>
<key>CFBundleIconFile</key><string>AppIcon.icns</string>
<key>CFBundleExecutable</key><string>screensaver-timer</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>1.4.2</string>
<key>CFBundleVersion</key><string>8</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSAppSleepDisabled</key><true/>
</dict></plist>
PLIST
/usr/bin/plutil -lint "$APP/Contents/Info.plist"
/usr/bin/codesign --force --sign - "$APP"
/usr/bin/codesign --verify --deep --strict "$APP"
printf '%s\n' "$APP"
