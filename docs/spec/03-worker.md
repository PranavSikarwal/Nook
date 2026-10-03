# Worker

The Worker is a Python package in `worker/`, managed with `uv`. It reads requests from stdin, runs the agent, and writes events to stdout. `01-contracts.md` defines the messages.

## Dependencies

`deepagents`, `langchain-openai`, `langgraph-checkpoint-postgres`, `psycopg[binary,pool]`, `pypdf`, `pydantic`, `jsonschema`. Dev: `pytest`, `pytest-asyncio`. Python 3.13.

The existing `model-check/check_model.py` proves the model works with Deep Agents. Copy its model construction into the Worker. Do not import from it.

## Layout

```
worker/
  pyproject.toml
  src/nook_worker/
    main.py          read stdin, dispatch, write stdout
    protocol.py      pydantic models for every message, validated against contracts/
    agent.py         builds the Deep Agent
    attachments.py   turns Attachments into model content
    titles.py        makes a Chat title
    config.py        reads environment variables
  tests/
```

## Configuration

The Daemon passes configuration as environment variables when it starts the Worker.

| Variable | Meaning |
| --- | --- |
| `NOOK_BASE_URL` | Model endpoint base URL |
| `NOOK_API_KEY` | API key, read from the Keychain by the Daemon |
| `NOOK_MODEL` | Model name |
| `NOOK_MAX_INPUT_TOKENS` | `1000000` |
| `NOOK_SUMMARIZE_AT_TOKENS` | `750000` |
| `NOOK_DATABASE_URL` | Postgres connection string without a password in logs |

## Agent construction

1. Build `ChatOpenAI(base_url=..., api_key=..., model=..., use_responses_api=False, profile={"max_input_tokens": 1_000_000}, streaming=True)`. ADR 0003 explains `use_responses_api=False`. The profile makes Deep Agents use fraction-based summarization.
2. Register a harness profile before `create_deep_agent`. It excludes the `execute` tool and turns off the general-purpose subagent, so the agent has no shell and no `task` tool. Use `register_harness_profile` with `HarnessProfile(excluded_tools=frozenset({"execute"}), general_purpose_subagent=GeneralPurposeSubagentProfile(enabled=False))`. The file tools still exist, but they act on an in-memory state backend and never touch the disk. The profile key is the provider name `openai`, or a `provider:model` key if the plain provider key does not match your endpoint's model.
3. Call `create_deep_agent(model=model, tools=[], checkpointer=checkpointer, system_prompt=SYSTEM_PROMPT)`.
4. The system prompt tells the agent it is Nook, a concise desktop assistant, that it has no tools in this version, and that it should answer in Markdown.

Deep Agents version 0.7.21 does not add `write_todos` by default, and v1 does not need it.

## Checkpointer

1. Open an `AsyncConnectionPool` with `autocommit=True` and `row_factory=dict_row`, as `AsyncPostgresSaver` requires.
2. Set `search_path` to `worker` through the connection options.
3. Call `await checkpointer.setup()` once at start.
4. Pass `{"configurable": {"thread_id": chat_id}}` on every run.

## Handling `run`

1. Build the user content from `text` and the Attachments (`attachments.py`).
2. Call `agent.astream` with `stream_mode="messages"` and the Chat's `thread_id`.
3. Send `message_started` once, then a `text_delta` for each text chunk of the final reply.
4. Send `message_finished` with status `complete`.
5. Map failures to `error`: a connection failure to `endpoint_unreachable`, an HTTP error from the endpoint to `endpoint_error`, anything else to `internal`.

Cancel works by cancelling the asyncio task for that `request_id`, then sending `message_finished` with status `cancelled`. Memory keeps whatever the checkpointer saved before the cancel.

## Attachments to model content

| Kind | Content sent to the model |
| --- | --- |
| `image` | A content block `{"type": "image_url", "image_url": {"url": "data:<mime>;base64,..."}}` |
| `text` | A text block with the file name, then the file contents in a fenced block |
| `pdf` | Text extracted with `pypdf`, sent like a text file |

Text over 200,000 characters is cut, and a block ending in `[truncated]` tells the model so. A file that cannot be read produces an `error` with code `attachment_invalid`.

## Titles

After the Daemon sends `title`, the Worker makes one model call with the first user question and the instruction to reply with a title of at most six words and nothing else. It strips quotes and trailing punctuation. It sends `title_ready`. On any failure it sends `error` and the Daemon uses its fallback title.

## Summarization

Memory is summarized once it reaches 750,000 tokens. Deep Agents' default trigger is 85 percent of `max_input_tokens`. Task W5 finds the cleanest way to set it to 750,000, and checks that images count toward the total in a sensible way.

## Tests

1. Protocol tests read every line in `contracts/examples/` and check that the models accept them.
2. A fake model returns scripted chunks, so the whole loop runs without the Model endpoint.
3. A real-endpoint test is marked so it only runs when `NOOK_*` variables are set. It asks two related questions in one Chat and checks that the second answer uses the first.
