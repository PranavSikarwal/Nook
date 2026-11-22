# Attach an MCP server

Nook can use the tools of any MCP server that talks over standard input and
output. You name the server in `config.toml`. Nook runs it in the command
sandbox and asks you before every call, with the same approval flow as its other
tools. Nook does not search for servers, install them, or write their settings.

This page is for the person who attaches a server. It is the reference for the
settings, the sandbox rules, and the approval flow.

## Before you start

- The server must speak MCP over standard input and output (stdio). A server
  that only listens on HTTP or SSE does not work.
- The server must support protocol version 2024-11-05, 2025-03-26, or 2025-06-18.
- You are on macOS or Linux. Windows has no sandbox, so Nook turns MCP tools off
  there. Linux needs `bubblewrap` (`bwrap`).
- You trust the server as you trust any program you run. The sandbox limits what
  it can reach. It does not make a harmful server safe.

## Steps

1. Install the server first, in a folder you control. Do not use `npx -y`,
   `uvx`, or `pip install` as the command (see "Install before you attach").
2. Find the command that starts the server in stdio mode. Run it in a terminal
   to check that it waits for input.
3. Add a block to `config.toml` (see "Settings").
4. Restart Nook. The Daemon starts the server when it starts.
5. Check the Daemon log for `MCP server is ready` (see "Check that it works").
6. Ask the model to use the tool. An approval card appears before each call.

## Settings

Each server is one `[[tools.mcp_servers]]` block. The Panel's Settings do not
edit it. Edit `config.toml` in the Nook application folder. Nook writes the file so that
only your user can read it (mode `0600` on macOS and Linux), because `env` can
hold secrets. Saving the model settings in the Panel changes only `base_url` and
`model` and keeps your comments and server blocks. Windows keeps the default
file access.

```toml
[[tools.mcp_servers]]
name = "notes-sync"
command = "/Users/me/tools/notes-sync/bin/server"
args = ["--stdio"]
env = { SYNC_TOKEN = "value" }
network = true
read_paths = ["/Users/me/tools/notes-sync"]
write_paths = ["/Users/me/.local/share/notes-sync"]
timeout_seconds = 60
```

| Setting | Required | Meaning |
| --- | --- | --- |
| `name` | Yes | A short id. Letters, digits, `_` and `-`, up to 32 characters. Each name is used once. The model sees tools as `mcp__<name>__<tool>`. |
| `command` | Yes | The program to start. Use an absolute path, or a program on the sandbox `PATH` (see below). |
| `args` | No | Arguments for the program. Default: none. |
| `env` | No | Environment variables for the server. Default: none. A name that starts with `LD_` or `DYLD_` is refused in `env` and in `env_secrets`. |
| `env_secrets` | No | Maps an environment variable name to a secret name in the secret store. Default: none. A name that starts with `LD_` or `DYLD_` is refused in `env` and in `env_secrets`. |
| `network` | No | `true` lets the server use the network. Default: `false`. |
| `read_paths` | No | Absolute paths the server may read. Default: none. |
| `write_paths` | No | Absolute paths the server may read and write. Default: none. |
| `timeout_seconds` | No | The longest time one call may take. Default: 60. Values below 1 become 1, and values above 600 become 600. |

A path in `read_paths` or `write_paths` must be absolute and must exist. The
Daemon ignores any other path and logs `ignored path`.

## Secret store

Nook keeps sensitive values in the operating system secret store. On macOS it uses Keychain. On Windows it uses Credential Manager. On Linux it uses Secret Service. The service name in the secret store is `Nook MCP`. A headless Linux Daemon cannot use the secret store because Secret Service requires a desktop session.

Manage secrets with `nookctl`:
- To store a secret, run `nookctl secret set <name>`. The command reads the value from standard input and never from an argument. When you run the command on a terminal, echo is off.
- To delete a secret, run `nookctl secret delete <name>`.

