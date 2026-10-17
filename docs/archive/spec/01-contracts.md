# Contracts

Two protocols carry all traffic between layers. Both use UTF-8 JSON, one object per line, ending in `\n`. A single JSON Schema file defines each protocol. The files live in `contracts/`, and every layer validates against them.

- `contracts/panel-daemon.schema.json`
- `contracts/daemon-worker.schema.json`
- `contracts/examples/` holds sample lines for every message. Each layer's tests read these lines, so a change to a message breaks the layers that fall out of step.

Every object has a `type` field. A reader ignores unknown fields and rejects unknown `type` values with an `error`.

## Shared shapes

```json
Attachment = {
  "id": "uuid",
  "kind": "image | text | pdf",
  "name": "report.pdf",
  "mime": "application/pdf",
  "size_bytes": 12345,
  "path": "/Users/.../Nook/attachments/<chat_id>/<id>-report.pdf"
}
```

```json
ErrorInfo = {
  "code": "endpoint_unreachable | endpoint_error | worker_crashed | database_unavailable | attachment_invalid | invalid_request | internal",
  "message": "Human-readable text for display",
  "retryable": true
}
```

## Panel and Daemon

Transport is a Unix domain socket at `~/Library/Application Support/Nook/daemon.sock`, permissions `0600`. The Panel can open one connection and keep it for its whole life. Requests carry an `id` that the Daemon echoes in every response and event it sends for that request.

Requests from the Panel:

| `type` | Fields | Reply |
| --- | --- | --- |
| `send_message` | `id`, `chat_id`, `text`, `attachments[]` | Stream of events below |
| `cancel` | `id`, `target_id` (the `id` of the `send_message`) | `cancelled` |
| `list_chats` | `id` | `chats` |
| `get_chat` | `id`, `chat_id` | `chat` |
| `delete_chat` | `id`, `chat_id` | `deleted` |
| `get_settings` | `id` | `settings` |
| `set_settings` | `id`, `base_url`, `model`, `api_key` (optional, omit to keep the stored key) | `settings` |
| `ping` | `id` | `pong` |

Events and replies from the Daemon:

| `type` | Fields |
| --- | --- |
| `message_started` | `id`, `chat_id`, `message_id` |
| `text_delta` | `id`, `chat_id`, `message_id`, `text` |
| `tool_call_started` | `id`, `chat_id`, `message_id`, `call_id`, `name`, `arguments` |
| `tool_call_finished` | `id`, `chat_id`, `message_id`, `call_id`, `result` |
| `message_finished` | `id`, `chat_id`, `message_id`, `status` (`complete` or `cancelled`) |
| `chat_titled` | `chat_id`, `title` (not tied to a request `id`) |
| `chats` | `id`, `chats[]` of `{chat_id, title, updated_at}` |
| `chat` | `id`, `chat_id`, `title`, `messages[]` |
| `deleted` | `id`, `chat_id` |
| `settings` | `id`, `base_url`, `model`, `has_api_key` (never the key itself) |
| `cancelled` | `id`, `target_id` |
| `pong` | `id` |
| `error` | `id`, `error` (ErrorInfo) |

Each entry in `messages[]` is `{message_id, role, text, status, error, attachments[], created_at}`. `role` is `user` or `assistant`. `status` is `complete`, `cancelled`, or `error`.

Rules:

1. The Panel creates `chat_id` as a UUID v4 when it opens a fresh Chat. The Daemon creates the Chat row on the first `send_message` for that id.
2. The Panel writes each Attachment file into `attachments/<chat_id>/` before it sends the request. The Daemon checks type, size, and count, and replies with `error` code `attachment_invalid` on a violation.
3. A `send_message` ends with exactly one of `message_finished` or `error`.
4. `tool_call_started` and `tool_call_finished` exist so tools can be added later. v1 never sends them.

## Daemon and Worker

Transport is the Worker's stdin (Daemon to Worker) and stdout (Worker to Daemon). The Worker writes logs to stderr only. Anything else on stdout breaks the framing.

On start the Worker sends `ready` once it has connected to Postgres and built the agent. The Daemon sends nothing before `ready`.

Requests from the Daemon:

| `type` | Fields |
| --- | --- |
| `run` | `request_id`, `chat_id`, `text`, `attachments[]` |
| `title` | `request_id`, `chat_id`, `first_message` |
| `cancel` | `request_id` |
| `delete_chat` | `request_id`, `chat_id` |
| `shutdown` | none |

Events from the Worker:

| `type` | Fields |
| --- | --- |
| `ready` | `version` |
| `message_started` | `request_id`, `message_id` |
| `text_delta` | `request_id`, `message_id`, `text` |
| `tool_call_started` | `request_id`, `message_id`, `call_id`, `name`, `arguments` |
| `tool_call_finished` | `request_id`, `message_id`, `call_id`, `result` |
| `message_finished` | `request_id`, `message_id`, `status` |
| `title_ready` | `request_id`, `title` |
| `deleted` | `request_id`, `chat_id` |
| `error` | `request_id`, `error` (ErrorInfo) |

Rules:

1. `message_id` is a UUID the Worker creates for the assistant Message. The Daemon uses it as the row id.
2. The Worker handles one `run` at a time per `chat_id`. It may handle runs for different Chats at once.
3. `cancel` stops the matching `run`. The Worker then sends `message_finished` with status `cancelled`.
4. The Worker exits on `shutdown` or when stdin closes.

## Versioning

The contract has no version field in v1. Both files change together in one commit, and the examples in `contracts/examples/` are updated in the same commit.
