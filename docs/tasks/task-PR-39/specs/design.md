```
Stage: 3 spec review
Gate 1: pending. 2026-11-14: the owner decided G1 to G8 (G4: the client secrets live in the operating system secret store behind one abstraction; G7: build it in this task; G8: fix the Linux `bwrap` argument exposure in this task). Round 3 asked for changes, and this version answers it. It needs round 4 and then the owner.
Gate 2: pending
```

# Google Docs through an MCP server

## Why

The owner wants the assistant to read and write Google Docs.
`docs/plans/app-integrations.md` already decides the route: a Google Docs MCP
server (a program that offers tools over standard input and output) attached
with the existing `[[tools.mcp_servers]]` setting. Three things are missing:

1. No page tells the owner which server to attach, which settings to use, and
   how to create the Google sign-in app (the OAuth client).
2. The owner decided (G4) that no client secret or token lives in a file or in
   `config.toml`. Every such value goes in the operating system secret store
   (macOS Keychain, Windows Credential Manager, Linux Secret Service), through
   one Nook abstraction. Nook has no such abstraction. `keychain.rs` reaches the
   macOS Keychain only, for the model API key, and does nothing on Linux and
   Windows.
3. No test proves that a Docs-style tool is listed, authorized and called
   through the MCP path, and that a secret reaches the server and leaks to no
   log, no error text and no tool result.

A real Google Docs server cannot run in CI, because it needs a Google account
and the network. A fake MCP server stands in for it.

## Scope

In scope:

1. A secret store in the Daemon: one `SecretStore` trait, one store for the
   operating system built on the `keyring-core` crate and its three store
   crates (G7), and one in-memory store for tests.
2. A new MCP server setting, `env_secrets`: a map from an environment variable
   name to the name of a secret in the store. The Daemon reads the secret when
   it starts the server.
3. A new command, `nookctl secret set <name>` and `nookctl secret delete
   <name>`, so that the owner puts a value in the store the same way on every
   system.
3a. Masking in the Daemon itself: a redacting log writer and a scrub of tool
   results, so that a resolved secret never leaves the Daemon as text.
3b. A change to the Linux sandbox (G8): the environment of the server no longer
   goes to `bwrap` as command-line arguments.
4. A sample `config.toml` block and the owner's setup steps, in
   `docs/mcp-servers.md`.
5. A `docs` mode in the fake MCP server, and tests in
   `daemon/crates/nook-core/tests/mcp_tool_tests.rs`.
6. A test that the sample block in `docs/mcp-servers.md` loads as a valid server
   with the real config loader, so the sample cannot drift.
7. One ADR, one sentence in `docs/architecture.md`, one paragraph in
   `docs/testing.md` and one line in `CHANGELOG.md`.
8. The follow-up tasks (G2 and G5) written into `docs/mcp-servers.md`.

Out of scope:

1. Moving the model API key onto the new store. `keychain.rs` and its
   behavior stay as they are. It is a follow-up.
2. A per-server tool filter and a change to the 64-tool cap (G5 follow-up).
3. Scopes beyond `documents`, and Sheets, Slides, Calendar and Drive tools (G2
   follow-up).
4. A change to `contracts/` or `policy/`, and a live call to Google in CI.
5. Notes, Windows MCP support, and every other part of `app-integrations.md`.
6. `CHECKLIST.md` and `docs/plans/`. The housekeeping task owns them. The status
   line of `docs/plans/app-integrations.md` stays as it is.

## Design

### The server

The spec names `@piotr-agier/google-drive-mcp`, pinned to version `2.13.0`
(G1). It is an MIT-licensed server for Google Drive, Docs, Sheets, Slides and
Calendar. The orchestrator checked it on 2026-11-14:

1. `npm install --prefix <folder> @piotr-agier/google-drive-mcp@2.13.0`
   installs it. Its entry file is `dist/index.js`, and it needs Node 18 or newer.
2. Started in stdio mode with no token and no credentials, it answers
   `initialize` and `tools/list`. It lists 130 tools. It opens no browser at
   that point.
