#!/bin/sh
set -e

PLIST_PATH="$HOME/Library/LaunchAgents/tech.nook.daemon.plist"

if [ -f "$PLIST_PATH" ]; then
    launchctl bootout "gui/$(id -u)" "$PLIST_PATH" 2>/dev/null || launchctl unload "$PLIST_PATH" 2>/dev/null || true
    rm -f "$PLIST_PATH"
    echo "Uninstalled LaunchAgent: $PLIST_PATH"
else
    echo "LaunchAgent plist not found: $PLIST_PATH"
fi
