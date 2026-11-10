# Nook

Nook is a cross-platform desktop overlay assistant. The Tauri Panel opens with
a global shortcut, sends requests to a local Rust Daemon, and renders streamed
responses from a Python Worker.

## Install

Download a platform package from [GitHub Releases](https://github.com/PranavSikarwal/Nook/releases).
macOS uses an Apple Silicon disk image. Debian and Ubuntu use a `.deb` package.
Windows packages are available for preview testing.

Nook packages the Panel, Daemon, and Worker runtime. PostgreSQL is a separate,
user-managed prerequisite.

On macOS, run the installer to replace `Nook.app`. The installer keeps
`~/Library/Application Support/Nook/config.toml` during an upgrade.

## Configure

Set the model endpoint, model name, and database connection in the platform
configuration file:

- macOS: `~/Library/Application Support/Nook/config.toml`
- Linux: `~/.config/nook/config.toml`
- Windows: `%APPDATA%\Nook\config.toml`

```toml
base_url = "https://models.example.internal/v1"
model = "your-model-name"
database_url = "postgres://localhost/nook"
max_input_tokens = 1000000
summarize_at_tokens = 750000
```

The Daemon reads `NOOK_API_KEY` when it is set. On macOS, it otherwise reads
the API key from Keychain. Nook reports a database error until the configured
PostgreSQL service accepts connections.

## Develop

Start the Daemon from the repository root:

```sh
cd daemon
rtk cargo run -p nookd
```

Start the Panel in another terminal:

```sh
cd panel
rtk npm install
rtk npm run tauri dev
```

The development stack is separate from the installed application. A development
Daemon can use source-checkout configuration and Worker files.

## Test

Run the checks for the component that changed:

```sh
cd daemon && rtk cargo test
cd worker && rtk uv run pytest
cd panel && rtk npm run build
rtk uv run --with pytest --with jsonschema pytest tests/contracts
```

See [docs/testing.md](docs/testing.md) for native desktop and package checks.

## Documentation

- [Architecture](docs/architecture.md)
- [Contracts](docs/contracts.md)
- [Tools and authorization](docs/tools-and-authorization.md)
- [Release and installation](docs/release.md)
- [Testing](docs/testing.md)
- [Implementation plan](docs/plan.md)
- [Architecture decisions](docs/adr/)
- [Contribution guide](CONTRIBUTING.md)
