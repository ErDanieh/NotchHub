#!/bin/zsh
# Builds NotchHub.app (menu bar app, no Dock icon) into ./build
set -e
cd "$(dirname "$0")/.."
swift build -c release
APP=build/NotchHub.app
rm -rf "$APP"; mkdir -p "$APP/Contents/MacOS"
cp .build/release/NotchHub "$APP/Contents/MacOS/NotchHub"
cat > "$APP/Contents/Info.plist" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>local.notchhub</string>
<key>CFBundleName</key><string>NotchHub</string>
<key>CFBundleExecutable</key><string>NotchHub</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleShortVersionString</key><string>0.1</string>
<key>LSMinimumSystemVersion</key><string>14.0</string>
<key>LSUIElement</key><true/>
<key>NSCalendarsFullAccessUsageDescription</key><string>NotchHub shows your upcoming events and reminds you before they start.</string>
<key>NSCameraUsageDescription</key><string>NotchHub shows a mirror preview of your camera. It is only on while the Mirror tab is open.</string>
<key>NSAudioCaptureUsageDescription</key><string>NotchHub listens to the sound your Mac is playing so the halo can follow the beat. Nothing is recorded or saved.</string>
<key>NSAppleEventsUsageDescription</key><string>NotchHub shows what is playing in Music and Spotify and lets you pause, play and skip.</string>
</dict></plist>
PL
# Sign in a clean temp folder (the Desktop is iCloud-synced and adds Finder metadata that breaks codesign).
# Stable signature keeps Calendar/Camera/Automation permissions between builds. Identities whose
# certificate is revoked (macOS then calls the app malware) are skipped; falls back to ad-hoc.
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
sign_with() {
  rm -rf "$STAGE/NotchHub.app"
  ditto --norsrc --noextattr "$APP" "$STAGE/NotchHub.app"
  codesign --force --sign "$1" "$STAGE/NotchHub.app" >/dev/null 2>&1
}
SIGNED=""
for ID in $(security find-identity -p codesigning -v | grep "Apple Development" | grep -v REVOKED | awk '{print $2}'); do
  if sign_with "$ID" && ! spctl -a -vv "$STAGE/NotchHub.app" 2>&1 | grep -q REVOKED; then SIGNED="$ID"; break; fi
done
if [ -z "$SIGNED" ]; then sign_with "-"; SIGNED="ad-hoc"; fi
echo "Signed with: $SIGNED"
APP="$STAGE/NotchHub.app"

# Replace the installed copy and relaunch
DEST="$HOME/Applications/NotchHub.app"
mkdir -p "$HOME/Applications"
osascript -e 'tell application id "local.notchhub" to quit' >/dev/null 2>&1 || true
pkill -x NotchHub 2>/dev/null || true
sleep 0.5
rm -rf "$DEST"
cp -R "$APP" "$DEST"
echo "Installed $DEST"
open "$DEST"
