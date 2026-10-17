#!/bin/sh
# Runs the Daemon checks on Linux inside a container that has bubblewrap.
# Usage: scripts/test-linux.sh [cargo test arguments]
# The container needs relaxed seccomp, AppArmor, and system paths so that
# bubblewrap can create namespaces and mount /proc.
set -eu

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

docker run --rm -i \
    -v "$ROOT_DIR":/repo \
    -v nook-linux-cargo:/usr/local/cargo/registry \
    --security-opt seccomp=unconfined \
    --security-opt apparmor=unconfined \
    --security-opt systempaths=unconfined \
    -e NOOK_REQUIRE_SANDBOX=1 \
    -e CARGO_TARGET_DIR=/tmp/target \
    rust:1-bookworm sh -s -- "$@" <<'INNER'
set -eu
apt-get update -qq >/dev/null
apt-get install -y -qq bubblewrap >/dev/null
rustup component add clippy >/dev/null 2>&1
cd /repo/daemon
if [ "$#" -eq 0 ]; then
    cargo clippy --locked --workspace --all-targets -- -D warnings
    cargo test --locked --workspace
else
    cargo test --locked --workspace "$@"
fi
INNER
