#!/bin/sh
set -e

# Nook installer script
# Can be run via: curl -fsSL https://raw.githubusercontent.com/PranavSikarwal/Nook/main/install.sh | sh
# Or locally:     ./install.sh

GITHUB_REPO="PranavSikarwal/Nook"
BIN_DIR="${HOME}/.local/bin"
TMP_FILES=""
MOUNT_DIR=""
INSTALLED_SUCCESS=0

cleanup() {
    exit_code=$?
    if [ -n "$MOUNT_DIR" ] && [ -d "$MOUNT_DIR" ]; then
        hdiutil detach "$MOUNT_DIR" -quiet 2>/dev/null || true
        rm -rf "$MOUNT_DIR" 2>/dev/null || true
    fi
    for f in $TMP_FILES; do
        rm -rf "$f" 2>/dev/null || true
    done
    if [ "$exit_code" -ne 0 ] && [ "$INSTALLED_SUCCESS" -ne 1 ]; then
        echo "Installation failed." >&2
    fi
}
trap cleanup EXIT INT TERM

install_macos_app() {
    src_app="$1"
    if [ ! -d "$src_app" ]; then
        echo "Error: Source application not found at $src_app" >&2
        exit 1
    fi

    target_dir=""
    if [ -w "/Applications" ]; then
        target_dir="/Applications"
    else
        target_dir="${HOME}/Applications"
        mkdir -p "$target_dir"
    fi

    echo "Installing Nook.app to ${target_dir}/Nook.app..."
    rm -rf "${target_dir}/Nook.app"
    if cp -R "$src_app" "${target_dir}/Nook.app"; then
        echo "Successfully installed Nook.app to ${target_dir}/Nook.app"
    else
        echo "Error: Failed to copy Nook.app to ${target_dir}" >&2
        exit 1
    fi
}

secure_curl() {
    curl --proto '=https' --tlsv1.2 "$@"
}

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
        install_macos_app "$ROOT_DIR/build/Nook.app"
    fi
else
    echo "Fetching latest release from GitHub ($GITHUB_REPO)..."
    API_URL="https://api.github.com/repos/${GITHUB_REPO}/releases/latest"
    RELEASE_DATA=$(secure_curl -fsSL "$API_URL" 2>/dev/null || true)

    if [ -z "$RELEASE_DATA" ]; then
        echo "Notice: No pre-built release found on GitHub yet."
        echo "To build from source, clone the repository and run ./install.sh:"
        echo "  git clone https://github.com/${GITHUB_REPO}.git && cd Nook && ./install.sh"
        exit 1
    fi

    TAG_NAME=$(echo "$RELEASE_DATA" | grep '"tag_name":' | head -n 1 | cut -d '"' -f 4)
    VERSION="${TAG_NAME#v}"
    echo "Latest release: $TAG_NAME"

    find_asset_url() {
        pattern="$1"
        echo "$RELEASE_DATA" | grep '"browser_download_url":' | cut -d '"' -f 4 | grep -E "$pattern" | head -n 1
    }

    if [ "$PLATFORM" = "macos" ]; then
        if [ "$ARCH_NORM" = "arm64" ]; then
            DMG_URL=$(find_asset_url "Nook_.*(aarch64|arm64).*\.dmg$")
        else
            DMG_URL=$(find_asset_url "Nook_.*(x86_64|x64|amd64).*\.dmg$")
        fi
        if [ -z "$DMG_URL" ]; then
            DMG_URL=$(find_asset_url "Nook_.*\.dmg$")
        fi

        if [ -z "$DMG_URL" ]; then
            echo "Error: No compatible macOS .dmg release asset found for $ARCH_NORM." >&2
            exit 1
        fi

        TMP_DMG=$(mktemp /tmp/nook-installer.XXXXXX.dmg)
        TMP_FILES="$TMP_FILES $TMP_DMG"

        echo "Downloading $DMG_URL..."
        if ! secure_curl -fSL "$DMG_URL" -o "$TMP_DMG"; then
            echo "Error: Failed to download release asset." >&2
            exit 1
        fi

        echo "Mounting disk image..."
        MOUNT_DIR=$(mktemp -d /tmp/nook-mount.XXXXXX)
        hdiutil attach "$TMP_DMG" -mountpoint "$MOUNT_DIR" -nobrowse -quiet
        install_macos_app "$MOUNT_DIR/Nook.app"
        hdiutil detach "$MOUNT_DIR" -quiet
        rm -rf "$MOUNT_DIR"
        MOUNT_DIR=""
    elif [ "$PLATFORM" = "linux" ]; then
        if [ "$ARCH_NORM" = "x86_64" ]; then
            DEB_URL=$(find_asset_url "nook_.*(amd64|x86_64|x64).*\.deb$")
        else
            DEB_URL=$(find_asset_url "nook_.*(arm64|aarch64).*\.deb$")
        fi

        if [ -n "$DEB_URL" ]; then
            TMP_DEB=$(mktemp /tmp/nook-installer.XXXXXX.deb)
            TMP_FILES="$TMP_FILES $TMP_DEB"

            echo "Downloading $DEB_URL..."
            if ! secure_curl -fSL "$DEB_URL" -o "$TMP_DEB"; then
                echo "Error: Failed to download Debian package." >&2
                exit 1
            fi
            echo "Installing Debian package..."
            sudo dpkg -i "$TMP_DEB" || sudo apt-get install -f -y
        else
            if [ "$ARCH_NORM" = "x86_64" ]; then
                APPIMAGE_URL=$(find_asset_url "nook_.*(amd64|x86_64|x64).*\.AppImage$")
            else
                APPIMAGE_URL=$(find_asset_url "nook_.*(arm64|aarch64).*\.AppImage$")
            fi
            if [ -n "$APPIMAGE_URL" ]; then
                TMP_APPIMAGE="${BIN_DIR}/nook-panel"
                echo "Downloading AppImage to $TMP_APPIMAGE..."
                if ! secure_curl -fSL "$APPIMAGE_URL" -o "$TMP_APPIMAGE"; then
                    echo "Error: Failed to download AppImage." >&2
                    exit 1
                fi
                chmod +x "$TMP_APPIMAGE"
            else
                echo "Error: No compatible Linux release asset (.deb or .AppImage) found." >&2
                exit 1
            fi
        fi
    fi
fi

# 3. Create global 'nook' CLI wrapper
cat << 'EOF' > "$BIN_DIR/nook"
#!/bin/sh
# Nook desktop launcher
if [ "$(uname -s)" = "Darwin" ]; then
    if [ -d "/Applications/Nook.app" ]; then
        open "/Applications/Nook.app"
    elif [ -d "$HOME/Applications/Nook.app" ]; then
        open "$HOME/Applications/Nook.app"
    elif [ -x "$HOME/.local/bin/nook-panel" ]; then
        exec "$HOME/.local/bin/nook-panel" "$@"
    elif command -v nook-panel >/dev/null 2>&1; then
        exec nook-panel "$@"
    else
        echo "Error: Nook.app not found in /Applications or $HOME/Applications." >&2
        exit 1
    fi
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

INSTALLED_SUCCESS=1

echo ""
echo "=== Nook installed successfully! ==="
echo "Launch Nook using:"
echo "  - Terminal command: nook"
echo "  - Global shortcut: Option+Space (macOS) / Alt+Space (Linux & Windows)"