3. It has an external token mode. When `GOOGLE_DRIVE_MCP_REFRESH_TOKEN`,
   `GOOGLE_DRIVE_MCP_CLIENT_ID` and `GOOGLE_DRIVE_MCP_CLIENT_SECRET` are in its
   environment, it reads no credentials file and no token file, and it mints
   access tokens from the refresh token. Started with three fake values, it
   answers `initialize`. A real Docs call in this mode was not run.
4. The one-time sign-in is the `auth` command of the same program. It opens a
   browser, so it runs outside Nook. It writes `tokens.json`, which holds the
   refresh token of the account (the `refreshToken` field of the account record,
   read from the package source and not from a live run).
5. `GOOGLE_DRIVE_MCP_SCOPES` narrows the scopes that `auth` asks Google for.
   Without it, the server asks for eight scopes, including all of Drive and
   Calendar. In external token mode, the server takes the scope of the account
   from `GOOGLE_DRIVE_MCP_SCOPES` (or the eight defaults), not from the token
   (read from the source by the round 3 reviewer). The server refuses a tool
   itself, before any Google call, when that scope does not cover it. Its error
   text then tells the model to run `manage_accounts add`, which the server
   refuses in this mode. Without `GOOGLE_DRIVE_MCP_SCOPES` the server gates
   nothing, and Google refuses the calls. The sample always sets the variable.

The sign-in, a token refresh and a real Docs call were not run. Nothing in CI
runs them. The docs say so, and the owner's live check (G6) closes that.

### The secret store

A new module `daemon/crates/nook-core/src/secrets.rs` holds:

1. The trait `SecretStore` with `get(name) -> Result<Option<String>, SecretError>`,
   `set(name, value) -> Result<(), SecretError>` and
   `delete(name) -> Result<(), SecretError>`. A name is a non-empty text, for
   example `google-docs-refresh-token`. The name `model-api-key` is refused,
   so that `nookctl secret delete` cannot remove the model key, and
   `env_secrets` cannot hand it to a server.
2. `SecretError`, a small enum with a fixed user text for each case: missing,
   locked or unavailable, and unreadable. Nothing formats the raw error of the
   store, because the error of the `keyring-core` crate can hold the secret
   bytes in its `Debug` text (`BadEncoding`, `BadDataFormat`).
3. `OsSecretStore`. It holds a store handle from the `keyring-core` crate and
   builds one entry for each name, under the service `Nook MCP`. The store
   crates are `apple-native-keyring-store` (macOS Keychain),
   `windows-native-keyring-store` (Credential Manager) and
   `zbus-secret-service-keyring-store` (Linux Secret Service, pure Rust, no
   system library). They sit behind a Cargo feature `os-store`, which `nookd`
   and `nookctl` enable and the Panel does not, because the Panel depends on
   `nook-core` and never reads a secret. This follows the advice of the
   `keyring` crate itself, which says that an application should use
   `keyring-core` and the store crates. The service `Nook MCP` is not the
   service `Nook` of the model API key, so the two do not share a namespace.
   The plan verifies the Linux backend and its need for a desktop session
   (D-Bus), and commits `Cargo.lock`, because `package.yml` builds `--locked`.
4. `MemorySecretStore`, for tests. Tests of `OsSecretStore` pass the mock store
   of the `keyring-core` crate (`keyring_core::mock::Store`). No test touches a
   real store.

The Daemon fails closed. A store that cannot answer (no Secret Service on a
headless Linux machine, a locked keychain, an error) is the same as a missing
secret: the server does not start, and the report holds the fixed text of the
`SecretError` and the name of the secret, never the raw error.

A read can wait for an unlock or an access prompt, and servers start together.
The Daemon therefore calls the store through `spawn_blocking` with a timeout of
10 seconds. A timeout is a failure that keeps the server off.

The functions that `nookctl` uses live in the same module, so that tests reach
them: `set_from_reader(store, name, reader)` reads the value from a reader and
trims one trailing `\r\n` or `\n`. It refuses an empty value. The command
reads standard input, never an argument, because an argument shows in `ps` and
in the shell history. On a terminal it turns echo off, with the `rpassword`
crate (a new dependency that the plan names and checks for its license). The
`secret` commands run before `nookctl` connects to the Daemon socket, because
the owner stores the secrets before Nook runs.

