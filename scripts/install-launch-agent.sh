#!/bin/sh
set -e

PLIST_DIR="$HOME/Library/LaunchAgents"
LOGS_DIR="$HOME/Library/Logs/Nook"
PLIST_PATH="$PLIST_DIR/tech.nook.daemon.plist"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

NOOKD_BIN="$ROOT_DIR/daemon/target/release/nookd"
if [ ! -f "$NOOKD_BIN" ]; then
    NOOKD_BIN="$ROOT_DIR/daemon/target/debug/nookd"
fi

if [ ! -f "$NOOKD_BIN" ]; then
    echo "Building nookd..."
    (cd "$ROOT_DIR/daemon" && cargo build --bin nookd)
    NOOKD_BIN="$ROOT_DIR/daemon/target/debug/nookd"
fi

mkdir -p "$PLIST_DIR" "$LOGS_DIR"

cat <<EOF > "$PLIST_PATH"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>tech.nook.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>$NOOKD_BIN</string>
    </array>
    <key>WorkingDirectory</key>
    <string>$ROOT_DIR</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>$HOME/.local/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    </dict>
    <key>KeepAlive</key>
    <true/>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$LOGS_DIR/daemon.log</string>
    <key>StandardErrorPath</key>
    <string>$LOGS_DIR/daemon.log</string>
</dict>
</plist>
EOF

# Unload if already loaded
launchctl bootout "gui/$(id -u)" "$PLIST_PATH" 2>/dev/null || launchctl unload "$PLIST_PATH" 2>/dev/null || true
# Load
launchctl bootstrap "gui/$(id -u)" "$PLIST_PATH" 2>/dev/null || launchctl load "$PLIST_PATH"

echo "Installed and loaded LaunchAgent at $PLIST_PATH"