The name `model-api-key` is refused for every secret command and for `env_secrets`.

The `nookctl` binary is a separate asset of the release. Download the `nookctl` binary of the release from the Releases page of the Nook repository at `https://github.com/PranavSikarwal/Nook/releases`.

## What the server can see

The Daemon starts the server inside the same sandbox as the command tool.

- **Program path.** The sandbox `PATH` is `/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin`,
  plus `/opt/homebrew/bin` on macOS. Your shell `PATH` is not used. A program in
  `~/.local/bin` or a version manager needs its absolute path in `command`.
- **Files.** The server reads the system folders and your `read_paths`. It
  writes only in `write_paths` and in its own home folder. A program may start
  from any path, but a read outside the allowed folders fails. A Node server
  whose script lies outside `read_paths` stops with `EPERM`.
- **Home folder.** `HOME` and `TMPDIR` point to a new empty folder. Nook deletes
  it when the server stops. Anything the server saves there is gone at the next
  start.
- **Environment.** The server gets `PATH`, `HOME`, `TMPDIR`, `LANG=C.UTF-8`,
  your `env` entries, and your resolved `env_secrets` values. It does not get the
  Daemon environment, your shell variables, or your API key.
- **Network.** Off unless `network = true`.
- **Processes.** The Daemon stops the server and everything it started when the
  server stops or the Daemon exits.

## Install before you attach

The home folder is new at each start and the network is off by default. A
command such as `npx -y some-server` or `uvx some-server` then downloads the
server every time, and fails if the network is off. Do this instead:

1. Install the server once, outside Nook, into a fixed folder.
2. Point `command` at the installed program.
3. Add the install folder to `read_paths`.
4. Keep `network = false` unless the server calls an online service.

## What happens on a call

1. The model calls `mcp__<name>__<tool>` with arguments.
2. The Daemon checks the arguments against the input schema that the server
   published. A call with wrong arguments fails without a prompt.
3. Cedar decides. The `mcp_call` action has the context values `server` and
   `tool`. A call asks first unless you hold a grant for it.
4. The approval card shows the server, the tool, and the first 300 characters of
   the arguments. You can choose **Allow once**, **Allow this tool for this
   chat**, **Always allow this tool**, or **Deny**.
5. After an allow, the Daemon sends the call to the server and waits up to
   `timeout_seconds`.
6. The result goes back to the model as text. The Daemon cuts it to 64 KiB.

Text parts pass through. An image, audio part, or binary resource becomes a short
marker. A resource link shows its address. A result with only structured content
becomes its JSON. An `isError` result returns an error.

A grant covers one tool of one server. **Allow this tool for this chat** lasts
for the chat. **Always allow this tool** lasts until you revoke it in Settings,
Permissions.
YAML cannot loosen a Cedar rule, and no setting lets a call skip the prompt
without a grant.

## Which tools Nook offers

The Daemon offers a tool only if all of this holds:

- The tool name has no `:` and the cleaned name is valid for the model.
- The input schema is a valid JSON Schema of type `object`.
- The cleaned name does not collide with another tool.
- The server has at most 64 tools. Nook ignores the rest.

A tool without a description gets its name as the description. Every description
ends with `(MCP server: <name>)`. A tool that fails a check is left out, and the
Daemon logs the reason.

## When a server stops

A server that does not answer `initialize` and `tools/list` in 10 seconds each
is off for the session, and the Daemon logs why.

A call that reaches a stopped server returns `Error: the MCP server stopped`. The
Daemon does not repeat that call, because the server may have acted before it
stopped. The next call starts the server again. The Daemon waits at least 5
seconds between two starts of one server, so a server that crashes at once
stays stopped for that time.

## What cannot work

- **Apple events and AppleScript.** macOS refuses Apple events from any process
  that runs under a sandbox profile, even a profile that allows everything. A
  server that controls Notes, Calendar, Mail or Finder through AppleScript or
  JXA fails with a privilege error. No setting fixes this. See
  `plans/app-integrations.md`.