`keychain.rs` is not part of the store and does not change. Rule 4 of
`AGENTS.md` asks for one source of truth. The model API key keeps its current
code until a follow-up moves it, because moving it changes behavior on Linux and
Windows.

### The `env_secrets` setting

`McpServerConfig` gets one new field:

```toml
env_secrets = { GOOGLE_DRIVE_MCP_CLIENT_SECRET = "google-docs-client-secret" }
```

1. The key is the environment variable that the server gets. The value is the
   name of the secret in the store.
2. `Config` is plain `serde` and has no validation step, and a refusal at load
   time would fail the whole file. The Daemon therefore checks the setting
   when it starts the servers (`start_reporting`), as it checks the server
   name today. A key that also appears in `env`, an empty secret name and the
   name `model-api-key` each put that one server off for the session, with a
   fixed report. The other servers start.
3. When the Daemon starts a server, it reads each secret. It puts the values in
   the server `env` after the entries of `env`. If one secret is missing or the
   store fails, the server is off for the session, and the report says:
   `The secret "<name>" is not in the secret store. Run: nookctl secret set <name>`
   (or the fixed text of the other `SecretError` cases). A restart of a server
   that stopped can fail the same way. The restart only logs the fixed text,
   and the call returns `Error: the MCP server stopped`. The page tells the
   owner to read `daemon.log`.
4. Masking. The mask list of today is built by the Panel from `config.toml`
   (`panel/src-tauri/src/lib.rs`), so it cannot hold a value from the store, and
   a `nookd` started by hand has no mask. The Daemon masks its own output:
   1. A redacting writer wraps the log writer of `nookd` (`nookd/src/main.rs`).
      It reads a shared list of secret values (an `Arc<RwLock<Vec<String>>>`).
   2. `start_server` adds each resolved value of 4 or more characters to the
      list before it starts the child.
   3. The server stderr line that the Daemon logs (`mcp.rs`, `debug!`) goes
      through the same list.
   4. The Panel keeps its own pass as a second layer.
5. Tool results. A server can print its own secret into a result or an error.
   A result goes to the model, the chat record and the Panel. `McpProvider::call`
   therefore replaces each resolved value of 4 or more characters in the result
   text and in the error text with `***`. The scrub is best effort: a server
   that encodes the secret (base64, a part of it) is not caught. The page says
   so.
6. The environment reaches the sandbox without a command line (G8). On Linux,
   `sandbox.rs` passes each value to `bwrap` as `--setenv <name> <value>`, which
   any user of the machine can read from `/proc/<pid>/cmdline` for the life of
   the server. The change sets the environment with `command.envs(...)` on the
   `bwrap` process (which already starts with `env_clear()`), and drops
   `--clearenv` and `--setenv`. `/proc/<pid>/environ` is readable by the owner
   of the process only. The sandbox limits stay the same. This changes the
   environment of every sandboxed command and every MCP server on Linux, so
   `sandbox_tests.rs` (the test that expects `--setenv`) changes, and a new
   Linux test checks that the `bwrap` command line holds no value. macOS passes
   the environment with `Command::envs` already and does not change.

### Sample block

The page gives this block, with the paths of a macOS user. The values that the
owner must change are named in the text around it.

```toml
[[tools.mcp_servers]]
name = "google-docs"
command = "/usr/local/bin/node"
args = ["/Users/me/tools/google-docs-mcp/node_modules/@piotr-agier/google-drive-mcp/dist/index.js"]
env = { GOOGLE_DRIVE_MCP_CLIENT_ID = "123456789-abc.apps.googleusercontent.com", GOOGLE_DRIVE_MCP_SCOPES = "documents" }
env_secrets = { GOOGLE_DRIVE_MCP_CLIENT_SECRET = "google-docs-client-secret", GOOGLE_DRIVE_MCP_REFRESH_TOKEN = "google-docs-refresh-token" }
network = true
read_paths = ["/Users/me/tools/google-docs-mcp"]
timeout_seconds = 120
```

