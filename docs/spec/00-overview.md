# Nook v1 overview

Nook is a macOS overlay chatbot. The user presses Option+Space, asks a question, reads the answer, and presses Option+Space again to close it. Terms in this spec follow `GLOSSARY.md`. Decisions that are hard to reverse are in `docs/adr/`.

## Goal

Replace opening a chat website for quick questions. Version 1 is a general chatbot. Later versions add tools and MCP servers so it can act as a personal assistant. The design keeps that path open without building any of it now.

## Scope

In v1:

- A Panel that opens and closes with Option+Space.
- Streaming answers from the Model endpoint, rendered as Markdown.
- Chats with multiple questions each, saved in Postgres.
- A History view to open or delete past Chats.
- Attachments of images, text files, and PDFs, added by paste or a paperclip button.
- A Stop button that cancels a reply in progress.

Out of v1:

- Tools, MCP servers, shell access, and file system access for the agent.
- More than one user, cloud sync, and platforms other than macOS.
- Voice input.
- The Responses API (ADR 0003).
- Renaming Chats and searching History.
- Drag and drop of Attachments.

## Architecture

Three processes talk to one Postgres database.

```
Panel (SwiftUI)  --Unix socket, JSON lines-->  Daemon (Rust)  --stdin/stdout, JSON lines-->  Worker (Python, Deep Agents)
                                                    |                                              |
                                                    +---------------- Postgres -------------------+
                                                       schema "app"                    schema "worker"
                                                                                   Model endpoint (HTTPS)
```

1. The Panel shows the UI and holds no data of its own.
2. The Daemon owns the Chat list and the Transcript. It starts and watches the Worker.
3. The Worker runs the agent. It owns Memory, which is the LangGraph checkpoint data.
4. The Model endpoint is the self-hosted, OpenAI-compatible service. Only the Worker talks to it.

The three specs for the layers are `03-worker.md`, `04-daemon.md`, and `05-panel.md`. The messages between them are in `01-contracts.md`. The tables are in `02-data-model.md`.

## Main flows

Ask a question in a new Chat:

1. The user presses Option+Space. The Panel opens as a compact input with a fresh Chat id.
2. The user types a question, optionally pastes an image, and presses Return.
3. The Panel sends `send_message` to the Daemon.
4. The Daemon creates the Chat, saves the user Message, and sends `run` to the Worker.
5. The Worker streams events. The Daemon forwards each one to the Panel and saves the final text.
6. The Panel grows into the full Transcript and shows the text as it arrives.
7. After the first reply, the Worker makes a short extra model call for a title. The Daemon saves it and tells the Panel.

Ask a follow-up in the same Chat: the Panel sends `send_message` with the same Chat id. The Worker loads Memory from the checkpoint for that Chat and continues.

Open a past Chat:

1. The user clicks the History button.
2. The Panel sends `list_chats` and shows the list, newest first.
3. The user clicks a Chat. The Panel sends `get_chat` and shows its Transcript.
4. The user can keep asking in that Chat, or press Option+Space to close.

Close and reopen: Option+Space always opens a fresh Chat. Earlier Chats stay in History.

## Failure behavior

| Failure | What the user sees |
| --- | --- |
| Model endpoint unreachable or returns an error | The question stays in the Transcript. An inline error with a Retry button appears under it. |
| Worker crashes during a reply | Same inline error. The Daemon restarts the Worker with a growing delay. |
| Postgres is not running | The Panel shows "Database unavailable". The Daemon retries for 30 seconds, then reports the error each time the Panel asks. |
| Attachment too large or of an unsupported type | The chip shows the problem and cannot be sent. |

## Settled decisions

- The Daemon and Worker start as separate processes. The LaunchAgent starts the Daemon at login. The Daemon starts the Worker.
- Chat ids are UUIDs the Panel creates. The Daemon creates the Chat row on the first message.
- The context window is 1M tokens. Memory is summarized once it reaches 750k tokens.
- Deep Agents stays in v1 with no custom tools, an in-memory file backend, no `execute` tool, and no subagents.
- Model settings live in `~/Library/Application Support/Nook/config.toml`. The API key lives in the macOS Keychain. The Daemon owns both, and the Panel changes them through `set_settings`. The hotkey lives in the Panel's user defaults.
- A Chat title comes from one extra model call of at most six words. If that call fails, the title is the first 40 characters of the first question.
- The default hotkey is Option+Space and can be changed in settings.

## Open points for implementers

These need a short test before the task is called done. Each one names the task that owns it.

1. How to set Deep Agents' summarization trigger to 750k tokens. The library default is 85 percent of the model's `max_input_tokens`. Task W5 finds the cleanest override.
2. Whether `AsyncPostgresSaver` can write to a schema other than `public` through the connection's `search_path`. Task W3 verifies it. If it cannot, the Worker keeps its tables in `public` and the Daemon keeps its tables in `app`.
3. Whether a non-activating `NSPanel` can take keyboard input without making the app active. Task P0 verifies it.
4. How the token counter treats images. Deep Agents counts tokens approximately, and images may be undercounted. Task W5 checks this with a Chat that has many images.