- **A login in a browser.** A server that opens a browser for a sign-in has no
  window or default browser in the sandbox. Sign in once outside Nook. Store a
  refresh token or client secret in the secret store and pass it with
  `env_secrets`. If a server rewrites its token file, list the folder in
  `write_paths`.
- **A server that needs a variable that starts with `LD_` or `DYLD_`.** The
  Daemon refuses any variable name in `env` or `env_secrets` that begins with
  `LD_` or `DYLD_`. A server that needs such a variable cannot start.
- **Servers on a network port.** Only stdio works.
- **Files outside the paths you list.** The server cannot read your documents or
  your credentials unless you list the folder.
- **Windows.** MCP tools are off.

## Limit what a server can do

- Give `read_paths` the install folder and nothing more. Do not list your home
  folder.
- Keep `network = false`. A server with the network and a readable folder can
  send what it reads to anyone.
- Give `write_paths` one data folder, not a folder that holds your work.
- Store secrets in `env_secrets` instead of `env`. The `config.toml` file stores
  `env` values as plain text. The `env_secrets` setting reads values from the
  operating system secret store.
- Treat the tool descriptions and the results as untrusted text. A malicious
  server can write instructions into them. The approval prompt still shows what
  is about to run.
- Use **Allow once** for a new server. Use **Always allow this tool** only for a
  tool that only reads.

## Check that it works

Open Settings, then Tools. The tab lists the file tools, the command tool, and
each MCP server. Each row shows its state and how many tools it offers:

- **Ready.** The source works.
- **Off.** The source did not start. The row says why, for example `The command is
  empty` or `The MCP server stopped`.
- **Stopped.** An MCP server started and then exited. The next call starts it
  again.

Refresh reloads the list. A source that you did not configure does not appear.
The tab shows the state when the Daemon started and changes only for a server
that stops later.

For more detail, read the Daemon log.

When Nook starts the Daemon, the Panel keeps its log in `daemon.log` in the Nook
application folder. The file is private to your user (mode `0600` on macOS and
Linux). When it reaches 1 MiB, it becomes `daemon.log.1`, and Nook keeps no older
file. The Daemon masks these values from every log line itself, also when started
by hand: the database password, the API key in the environment, `env` values of
4 or more characters, and resolved secrets of 4 or more characters from `env_secrets`. If you start
`nookd` yourself, the log goes to the terminal, and you must quit Nook first
because two Daemons cannot share one socket.

These lines matter:

| Log line | Meaning |
| --- | --- |
| `MCP server is ready` with `tools` | The server started and Nook accepted that many tools. |
| `MCP server <name> is off: <reason>` | The server did not start or did not answer in 10 seconds. |
| `Ignored MCP server ...: the name is not valid` | The name breaks the name rule, is used twice, or the command is empty. |
| `ignored path <path>` | A path is not absolute or does not exist. |
| `MCP server <name>: ignored tool "<tool>": <reason>` | Nook left one tool out. The reason says why. |
| `MCP server <name>: more than 64 tools` | Nook kept the first 64. |
| `MCP server <name> did not restart` | The restart after a stop failed. |

If no server starts, Nook offers no MCP tool. If the model does not see a tool,
read these lines first.

## Example: Google Docs

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

Why each value is set:
- `command` is the absolute path of `node`, and `args` holds the script path. The sandbox `PATH` does not hold a version manager. A script with `#!/usr/bin/env node` can fail. If `node` lies outside the system folders, add its folder to `read_paths`.
- `network = true` is required because the server calls Google APIs.
- There is no `write_paths`. In external token mode, the server writes no token file. Its home folder is a temporary folder that Nook removes on exit.
- `env` holds the client ID and the scope name. The client ID is not a secret.
- `env_secrets` maps the client secret and the refresh token to items in the secret store. The `config.toml` file never stores their values.
- `timeout_seconds = 120` gives Google calls time to complete. The server retries up to 3 times on transient failures.

