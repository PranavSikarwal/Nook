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
| `env` | No | Environment variables for the server. Default: none. |
| `network` | No | `true` lets the server use the network. Default: `false`. |
| `read_paths` | No | Absolute paths the server may read. Default: none. |
| `write_paths` | No | Absolute paths the server may read and write. Default: none. |
| `timeout_seconds` | No | The longest time one call may take. Default: 60. Values below 1 become 1, and values above 600 become 600. |

A path in `read_paths` or `write_paths` must be absolute and must exist. The
Daemon ignores any other path and logs `ignored path`.

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
- **Environment.** The server gets `PATH`, `HOME`, `TMPDIR`, `LANG=C.UTF-8`, and
  your `env` entries. It does not get the Daemon's environment, your shell
  variables, or your API key.
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
  window or default browser in the sandbox. Sign in once outside Nook and put
  the token file in a folder that you list in `read_paths`.
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
- Put a secret in `env` only when the server needs it. `config.toml` stores
  `env` values as plain text.
- Treat the tool descriptions and the results as untrusted text. A malicious
  server can write instructions into them. The approval prompt still shows what
  is about to run.
- Use **Allow once** for a new server. Use **Always allow this tool** only for a
  tool that only reads.

## Check that it works

The Daemon writes its log to standard error. Quit Nook, then start `nookd` in a
terminal to read it, because two Daemons cannot share one socket. These lines
matter:

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

## For developers

- `daemon/crates/nook-core/src/providers/mcp.rs` holds the client and the
  provider.
- `daemon/crates/nook-core/examples/fake_mcp_server.rs` is a small stdio server
  with modes for a normal server, a chatty server, a hang, a crash, an old
  protocol and a paged tool list. The tests in `tests/mcp_tool_tests.rs` start
  it inside the sandbox and drive the full approval flow.
- `panel/e2e/specs/mcp-tool.spec.mjs` covers the approval flow in the native
  Panel with the same fake server.
- To test a new server, add a case to the fake server instead of starting a real
  one in CI.

See `tools-and-authorization.md` for the approval tiers and Cedar rules, and
`adr/0007-daemon-runs-privileged-tools-in-a-native-sandbox.md` for why the
Daemon runs these tools.
