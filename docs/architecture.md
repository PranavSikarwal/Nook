# Nook architecture

Nook is a cross-platform desktop overlay assistant. The Tauri Panel starts or
connects to the Rust Daemon. The Daemon starts the Python Worker. The Daemon
and Worker use PostgreSQL through the configured `database_url`.

The implementation lives in `panel/`, `daemon/`, and `worker/`. The protocol
schemas live in `contracts/`. Tool policy files live in `policy/`.

## Processes

1. The Panel renders chats, settings, shortcuts, attachments, and approval
   cards. It uses Tauri with a React frontend.
2. The Daemon owns the local IPC endpoint, chat transcript storage, database
   migrations, Worker supervision, and configuration.
3. The Worker runs the Deep Agents graph, stores its checkpoint memory, calls
   the configured model endpoint, and exposes registered tools.

On macOS and Linux, the Panel and Daemon communicate through a local Unix
socket. Windows uses a named pipe scoped to the current account. A Daemon
refuses to start when another Daemon already answers on that socket, and it
removes the socket on shutdown only if the file is still its own. The Daemon
and Worker exchange newline-delimited JSON through standard input and output.

## Stored data

The user manages PostgreSQL. Nook does not install or operate PostgreSQL.
`database_url` selects the database. The Daemon tries to connect for 30 seconds,
with a 5 second limit on each attempt, and logs every failed attempt. If it still
cannot connect, it starts without a database, and requests fail with
`database_unavailable`. It then tries again every 10 seconds. When a try works,
it applies the migrations, repairs open replies, loads the stored grants and
preferences, and starts to serve requests. The Worker restarts by itself while
PostgreSQL is down. The Daemon stores chats, messages, and
attachment metadata. The Worker stores conversation checkpoints using the same
chat identifier as its thread identifier.

Nook keeps its configuration in the platform application-data directory. The
macOS location is `~/Library/Application Support/Nook/config.toml`. The Daemon
reads the API key from the environment or the macOS Keychain.

## Tool boundary

The Worker validates tool arguments and asks the Daemon to authorize an
execution. The Daemon is the intended owner of policy files and scoped grants.
The current Cedar component exists in `daemon/crates/nook-core/src/policy.rs`.
The Phase 2 closeout plan tracks the remaining runtime integration work.

`nook:web_search` and `nook:web_fetch` are the active built-in tools. Search
uses DuckDuckGo. Fetching a page requires a user approval flow. Nook treats
search results and fetched content as untrusted input.

## Current references

- [Contracts](contracts.md)
- [Tools and authorization](tools-and-authorization.md)
- [Release and installation](release.md)
- [Testing](testing.md)
- [Implementation plan](plan.md)
- [Task checklist](../CHECKLIST.md)
- [Architecture decisions](adr/)
