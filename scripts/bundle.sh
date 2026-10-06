#!/bin/sh
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PANEL_DIR="$ROOT_DIR/panel"
BUILD_DIR="$ROOT_DIR/build"
BUNDLE_DIR="$BUILD_DIR/Nook.app"

echo "Building Nook frontend..."
(cd "$PANEL_DIR" && npm run build)

echo "Building Nook release binary with Tauri..."
(cd "$PANEL_DIR/src-tauri" && cargo build --release)

VERSION=$(node -p "require('$PANEL_DIR/src-tauri/tauri.conf.json').version || '0.1.0'")

echo "Creating bundle structure at $BUNDLE_DIR (version $VERSION)..."
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/Contents/MacOS"
mkdir -p "$BUNDLE_DIR/Contents/Resources"

cp "$PANEL_DIR/src-tauri/target/release/nook-panel" "$BUNDLE_DIR/Contents/MacOS/Nook"
chmod +x "$BUNDLE_DIR/Contents/MacOS/Nook"

# Embed nookd daemon binary into bundle so the app starts it automatically
NOOKD_SRC="$ROOT_DIR/daemon/target/release/nookd"
if [ ! -f "$NOOKD_SRC" ]; then
    NOOKD_SRC="$ROOT_DIR/daemon/target/debug/nookd"
fi
if [ -f "$NOOKD_SRC" ]; then
    cp "$NOOKD_SRC" "$BUNDLE_DIR/Contents/MacOS/nookd"
    chmod +x "$BUNDLE_DIR/Contents/MacOS/nookd"
    echo "Embedded nookd into app bundle at Contents/MacOS/nookd"
fi

if [ -f "$PANEL_DIR/src-tauri/icons/icon.icns" ]; then
    cp "$PANEL_DIR/src-tauri/icons/icon.icns" "$BUNDLE_DIR/Contents/Resources/icon.icns"
fi

cat <<EOF > "$BUNDLE_DIR/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>tech.nook.desktop</string>
    <key>CFBundleName</key>
    <string>Nook</string>
    <key>CFBundleDisplayName</key>
    <string>Nook</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>Nook</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSMinimumSystemVersion</key>
    <string>14.0</string>
</dict>
</plist>
EOF

SIGN_IDENTITY="${SIGN_IDENTITY:--}"
echo "Signing bundle with identity '$SIGN_IDENTITY'..."
codesign --force --deep --sign "$SIGN_IDENTITY" "$BUNDLE_DIR"

echo "Nook.app successfully built and signed at $BUNDLE_DIR"
