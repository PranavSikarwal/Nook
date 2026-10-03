# Nook

Nook is a macOS overlay chatbot. It lets you ask questions with Option+Space, reads answers as Markdown, and dismisses with the same key.

## Architecture

Nook has three processes:
1. Panel: SwiftUI overlay in `app/`.
2. Daemon: Rust supervisor and storage coordinator in `daemon/`.
3. Worker: Python agent runner in `worker/`.

The Daemon and Worker connect to a local Postgres database named `nook`.

## Prerequisites

- macOS 14 or newer
- Swift command line tools (`swift --version`)
- Rust toolchain (`cargo --version`)
- Python package manager `uv` (`uv --version`)
- Postgres running locally with database `nook`

## Build and run each layer

### Worker

The Worker runs the agent using Deep Agents.

Install dependencies:
```sh
cd worker
uv sync
```

Run tests:
```sh
cd worker
uv run pytest
```

Run the Worker directly:
```sh
cd worker
uv run python -m nook_worker
```

### Daemon

The Daemon manages the Chat list, Transcript, and Worker supervisor.

Build the Daemon and tools:
```sh
cd daemon
cargo build
```

Run tests:
```sh
cd daemon
cargo test
```

Run the Daemon:
```sh
cd daemon
cargo run --bin nookd
```

Run the command-line client:
```sh
cd daemon
cargo run --bin nookctl -- --help
```

### Panel

The Panel renders the overlay.

Build the Panel:
```sh
cd app
swift build
```

Run Panel tests:
```sh
cd app
swift test
```

Bundle the application:
```sh
./scripts/bundle.sh
```

### Model check

Test your Model endpoint before running the agent:
```sh
cd model-check
NOOK_BASE_URL="https://..." NOOK_API_KEY="..." NOOK_MODEL="..." uv run check_model.py
```
