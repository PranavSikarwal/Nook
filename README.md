# Nook

Nook is a cross-platform desktop overlay assistant. It lets you ask questions with a global shortcut (`Option+Space` on macOS, `Alt+Space` on Linux and Windows), streams replies as formatted Markdown, and moves behind other windows naturally upon click-away.

## Architecture

Nook consists of three processes:
1. **Panel**: Cross-platform desktop overlay in `panel/` built with Tauri v2, React 19, and Tailwind CSS. It automatically manages the background daemon.
2. **Daemon**: Rust supervisor in `daemon/`. It coordinates chats, transcripts, migrations, and watches the worker process over a local socket.
3. **Worker**: Python process in `worker/`. It runs Deep Agents, persists conversation memory in Postgres, and streams completions from an OpenAI-compatible model endpoint.

All processes share a local PostgreSQL database named `nook`.

---

## Downloads

Download native pre-built packages from [GitHub Releases](https://github.com/PranavSikarwal/Nook/releases):

- **macOS (Apple Silicon)**: `Nook_<version>_aarch64.dmg`
- **Ubuntu / Debian**: `nook_<version>_amd64.deb` or `Nook_<version>_amd64.AppImage`
- **Windows (x64)**: `Nook_<version>_x64-setup.exe` or `Nook_<version>_x64.msi`

---

## Quick installation

### One-command install (macOS & Linux)

Install Nook directly with a single command:
```sh
curl -fsSL https://raw.githubusercontent.com/PranavSikarwal/Nook/main/install.sh | sh
```

Or from a cloned repository:
```sh
./install.sh
```

This installs `Nook.app` to `/Applications` on macOS (or Debian package on Linux) and sets up the global `nook` command in `~/.local/bin`.

---

## How Nook starts

Nook is a self-starting desktop application. You do not need to manage background services or terminal scripts:

1. **Launch `Nook`** (via app icon, terminal command `nook`, or global hotkey).
2. **Automatic daemon startup**: Nook automatically detects and starts `nookd` in the background if it is not already running.
3. **Clean exit**: When you quit Nook, any child daemon process is terminated cleanly.

### Custom binary location

If you want Nook to use a specific `nookd` binary, specify its path via an environment variable in your shell profile or `.env`:
```sh
export NOOKD_PATH="/custom/path/to/nookd"
```

If `NOOKD_PATH` is not set, Nook locates the binary automatically:
1. Embedded inside the app bundle (`Nook.app/Contents/MacOS/nookd`) or next to `nook-panel`.
2. In your workspace build paths (`daemon/target/release/nookd`).
3. In your standard path (`~/.local/bin/nookd`, `~/.cargo/bin/nookd`, `/usr/local/bin/nookd`).

---

## Configuration and credentials

Nook connects to any self-hosted or remote OpenAI-compatible endpoint.

Configuration files are located at:
- **macOS**: `~/Library/Application Support/Nook/config.toml`
- **Linux**: `~/.config/nook/config.toml`
- **Windows**: `%APPDATA%\Nook\config.toml`

### Example `config.toml`
```toml
base_url = "https://models.example.internal/v1"
model = "your-model-name"
database_url = "postgres://localhost/nook"
max_input_tokens = 1000000
summarize_at_tokens = 750000
```

On all platforms, you can also supply configuration via a local `.env` file at the repository root:
```env
NOOK_BASE_URL=https://models.example.internal/v1
NOOK_MODEL=your-model-name
NOOK_API_KEY=your-api-key
NOOK_DATABASE_URL=postgres://localhost/nook
```

On macOS, you can also store your API key in Keychain:
```sh
security add-generic-password -s "Nook" -a "model-api-key" -w "your-api-key"
```

---

## Development and tests

### Run tests

Run daemon unit tests:
```sh
cd daemon
cargo test
cargo clippy --workspace --all-targets -- -D warnings
```

Run worker tests:
```sh
cd worker
uv run pytest
```

Build and test the frontend panel:
```sh
cd panel
npm run build
cd src-tauri
cargo clippy --all-targets -- -D warnings
```

Run end-to-end integration test:
```sh
./scripts/smoke.sh
```

---

## CLI usage (`nookctl`)

The `nookctl` CLI lets you interact with the running daemon directly:

Ping the daemon:
```sh
cargo run --manifest-path daemon/Cargo.toml --bin nookctl -- ping
```

Ask a question:
```sh
cargo run --manifest-path daemon/Cargo.toml --bin nookctl -- send "What is 2 + 2?"
```

Attach a file or image:
```sh
cargo run --manifest-path daemon/Cargo.toml --bin nookctl -- send "Summarize this document" --attach /path/to/report.pdf
```

List conversations:
```sh
cargo run --manifest-path daemon/Cargo.toml --bin nookctl -- list
```

Show a transcript:
```sh
cargo run --manifest-path daemon/Cargo.toml --bin nookctl -- show <chat-id>
```

Delete a conversation:
```sh
cargo run --manifest-path daemon/Cargo.toml --bin nookctl -- delete <chat-id>
```

---

## License

This project is licensed under the [MIT License](LICENSE).
