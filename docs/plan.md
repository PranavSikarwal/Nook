# Nook v1 plan

Each task has an id, what to do, what it depends on, and a "done when" check. The specs in `docs/spec/` hold the detail. The checklist at `CHECKLIST.md` lists the same ids with checkboxes.

## Order of work

1. Do phase S and phase C first. They unblock everything else.
2. Then three tracks can run in parallel: Worker (W), Daemon (D), and Panel (P). They meet in phase I.
3. P0 can start on day one. It needs only the Swift toolchain.
4. D4 is the first task that needs both the Worker and the Daemon.

Each task gets its own git branch named after its id, for example `w3-checkpointer`.

## Phase S: Setup

**S1. Install the toolchain.**
Install Rust with `rustup`. Start Postgres.app and create a database named `nook`. Keep the existing Swift command line tools and `uv`.
Depends on: nothing.
Done when: `cargo --version`, `psql -d nook -c "select 1"`, `swift --version`, and `uv --version` all succeed.

**S2. Create the repo skeleton.**
Add `app/`, `daemon/`, `worker/`, `contracts/`, `scripts/`, and a `.gitignore` for Swift, Rust, Python, and `.scratch/`. Add a top-level `README.md` with the commands to build and run each layer. Leave `model-check/` where it is.
Depends on: nothing.
Done when: the folders exist and the first commit is made on `main`.

**S3. Run the model check against the real endpoint.**
Run `uv run check_model.py` in `model-check/` with the three `NOOK_*` variables set. Save the output, with any keys removed, in `docs/model-check-result.md`.
Depends on: nothing. Needs the endpoint details from the owner.
Done when: steps 1, 2, and 3 print PASS, or the failure output is recorded and the owner has decided what to do.

**S4. Add an image step to the model check.**
Add a fourth step to `check_model.py`. It sends a small PNG as an `image_url` content part on `/v1/chat/completions` and checks that the reply describes a feature the image contains, such as its color. Record the result in `docs/model-check-result.md`.
Depends on: S3.
Done when: the new step prints PASS on the real endpoint.

## Phase C: Contracts

**C1. Write the JSON Schemas and examples.**
Write `contracts/panel-daemon.schema.json` and `contracts/daemon-worker.schema.json` from `docs/spec/01-contracts.md`. Add one example line per message type in `contracts/examples/`, in `panel-daemon.ndjson` and `daemon-worker.ndjson`.
Depends on: S2.
Done when: every example line validates against its schema, and every message type in the spec has at least one example.

**C2. Add contract tests in tests/contracts.**
Write `tests/contracts/test_contracts.py` (Python, run with `pytest`) that validates every line in the examples against the schemas, checks message type coverage, and tests negative rejection of invalid payloads.
Depends on: C1.
Done when: `uv run --with pytest --with jsonschema pytest tests/contracts` passes, and fails when one example line is edited to be invalid.

## Phase W: Worker

**W1. Worker skeleton with a fake agent.**
Create the package from `docs/spec/03-worker.md`. Read `run`, `cancel`, and `shutdown` from stdin. Send `ready` and then scripted events from a fake agent. Add protocol models and tests that read the examples.
Depends on: C1.
Done when: `uv run pytest` passes, and piping a `run` line into `python -m nook_worker` prints `ready`, `message_started`, `text_delta`, and `message_finished` lines.

**W2. Real agent with streaming.**
Build the Deep Agent as described in the spec: Chat Completions, the harness profile that removes `execute` and the subagent, no tools. Stream text deltas from `astream`.
Depends on: W1, S3.
Done when: with `NOOK_*` set, a `run` line produces a streamed answer from the real model, and a prompt that asks the agent to run a shell command gets an answer without any tool call.

**W3. Postgres checkpointer and Memory.**
Add `AsyncPostgresSaver` with a connection pool, `setup()`, and `thread_id` equal to the Chat id. Try the `worker` schema through `search_path`. If it does not work, use `public` and update `docs/spec/02-data-model.md`.
Depends on: W2, S1.
Done when: two `run` lines with the same `chat_id` work as a conversation (the second answer uses the first), and restarting the Worker between them keeps that Memory.

**W4. Attachments.**
Implement `attachments.py` for images, text files, and PDFs with the limits in `docs/spec/02-data-model.md`.
Depends on: W2, S4.
Done when: a `run` with a PNG gets an answer about the image, a run with a text file and a run with a PDF each get answers about their contents, and a 300,000-character file produces the truncation note.