Why each value:

1. `command` is the absolute path of `node`, and `args` holds the script. The
   sandbox `PATH` does not hold a version manager, so a script with
   `#!/usr/bin/env node` can fail. If `node` lies outside the system folders
   (for example under `nvm`), its folder goes into `read_paths`.
2. `network = true` is required, because the server calls Google. Nook has no
   per-host network rule, so the server can reach any host. The page says so.
3. There is no `write_paths`. In external token mode the server writes no
   token file, and its `HOME` is a temporary folder that Nook removes.
4. `env` holds the client ID, which is not a secret, and the scope name.
   `env_secrets` names the two secrets: the client secret and the refresh
   token. `config.toml` never holds their values.
5. `timeout_seconds = 120`, because a Google call can retry (the server retries
   up to 3 times).

### What the model sees

The model sees the tools as `mcp__google-docs__<tool>`, for example
`mcp__google-docs__readGoogleDoc`. Each call asks the owner first. The
approval card names the server, the tool and the first 300 characters of the
arguments. The card cuts a longer argument, so it does not always show a whole
write. The page tells the owner to choose **Deny** when the card ends with
`...`, and to use **Always allow this tool** only for tools that read.

The server lists 130 tools, and Nook offers the first 64 (G5, accepted). In
version 2.13.0, these 64 hold all Docs tools except `readSmartChips` and
`createFootnote`, and many Drive tools such as `deleteItem`, `shareFile`,
`addPermission` and `uploadFile`. They are safe only because the scope blocks
them (in the server itself, before any Google call, see "The server" point 5) and each call asks first. The page lists the removal of the cap
as a follow-up (see "Follow-up tasks").

With the scope `documents` only (read from the package source, not run):

1. `readGoogleDoc` works. It calls only the Docs API.
2. `insertText` works. It tries Drive first and falls back to the Docs API.
3. `createGoogleDoc` fails, because it calls `drive.files.create`. Creating a
   document needs a Drive scope, which is a follow-up.
4. The Drive tools fail inside the server, and the failure returns to the model as an
   error result.

The page lists these cases and says that they come from the source of
version 2.13.0 and not from a live run. The owner's live check (G6) confirms
or corrects the list.

### The steps for the owner

The page gives these steps. The Google Cloud Console changes its menus, so the
page names the date that it was checked and says what to look for when a name
differs. The owner does this once. The owner pastes no secret into chat, a
commit or a pull request. The page marks steps 1 to 9 as Google steps and steps
10 to 15 as Nook steps.

1. Open `https://console.cloud.google.com` and sign in with the Google account
   that owns the documents.
2. Open the project picker, choose **New project**, name it `Nook Google Docs`,
   and choose **Create**.
3. Open **APIs & Services**, then **Library**. Search for **Google Docs API**
   and choose **Enable**.
4. Open **Google Auth Platform** (or **APIs & Services**, then **OAuth consent
   screen**). Choose **Get started**. Set the app name `Nook`, a support email
   and a contact email. Choose the audience **External** (G3).
5. Open **Data Access**. Choose **Add or remove scopes** and add
   `https://www.googleapis.com/auth/documents` (G2). Save.
6. Open **Audience**. Keep the status **Testing**. Under **Test users**, choose
   **Add users**, and add the Google account of step 1.
7. Open **Clients**. Choose **Create client**, application type **Desktop app**,
   name `Nook`, and **Create**. Copy the client ID. Choose **Download JSON** and
   keep the file open for step 12. This file holds the client secret.
8. Install the server in a folder you control:
   `npm install --prefix ~/tools/google-docs-mcp @piotr-agier/google-drive-mcp@2.13.0`.
9. Sign in once, in a terminal outside Nook, with the downloaded file and the
   narrow scope:
   `GOOGLE_DRIVE_OAUTH_CREDENTIALS=<downloaded file> GOOGLE_DRIVE_MCP_TOKEN_PATH=<a temporary file> GOOGLE_DRIVE_MCP_SCOPES=documents node <install folder>/node_modules/@piotr-agier/google-drive-mcp/dist/index.js auth`.
   The browser opens. Approve the access. The temporary token file appears.
