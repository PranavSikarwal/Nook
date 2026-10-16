# Nook contracts

The JSON Schema files in `contracts/` are the protocol authority. This page
routes readers to those files and identifies the transports that use them.

## Schema authority

- `contracts/panel-daemon.schema.json` defines Panel and Daemon messages.
- `contracts/daemon-worker.schema.json` defines Daemon and Worker messages.
- `contracts/examples/` contains newline-delimited examples checked by contract
  tests.

A schema change must update the matching examples and every implementation that
encodes or decodes the changed message.

## Transports

The Panel and Daemon use newline-delimited JSON over a user-local IPC endpoint.
macOS and Linux use a Unix socket. Windows uses a named pipe. The Daemon starts
the Worker and exchanges newline-delimited JSON through the Worker's standard
input and output.

## Tool messages

The schema includes approval and tool-execution messages. The current runtime
wiring for Cedar and YAML policy ownership is tracked in the closeout plan. The
schemas remain authoritative even while that work is incomplete.

## Verification

Run the contract test suite after changing a schema or an example:

```sh
rtk uv run --with pytest --with jsonschema pytest tests/contracts
```
