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

## Tool authorization

The Worker asks the Daemon before it runs a tool. Two Daemon and Worker
messages carry this exchange.

- `authorize_tool` goes from Worker to Daemon. It carries `request_id`,
  `message_id`, `call_id`, `chat_id`, `tool_name`, and `arguments`. All fields
  are required. `arguments` is a string that holds JSON text. The message allows
  no other fields.
- `authorization_result` goes from Daemon to Worker. It carries `request_id`,
  `call_id`, and `decision`. `decision` is `allow` or `deny`. The optional
  `reason` is a string.

The flow has five steps.

1. The Worker validates the tool arguments.
2. The Worker sends `authorize_tool`.
3. The Worker waits for `authorization_result` with the same `call_id`.
4. The Worker runs the tool executor only if `decision` is `allow`.
5. The Worker does not run the executor if `decision` is `deny`.

The Daemon owns host normalization and argument-digest normalization. The digest
is the SHA-256 hex of the canonical JSON of the arguments. Canonical JSON uses
sorted keys and no whitespace. The Worker never sends a digest.

## Verification

Run the contract test suite after changing a schema or an example:

```sh
rtk uv run --with pytest --with jsonschema pytest tests/contracts
```
