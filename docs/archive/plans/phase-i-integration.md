# Phase I implementation plan: Integration

Phase I integrates the Panel, Daemon, Worker, and Database into an operational system. It validates the multi-turn chat flow, attachments, listing, and deletion through automated smoke testing, and provides an end-to-end installation runbook for a clean machine.

## Architectural context

- The Daemon (`nookd`) supervises `nook_worker` over stdin and stdout using line-delimited JSON.
- `nookctl` talks to `nookd` over the Unix domain socket at `~/Library/Application Support/Nook/daemon.sock`.
- The Worker manages LangGraph checkpoint memory in Postgres (`nook` database) and streams replies from the OpenAI-compatible model endpoint.
- Model credentials (`NOOK_BASE_URL`, `NOOK_API_KEY`, `NOOK_MODEL`) are read from the environment or `.env` without printing or logging secrets.

## Phasing and tasks

### I1. End-to-end smoke test (`scripts/smoke.sh`)

1. Create `scripts/smoke.sh` executable script.
2. Load configuration from `.env` if present in the repository root without logging secret values.
3. Verify prerequisites before starting:
   - PostgreSQL is running and accepts connections to `nook`.
   - Python virtual environment and dependencies in `worker/` are installed (`uv sync --project worker`).
   - Binaries `nookd` and `nookctl` are built (`cargo build -p nookd -p nookctl` if missing).
4. Allow `nook-core` configuration to accept environment variable overrides (`NOOK_BASE_URL`, `NOOK_MODEL`, `NOOK_DATABASE_URL`) so that environment settings seamlessly apply to `nookd`.
5. Start `nookd` in the background with stdout and stderr redirected to a temporary smoke test log file.
6. Poll `nookctl ping` until `pong` arrives or a 30-second timeout expires.
7. Execute automated conversational test flow using a dedicated test `CHAT_ID`:
   - Turn 1: Send a question establishing memory (for example: "Remember that the secret color is cobalt blue. Reply with OK.") and verify completion.
   - Turn 2: Send a follow-up in the same chat asking for the secret color. Verify the response contains "cobalt blue", proving LangGraph checkpoint memory persistence.
   - Turn 3: Generate a small test image (32x32 PNG) and send a question with `--attach <image-path>`. Verify response completion.
8. Verify chat management commands:
   - Call `nookctl list` and assert that the test `CHAT_ID` appears in the output.
   - Call `nookctl delete <CHAT_ID>` and assert successful deletion.
   - Call `nookctl list` and assert that the test `CHAT_ID` is no longer present.
9. Cleanup and exit code:
   - Register exit trap ensuring background `nookd` is terminated via SIGTERM.
   - Remove temporary test image and artifacts.
   - Exit with code 0 on success, or non-zero on any failure.

### I2. Installation and runbook (`README.md`)

1. Update top-level `README.md` to provide a complete guide from clean machine to working Nook.
2. Document toolchain setup:
   - macOS 14+ requirements.
   - Swift Command Line Tools (`xcode-select --install`).
   - Rust toolchain (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).
   - `uv` Python package manager (`brew install uv` or `curl -LsSf https://astral.sh/uv/install.sh | sh`).
   - PostgreSQL 15+ (`brew install postgresql@15`, `brew services start postgresql@15`, `createdb nook`).
3. Document credentials and model endpoint configuration:
   - Configuration file at `~/Library/Application Support/Nook/config.toml`.
   - API key storage in macOS Keychain via `security add-generic-password -s Nook -a model-api-key -w <api-key>` or `.env`.
4. Document background daemon service:
   - Building release binary (`cargo build --release --bin nookd`).
   - Installing the LaunchAgent using `scripts/install-launch-agent.sh`.
   - Log inspection at `~/Library/Logs/Nook/daemon.log`.
5. Document worker environment:
   - Installing Python dependencies (`cd worker && uv sync`).
6. Document app bundling and login item:
   - Running `./scripts/bundle.sh` to produce `build/Nook.app`.
   - Copying `Nook.app` to `/Applications/`.
   - Adding to Login Items via System Settings > General > Login Items.
   - Launching with Option+Space overlay hotkey.
7. Document test commands and smoke test run:
   - Unit tests: `(cd daemon && cargo test)`, `(cd worker && uv run pytest)`, `(cd app && swift run NookTests)`.
   - End-to-end smoke test: `./scripts/smoke.sh`.
8. Document troubleshooting for common issues:
   - Socket connection failures.
   - Database availability.
   - Model endpoint connection.
   - macOS accessibility or hotkey behavior.
