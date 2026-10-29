#!/usr/bin/env bash
# Builds dist/Flare.app and dist/Flare-<version>-macos.zip from a release build.
# Optional icon: put a 1024x1024 PNG at assets/icon/flare.png (trim black corners first).
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(cargo metadata --no-deps --format-version 1 --locked |
  python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="flare-desktop"))')
cargo build --release --locked -p flare-desktop
app=dist/Flare.app
rm -rf "$app" && mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/flare-desktop "$app/Contents/MacOS/Flare"
icon_key=""
if [ -f assets/icon/flare.png ]; then
  set_dir=$(mktemp -d)/Flare.iconset && mkdir -p "$set_dir"
  for s in 16 32 128 256 512; do
    sips -z $s $s assets/icon/flare.png --out "$set_dir/icon_${s}x${s}.png" >/dev/null
    sips -z $((s*2)) $((s*2)) assets/icon/flare.png --out "$set_dir/icon_${s}x${s}@2x.png" >/dev/null
  done
  iconutil -c icns "$set_dir" -o "$app/Contents/Resources/Flare.icns"
  icon_key="<key>CFBundleIconFile</key><string>Flare</string>"
fi
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Flare</string>
<key>CFBundleDisplayName</key><string>Flare</string>
<key>CFBundleIdentifier</key><string>org.flare-animate.flare</string>
<key>CFBundleExecutable</key><string>Flare</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>NSHighResolutionCapable</key><true/>
$icon_key
</dict></plist>
PLIST
(cd dist && ditto -c -k --keepParent Flare.app "Flare-$version-macos.zip")
echo "dist/Flare-$version-macos.zip"