10. Open the temporary token file and copy the value of `refreshToken` in the
    account record (`accounts.<alias>.refreshToken`, read from the package
    source and not from a live run).
11. Put the refresh token in the secret store: run
    `nookctl secret set google-docs-refresh-token` and paste the value at the
    prompt. The prompt shows nothing. Nook does not need to run for this.
    `nookctl` is a separate download in the release assets of Nook, and the page
    names where to get it.
12. Put the client secret in the store the same way:
    `nookctl secret set google-docs-client-secret`. The value is the
    `client_secret` field of the downloaded file.
13. Delete the downloaded file and the temporary token file. Both hold secrets
    that now live in the store.
14. Add the block to `config.toml` with your client ID and your paths, restart
    Nook, and look for `MCP server is ready` in `daemon.log`.
15. Ask the model to read a document. Give it the document address. Approve the
    call on the card.

The page adds one section, "When Google Docs stops working", with these causes:

1. The refresh token expired. An External app in Testing status expires its
   refresh token after 7 days. Run steps 9 to 11 again, then restart Nook.
2. The scope changed. A new scope needs a new sign-in (steps 9 to 11).
3. A secret is missing or the store is locked. The server is off, and the
   report names the secret (a restart shows only `Error: the MCP server
   stopped`, so read `daemon.log`). Run the `nookctl secret set` command that it
   shows.
   On Linux, the store needs a running Secret Service and an unlocked keyring.
   On macOS, the first read by `nookd` of an item that `nookctl` created can
   show a Keychain prompt. Choose **Always Allow**.
4. The call timed out. Raise `timeout_seconds`, up to 600.
5. The server did not start. The Daemon gives a server 10 seconds each for
   `initialize` and `tools/list`, and a server that misses this is off for the
   session. A cold start of a large Node package in the sandbox can be slow.
   Start the server by hand to see how long it takes, and read `daemon.log`.
6. The log shows `***` where a value should be. The Daemon replaces every `env`
   value of 4 or more characters, and every secret, with `***` in `daemon.log`,
   so the client ID and the scope name are hidden there. Read the `config.toml`
   file for the values.

### The fake server and the tests

The fake server gets a `docs` mode. In this mode it lists only Docs-style tools,
named like the real server where a real tool exists. `normal` mode and its
exact-list test do not change. Each tool is stateless except for the file that
`insertText` writes.

1. `readGoogleDoc` (argument `documentId`): returns a fixed document text.
2. `insertText` (arguments `documentId`, `text`): appends one line to
   `inserted.log` in the folder named by the environment variable
   `FAKE_DOCS_RECORD_DIR`, and returns `ok`. The file is the record that the
   server received the call.
3. `checkEnv` (arguments `name`, `expected`): returns `match` when the
   environment variable `name` equals `expected`, and `differ` or `unset`
   otherwise. It never returns the value, so the test does not put a secret in a
   tool result.
3a. `echoEnv` (argument `name`): returns the value of the environment variable
   `name`. Only the scrub test calls it, to play a server that prints its own
   secret.
4. `connect` (argument `port`): opens a TCP connection to `127.0.0.1:<port>` with
   `TcpStream::connect_timeout`, as a Google call opens one, and returns
   `connected` or an error result.

The tests go into `mcp_tool_tests.rs`. Each checks one behavior. Each uses a
server named `google-docs`, built like the sample block: `network = true`, a
client ID in `env`, and the two secrets in `env_secrets`, read from a
`MemorySecretStore` through the same code path as production. Tests 3 and 4 add
a temporary record folder in `write_paths`. The tests use the real policy
files, the real sandbox and the fake Worker, as the suite does. In CI they fail
instead of skip, because `ci.yml` sets `NOOK_REQUIRE_SANDBOX`. Elsewhere they
skip with a message when the sandbox is missing, as the other MCP tests do.

The store must reach `McpProvider::start`, `start_reporting` and `live`. About
25 tests call `McpProvider::start(configs, &sandbox, limits)` today. The plan
makes this signature change its first task, and the existing tests pass an
empty `MemorySecretStore`.