Setup steps:

These steps were written on 2026-11-14 from the Google and package pages. Nobody has followed all of them in Google yet, because CI cannot sign in to Google. The Google Cloud Console changes menus over time. Look for matching labels if console text differs. Steps 1 to 9 are Google steps. Steps 10 to 15 are Nook steps.

1. Open `https://console.cloud.google.com` in your browser. Sign in with the Google account that owns the documents.
2. Open the project picker. Choose **New project**. Enter the project name `Nook Google Docs`. Choose **Create**.
3. Open **APIs & Services**, then choose **Library**. Search for **Google Docs API**. Choose **Enable**.
4. Open **Google Auth Platform** (or **APIs & Services**, then **OAuth consent screen**). Choose **Get started**. Set the application name to `Nook`. Enter a support email and a contact email. Choose the audience **External**.
5. Open **Data Access**. Choose **Add or remove scopes**. Add `https://www.googleapis.com/auth/documents`. Save the change.
6. Open **Audience**. Keep the status as **Testing**. Under **Test users**, choose **Add users**. Add the Google account from step 1.
7. Open **Clients**. Choose **Create client**. Set the application type to **Desktop app**. Set the name to `Nook`. Choose **Create**. Copy the client ID. Choose **Download JSON** and keep the file for step 12. This file holds the client secret.
8. Install the server in a folder that you control:
   `npm install --prefix ~/tools/google-docs-mcp @piotr-agier/google-drive-mcp@2.13.0`
9. Sign in once in a terminal outside Nook with the downloaded file and the narrow scope:
   `GOOGLE_DRIVE_OAUTH_CREDENTIALS=<downloaded file> GOOGLE_DRIVE_MCP_TOKEN_PATH=<a temporary file> GOOGLE_DRIVE_MCP_SCOPES=documents node <install folder>/node_modules/@piotr-agier/google-drive-mcp/dist/index.js auth`
   The browser opens. Approve the access. The temporary token file appears.
10. Open the temporary token file. Copy the value of `refreshToken` from the account record (`accounts.<alias>.refreshToken`, read from the package source and not from a live run).
11. To store the refresh token in the secret store, run `nookctl secret set google-docs-refresh-token`. Paste the value at the prompt. The terminal displays no characters. Nook does not need to run for this command.
12. To store the client secret in the secret store, run `nookctl secret set google-docs-client-secret`. The value is the `client_secret` field in the downloaded JSON file.
13. Delete the downloaded JSON file and the temporary token file. Both files hold secrets that now live in the secret store.
14. Add the configuration block to `config.toml` with your client ID and your paths. Restart Nook. Look for `MCP server is ready` in `daemon.log`.
15. Ask the model to read a document. Give the model the document address. Approve the call on the approval card.

Tool behavior under the narrow scope:

With the scope `documents` only (read from the package source and not from a live run):
- `readGoogleDoc` works. It calls only the Docs API.
- `insertText` works. It checks Drive first and falls back to the Docs API.
- `createGoogleDoc` fails because it calls `drive.files.create`. Creating a document requires a Drive scope.
- Drive tools fail inside the server. The server returns an error result to the model.

Scope behavior:

In external token mode, the server takes account scopes from `GOOGLE_DRIVE_MCP_SCOPES` (or the eight defaults), not from the token (read from the package source). The server refuses a tool itself before any Google call when that scope does not cover the tool. Its error text then tells the model to run `manage_accounts add`, which the server refuses in this mode. Without `GOOGLE_DRIVE_MCP_SCOPES`, the server gates nothing, and Google refuses calls. The sample block always sets this variable.

Network risk:

