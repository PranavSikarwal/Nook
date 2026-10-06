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
    if [ "${NOOK_USER_ONLY:-0}" = "1" ] || [ ! -w "/Applications" ]; then
        target_dir="${HOME}/Applications"
        mkdir -p "$target_dir"
    else
        target_dir="/Applications"
    fi

    echo "Installing Nook.app to ${target_dir}/Nook.app..."
    rm -rf "${target_dir}/Nook.app"
    if cp -R "$src_app" "${target_dir}/Nook.app"; then
        echo "Successfully installed Nook.app to ${target_dir}/Nook.app"
    else
        echo "Error: Failed to copy Nook.app to ${target_dir}" >&2
        exit 1
    fi

    # Also install panel and daemon binaries directly into ~/.local/bin
    if [ -f "${target_dir}/Nook.app/Contents/MacOS/nook-panel" ]; then
        cp "${target_dir}/Nook.app/Contents/MacOS/nook-panel" "$BIN_DIR/nook-panel"
        chmod +x "$BIN_DIR/nook-panel"
        echo "Installed nook-panel to ${BIN_DIR}/nook-panel"
    elif [ -f "${target_dir}/Nook.app/Contents/MacOS/Nook" ]; then
        cp "${target_dir}/Nook.app/Contents/MacOS/Nook" "$BIN_DIR/nook-panel"
        chmod +x "$BIN_DIR/nook-panel"
        echo "Installed nook-panel to ${BIN_DIR}/nook-panel"
    fi
    if [ -f "${target_dir}/Nook.app/Contents/MacOS/nookd" ]; then
        cp "${target_dir}/Nook.app/Contents/MacOS/nookd" "$BIN_DIR/nookd"
        chmod +x "$BIN_DIR/nookd"
        echo "Installed nookd to ${BIN_DIR}/nookd"
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

    if [ ! -d "$ROOT_DIR/worker/.venv" ] && command -v uv >/dev/null 2>&1; then
        echo "Setting up worker virtual environment..."
        (cd "$ROOT_DIR/worker" && uv sync --quiet || true)
    fi

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
    TAG_NAME=""

    # 1. Try authenticated API if token exists
    if [ -n "$GITHUB_TOKEN" ] || [ -n "$GH_TOKEN" ]; then
        AUTH_HEADER="Authorization: token ${GITHUB_TOKEN:-$GH_TOKEN}"
        API_URL="https://api.github.com/repos/${GITHUB_REPO}/releases/latest"
        RELEASE_DATA=$(secure_curl -H "$AUTH_HEADER" -fsSL "$API_URL" 2>/dev/null || true)
        if [ -n "$RELEASE_DATA" ]; then
            TAG_NAME=$(echo "$RELEASE_DATA" | grep '"tag_name":' | head -n 1 | cut -d '"' -f 4)
        fi
    fi

    # 2. Fall back to GitHub web redirect (not rate-limited)
    if [ -z "$TAG_NAME" ]; then
        REDIRECT_HEADER=$(curl -sI "https://github.com/${GITHUB_REPO}/releases/latest" 2>/dev/null | grep -i "^location:" | head -n 1 | tr -d '\r\n')
        if [ -n "$REDIRECT_HEADER" ]; then
            TAG_NAME=$(echo "$REDIRECT_HEADER" | sed -e 's/.*tag\///')
        fi
    fi

    if [ -z "$TAG_NAME" ]; then
        echo "Error: Could not resolve latest release tag from GitHub." >&2
        exit 1
    fi

    VERSION="${TAG_NAME#v}"
    echo "Latest release: $TAG_NAME"
    DOWNLOAD_BASE="https://github.com/${GITHUB_REPO}/releases/download/${TAG_NAME}"

    if [ "$PLATFORM" = "macos" ]; then
        if [ "$ARCH_NORM" = "arm64" ]; then
            DMG_NAME="Nook_${VERSION}_aarch64.dmg"
        else
            DMG_NAME="Nook_${VERSION}_x64.dmg"
        fi
        DMG_URL="${DOWNLOAD_BASE}/${DMG_NAME}"

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

        # If nookd was not inside the DMG, fetch standalone daemon asset if available
        if [ ! -f "$BIN_DIR/nookd" ]; then
            NOOKD_URL="${DOWNLOAD_BASE}/nookd_macOS_${ARCH_NORM}"
            if secure_curl -fSL "$NOOKD_URL" -o "$BIN_DIR/nookd" 2>/dev/null; then
                chmod +x "$BIN_DIR/nookd"
                echo "Downloaded nookd daemon to $BIN_DIR/nookd"
            fi
        fi
        if [ ! -f "$BIN_DIR/nookctl" ]; then
            NOOKCTL_URL="${DOWNLOAD_BASE}/nookctl_macOS_${ARCH_NORM}"
            if secure_curl -fSL "$NOOKCTL_URL" -o "$BIN_DIR/nookctl" 2>/dev/null; then
                chmod +x "$BIN_DIR/nookctl"
                echo "Downloaded nookctl control tool to $BIN_DIR/nookctl"
            fi
        fi
    elif [ "$PLATFORM" = "linux" ]; then
        if [ "$ARCH_NORM" = "x86_64" ]; then
            DEB_NAME="nook_${VERSION}_amd64.deb"
            APPIMAGE_NAME="Nook_${VERSION}_amd64.AppImage"
        else
            DEB_NAME="nook_${VERSION}_arm64.deb"
            APPIMAGE_NAME="Nook_${VERSION}_arm64.AppImage"
        fi
        DEB_URL="${DOWNLOAD_BASE}/${DEB_NAME}"
        APPIMAGE_URL="${DOWNLOAD_BASE}/${APPIMAGE_NAME}"

        TMP_DEB=$(mktemp /tmp/nook-installer.XXXXXX.deb)
        TMP_FILES="$TMP_FILES $TMP_DEB"

        echo "Downloading $DEB_URL..."
        if secure_curl -fSL "$DEB_URL" -o "$TMP_DEB" 2>/dev/null; then
            echo "Installing Debian package..."
            sudo dpkg -i "$TMP_DEB" || sudo apt-get install -f -y
        else
            TMP_APPIMAGE="${BIN_DIR}/nook-panel"
            echo "Downloading AppImage to $TMP_APPIMAGE..."
            if ! secure_curl -fSL "$APPIMAGE_URL" -o "$TMP_APPIMAGE"; then
                echo "Error: Failed to download Linux release asset (.deb or .AppImage)." >&2
                exit 1
            fi
            chmod +x "$TMP_APPIMAGE"
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

# 4. Initialize default configuration if missing
CONFIG_DIR="$HOME/Library/Application Support/Nook"
if [ "$PLATFORM" = "linux" ]; then
    CONFIG_DIR="$HOME/.config/nook"
fi
mkdir -p "$CONFIG_DIR"
CONFIG_FILE="$CONFIG_DIR/config.toml"
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Initializing default configuration at $CONFIG_FILE..."
    cat << 'EOF' > "$CONFIG_FILE"
base_url = "https://proxy-foundry.centralindia.cloudapp.azure.com/v1"
model = "gpt-oss-120b-medium"
database_url = "postgres://localhost/nook"
max_input_tokens = 1000000
summarize_at_tokens = 750000
EOF
fi

INSTALLED_SUCCESS=1

echo ""
echo "=== Nook installed successfully! ==="
echo "Launch Nook using:"
echo "  - Terminal command: nook"
echo "  - Global shortcut: Option+Space (macOS) / Alt+Space (Linux & Windows)"
