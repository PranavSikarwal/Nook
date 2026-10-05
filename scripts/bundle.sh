#!/bin/sh
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_DIR="$ROOT_DIR/app"
BUILD_DIR="$ROOT_DIR/build"
BUNDLE_DIR="$BUILD_DIR/Nook.app"

echo "Building Nook release binary with SwiftPM..."
(cd "$APP_DIR" && swift build -c release --product Nook)

BINARY_PATH="$APP_DIR/.build/release/Nook"
if [ ! -f "$BINARY_PATH" ]; then
    # In some toolchain setups, the path may be arm64-apple-macosx/release
    BINARY_PATH=$(find "$APP_DIR/.build" -type f -name Nook -perm +111 | head -n 1)
fi

if [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Release binary not found." >&2
    exit 1
fi

echo "Creating bundle structure at $BUNDLE_DIR..."
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/Contents/MacOS"
mkdir -p "$BUNDLE_DIR/Contents/Resources"

cp "$BINARY_PATH" "$BUNDLE_DIR/Contents/MacOS/Nook"
chmod +x "$BUNDLE_DIR/Contents/MacOS/Nook"

cat <<EOF > "$BUNDLE_DIR/Contents/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>tech.nook.app</string>
    <key>CFBundleName</key>
    <string>Nook</string>
    <key>CFBundleDisplayName</key>
    <string>Nook</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>Nook</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
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