1. `a_docs_style_tool_is_listed_under_the_hyphenated_server_name`: the listed
   names are `mcp__google-docs__readGoogleDoc`, `mcp__google-docs__insertText`,
   `mcp__google-docs__checkEnv`, `mcp__google-docs__echoEnv` and
   `mcp__google-docs__connect`. The description of `readGoogleDoc` ends with
   `(MCP server: google-docs)`.
2. `a_docs_style_call_asks_first_and_shows_the_server_tool_and_arguments`: the
   approval card names `google-docs`, `insertText` and the arguments, with the
   four actions. The Cedar context carries `server` and `tool`. The card does
   not hold a secret value.
3. `an_approved_docs_write_reaches_the_server`: after **Allow once**,
   `inserted.log` holds the line.
4. `a_denied_docs_write_never_reaches_the_server`: after **Deny**, `inserted.log`
   does not exist. Tests 3 and 4 share one helper that builds the path of
   `inserted.log`.
5. `a_secret_from_the_store_reaches_the_server_as_an_env_value`: `checkEnv` for
   `GOOGLE_DRIVE_MCP_CLIENT_SECRET` with the stored value returns `match`, and
   `checkEnv` for `GOOGLE_DRIVE_MCP_CLIENT_ID` with the value of `env` returns
   `match`.
6. `a_missing_secret_keeps_the_server_off_and_names_the_secret`: with an empty
   store, no `mcp__google-docs__` tool is listed, and the tool source report for
   `google-docs` is off with a message that names the secret and the command.
7. `a_store_error_that_holds_the_secret_bytes_never_reaches_a_report_or_the_log`:
   a store whose `get` returns an error with the secret bytes in its `Debug`
   text keeps the server off, and neither the report nor the captured log holds
   the bytes or the text.
8. `a_store_that_never_answers_keeps_the_server_off_after_the_timeout`: a store
   whose `get` never returns puts the server off, and the other servers start.
9. `a_restart_with_a_missing_secret_logs_a_fixed_text_and_the_call_fails`: after
   the server process dies, the secret is removed from the store, and the next
   call returns `Error: the MCP server stopped`. The log names the secret.
10. `a_secret_printed_by_a_server_is_scrubbed_from_the_result`: `echoEnv` for
    the client secret returns `***`, not the value.
11. `a_server_that_lists_one_name_in_env_and_env_secrets_is_skipped_with_a_report`,
    `an_env_secrets_entry_with_an_empty_name_is_skipped_with_a_report` and
    `an_env_secrets_entry_for_the_model_key_is_skipped_with_a_report`: each puts
    only that server off. They sit next to
    `servers_that_cannot_start_or_have_bad_settings_are_skipped`.
12. `a_docs_server_with_network_true_can_connect_out`: `connect` to a local
    listener returns `connected` (pattern: `terminal_tool_tests.rs:411-433`).

Tests in `sandbox_tests.rs` (Linux only):

1. `the_bwrap_command_line_holds_no_environment_value`: a server with a secret in
   its environment runs, and its `bwrap` command line holds no value.
2. The test that expects `--setenv` changes to the new behavior.

Unit tests in `secrets.rs` and elsewhere:

1. `the_memory_store_returns_what_was_set_and_nothing_after_delete`.
2. `the_os_store_round_trips_through_the_keyring_core_mock_store`.
3. `set_from_reader_trims_one_trailing_newline_or_crlf_and_refuses_an_empty_value`.
4. `the_model_key_name_is_refused_by_every_store_call`.
5. `the_redacting_log_writer_masks_a_value_that_was_added_after_it_started`:
   through the real writer, a secret added to the shared list never appears in
   a captured log line.
6. `nookctl_secret_set_reads_standard_input_and_takes_no_value_argument`: a
   binary-level test that runs without a Daemon socket.
7. `an_env_secrets_field_in_config_toml_is_read_by_the_real_parser`: the field
   name reaches `McpServerConfig`. Today the parser would drop an unknown field
   without an error, because the struct has no `deny_unknown_fields`.

