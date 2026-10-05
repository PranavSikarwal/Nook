#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# 1. Load environment variables from .env if present without echoing any secrets
if [[ -f "$ROOT_DIR/.env" ]]; then
    set -a
    # shellcheck disable=SC1091
    . "$ROOT_DIR/.env"
    set +a
fi

echo "=== Nook End-to-End Smoke Test ==="

# 2. Check PostgreSQL availability
if ! pg_isready -d "${NOOK_DATABASE_URL:-postgres://localhost/nook}" >/dev/null 2>&1; then
    echo "Error: PostgreSQL is not ready or database 'nook' is unreachable." >&2
    exit 1
fi
echo "PostgreSQL is reachable."

# 3. Build daemon binaries if missing
NOOKD_BIN="$ROOT_DIR/daemon/target/debug/nookd"
NOOKCTL_BIN="$ROOT_DIR/daemon/target/debug/nookctl"

if [[ ! -f "$NOOKD_BIN" ]] || [[ ! -f "$NOOKCTL_BIN" ]]; then
    echo "Building nookd and nookctl..."
    (cd "$ROOT_DIR/daemon" && cargo build -p nookd -p nookctl)
fi

# 4. Verify worker virtual environment
if [[ ! -d "$ROOT_DIR/worker/.venv" ]]; then
    echo "Error: worker environment not found at $ROOT_DIR/worker/.venv. Run 'uv sync' in worker directory first." >&2
    exit 1
fi

# 5. Stop any existing running nookd process to ensure a clean test instance
if pgrep -x "nookd" >/dev/null 2>&1; then
    echo "Stopping existing nookd process..."
    pkill -TERM -x "nookd" || true
    sleep 2
fi

SOCKET_PATH="$HOME/Library/Application Support/Nook/daemon.sock"
if [[ -S "$SOCKET_PATH" ]]; then
    rm -f "$SOCKET_PATH"
fi

# 6. Start daemon in background and capture logs
SMOKE_LOG=$(mktemp /tmp/nookd-smoke.XXXXXX.log)
echo "Starting nookd in background (log: $SMOKE_LOG)..."
"$NOOKD_BIN" > "$SMOKE_LOG" 2>&1 &
DAEMON_PID=$!

TEST_IMG=""

cleanup() {
    local exit_code=$?
    echo "Cleaning up smoke test resources..."
    if [[ -n "${DAEMON_PID:-}" ]] && kill -0 "$DAEMON_PID" 2>/dev/null; then
        echo "Stopping daemon (PID $DAEMON_PID)..."
        kill -TERM "$DAEMON_PID" 2>/dev/null || true
        wait "$DAEMON_PID" 2>/dev/null || true
    fi
    if [[ -n "${TEST_IMG:-}" ]] && [[ -f "$TEST_IMG" ]]; then
        rm -f "$TEST_IMG"
    fi
    if [[ $exit_code -ne 0 ]]; then
        echo "Smoke test FAILED with exit code $exit_code." >&2
        if [[ -f "$SMOKE_LOG" ]]; then
            echo "--- Last 50 lines of daemon log ---" >&2
            tail -n 50 "$SMOKE_LOG" >&2 || true
            echo "-----------------------------------" >&2
        fi
    else
        echo "Smoke test PASSED successfully."
        rm -f "$SMOKE_LOG"
    fi
    exit "$exit_code"
}
trap cleanup EXIT INT TERM

# 7. Wait for daemon to become ready
echo "Waiting for nookd socket and ping..."
READY=0
for _ in $(seq 1 30); do
    if [[ -S "$SOCKET_PATH" ]] && "$NOOKCTL_BIN" ping >/dev/null 2>&1; then
        READY=1
        break
    fi
    if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
        echo "Error: nookd process exited prematurely." >&2
        exit 1
    fi
    sleep 1
done

if [[ "$READY" -ne 1 ]]; then
    echo "Error: nookd failed to respond to ping within 30 seconds." >&2
    exit 1
fi
echo "nookd is ready and responding to ping."

# 8. Generate test chat ID
CHAT_ID=$(uuidgen | tr '[:upper:]' '[:lower:]')
echo "Using test Chat ID: $CHAT_ID"

# 9. Turn 1: Establish memory
echo "Sending Turn 1: establishing memory..."
TURN1_RESP=$("$NOOKCTL_BIN" send "Remember that the secret phrase is CRIMSON-SPARROW-42. Reply with only OK." --chat "$CHAT_ID")
echo "Turn 1 reply received."
if [[ -z "$TURN1_RESP" ]]; then
    echo "Error: Turn 1 response was empty." >&2
    exit 1
fi

# 10. Turn 2: Test memory recall
echo "Sending Turn 2: checking memory recall in same chat..."
TURN2_RESP=$("$NOOKCTL_BIN" send "What is the secret phrase?" --chat "$CHAT_ID")
echo "Turn 2 reply: $TURN2_RESP"
if ! echo "$TURN2_RESP" | grep -iq "CRIMSON-SPARROW-42"; then
    echo "Error: Turn 2 failed to recall 'CRIMSON-SPARROW-42'." >&2
    exit 1
fi
echo "Turn 2 successfully recalled secret phrase."

# 11. Turn 3: Attach an image
echo "Preparing test image attachment..."
TEST_IMG="/tmp/smoke-test-${CHAT_ID}.png"
echo "iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAIAAAD8GO2jAAAAKElEQVR4nO3NsQ0AAAzCMP5/un0CNkuZ41wybXsHAAAAAAAAAAAAxR4yw/wuPL6QkAAAAABJRU5ErkJggg==" | base64 -d > "$TEST_IMG"

echo "Sending Turn 3: question with image attachment..."
TURN3_RESP=$("$NOOKCTL_BIN" send "What color is this image?" --chat "$CHAT_ID" --attach "$TEST_IMG")
echo "Turn 3 reply: $TURN3_RESP"
if [[ -z "$TURN3_RESP" ]]; then
    echo "Error: Turn 3 response was empty." >&2
    exit 1
fi
echo "Turn 3 successfully processed image attachment."

# 12. List chats and verify test chat presence
echo "Listing chats..."
LIST_RESP=$("$NOOKCTL_BIN" list)
echo "$LIST_RESP"
if ! echo "$LIST_RESP" | grep -q "$CHAT_ID"; then
    echo "Error: Chat ID $CHAT_ID not found in chat list." >&2
    exit 1
fi
echo "Chat ID $CHAT_ID verified in chat list."

# 13. Delete chat and verify removal
echo "Deleting chat $CHAT_ID..."
DEL_RESP=$("$NOOKCTL_BIN" delete "$CHAT_ID")
echo "$DEL_RESP"

echo "Verifying chat deletion..."
LIST_AFTER=$("$NOOKCTL_BIN" list)
if echo "$LIST_AFTER" | grep -q "$CHAT_ID"; then
    echo "Error: Chat ID $CHAT_ID is still present after deletion." >&2
    exit 1
fi
echo "Chat ID $CHAT_ID confirmed removed from chat list."
