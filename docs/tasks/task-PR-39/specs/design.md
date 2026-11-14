```
Stage: 4 gate 1
Gate 1: pending. 2026-11-14: the owner decided G1 to G7 (G4: the client secrets live in the operating system secret store behind one abstraction; G7: build it in this task with the `keyring` crate). The spec changed after that, so it goes through a review round again and back to the owner.
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
   through the MCP path, and that a secret reaches the server and nowhere else.

A real Google Docs server cannot run in CI, because it needs a Google account
and the network. A fake MCP server stands in for it.

## Scope

In scope:

1. A secret store in the Daemon: one `SecretStore` trait, one store for the
   operating system built on the `keyring` crate (G7), and one in-memory store
   for tests.
2. A new MCP server setting, `env_secrets`: a map from an environment variable
   name to the name of a secret in the store. The Daemon reads the secret when
   it starts the server.
3. A new command, `nookctl secret set <name>` and `nookctl secret delete
   <name>`, so that the owner puts a value in the store the same way on every
   system.
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
   Calendar. In external token mode, the refresh token carries the scopes that
   were granted at sign-in.

The sign-in, a token refresh and a real Docs call were not run. Nothing in CI
runs them. The docs say so, and the owner's live check (G6) closes that.

### The secret store

A new module `daemon/crates/nook-core/src/secrets.rs` holds:

1. The trait `SecretStore` with `get(name) -> Result<Option<String>>`,
   `set(name, value) -> Result<()>` and `delete(name) -> Result<()>`. A name is
   a non-empty text, for example `google-docs-refresh-token`.
2. `OsSecretStore`, built on the `keyring` crate (MIT or Apache-2.0, version
   4.2.0 on 2026-11-14). It stores each secret under the service `Nook` and the
   name as the account. The crate picks the Keychain on macOS, the Credential
   Manager on Windows and the Secret Service on Linux. The plan verifies the
   Linux backend and its need for a desktop session (D-Bus), which the spec read
   only from the crate README.
3. `MemorySecretStore`, for tests. The `keyring` crate also offers a mock store,
   and the test for `OsSecretStore` uses it. No test touches a real store.

The Daemon fails closed. A store that cannot answer (no Secret Service on a
headless Linux machine, a locked keychain) is the same as a missing secret:
the server does not start, and the report names the secret and the cause.

The functions that `nookctl` uses live in the same module, so that tests reach
them: `set_from_reader(store, name, reader)` reads the value from a reader and
trims one trailing newline. The command reads standard input, never an
argument, because an argument shows in `ps` and in the shell history. On a
terminal it turns echo off.

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
2. The config loader refuses a key that also appears in `env`, and an empty
   name. Both are errors at load time with a fixed message.
3. When the Daemon starts a server, it reads each secret. It puts the values in
   the server `env` after the entries of `env`. If one secret is missing or the
   store fails, the server is off for the session, and the report says:
   `The secret "<name>" is not in the secret store. Run: nookctl secret set <name>`.
4. The values are secrets. The Daemon adds them to the list of values that it
   masks as `***` in `daemon.log` and in the Worker and server stderr files. The
   plan finds the one place where the list is built (`daemon_log.rs`, from the
   config) and adds the resolved values there, or resolves the secrets before
   that list is built.
5. The values reach the sandbox as the other `env` entries do. On Linux,
   bubblewrap gets each value as a command-line argument, so another process of
   the same user can read it with `ps`. The page says so. On macOS the value
   does not appear in a command line.

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
them at Google and each call asks first. The page lists the removal of the cap
as a follow-up (see "Follow-up tasks").

With the scope `documents` only (read from the package source, not run):

1. `readGoogleDoc` works. It calls only the Docs API.
2. `insertText` works. It tries Drive first and falls back to the Docs API.
3. `createGoogleDoc` fails, because it calls `drive.files.create`. Creating a
   document needs a Drive scope, which is a follow-up.
4. The Drive tools fail at Google, and the failure returns to the model as an
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
10. Open the temporary token file and copy the value of `refreshToken`.
11. Put the refresh token in the secret store: run
    `nookctl secret set google-docs-refresh-token` and paste the value at the
    prompt. The prompt shows nothing.
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
   report names the secret. Run the `nookctl secret set` command that it shows.
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
3. `readEnv` (argument `name`): returns the value of the environment variable
   `name`, or `unset`. The test uses it to see what the server received.
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

1. `a_docs_style_tool_is_listed_under_the_hyphenated_server_name`: the listed
   names are `mcp__google-docs__readGoogleDoc`, `mcp__google-docs__insertText`,
   `mcp__google-docs__readEnv`, and `mcp__google-docs__connect`. The
   description of `readGoogleDoc` ends with `(MCP server: google-docs)`.
2. `a_docs_style_call_asks_first_and_shows_the_server_tool_and_arguments`: the
   approval card names `google-docs`, `insertText` and the arguments, with the
   four actions. The Cedar context carries `server` and `tool`.
3. `an_approved_docs_write_reaches_the_server`: after **Allow once**,
   `inserted.log` holds the line.
4. `a_denied_docs_write_never_reaches_the_server`: after **Deny**, `inserted.log`
   does not exist. Tests 3 and 4 share one helper that builds the path of
   `inserted.log`.
5. `a_secret_from_the_store_reaches_the_server_as_an_env_value`: `readEnv` for
   `GOOGLE_DRIVE_MCP_CLIENT_SECRET` returns the stored value, and `readEnv` for
   `GOOGLE_DRIVE_MCP_CLIENT_ID` returns the value of `env`.
6. `a_missing_secret_keeps_the_server_off_and_names_the_secret`: with an empty
   store, no `mcp__google-docs__` tool is listed, and the tool source report for
   `google-docs` is off with a message that names the secret and the command.
7. `a_store_that_fails_keeps_the_server_off`: a store whose `get` returns an
   error behaves like a missing secret, and the report does not hold the
   secret value.
8. `a_docs_server_with_network_true_can_connect_out`: `connect` to a local
   listener returns `connected` (pattern: `terminal_tool_tests.rs:411-433`).

Unit tests in `secrets.rs` and `config.rs`:

1. `the_memory_store_returns_what_was_set_and_nothing_after_delete`.
2. `the_os_store_round_trips_through_the_keyring_mock_store`.
3. `set_from_reader_trims_one_trailing_newline_and_refuses_an_empty_value`.
4. `a_server_that_lists_one_name_in_env_and_env_secrets_is_refused`.
5. `an_env_secrets_entry_with_an_empty_name_is_refused`.
6. `the_log_masking_list_holds_the_values_of_resolved_secrets`: a resolved
   secret of 4 or more characters is in the list that `daemon_log.rs` builds.

These tests check code that does not exist yet, so each fails first for one
reason: the module, the setting or the `docs` mode is missing. The plan writes
each test, runs it, sees that failure, and then adds the code.

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
   7. The bullet list under "For developers". It lists the fake server modes and
      must name `docs`.
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

The owner asked for these two tasks to be written into the docs (G2, G5). They
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
9. **A new dependency and new Daemon code in the secret path.** The `keyring`
   crate and its native libraries enter the build for all three systems. The
   plan checks that the Linux and Windows CI jobs still build and pass, and
   that the crate needs no system library that the CI images lack. A secret that
   leaks to a log, a card or an error text is the main failure. The tests 5 to 7
   and the masking test guard it.
10. **The store differs by system.** On Linux, the store needs a Secret Service
    and a desktop session, so a headless Linux Daemon cannot start a server that
    uses `env_secrets`. On macOS, a first read by a different binary can show a
    Keychain prompt. On Windows, MCP servers do not run, so `env_secrets` has no
    use there yet, but `nookctl secret` works. The page says each.
11. **One secret in the process list on Linux.** Bubblewrap receives each `env`
    value on its command line, so another process of the same user can read a
    secret with `ps` while the server runs. This is the same exposure that an
    `env` value has today. The page says so.
12. **An ADR number clash.** Another task can add an ADR with the same number.
    The plan reads `origin/main` before it names the file.