These tests check code that does not exist yet, so each fails first for one
reason: the module, the setting, the writer or the `docs` mode is missing. The
plan writes each test, runs it, sees that failure, and then adds the code.

The sample test is `the_google_docs_sample_block_loads_as_a_valid_server`. It is
in a new file, `daemon/crates/nook-core/tests/docs_sample_tests.rs`, because no
test reads `docs/` today. It:

1. Finds `docs/mcp-servers.md` from `CARGO_MANIFEST_DIR` and takes the first
   `toml` block under the heading "Example: Google Docs".
2. Wraps the block with `base_url` and `model`, because `Config` has no default
   for them and the block alone does not parse.
3. Parses it with `toml::from_str::<Config>`, the check that the loader makes.
4. Asserts the name with the same rule as the loader (a `pub` helper that the
   loader and the test both call, so that they cannot drift), the value of
   `network`, the `env` keys, the `env_secrets` keys, that no secret name
   appears in `env`, and that every path is absolute.
5. Has `#[cfg(unix)]`, because `/Users/me/...` is not absolute on Windows.

## Central docs that change

1. `docs/mcp-servers.md`:
   1. The settings table gets the row `env_secrets` and a short section on the
      secret store and `nookctl secret`.
   2. The new section "Example: Google Docs" (sample block, the owner's steps,
      the limits, the tool list for the narrow scope).
   3. The new section "When Google Docs stops working".
   4. The new section "Next steps for Google Workspace" (see "Follow-up tasks").
   5. The passage "A login in a browser" (under "What cannot work"). It tells the
      owner to put the token in a folder in `read_paths`. It must point to the
      secret store and `env_secrets` for a refresh token or a client secret.
   6. The line that says "Put a secret in `env` only when the server needs it"
      (under "Limit what a server can do"). It must point to `env_secrets`.
   7. The "Environment" bullet under "What the server can see". It lists `env`
      only and must add `env_secrets`.
   8. The paragraph in "Check that it works" that names the masked `env` values
      in `daemon.log`. It must add the resolved secrets and say that the
      Daemon masks them itself.
   9. The bullet list under "For developers". It lists the fake server modes and
      must name `docs`.
   10. A note on where to get `nookctl` (a separate asset of the release).
2. `docs/architecture.md`: one sentence next to the Keychain line that names the
   secret store and its use for MCP servers.
3. `docs/testing.md`: one paragraph on the Docs-style MCP tests, the store tests
   and the sample test.
4. `docs/adr/`: one new ADR, "Secrets live in the operating system secret store
   behind one abstraction". The plan takes its number from the highest number
   on `origin/main` at that time, because other tasks may add an ADR.
5. `CHANGELOG.md`: one line under "Unreleased".

`docs/contracts.md`, `docs/tools-and-authorization.md` and `docs/release.md` do
not change, because no message, policy or release step changes. `config.toml` is
not in `contracts/`. This task is not a release, so it changes no version.

## Follow-up tasks

The owner asked for the first two tasks to be written into the docs (G2, G5). The third is an addition of the spec. They
go into the section "Next steps for Google Workspace" of `docs/mcp-servers.md`,
not into `docs/plans/`, which the housekeeping task owns.

1. **Full Google Workspace.** When `documents` works well, widen the scope step
   by step to Sheets, Slides, Calendar and Drive, so that Nook works as an
   assistant for the whole Workspace. Each added scope needs a new sign-in, a
   change on the consent screen, and a new review of the risk of the tools it
   opens (for example `uploadFile` with a Drive scope).
2. **All 130 tools.** Remove the narrow scope and the 64-tool cap (a per-server
   tool filter or a higher cap) so that all tools work. This is Nook code in
   `mcp.rs` and `config.rs`, with its own spec.
3. **The model API key on the secret store.** Move `keychain.rs` onto the store,
   so that Linux and Windows also keep the key in the system store.

## Rule changes

None. The task changes no rule and no binding document other than the
documents above.

## Risks

