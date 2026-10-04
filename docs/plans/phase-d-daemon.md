# Phase D engineering plan: Daemon

Phase D implements the Nook Daemon and the command line tool in Rust. The Daemon listens on a local Unix domain socket, manages chat and transcript data in Postgres under the `app` schema, supervises the Python Worker process, and coordinates message routing.

## System boundaries and communication

```
Panel   <--- Unix domain socket (JSON lines) --->  Daemon
nookctl <--- Unix domain socket (JSON lines) --->  Daemon
                                                   Daemon  <--- stdin / stdout (JSON lines) --->  Worker
```

1. The Daemon binds a Unix domain socket at `~/Library/Application Support/Nook/daemon.sock` with file mode `0600`.
2. Clients such as the Panel and `nookctl` send newline-delimited JSON requests over this socket.
3. The Daemon responds with newline-delimited JSON events and replies.
4. The Daemon supervises the Worker child process using pipes for stdin and stdout.
5. The Daemon persists user messages, assistant replies, chat metadata, and attachment records in the local Postgres database.

## Module and workspace layout

```
daemon/
  Cargo.toml
  migrations/
    0001_init.sql
  crates/
    nook-core/
      Cargo.toml
      src/
        lib.rs
        config.rs
        protocol.rs
        db.rs
        keychain.rs
        supervisor.rs
        server.rs
      tests/
        protocol_tests.rs
        db_tests.rs
        supervisor_tests.rs
    nookd/
      Cargo.toml
      src/
        main.rs
    nookctl/
      Cargo.toml
      src/
        main.rs
```

## Sequential runtime flows

### Daemon startup flow

1. Read configuration settings from `~/Library/Application Support/Nook/config.toml` or create defaults.
2. Read the API key from the macOS Keychain under service `Nook` and account `model-api-key`. If `NOOK_API_KEY` exists in the process environment, use it instead.
3. Connect to Postgres using the configured database URL. Attempt connection retries for thirty seconds. If connections fail, continue running and return `database_unavailable` errors for incoming database operations.
4. Apply pending database migrations from `migrations/` in ascending version order.
5. Search `app.messages` for any assistant replies still in an incomplete state from previous runs and update their status to `error` with error code `internal`.
6. Start the Worker process supervisor and wait for the `ready` event.
7. Unlink any pre-existing socket file at `~/Library/Application Support/Nook/daemon.sock`.
8. Bind the Unix domain socket and set file permissions to `0600`.
9. Accept incoming client connections and spawn an asynchronous task for each connected client stream.

### Message transmission flow

1. Parse and validate the incoming `send_message` request. Verify valid UUID fields, non-empty content, and attachment size limits.
2. Insert a new record into `app.chats` if the chat ID does not exist yet.
3. Insert the user message record and its attachment records into `app.messages` and `app.attachments`.
4. Generate a new `request_id` for the Worker and map it to the client request ID.
5. Write the `run` request line to the Worker stdin stream.
6. Read event lines from the Worker. Rewrite events with client identifiers and forward them to the client socket.
7. Accumulate `text_delta` fragments in memory.
8. When the Worker emits `message_finished`, persist the complete assistant message text into `app.messages` with status `complete`.
9. If the Worker emits an error event, persist the assistant message with status `error`, save the error fields, and forward the error event.
10. After the first completed assistant reply in a chat, dispatch a `title` request to the Worker. When `title_ready` arrives, update `app.chats.title` and emit `chat_titled` to the client.

### Chat deletion flow

1. Forward a `delete_chat` request to the Worker so it clears checkpoints from the `worker` database schema.
2. Delete the record from `app.chats`. Database foreign keys cascade the deletion to `app.messages` and `app.attachments`.
3. Remove the local folder at `~/Library/Application Support/Nook/attachments/<chat_id>`.
4. Emit the `deleted` event to the requesting client.

### Worker supervision flow