**W5. Summarization at 750k tokens.**
Set Deep Agents' summarization trigger to 750,000 tokens. Find the cleanest override and record it in `docs/spec/03-worker.md`. Check how images count toward the total.
Depends on: W3.
Done when: a test with a lowered limit (set through `NOOK_SUMMARIZE_AT_TOKENS`) shows summarization starting at the set value, and the note on image counting is written in the spec.

**W6. Titles.**
Implement `title` and `title_ready` as described in the spec.
Depends on: W2.
Done when: a `title` request for "How do LangGraph checkpointers work?" returns a title of at most six words without quotes.

**W7. Cancel and error mapping.**
Cancel a running reply and map failures to the error codes in the contract.
Depends on: W2.
Done when: a `cancel` line ends a streaming run with `message_finished` status `cancelled`, a wrong base URL gives `endpoint_unreachable`, and a wrong model name gives `endpoint_error`.

**W8. Delete a Chat's Memory.**
Implement `delete_chat` with `adelete_thread`.
Depends on: W3.
Done when: after `delete_chat`, a `run` with the same `chat_id` does not remember earlier questions, and the checkpoint rows for that thread are gone.

## Phase D: Daemon

**D1. Workspace, config, and socket server.**
Create the Cargo workspace from `docs/spec/04-daemon.md`. Read the config file. Bind the socket with permissions `0600`. Answer `ping`. Add protocol types and tests that read the examples.
Depends on: C1, S1.
Done when: `cargo test` passes, and `nookd` answers a `ping` sent with `nc -U` over the socket.

**D2. Migrations and database layer.**
Write `0001_init.sql` from `docs/spec/02-data-model.md`. Apply migrations at start. Add functions to create a Chat, add a Message with Attachments, list Chats, get a Chat, and delete a Chat. Mark open replies as `error` at start.
Depends on: D1.
Done when: `cargo test` database tests pass against a temporary schema, and deleting a Chat removes its Messages and Attachment rows.

**D3. Worker supervisor.**
Start the Worker, wait for `ready`, route events by `request_id`, and restart with the delays from the spec after a crash. Include the fake Worker script for tests.
Depends on: D1, W1.
Done when: a test with the fake Worker shows events routed to the right request, and killing the Worker process produces a restart and a `worker_crashed` error for any open request.

**D4. The `send_message` flow.**
Implement the flow in `docs/spec/04-daemon.md`: validate, save the user Message, run the Worker, forward events, save the assistant Message, and start the title call after the first reply.
Depends on: D2, D3, W2.
Done when: with the real Worker, `nc -U` on the socket with a `send_message` line streams a reply, and the Messages appear in `app.messages`.

**D5. List, get, and delete.**
Implement `list_chats`, `get_chat`, and `delete_chat`, including the call to the Worker and the removal of the attachments folder.
Depends on: D4, W8.
Done when: `list_chats` shows the Chat from D4 with its title, `get_chat` returns its Messages, and `delete_chat` removes the rows, the Worker's checkpoints, and the folder.

**D6. Cancel.**
Implement `cancel` by forwarding to the Worker.
Depends on: D4, W7.
Done when: cancelling a long reply stops the stream, and the saved assistant Message has status `cancelled`.

**D7. Settings and Keychain.**
Implement `get_settings` and `set_settings`. Write the API key to the Keychain. Restart the Worker after a change. Let `NOOK_API_KEY` override the Keychain.
Depends on: D3.
Done when: `set_settings` followed by `get_settings` shows the new values with `has_api_key` true, the key does not appear in any file, and the Worker restarts.

**D8. `nookctl`.**
Build the command-line client described in the spec.
Depends on: D4, D5.
Done when: `nookctl send "hello"` streams an answer, and `nookctl list`, `show`, and `delete` work on that Chat.

**D9. LaunchAgent scripts.**
Write `scripts/install-launch-agent.sh` and `scripts/uninstall-launch-agent.sh`.
Depends on: D4.
Done when: after install and a login, `nookctl ping` succeeds without starting `nookd` by hand, and after uninstall it fails.

## Phase P: Panel

**P0. Overlay spike.**
Build a Swift package with an `NSPanel` that opens and closes with Option+Space, takes typing, shows over a full-screen app, and closes on Escape and on click-away. Write `scripts/bundle.sh` to make the `.app`. Record the answer to open point 3 in the overview.
Depends on: nothing.
Done when: the bundled `Nook.app` launches without a Dock icon, passes the manual check list in `docs/spec/05-panel.md`, and the result is written in `app/README.md`.

