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

    worker_path="$src_app/Contents/Resources/worker/release_entry/release_entry"
    if [ ! -x "$worker_path" ]; then
        worker_path="$src_app/Contents/Resources/worker/release_entry"
    fi
    if [ ! -f "$src_app/Contents/Resources/daemon/nookd" ] || [ ! -x "$worker_path" ]; then
        echo "Error: Source application is missing the daemon or Worker runtime." >&2
        exit 1
    fi

    echo "Installing Nook.app to ${target_dir}/Nook.app..."
    rm -rf "${target_dir}/Nook.app"
    if cp -R "$src_app" "${target_dir}/Nook.app"; then
        echo "Successfully installed Nook.app to ${target_dir}/Nook.app"
    else
        echo "Error: Failed to copy Nook.app to ${target_dir}" >&2
        exit 1
    fi

    if [ ! -f "${target_dir}/Nook.app/Contents/Resources/daemon/nookd" ]; then
        echo "Error: Nook.app does not contain the daemon runtime." >&2
        exit 1
    fi
    worker_path="${target_dir}/Nook.app/Contents/Resources/worker/release_entry"
    if [ ! -x "$worker_path" ] && [ -x "${worker_path}/release_entry" ]; then
        worker_path="${worker_path}/release_entry"
    fi
    if [ ! -x "$worker_path" ]; then
        echo "Error: Nook.app does not contain the Worker runtime." >&2
        exit 1
    fi

    # Also install the panel launcher into ~/.local/bin.
    if [ -f "${target_dir}/Nook.app/Contents/MacOS/nook-panel" ]; then
        cp "${target_dir}/Nook.app/Contents/MacOS/nook-panel" "$BIN_DIR/nook-panel"
        chmod +x "$BIN_DIR/nook-panel"
        echo "Installed nook-panel to ${BIN_DIR}/nook-panel"
    elif [ -f "${target_dir}/Nook.app/Contents/MacOS/Nook" ]; then
        cp "${target_dir}/Nook.app/Contents/MacOS/Nook" "$BIN_DIR/nook-panel"
        chmod +x "$BIN_DIR/nook-panel"
        echo "Installed nook-panel to ${BIN_DIR}/nook-panel"
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

    if ! command -v uv >/dev/null 2>&1; then
        echo "Error: Install uv before installing from a source checkout." >&2
        exit 1
    fi
    echo "Setting up Worker virtual environment..."
    (cd "$ROOT_DIR/worker" && uv sync --locked)

    cp "$ROOT_DIR/panel/src-tauri/target/release/nook-panel" "$BIN_DIR/nook-panel"
    chmod +x "$BIN_DIR/nook-panel"

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
        if [ "$ARCH_NORM" != "arm64" ]; then
            echo "Error: macOS release installers are currently published for Apple Silicon only." >&2
            exit 1
        fi
        DMG_NAME="Nook_${VERSION}_aarch64.dmg"
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


        if [ "${NOOK_INSTALL_CLI:-0}" = "1" ]; then
            NOOKCTL_URL="${DOWNLOAD_BASE}/nookctl_macOS_${ARCH_NORM}"
            TMP_NOOKCTL=$(mktemp /tmp/nookctl-installer.XXXXXX)
            TMP_FILES="$TMP_FILES $TMP_NOOKCTL"
            secure_curl -fSL "$NOOKCTL_URL" -o "$TMP_NOOKCTL"
            install -m 755 "$TMP_NOOKCTL" "$BIN_DIR/nookctl"
        fi
    elif [ "$PLATFORM" = "linux" ]; then
        if [ "$ARCH_NORM" != "x86_64" ]; then
            echo "Error: Linux release installers are currently published for x86_64 only." >&2
            exit 1
        fi
        DEB_NAME="nook_${VERSION}_amd64.deb"
        DEB_URL="${DOWNLOAD_BASE}/${DEB_NAME}"

        TMP_DEB=$(mktemp /tmp/nook-installer.XXXXXX.deb)
        TMP_FILES="$TMP_FILES $TMP_DEB"

        echo "Downloading $DEB_URL..."
        if secure_curl -fSL "$DEB_URL" -o "$TMP_DEB" 2>/dev/null; then
            echo "Installing Debian package..."
            if ! dpkg-deb -c "$TMP_DEB" | grep -q 'nookd'; then
                echo "Error: Debian package is missing nookd." >&2
                exit 1
            fi
            if ! dpkg-deb -c "$TMP_DEB" | grep -q 'worker/release_entry'; then
                echo "Error: Debian package is missing the Worker runtime." >&2
                exit 1
            fi
            sudo dpkg -i "$TMP_DEB" || sudo apt-get install -f -y
        else
            echo "Error: No Debian package was available. The AppImage path is not supported by this installer." >&2
            exit 1
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

if ! command -v pg_isready >/dev/null 2>&1; then
    echo "PostgreSQL is required. Install PostgreSQL and ensure pg_isready is on PATH." >&2
    exit 1
fi
if ! pg_isready -d "$(sed -n 's/^database_url = \"\(.*\)\"$/\1/p' "$CONFIG_FILE")" >/dev/null 2>&1; then
    echo "PostgreSQL is not reachable at the configured database URL in $CONFIG_FILE." >&2
    echo "Start PostgreSQL or update database_url before launching Nook." >&2
    exit 1
fi

INSTALLED_SUCCESS=1

echo ""
echo "=== Nook installed successfully! ==="
echo "Launch Nook using:"
echo "  - Terminal command: nook"
echo "  - Global shortcut: Option+Space (macOS) / Alt+Space (Linux & Windows)"