1. **The live path is untested.** The sign-in, the token refresh and a real Docs
   call do not run in CI, and external token mode with a real token was not
   run. The tool list for the narrow scope comes from the source of version
   2.13.0 and not from a live run. The docs say so. The owner's live check (G6)
   reduces the risk. The sample can be wrong in a detail that only a live run
   shows. The owner also reports the `node` path, the start time and the
   `daemon.log` lines. On macOS, `(allow network*)` may also allow a connection
   to the Daemon's Unix socket (mode `0600`, same user). Nobody tested this, and
   the live check on macOS is the first look at it.
2. **A broad network.** `network = true` lets the server reach any host. A
   server with the network and a secret in its environment can send the secret
   out. The secrets are the client secret and the refresh token. G1 asks the
   owner to accept this for a third-party server. The page tells the owner to
   pin the version and to use the narrow scope.
3. **The 64-tool cap.** Nook offers the first 64 of 130 tools. The model gets
   Drive tools that the narrow scope blocks. This costs context and a few
   failed calls. It does not cost safety, because each call asks first and the
   scope limits Google's answer. A later Drive scope removes that protection, so
   the follow-up that widens the scope must review this again.
4. **A moving package.** The server is a third-party program. The pin
   `2.13.0` and the "checked on" date are the guard. A newer version needs a
   new check.
5. **Google changes.** The Cloud Console menus and the rules for unverified apps
   change. The page names the date of the check and links the Google pages.
6. **Prompt injection and cut cards.** A document can hold text that tries to
   steer the model. The page says that document text is untrusted, as it says
   for every tool result. The approval card shows only the first 300
   characters of the arguments, so a long write is cut. The page tells the
   owner to choose **Deny** when the card ends with `...`.
7. **A slow start.** The Daemon gives a server 10 seconds each for `initialize`
   and `tools/list`. A cold start of a large Node package in the sandbox may
   miss this, and the server is then off for the session. The spec did not
   measure the start time in the sandbox. The owner's live check does.
8. **Masked log values.** The Daemon log replaces each `env` value of 4 or more
   characters, and each resolved secret, with `***`. The page says so, so that
   the owner does not look for the client ID in `daemon.log`.
9. **New dependencies and new Daemon code in the secret path.** The
   `keyring-core` crate, three store crates and `rpassword` enter the build.
   The Linux store is pure Rust and needs no system library, but its tree
   (`zbus`) enters `Cargo.lock` for all three systems, and the repository has no
   `cargo deny` or `cargo audit` step. The OS store sits behind a feature, so
   the Panel does not link it. The plan checks that the Linux and Windows CI
   jobs build and pass, and commits `Cargo.lock`. A secret that leaks to a log,
   an error text, a card or a tool result is the main failure. Tests 5 to 10 and
   the writer and the `nookctl` tests guard it.
10. **The store differs by system.** On Linux, the store needs a Secret Service
    and a desktop session, so a headless Linux Daemon cannot start a server that
    uses `env_secrets`. A locked keyring can wait for the owner, and the 10
    second timeout turns that into a fail-closed result. On macOS, a first read
    by a different binary can show a Keychain prompt (not verified; the crate
    says nothing about it). On Windows, MCP servers do not run, so `env_secrets`
    has no use there yet, but `nookctl secret` works. A Windows credential is
    limited to a small size (the reviewer read 2560 bytes from the Windows API
    constant, and a longer value fails with `TooLong`). A Google refresh token is
    far shorter. The page says each.
11. **The Linux environment change touches every sandboxed command.** G8 moves the
    environment from the `bwrap` arguments to the `bwrap` process environment.
    The limits of the sandbox stay the same, but every MCP server and terminal
    command on Linux gets its environment a new way. `sandbox_tests.rs` covers
    it. The live check cannot cover it on macOS, which did not change.
12. **The scrub is best effort.** The Daemon replaces a secret in a result only
    when the server returns it as the same text. A server that encodes it passes
    it. The page says so. The approval card and the model never receive the
    secret from Nook, so this guards only a server that prints its own
    environment.
13. **An ADR number clash.** Another task can add an ADR with the same number.
    The plan reads `origin/main` before it names the file.