Setting `network = true` lets the server contact any host on the network. Nook has no per-host network filter. A malicious server with network access and secrets in its environment can send secrets out. Pin the package version to `2.13.0` and set the narrow scope `documents` to reduce this risk.

Tool cap:

The server offers 130 tools. Nook offers the first 64 tools and ignores the rest. In version 2.13.0, these 64 tools include all Docs tools except `readSmartChips` and `createFootnote`. They also include Drive tools such as `deleteItem`, `shareFile`, `addPermission`, and `uploadFile`. These Drive tools remain safe because the narrow scope blocks them inside the server before any Google call, and each call asks for your approval first.

Live check status:

CI did not run the live path (sign-in, token refresh, real Docs calls, and macOS socket checks), and that path waits for the live check by the owner.

Warnings:

Document text is untrusted text. A document can contain text that attempts to steer the model. Treat every document result as untrusted input.

The approval card displays only the first 300 characters of arguments. A long write operation is cut. Choose **Deny** when the approval card ends with `...`. Use **Always allow this tool** only for tools that read.

## When Google Docs stops working

1. The refresh token expired. An External application in Testing status expires its refresh token after 7 days. If the token expires, run steps 9 to 11 again, and restart Nook.
2. The scope changed. A new scope requires a new sign-in. If the scope changes, run steps 9 to 11 again.
3. A secret is missing or the secret store is locked. The server stays off, and the report names the secret. A restart shows only `Error: the MCP server stopped`, so check `daemon.log`. Run the `nookctl secret set` command that the report shows. On Linux, the secret store requires a running Secret Service and an unlocked keyring. On macOS, the first read by `nookd` of an item that `nookctl` created can show a Keychain prompt (not verified). Choose **Always Allow**.
4. The call timed out. Raise `timeout_seconds` in `config.toml`, up to 600.
5. The server did not start. The Daemon allows a server 10 seconds each for `initialize` and `tools/list`. A server that exceeds this limit stays off for the session. A cold start of a large Node package in the sandbox can be slow. Start the server by hand in a terminal to measure the start time, and read `daemon.log`.
6. The log shows `***` where a value is expected. The Daemon replaces every `env` value of 4 or more characters, and every secret of 4 or more characters, with `***` in `daemon.log`. The mask replaces exact text. A server that encodes or splits a value, or two secrets that overlap, can show a part of it. The Daemon masks these values itself, also when started by hand. The client ID and the scope name stay hidden in the log. Read `config.toml` to inspect those values.

## Next steps for Google Workspace

1. Full Google Workspace. When `documents` works well, widen the scope step by step to Sheets, Slides, Calendar, and Drive, so that Nook works as an assistant for the whole Workspace. Each added scope needs a new sign-in, a change on the consent screen, and a new review of the risk of the tools that it opens (such as `uploadFile` with a Drive scope).
2. All 130 tools. Remove the narrow scope and the 64-tool cap (through a per-server tool filter or a higher cap) so that all tools work. This step requires Nook code in `mcp.rs` and `config.rs`, with its own spec.
3. The model API key on the secret store. Move `keychain.rs` onto the secret store, so that Linux and Windows also keep the model API key in the system store.

## For developers

- `daemon/crates/nook-core/src/providers/mcp.rs` holds the client and the
  provider.
- `daemon/crates/nook-core/examples/fake_mcp_server.rs` is a small stdio server
  with modes for a normal server, a chatty server, a hang, a crash, an old
  protocol, a paged tool list, and docs. The tests in `tests/mcp_tool_tests.rs`
  start it inside the sandbox and drive the full approval flow.
- `panel/e2e/specs/mcp-tool.spec.mjs` covers the approval flow in the native
  Panel with the same fake server.
- To test a new server, add a case to the fake server instead of starting a real
  one in CI.

See `tools-and-authorization.md` for the approval tiers and Cedar rules, and
`adr/0007-daemon-runs-privileged-tools-in-a-native-sandbox.md` for why the
Daemon runs these tools.
