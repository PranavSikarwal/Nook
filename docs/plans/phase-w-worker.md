# Phase W engineering plan: Worker

Phase W implements the Worker process in `worker/`. The Worker executes the Deep Agent, streams text deltas to the Daemon, records checkpoints in Postgres, and manages attachments, titles, cancellations, and memory deletion.

## System boundaries and communication

```
Daemon  <--- stdin (JSON lines) --->  Worker
        <--- stdout (JSON lines) --->
```

1. The Daemon spawns the Worker as a child process.
2. The Daemon provides runtime settings through environment variables.
3. The Daemon sends requests over stdin as single-line JSON objects ending with `\n`.
4. The Worker writes event lines to stdout ending with `\n` and flushes stdout immediately after each write.

## Module layout

```
worker/
  pyproject.toml
  src/nook_worker/
    __init__.py
    main.py
    config.py
    protocol.py
    checkpointer.py
    agent.py
    attachments.py
    titles.py
    fake_agent.py
  tests/
    conftest.py
    test_protocol.py
    test_fake_worker.py
    test_attachments.py
    test_titles.py
    test_checkpointer.py
    test_real_agent.py
```

## Sequential runtime flow

1. The Daemon starts `nook_worker` with configuration variables in the environment.
2. The Worker reads configuration values with `config.py`.
3. The Worker initializes the Postgres connection pool and runs checkpointer database setup.
4. The Worker writes a `ready` event line to stdout and flushes the stream.
5. The Worker starts an asynchronous reader on stdin to process incoming request lines.
6. When a `run` request arrives, the Worker creates an asynchronous task, loads attachment files, invokes the agent stream, and writes `message_started` and `text_delta` events.
7. When the agent stream ends, the Worker writes a `message_finished` event with status `complete`.
8. When a `cancel` request arrives, the Worker cancels the active task and writes a `message_finished` event with status `cancelled`.
9. When a `title` request arrives, the Worker invokes the title prompt on the model and writes a `title_ready` event.
10. When a `delete_chat` request arrives, the Worker removes thread checkpoints from Postgres and writes a `deleted` event.
11. When a `shutdown` request arrives, the Worker closes database connections and terminates cleanly with exit code 0.

## Tasks and implementation sequence

### W1. Worker skeleton with a fake agent
1. Create `worker/pyproject.toml` with project metadata and dependencies.
2. Implement `protocol.py` with Pydantic models matching `contracts/daemon-worker.schema.json`.
3. Implement `fake_agent.py` to yield scripted events for testing the event loop.
4. Implement `main.py` with asynchronous stdin line processing and event dispatch.
5. Write `tests/test_protocol.py` to validate models against `contracts/examples/daemon-worker.ndjson`.
6. Write `tests/test_fake_worker.py` to verify stdin and stdout message flows.
7. Verify piping a `run` request into `python -m nook_worker` produces `ready`, `message_started`, `text_delta`, and `message_finished` lines.

### W2. Real agent with streaming
1. Implement `config.py` using `pydantic-settings` to load `NOOK_*` variables.
2. Implement `agent.py` to construct `ChatOpenAI` with `use_responses_api=False`, `streaming=True`, and `profile={"max_input_tokens": 1000000}`.
3. Configure `HarnessProfile` in `agent.py` to exclude `execute` and disable the general purpose subagent.
4. Build `create_deep_agent` with empty tools and a system prompt defining Nook as a desktop assistant.
5. Stream message chunks from `agent.astream` with `stream_mode="messages"`.
6. Write `tests/test_real_agent.py` to verify responses and ensure tool calls remain blocked.

### W3. Postgres checkpointer and memory
1. Implement `checkpointer.py` using `AsyncConnectionPool` and `AsyncPostgresSaver`.
2. Configure `search_path` to `worker` in the database connection string.
3. Run `checkpointer.setup()` on startup to build the required tables.
4. Attach the checkpointer to the agent with `thread_id` set to the Chat id.
5. Write `tests/test_checkpointer.py` to verify multi-turn memory retention across simulated restarts.

### W4. Attachments
1. Implement `attachments.py` to read images, text files, and PDF documents.
2. Convert image files to base64 data URLs in image content blocks.
3. Wrap text files in fenced code blocks with the source file name.
4. Extract text from PDF documents using `pypdf`.
5. Enforce the limit of 200000 characters per file and append the `[truncated]` marker when exceeded.
6. Return an `attachment_invalid` error event when a file is missing or unreadable.
7. Write `tests/test_attachments.py` to verify text extraction, truncation, and image blocks.

### W5. Summarization at 750k tokens
1. Configure summarization to trigger at 750000 tokens using `NOOK_SUMMARIZE_AT_TOKENS`.
2. Override `SummarizationMiddleware` or adjust the model profile so summarization starts at 750000 tokens.
3. Verify that the token counter assigns 85 tokens per image block without counting base64 characters.
4. Record the summarization override and image token details in `docs/spec/03-worker.md`.

### W6. Titles
1. Implement `titles.py` to query the model for a concise title based on the first user message.
2. Constrain the title to at most six words.
3. Strip surrounding quotation marks and trailing punctuation marks from the result.
4. Return `title_ready` on success or an error event on failure.
5. Write `tests/test_titles.py` to verify cleaning logic and length limits.

### W7. Cancel and error mapping
1. Maintain an active task mapping indexed by `request_id` in `main.py`.
2. Cancel the matching task when receiving a `cancel` request and emit `message_finished` with status `cancelled`.
3. Catch connection errors and map them to `endpoint_unreachable`.
4. Catch HTTP errors from the endpoint and map them to `endpoint_error`.
5. Catch database connection failures and map them to `database_unavailable`.
6. Catch invalid request inputs and map them to `invalid_request`.
7. Catch unhandled errors and map them to `internal`.

### W8. Delete chat memory
1. Handle `delete_chat` requests in `main.py`.
2. Call `checkpointer.adelete_thread(chat_id)` to remove all checkpoint records for the thread.
3. Emit a `deleted` event with the Chat id on success.
4. Write tests in `test_checkpointer.py` to verify table rows are removed and prior context is forgotten.

## Verification commands

```sh
uv run ruff check .
uv run ruff format --check .
uv run pyright
uv run pytest
```
