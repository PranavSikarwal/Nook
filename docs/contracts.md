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

The Panel and Daemon exchange `approval_requested` and `approval_decision`. The
Daemon sends `approval_requested` to the Panel only when a tool call needs the
user. The Panel answers with `approval_decision`.

`approval_requested` carries `actions`, an array of action objects. Each object
has three required string fields and allows no other fields:

- `id` is one of `allow_once`, `allow_for_chat`, `allow_for_chat_host`,
  `always_allow`, or `deny`.
- `label` is the text of the button. It is not empty.
- `description` says in plain words what the action covers. It is not empty.

The Panel draws one button for each object, using `label` and `description`
from the message. `approval_decision` carries the `id` of the chosen action in
its `action` field.

The Daemon and Worker do not use `approval_requested` or `approval_decision`.
The Daemon and Worker schema does not list them.

## Tool authorization

The Worker asks the Daemon before it runs a tool. Two Daemon and Worker
messages carry this exchange.

- `authorize_tool` goes from Worker to Daemon. It carries `request_id`,
  `message_id`, `call_id`, `chat_id`, `tool_name`, and `arguments`. All fields
  are required. `arguments` is a string that holds JSON text. The message allows
  no other fields.
- `authorization_result` goes from Daemon to Worker. It carries `request_id`,
  `call_id`, and `decision`. `decision` is `allow` or `deny`. The optional
  `reason` is a short fixed string. The message allows no other fields.

The Worker steps:

1. The Worker validates the tool arguments.
2. The Worker sends `authorize_tool`.
3. The Worker waits for `authorization_result` with the same `request_id` and
   `call_id`. It ignores any other result.
4. The Worker runs the tool executor only if `decision` is `allow`.
5. The Worker does not run the executor if `decision` is `deny`.

The Daemon steps:

1. The Daemon denies the call if `chat_id` is not the chat of the running
   request.
2. The Daemon builds one normalized request from the tool name and arguments.
   It denies the call if the tool is unknown or the arguments are invalid.
3. The Daemon asks Cedar. If Cedar allows the call, the Daemon sends `allow`.
   This covers tools in the `allow` tier and calls that match a scoped grant.
4. The Daemon sends `deny` if the tool is in the `deny` tier.
5. Otherwise the Daemon sends `approval_requested` to the Panel. The action
   objects come from `policy/tools.yaml`.
6. The Panel sends `approval_decision`. The Daemon rejects an action that it did
   not offer.
7. The Daemon records a scoped grant for the chosen action and asks Cedar again.
   It sends `allow` only if Cedar allows the call.

A `once` grant works for one call only. Cancel sends `deny` with the reason
`Cancelled` for each call that still waits.

The Daemon owns host normalization and argument-digest normalization. The digest
is the SHA-256 hex of the canonical JSON of the arguments. Canonical JSON uses
sorted keys and no whitespace. The Worker never sends a digest.

The `reason` is one of a small set of fixed strings, such as `Invalid
arguments`, `Unknown tool`, `Denied by the user`, `Authorization failed`, `Approval timed out`, `Approval could not be shown`, and
`Cancelled`. It never contains error detail. The Daemon writes the detail to its
log.

## Tools that the Daemon runs

The Daemon runs tools that can touch the local machine. These tools reach the
Worker through a catalog, and the Worker asks the Daemon to run each call.

- `tool_catalog` goes from Daemon to Worker once, right after the Worker reports
  `ready`. It lists each tool with `id`, `name` (the name the model sees),
  `description`, and `input_schema` (a JSON Schema of type `object`). The
  Daemon sends no catalog when it has no such tool.
- `execute_tool` goes from Worker to Daemon. It has the same fields as
  `authorize_tool`.
- `tool_result` goes from Daemon to Worker. It carries `request_id`, `call_id`,
  `status` (`ok`, `error`, or `denied`), and `output`.

The Worker steps:

1. The Worker builds one tool for each catalog entry and rebuilds its agent.
2. The Worker checks the arguments against `input_schema`. It sends nothing if
   they do not match.
3. The Worker sends `execute_tool` and waits for `tool_result` with the same
   `request_id` and `call_id`.
4. The Worker returns `output` to the model. If `status` is `denied`, it marks
   the reply denied, and the reply ends as it does for a web tool.

The Daemon steps are the authorization steps above, with two changes. After an
allow, the Daemon sends `tool_call_started` to the Panel, runs the tool, sends
`tool_call_finished`, and then sends `tool_result`. After a deny, it sends
`tool_result` with `denied` and the fixed reason, and no tool events.

The Daemon rejects a mix-up between the two paths. `authorize_tool` for a tool
that runs in the Daemon gets `deny` with `Tool runs in the Daemon`.
`execute_tool` for a tool that runs in the Worker gets `denied` with `Tool runs
in the Worker`. `output` is cut at 64 KiB. Cancel aborts a running call and
sends no `tool_result`.

## Verification

Run the contract test suite after changing a schema or an example:

```sh
rtk uv run --with pytest --with jsonschema pytest tests/contracts panel/e2e/test_fake_worker.py
```

The second file checks that the deterministic fake Worker only sends messages
that the Daemon and Worker schema allows.