**P1. Daemon client and protocol types.**
Write `Protocol.swift` and `DaemonClient.swift`: connect to the socket, send requests, decode events, and reconnect. Add decoding tests that read the examples.
Depends on: P0, C1.
Done when: `swift test` passes, and a small test harness connected to a running `nookd` receives the `pong` for a `ping`.

**P2. Compact and expanded views with streaming.**
Build the compact input, the expanded Transcript, and streaming text appended from `text_delta`. Each opening starts a fresh Chat id.
Depends on: P1, D4.
Done when: a question typed in the Panel streams a reply from the real stack, and pressing Option+Space twice opens a fresh Chat.

**P3. Markdown rendering.**
Render the assistant's text as Markdown, with code blocks and a Copy button. Pick the library or `AttributedString` approach after testing code blocks, lists, and tables, and record the choice.
Depends on: P2.
Done when: a reply with a code block, a list, and a table renders correctly while it streams, and the Copy button copies the code.

**P4. History.**
Build the History button, the list, opening a past Chat, the New chat button, and delete with confirmation. Update titles live from `chat_titled`.
Depends on: P2, D5.
Done when: a Chat created in P2 appears in History with a title, opens with its full Transcript, accepts a follow-up question, and can be deleted.

**P5. Attachments.**
Build Cmd+V paste for images and files, the paperclip button, the chips, validation, and copying into the attachments folder.
Depends on: P2, W4.
Done when: a pasted screenshot gets an answer about the image, a PDF attached with the paperclip gets an answer about its contents, and a file over 10 MB shows a red chip and is not sent.

**P6. Settings.**
Build the settings window for the base URL, model, API key, and hotkey, using `get_settings` and `set_settings`. Store the hotkey in user defaults.
Depends on: P2, D7.
Done when: changing the hotkey takes effect without restarting, and changing the model name changes the model used for the next question.

**P7. Errors, Stop, and Retry.**
Show inline errors with a Retry button, the Stop button during a reply, and the disconnected state when the Daemon is not running.
Depends on: P2, D6.
Done when: stopping a long reply ends it and keeps the partial text, stopping the Daemon shows "Nook daemon not running", and Retry on a failed reply sends it again.

## Phase I: Integration

**I1. End-to-end smoke test.**
Write `scripts/smoke.sh`. It starts the Daemon, runs two related questions in one Chat with `nookctl`, attaches a small image to a third, lists the Chats, deletes the Chat, and exits with a non-zero code on any failure.
Depends on: D8, W4, W8.
Done when: `scripts/smoke.sh` passes against the real endpoint.

**I2. Install and runbook.**
Write the top-level `README.md` section that goes from a clean machine to a working Nook: toolchain, database, config, LaunchAgent, bundling, and a login item for the Panel.
Depends on: D9, P7, I1.
Done when: someone else follows the README on the same machine and reaches a working Option+Space overlay.

## Phase T: Tauri cross-platform migration

**T1. Tauri v2 scaffolding and window management.**
Initialize a Tauri v2 workspace. Configure a frameless, transparent, floating overlay that is draggable by its background, persists in the background across focus changes, and registers global hotkeys (`Option+Space` on macOS, `Alt+Space` on Linux and Windows).
Depends on: I1, I2.
Done when: the window opens, floats, is draggable, and toggles with the hotkey without auto-hiding on click-away.

**T2. UI design replication.**
Replicate the exact dark overlay design in a web frontend: compact input, expanded markdown transcript, code blocks with copy button, history drawer, attachment chips, settings view, stop and retry controls.
Depends on: T1.
Done when: the UI renders identically to the AppKit version and passes visual comparison checks.

**T3. Daemon client and contract integration.**
Connect the Tauri frontend to `nookd` using line-delimited JSON matching `contracts/panel-daemon.schema.json`. Expose typed commands and events for chat streaming, listing, deletion, and settings.
Depends on: T2, D8.
Done when: sending a question streams responses, loads history, and manages settings identically to the Swift panel.

**T4. Packaging and multi-platform distribution.**
Configure Tauri bundler for macOS (`.dmg`, `.app`), Ubuntu (`.deb`, `.AppImage`), and Windows (`.msi`, `.exe`). Add GitHub Actions matrix build workflow.
Depends on: T3.
Done when: packages build cleanly for macOS, Ubuntu, and Windows.
