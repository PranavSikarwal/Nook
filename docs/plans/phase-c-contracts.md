# Phase C implementation plan: Contracts

Contracts govern all communication between the Panel, Daemon, and Worker processes. They are implemented before writing application code.

## Goals

1. Define JSON Schemas for both communication channels:
   - Panel to Daemon Unix socket protocol (`contracts/panel-daemon.schema.json`).
   - Daemon to Worker stdin/stdout protocol (`contracts/daemon-worker.schema.json`).
2. Provide valid newline-delimited JSON example files for every message type.
3. Provide an automated validation script `scripts/check-contracts` that validates all examples against the schemas.

## Tasks

### Task C1: Write JSON schemas and examples

- Branch: `feat/c1-json-schemas`
- Dependent on: S2 (complete)
- Artifacts:
  - `contracts/panel-daemon.schema.json`:
    - 8 Panel requests: `send_message`, `cancel`, `list_chats`, `get_chat`, `delete_chat`, `get_settings`, `set_settings`, `ping`.
    - 13 Daemon replies/events: `message_started`, `text_delta`, `tool_call_started`, `tool_call_finished`, `message_finished`, `chat_titled`, `chats`, `chat`, `deleted`, `settings`, `cancelled`, `pong`, `error`.
    - Shared objects: `Attachment`, `ErrorInfo`.
  - `contracts/daemon-worker.schema.json`:
    - 5 Daemon requests: `run`, `title`, `cancel`, `delete_chat`, `shutdown`.
    - 9 Worker events: `ready`, `message_started`, `text_delta`, `tool_call_started`, `tool_call_finished`, `message_finished`, `title_ready`, `deleted`, `error`.
    - Shared objects: `Attachment`, `ErrorInfo`.
  - `contracts/examples/panel-daemon.ndjson`: 21 valid example lines.
  - `contracts/examples/daemon-worker.ndjson`: 14 valid example lines.
- Acceptance criteria:
  - Every message type has a schema definition and an example line.
  - All example lines validate against their schema.

### Task C2: Add contract check script

- Branch: `feat/c2-contract-check` (stacked on `feat/c1-json-schemas`)
- Dependent on: C1
- Artifacts:
  - `scripts/check-contracts`: Python script using `jsonschema`, run with `uv`.
- Acceptance criteria:
  - Script passes with exit code 0 on the valid examples.
  - Script fails with non-zero exit code when an example is deliberately mutated to be invalid.

## Execution sequence

1. Create branch `feat/c1-json-schemas`.
2. Author `contracts/panel-daemon.schema.json` and `contracts/daemon-worker.schema.json`.
3. Author `contracts/examples/panel-daemon.ndjson` and `contracts/examples/daemon-worker.ndjson`.
4. Validate examples against schemas.
5. Commit on `feat/c1-json-schemas`.
6. Create branch `feat/c2-contract-check`.
7. Author `scripts/check-contracts` and run positive and negative tests.
8. Commit on `feat/c2-contract-check`.
9. Update `CHECKLIST.md`.
