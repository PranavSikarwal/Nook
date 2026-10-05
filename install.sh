#!/bin/sh
set -e

# Nook installer script
# Can be run via: curl -fsSL https://raw.githubusercontent.com/PranavSikarwal/Nook/main/install.sh | sh
# Or locally:     ./install.sh

GITHUB_REPO="PranavSikarwal/Nook"
BIN_DIR="${HOME}/.local/bin"

echo "=== Installing Nook ==="

# 1. Detect operating system and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Darwin)
        PLATFORM="macos"
        ;;
    Linux)
        PLATFORM="linux"
        ;;
    *)
        echo "Error: Unsupported operating system: $OS" >&2
        exit 1
        ;;
esac

case "$ARCH" in
    arm64|aarch64)
        ARCH_NORM="arm64"
        ;;
    x86_64|amd64)
        ARCH_NORM="x86_64"
        ;;
    *)
        echo "Error: Unsupported architecture: $ARCH" >&2
        exit 1
        ;;
esac

echo "Platform: $PLATFORM ($ARCH_NORM)"
mkdir -p "$BIN_DIR"

# 2. Check if running inside a cloned repository
SCRIPT_PATH="$0"
IS_LOCAL=0
if [ -f "$SCRIPT_PATH" ]; then
    ROOT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" 2>/dev/null && pwd)"
    if [ -f "$ROOT_DIR/panel/src-tauri/Cargo.toml" ]; then
        IS_LOCAL=1
    fi
fi

if [ "$IS_LOCAL" -eq 1 ]; then
    echo "Running local repository build and installation..."
    (cd "$ROOT_DIR/daemon" && cargo build --release -p nookd -p nookctl)
    (cd "$ROOT_DIR/panel" && npm install --silent --ignore-scripts && npm run build)
    (cd "$ROOT_DIR/panel/src-tauri" && cargo build --release)

    cp "$ROOT_DIR/daemon/target/release/nookd" "$BIN_DIR/nookd"
    cp "$ROOT_DIR/daemon/target/release/nookctl" "$BIN_DIR/nookctl"
    cp "$ROOT_DIR/panel/src-tauri/target/release/nook-panel" "$BIN_DIR/nook-panel"
    chmod +x "$BIN_DIR/nookd" "$BIN_DIR/nookctl" "$BIN_DIR/nook-panel"

    if [ "$PLATFORM" = "macos" ]; then
        "$ROOT_DIR/scripts/bundle.sh"
        if [ -d "/Applications" ] && [ -w "/Applications" ]; then
            cp -R "$ROOT_DIR/build/Nook.app" "/Applications/Nook.app"
            echo "Installed Nook.app to /Applications/Nook.app"
        fi
    fi
else
    echo "Fetching latest release from GitHub ($GITHUB_REPO)..."
    API_URL="https://api.github.com/repos/${GITHUB_REPO}/releases/latest"
    RELEASE_DATA=$(curl --proto '=https' --tlsv1.2 -fsSL "$API_URL" 2>/dev/null || true)

    if [ -z "$RELEASE_DATA" ]; then
        echo "Notice: No pre-built release found on GitHub yet."
        echo "To build from source, clone the repository and run ./install.sh:"
        echo "  git clone https://github.com/${GITHUB_REPO}.git && cd Nook && ./install.sh"
        exit 1
    fi

    TAG_NAME=$(echo "$RELEASE_DATA" | grep '"tag_name":' | head -n 1 | cut -d '"' -f 4)
    VERSION="${TAG_NAME#v}"
    echo "Latest release: $TAG_NAME"

    DOWNLOAD_BASE="https://github.com/${GITHUB_REPO}/releases/download/${TAG_NAME}"

    if [ "$PLATFORM" = "macos" ]; then
        DMG_NAME="Nook_${VERSION}_${ARCH_NORM}.dmg"
        DMG_URL="${DOWNLOAD_BASE}/${DMG_NAME}"
        TMP_DMG=$(mktemp /tmp/nook-installer.XXXXXX.dmg)

        echo "Downloading $DMG_NAME..."
        if curl --proto '=https' --tlsv1.2 -fSL "$DMG_URL" -o "$TMP_DMG" 2>/dev/null; then
            echo "Mounting disk image and installing to /Applications..."
            MOUNT_DIR=$(mktemp -d /tmp/nook-mount.XXXXXX)
            hdiutil attach "$TMP_DMG" -mountpoint "$MOUNT_DIR" -nobrowse -quiet
            cp -R "$MOUNT_DIR/Nook.app" "/Applications/Nook.app"
            hdiutil detach "$MOUNT_DIR" -quiet
            rm -rf "$TMP_DMG" "$MOUNT_DIR"
            echo "Successfully installed Nook.app to /Applications/Nook.app"
        else
            echo "Could not download $DMG_URL directly."
        fi
    elif [ "$PLATFORM" = "linux" ]; then
        DEB_NAME="nook_${VERSION}_amd64.deb"
        DEB_URL="${DOWNLOAD_BASE}/${DEB_NAME}"
        TMP_DEB=$(mktemp /tmp/nook-installer.XXXXXX.deb)

        if curl --proto '=https' --tlsv1.2 -fSL "$DEB_URL" -o "$TMP_DEB" 2>/dev/null; then
            echo "Installing Debian package..."
            sudo dpkg -i "$TMP_DEB" || sudo apt-get install -f -y
            rm -f "$TMP_DEB"
            echo "Successfully installed Nook on Linux."
        fi
    fi
fi

# 3. Create global 'nook' CLI wrapper
cat << 'EOF' > "$BIN_DIR/nook"
#!/bin/sh
# Nook desktop launcher
if [ "$(uname -s)" = "Darwin" ] && [ -d "/Applications/Nook.app" ]; then
    open "/Applications/Nook.app"
elif [ -x "$HOME/.local/bin/nook-panel" ]; then
    exec "$HOME/.local/bin/nook-panel" "$@"
elif command -v nook-panel >/dev/null 2>&1; then
    exec nook-panel "$@"
else
    echo "Error: Nook application not found." >&2
    exit 1
fi
EOF

chmod +x "$BIN_DIR/nook"

echo ""
echo "=== Nook installed successfully! ==="
echo "Launch Nook using:"
echo "  - Terminal command: nook"
echo "  - Global shortcut: Option+Space (macOS) / Alt+Space (Linux & Windows)"
