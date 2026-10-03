# Phase C engineering plan: Contracts

Phase C creates the schema definitions and test examples for inter-process communication in Nook. All subsequent phases (Worker, Daemon, Panel) build directly against these contracts.

## Process boundaries and transports

```
Panel  <--- Unix socket (JSON lines) --->  Daemon  <--- stdin/stdout (JSON lines) --->  Worker
```

1. **Panel and Daemon**:
   - Transport: Unix domain socket at `~/Library/Application Support/Nook/daemon.sock`
   - Framing: UTF-8 JSON lines ending in `\n`
   - Schema file: `contracts/panel-daemon.schema.json`
   - Example file: `contracts/examples/panel-daemon.ndjson`

2. **Daemon and Worker**:
   - Transport: child process stdin (Daemon to Worker) and stdout (Worker to Daemon)
   - Framing: UTF-8 JSON lines ending in `\n`
   - Schema file: `contracts/daemon-worker.schema.json`
   - Example file: `contracts/examples/daemon-worker.ndjson`

## Shared data types

Both schemas share the following data shapes:

1. **UUID**:
   A canonical 36-character string matching `^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`.

2. **Attachment**:
   - `id`: UUID string
   - `kind`: enum string (`"image"`, `"text"`, `"pdf"`)
   - `name`: string
   - `mime`: string
   - `size_bytes`: integer (minimum 0, maximum 10485760 per spec limit of 10 MB)
   - `path`: string

3. **ErrorInfo**:
   - `code`: enum string (`"endpoint_unreachable"`, `"endpoint_error"`, `"worker_crashed"`, `"database_unavailable"`, `"attachment_invalid"`, `"invalid_request"`, `"internal"`)
   - `message`: string
   - `retryable`: boolean

4. **ChatMessage**:
   - `message_id`: UUID string
   - `role`: enum string (`"user"`, `"assistant"`)
   - `text`: string
   - `status`: enum string (`"complete"`, `"cancelled"`, `"error"`)
   - `error`: ErrorInfo (optional or null)
   - `attachments`: array of Attachment objects
   - `created_at`: ISO 8601 date-time string

## Message inventory

### Panel to Daemon protocol (21 types)

#### Panel requests (8 types)
1. `send_message`: `id` (UUID), `chat_id` (UUID), `text` (string), `attachments` (array of Attachment)
2. `cancel`: `id` (UUID), `target_id` (UUID)
3. `list_chats`: `id` (UUID)
4. `get_chat`: `id` (UUID), `chat_id` (UUID)
5. `delete_chat`: `id` (UUID), `chat_id` (UUID)
6. `get_settings`: `id` (UUID)
7. `set_settings`: `id` (UUID), `base_url` (string), `model` (string), `api_key` (string, optional)
8. `ping`: `id` (UUID)

#### Daemon replies and events (13 types)
9. `message_started`: `id` (UUID), `chat_id` (UUID), `message_id` (UUID)
10. `text_delta`: `id` (UUID), `chat_id` (UUID), `message_id` (UUID), `text` (string)
11. `tool_call_started`: `id` (UUID), `chat_id` (UUID), `message_id` (UUID), `call_id` (string), `name` (string), `arguments` (string)
12. `tool_call_finished`: `id` (UUID), `chat_id` (UUID), `message_id` (UUID), `call_id` (string), `result` (string)
13. `message_finished`: `id` (UUID), `chat_id` (UUID), `message_id` (UUID), `status` (`"complete"` or `"cancelled"`)
14. `chat_titled`: `chat_id` (UUID), `title` (string) (Note: push event not bound to a request `id`)
15. `chats`: `id` (UUID), `chats` (array of `{chat_id, title, updated_at}`)
16. `chat`: `id` (UUID), `chat_id` (UUID), `title` (string), `messages` (array of ChatMessage)
17. `deleted`: `id` (UUID), `chat_id` (UUID)
18. `settings`: `id` (UUID), `base_url` (string), `model` (string), `has_api_key` (boolean)
19. `cancelled`: `id` (UUID), `target_id` (UUID)
20. `pong`: `id` (UUID)
21. `error`: `id` (UUID), `error` (ErrorInfo)

### Daemon to Worker protocol (14 types)

#### Daemon requests (5 types)
1. `run`: `request_id` (UUID), `chat_id` (UUID), `text` (string), `attachments` (array of Attachment)
2. `title`: `request_id` (UUID), `chat_id` (UUID), `first_message` (string)
3. `cancel`: `request_id` (UUID)
4. `delete_chat`: `request_id` (UUID), `chat_id` (UUID)
5. `shutdown`: no additional fields beyond `type`

#### Worker events (9 types)
6. `ready`: `version` (string)
7. `message_started`: `request_id` (UUID), `message_id` (UUID)
8. `text_delta`: `request_id` (UUID), `message_id` (UUID), `text` (string)
9. `tool_call_started`: `request_id` (UUID), `message_id` (UUID), `call_id` (string), `name` (string), `arguments` (string)
10. `tool_call_finished`: `request_id` (UUID), `message_id` (UUID), `call_id` (string), `result` (string)
11. `message_finished`: `request_id` (UUID), `message_id` (UUID), `status` (`"complete"` or `"cancelled"`)
12. `title_ready`: `request_id` (UUID), `title` (string)
13. `deleted`: `request_id` (UUID), `chat_id` (UUID)
14. `error`: `request_id` (UUID), `error` (ErrorInfo)

---

## Detailed task breakdown

### Task C1: Write JSON schemas and examples

- Branch name: `feat/c1-json-schemas`
- Base branch: `main`
- Steps:
  1. Create directory `contracts/examples`.
  2. Write `contracts/panel-daemon.schema.json` using JSON Schema Draft 7 with `oneOf` matching all 21 types.
  3. Write `contracts/daemon-worker.schema.json` using JSON Schema Draft 7 with `oneOf` matching all 14 types.
  4. Write `contracts/examples/panel-daemon.ndjson` with 21 sample objects, one per line.
  5. Write `contracts/examples/daemon-worker.ndjson` with 14 sample objects, one per line.
  6. Validate that each sample line parses as single-line valid JSON.

### Task C2: Add contract check script

- Branch name: `feat/c2-contract-check`
- Base branch: `feat/c1-json-schemas` (stacked)
- Steps:
  1. Write `scripts/check-contracts` in Python with inline `uv` metadata (`# /// script`, `requires-python = ">=3.11"`, `dependencies = ["jsonschema>=4.20.0"]`).
  2. Implement validation:
     - Load both schema JSON files.
     - Read both `.ndjson` files line by line.
     - Validate every line against its schema.
     - Extract `type` values from both schemas and ensure every type appears at least once in the examples.
  3. Implement CLI flags:
     - Default mode runs positive validation.
     - `--test-invalid` mode mutates a sample line and verifies that validation fails.
  4. Make the script executable (`chmod +x scripts/check-contracts`).
  5. Run Ruff and Pyright checks on `scripts/check-contracts` to ensure full compliance with `CONTRIBUTING.md`.

---

## Verification and done criteria

1. **Positive test**:
   `uv run scripts/check-contracts` exits with code 0 and logs validation success for all 35 message types.
2. **Negative test**:
   `uv run scripts/check-contracts --test-invalid` exits with code 0 confirming that invalid inputs are detected and rejected.
3. **Linter checks**:
   `uv tool run ruff check .` and `uv tool run ruff format --check .` pass with 0 errors.
4. **Tracking**:
   Update `CHECKLIST.md` with C1 and C2 checked.
