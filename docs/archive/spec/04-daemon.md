# Daemon

The Daemon is a Rust program in `daemon/`. It listens on a Unix socket for the Panel, keeps the Chat list and Transcript in Postgres, and runs the Worker as a child process. `01-contracts.md` defines the messages and `02-data-model.md` defines the tables.

## Layout

A Cargo workspace with two binaries and one library.

```
daemon/
  Cargo.toml
  migrations/            SQL files, applied in order
  crates/
    nook-core/           protocol types, config, database access, worker supervisor
    nookd/               the daemon binary
    nookctl/             command-line client for testing without the Panel
```

Suggested crates: `tokio`, `serde`, `serde_json`, `sqlx` (Postgres, runtime tokio), `tracing`, `clap`, `toml`, `uuid`, `security-framework` (Keychain), `jsonschema`. These are suggestions. Pick others if there is a reason, and note the reason in the task.

## Start-up sequence

1. Read `~/Library/Application Support/Nook/config.toml`.
2. Read the API key from the Keychain (service `Nook`, account `model-api-key`). If the environment variable `NOOK_API_KEY` is set, it wins. This lets `nookctl` and tests run without the Keychain.
3. Connect to Postgres. Retry for 30 seconds. If it still fails, keep running and answer every request with `database_unavailable`.
4. Apply the migrations in `daemon/migrations/`.
5. Mark any assistant reply left open by a previous run as `error`.
6. Start the Worker (see below).
7. Remove a stale socket file, then bind the socket with permissions `0600`.

## Config file

```toml
base_url = "https://models.example.internal/v1"
model = "model-name"
database_url = "postgres://localhost/nook"
max_input_tokens = 1000000
summarize_at_tokens = 750000
```

The Daemon owns this file and the Keychain item. The Panel changes them only through `get_settings` and `set_settings`. A `set_settings` request writes the base URL and model to the file, writes the API key to the Keychain if one was sent, and restarts the Worker so it picks up the change. The reply `settings` reports `has_api_key` and never the key.

## Worker supervisor

1. Start `uv run --project worker python -m nook_worker` with the environment variables listed in `03-worker.md`. In the installed app, the command is the path set in the config file under `worker_command`.
2. Wait for `ready`. If it does not arrive within 60 seconds, kill the Worker and count it as a crash.
3. Read stdout line by line and route each event by `request_id`. Send stderr to the Daemon's log.
4. If the Worker exits unexpectedly, send `error` with code `worker_crashed` to every open request. Restart it after 1, 2, 4, 8, then 30 seconds, and reset the delay after a minute of healthy running.
5. On Daemon shutdown, send `shutdown`, wait 5 seconds, then kill the Worker.

## Handling `send_message`

1. Validate the request: the Chat id is a UUID, the text or Attachments are not both empty, and the Attachments obey the limits in `02-data-model.md`.
2. Create the Chat row if it does not exist. Save the user Message and its Attachment rows.
3. Send `run` to the Worker with a new `request_id` that the Daemon maps to the Panel's request `id`.
4. For each Worker event, rewrite it with the Panel's `id` and `chat_id` and forward it.
5. Accumulate `text_delta` values. On `message_finished`, save the assistant Message with its status. On `error`, save an assistant Message with status `error` and the error fields, then forward the `error`.
6. After the first successful reply in a Chat, send `title` to the Worker. On `title_ready`, save the title and send `chat_titled`. On failure, save the first 40 characters of the first question.

`cancel` finds the open request by `target_id` and forwards `cancel` to the Worker.

## Other requests

- `list_chats` returns all Chats ordered by `updated_at` descending.
- `get_chat` returns the Chat with its Messages ordered by `created_at`, each with its Attachments.
- `delete_chat` follows the steps in `02-data-model.md`.
- `get_settings` and `set_settings` follow the rules in the config file section.
- `ping` returns `pong`.

## `nookctl`

A command-line client that speaks the Panel and Daemon protocol. It exists so the Daemon and Worker can be checked without the Panel.

```
nookctl send "question" [--chat <uuid>] [--attach <path>]...   prints events as they stream
nookctl list
nookctl show <chat-uuid>
nookctl delete <chat-uuid>
nookctl ping
```

## LaunchAgent

`scripts/install-launch-agent.sh` writes `~/Library/LaunchAgents/tech.nook.daemon.plist` and loads it with `launchctl`. The plist runs `nookd`, keeps it alive, and sends its output to `~/Library/Logs/Nook/daemon.log`. A matching `scripts/uninstall-launch-agent.sh` removes it.

## Tests

1. A fake Worker, a small script that replays events from `contracts/examples/`, lets the Daemon be tested without Python or a model.
2. Protocol tests read every line in `contracts/examples/` and check that the Rust types accept them.
3. Database tests run against a temporary schema in the local Postgres.