1. Spawn the Worker executable as a child process using `uv run --project worker python -m nook_worker` or a configured command path.
2. Wait for the initial `ready` event within sixty seconds. If the timeout expires without a `ready` event, terminate the process and record a crash.
3. Read stdout line by line and route events by `request_id` to the matching client channel.
4. Route child process stderr output to the Daemon logging stream.
5. If the Worker terminates unexpectedly, broadcast an error event with code `worker_crashed` to all currently active requests.
6. Restart the Worker after exponential backoff intervals of 1, 2, 4, 8, and 30 seconds. Reset the backoff interval after one minute of uninterrupted execution.
7. On Daemon shutdown, send a `shutdown` line to the Worker, allow a grace period of five seconds, and terminate the process if it remains running.

## Tasks and implementation sequence

### D1. Workspace, config, and socket server
1. Create the Cargo workspace manifest in `daemon/Cargo.toml`.
2. Scaffold `nook-core`, `nookd`, and `nookctl` crate directories.
3. Implement `protocol.rs` with Serde models for both client-daemon and daemon-worker contracts.
4. Implement `config.rs` to parse `config.toml` and provide standard defaults.
5. Implement `server.rs` to bind the Unix domain socket with permissions `0600` and handle client framing.
6. Implement `ping` and `pong` message processing.
7. Write unit tests in `nook-core` validating contract examples against `protocol.rs`.
8. Verify that `nookd` starts and replies to a `ping` request sent via `nc -U`.

### D2. Migrations and database layer
1. Write `migrations/0001_init.sql` containing schema definitions for `app.chats`, `app.messages`, `app.attachments`, and indexes.
2. Implement migration runner using SQLx in `db.rs`.
3. Add database helper functions for creating chats, storing messages, listing chats, retrieving transcripts, and deleting chats.
4. Add startup repair logic to mark orphaned assistant messages as `error`.
5. Write database unit tests in `nook-core` running against isolated test schemas.

### D3. Worker supervisor
1. Implement child process spawning and standard stream piping in `supervisor.rs`.
2. Implement timeout handling for the initial `ready` event.
3. Implement event routing using asynchronous broadcast or oneshot channels keyed by `request_id`.
4. Implement crash detection, active request notification, and backoff restart delays.
5. Create a fake worker test script for supervisor testing without python dependencies.
6. Write unit tests verifying event routing, process restarts, and crash notifications.

### D4. The send_message flow
1. Implement request validation for `send_message`.
2. Connect database persistence, supervisor request routing, and event forwarding.
3. Accumulate streaming text fragments and save completed assistant replies.
4. Trigger title generation after the first assistant response and broadcast `chat_titled`.
5. Verify live streaming end to end over the Unix domain socket.

### D5. List, get, and delete
1. Implement `list_chats` returning chats ordered by `updated_at` descending.
2. Implement `get_chat` returning messages ordered by `created_at` ascending with attachments.
3. Implement `delete_chat` invoking Worker checkpoint deletion, database record deletion, and disk attachment directory cleanup.
4. Write tests verifying list ordering, message retrieval, and cascade deletion.

### D6. Cancel
1. Implement active request lookup by `target_id`.
2. Forward the `cancel` request to the Worker process.
3. Update the database assistant message status to `cancelled`.
4. Forward the `cancelled` event to the client socket.

### D7. Settings and Keychain
1. Implement `get_settings` and `set_settings` in `server.rs`.
2. Implement Keychain storage in `keychain.rs` using `security-framework` on macOS.
3. Support `NOOK_API_KEY` environment variable override.
4. Restart the Worker supervisor when settings change so updated settings take effect immediately.
5. Verify that `has_api_key` reports true without exposing the key value.

### D8. nookctl
1. Implement command line interface parsing in `nookctl` using `clap`.
2. Implement subcommands: `send`, `list`, `show`, `delete`, and `ping`.
3. Connect `nookctl` to `daemon.sock` using client protocol types.
4. Format streaming output cleanly in the terminal.

### D9. LaunchAgent scripts
1. Create `scripts/install-launch-agent.sh` writing `tech.nook.daemon.plist` to `~/Library/LaunchAgents/`.
2. Set stdout and stderr log destination to `~/Library/Logs/Nook/daemon.log`.
3. Create `scripts/uninstall-launch-agent.sh` to unload and remove the service.
4. Verify daemon service management using `launchctl`.

## Verification commands

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
