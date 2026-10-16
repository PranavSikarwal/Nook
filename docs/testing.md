# Testing

Nook uses unit, contract, package, and native desktop tests. The command depends
on the component that changed.

## Component checks

Run Daemon tests and Rust checks from `daemon/`:

```sh
rtk cargo test
rtk cargo clippy --workspace --all-targets -- -D warnings
```

Run Worker tests from `worker/`:

```sh
rtk uv run pytest
```

Run the Panel build from `panel/`:

```sh
rtk npm run build
```

## Contract checks

The schema examples protect compatibility across Panel, Daemon, and Worker.
Run this command after changing protocol files:

```sh
rtk uv run --with pytest --with jsonschema pytest tests/contracts
```

## Native and package checks

The native Tauri test gate contains deterministic desktop scenarios. The live
baseline uses repository secrets and does not validate pull request code.
Package workflows inspect the macOS, Debian, and Windows artifacts for the
bundled Daemon and Worker runtime.

## Tool-platform checks

The policy closeout must test startup rejection of malformed policy assets,
Cedar decisions, grant scoping, denial, cancellation, and approval resumption.
The next tool platform must test sandbox boundaries before it exposes terminal,
filesystem, browser, or native-system access.
