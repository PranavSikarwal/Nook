# Nook

Nook is a macOS overlay chatbot. It lets you ask questions with Option+Space, reads answers as Markdown, and dismisses with the same key.

## Architecture

Nook consists of three processes:
1. Panel: Cross-platform desktop overlay in `panel/` built with Tauri v2, React, and Tailwind CSS. It receives key events and presents the chat transcript.
2. Daemon: Rust background process in `daemon/`. It coordinates chats, transcripts, and watches the worker process over a Unix domain socket.
3. Worker: Python process in `worker/`. It runs Deep Agents, persists memory checkpoints in Postgres, and streams completions from the OpenAI-compatible model endpoint.

All processes share a local PostgreSQL database named `nook`.

## Clean machine installation

Follow these steps to set up Nook on a clean macOS machine.

### 1. Install toolchains and dependencies

Nook requires macOS 14 (Sonoma) or newer.

Install Apple Command Line Tools:
```sh
xcode-select --install
```

Install Homebrew if not already installed:
```sh
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

Install the Rust toolchain:
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Install the `uv` Python package manager:
```sh
brew install uv
```

Install and start PostgreSQL:
```sh
brew install postgresql@15
brew services start postgresql@15
```

### 2. Configure the database

Create the local database named `nook`:
```sh
createdb nook
```

Verify that PostgreSQL accepts connections:
```sh
pg_isready -d nook
```

The daemon automatically applies database migrations upon launch.

### 3. Configure model credentials

Nook connects to any self-hosted or remote OpenAI-compatible endpoint.

Configuration values live in `~/Library/Application Support/Nook/config.toml`:
```toml
base_url = "https://models.example.internal/v1"
model = "your-model-name"
database_url = "postgres://localhost/nook"
max_input_tokens = 1000000
summarize_at_tokens = 750000
```

Store your API key securely in the macOS Keychain:
```sh
security add-generic-password -s "Nook" -a "model-api-key" -w "your-api-key"
```

Alternatively, you can provide `NOOK_BASE_URL`, `NOOK_MODEL`, and `NOOK_API_KEY` in a local `.env` file at the repository root.

### 4. Set up worker dependencies

Install the Python virtual environment and dependencies:
```sh
cd worker
uv sync
cd ..
```

### 5. Install the daemon LaunchAgent

Build the daemon and install the background service:
```sh
(cd daemon && cargo build --release --bin nookd)
./scripts/install-launch-agent.sh
```

The LaunchAgent starts `nookd` immediately and keeps it running at login.

Check daemon logs to verify successful launch:
```sh
tail -f "$HOME/Library/Logs/Nook/daemon.log"
```

To stop and uninstall the service at any time:
```sh
./scripts/uninstall-launch-agent.sh
```

### 6. Build the panel app and set up login item

Compile the panel and package the `.app` bundle:
```sh
./scripts/bundle.sh
```

The script builds `build/Nook.app` and applies local ad-hoc code signing.

Copy the application to `/Applications`:
```sh
cp -R build/Nook.app /Applications/
```

Add Nook to your login items so it opens at startup:
```sh
osascript -e 'tell application "System Events" to make login item at end with properties {path:"/Applications/Nook.app", hidden:false}'
```

Launch Nook:
```sh
open /Applications/Nook.app
```

Press Option+Space to toggle the overlay.

## Verification and tests

### Smoke test

Run the end-to-end integration smoke test:
```sh
./scripts/smoke.sh
```

The script starts the daemon, asks two related multi-turn questions to test memory persistence, attaches an image, lists chats, and deletes the test chat.

### Unit tests

Run worker unit tests:
```sh
cd worker
uv run pytest
```

Run daemon unit tests:
```sh
cd daemon
cargo test
```

Build and check the panel:
```sh
cd panel
npm run build
cd src-tauri
cargo clippy --all-targets -- -D warnings
```

### Model check

Verify that your model endpoint works with Deep Agents and tool calling:
```sh
cd model-check
NOOK_BASE_URL="https://..." NOOK_API_KEY="..." NOOK_MODEL="..." uv run check_model.py
```

## CLI usage

The `nookctl` binary lets you interact with the running daemon from the terminal:

Ping the daemon:
```sh
cargo run --manifest-path daemon/Cargo.toml --bin nookctl -- ping
```

Send a question:
```sh
cargo run --manifest-path daemon/Cargo.toml --bin nookctl -- send "What is the capital of France?"
```

Attach a file or image:
```sh
cargo run --manifest-path daemon/Cargo.toml --bin nookctl -- send "Summarize this file" --attach /path/to/file.txt
```

List past chats:
```sh
cargo run --manifest-path daemon/Cargo.toml --bin nookctl -- list
```

Show chat transcript:
```sh
cargo run --manifest-path daemon/Cargo.toml --bin nookctl -- show <chat-id>
```

Delete a chat:
```sh
cargo run --manifest-path daemon/Cargo.toml --bin nookctl -- delete <chat-id>
```

## Troubleshooting

### Daemon socket not found
If `nookctl` reports that `daemon.sock` is missing:
1. Verify `nookd` is running with `pgrep -x nookd`.
2. Check `~/Library/Logs/Nook/daemon.log` for database connection or worker initialization errors.
3. If PostgreSQL was down, start it with `brew services start postgresql@15`.

### Database unavailable
If the panel shows "Database unavailable":
1. Verify the service is running: `pg_isready -d nook`.
2. Check whether the database was created: `psql -l | grep nook`.
3. Create the database if missing: `createdb nook`.

### Endpoint or authentication error
If the panel displays an inline error:
1. Confirm credentials with `model-check/check_model.py`.
2. Check that the Keychain entry exists: `security find-generic-password -s "Nook" -a "model-api-key"`.
3. Verify the model name and base URL in `~/Library/Application Support/Nook/config.toml`.
