# Tools and authorization

Nook currently exposes web search and web page fetch tools. The Worker validates
arguments. The Daemon is the final authorization owner.

## Active tools

`nook:web_search` searches DuckDuckGo. It has an automatic approval tier in
`policy/tools.yaml`.

`nook:web_fetch` retrieves public HTTP and HTTPS pages. It has a host-scoped
approval tier in `policy/tools.yaml`. The fetcher checks URL schemes, addresses,
response types, timeouts, and response size. It follows a redirect only when the
target has the same host as the requested URL. A redirect to another host ends
with an error that asks for the new URL, so the Daemon can ask the user again.
It blocks every address that is not globally routable. That includes private,
loopback, link-local, carrier-grade NAT, documentation, and other reserved
ranges. The approval prompt does not warn about redirects. The tool description
tells the model and the user that a redirect to another host needs its own
approval.

A fetch stops with `Request timed out` after 30 seconds in total, across all
redirects. Each read also has its own 15 second limit.

The Daemon and the Worker must read a URL the same way. A URL with a backslash,
a space, a control character, or a username is rejected by both. So is an IP
address that is not four decimal numbers, such as `2130706433` or `0x7f.1`. Both name the
host in lowercase IDNA ASCII, using the UTS 46 rules of the `idna` crate and the
`idna` Python package. The Worker looks up DNS on that ASCII name. The file
`contracts/host-vectors.json` lists accepted and rejected URLs, and a test on
each side reads it.

## Tools that run in the Daemon

Tools that read files, run commands, or call MCP servers run in the Daemon, not
in the Worker. [ADR 0007](adr/0007-daemon-runs-privileged-tools-in-a-native-sandbox.md)
records why. The plan is in
[plans/tp7-tp13-tool-providers.md](plans/tp7-tp13-tool-providers.md).

A provider groups these tools. At startup the Daemon builds a registry from the
config, checks every tool descriptor, and gives the Worker the catalog. A
descriptor is rejected if its name is not `a-z`, `0-9`, `_`, or `-` up to 64
characters, if it collides with a Worker tool or a Deep Agents tool, if its
description is empty, or if its input schema is not a valid object schema. The
Daemon checks the arguments of every call against the schema before it asks
Cedar.

The sandbox runs commands with explicit read roots, write roots, a network flag,
a filtered environment, a working directory, a timeout, and an output limit.
macOS uses Seatbelt (`sandbox-exec`) with a deny-by-default profile. Linux uses
bubblewrap. The Daemon runs a self-test at startup. If it fails, the Daemon does
not offer terminal or MCP tools and logs the reason. On Windows there is no
sandbox, so those tools stay off.

On timeout, cancel, or exit, the Daemon kills the whole process group, so a
background child cannot outlive the call.

## File tools

Three tools read and write local files. They run in the Daemon, and they exist
only when `config.toml` lists at least one usable folder:

```toml
[tools.filesystem]
roots = [
  { path = "/Users/me/projects/demo", access = "read_write" },
  { path = "/Users/me/notes", access = "read" },
]
max_read_bytes = 1048576
max_write_bytes = 1048576
```

A root path must be absolute and exist. The Daemon logs and skips any other
entry. `write_local_file` exists only if a `read_write` root exists.

| Tool | Arguments | Result |
| --- | --- | --- |
| `read_local_file` | `path` | The file as UTF-8 text. |
| `list_local_directory` | `path` | One entry per line. Folders end with `/`. |
| `write_local_file` | `path`, `content`, `overwrite` | A line with the byte count. |

Rules:

- A path must be absolute, inside one root, and free of `..`. The Daemon opens
  it through a directory handle of the root, so a symlink that points outside
  the root is an error. A symlink inside the root is followed.
- A root path may use a symlink. The Daemon matches the configured form and the
  real form, and it shows the real form in approvals and grants.
- `write_local_file` replaces an existing file only when `overwrite` is true.
  The parent folder must exist. Writes to a `read` root are refused.
- A read fails above `max_read_bytes`, and a write fails above
  `max_write_bytes`. A file that is not valid UTF-8 is an error.
- Cedar forbids every file tool on credential paths (`.ssh`, `.gnupg`, `.aws`,
  `.kube`, `.env`, and `.env.*`) and forbids writes inside `.git`.

Grants are per tool. `allow_for_chat_path` covers one exact path.
`allow_for_chat_dir` covers a folder and everything below it, but not a sibling
folder that shares a name prefix. A read grant does not allow a list or a
write. A write offers only `allow_once` and `allow_for_chat_path`.

A path outside the roots, a path with `..`, and a write to a read-only root are
refused before any prompt, with a fixed reason such as `Path is outside the
allowed folders`.

## Command tool

`run_command` runs one command in the sandbox. It exists only when the terminal
is on, at least one folder is configured, and the sandbox self-test passes:

```toml
[tools.terminal]
enabled = true
network = false
timeout_seconds = 30
max_output_bytes = 65536
```

| Argument | Meaning |
| --- | --- |
| `argv` | The program and its arguments, as a list. Up to 64 items of 4096 bytes. No shell runs unless the model starts one. |
| `cwd` | An absolute folder inside an allowed folder. The default is the first `read_write` folder. |
| `timeout_seconds` | 1 to 600. The default is `timeout_seconds` from the config. |

The command sees only the folder it runs in. It reads that folder and the system
directories, and it writes there only if the folder is `read_write`. It also
gets a private home folder that the Daemon removes after the run. Other
allowed folders stay closed. The network is off unless the config turns it on.

The environment holds only `PATH`, `HOME`, `TMPDIR`, `LANG`, `TERM`, and
`NO_COLOR`. No variable of the Daemon passes through.

The result is text with the exit code, then `--- stdout ---` and `--- stderr
---`. Each stream holds half of `max_output_bytes`, and a marker shows a cut. A
timeout and a death by signal return an error status. A non-zero exit code does
not, because the exit code is part of the answer. After the run, and on cancel,
the Daemon kills the whole process group.

Cedar allows a command only with an `allow_once` grant, so every command asks,
and the approval shows the full command line. Cedar forbids `sudo`, `su`,
`doas`, `pkexec`, and `osascript` by program name, even after the user approves.

## MCP servers

The Daemon can use tools of MCP servers that you name in `config.toml`. It does
not search for servers or install them.

```toml
[[tools.mcp_servers]]
name = "files"
command = "/opt/tools/files-server"
args = ["--stdio"]
env = { API_TOKEN = "value" }
network = false
read_paths = ["/opt/tools"]
write_paths = ["/Users/me/.cache/files-server"]
timeout_seconds = 60
```

The Daemon starts each server inside the command sandbox and speaks MCP over
standard input and output (`initialize`, `tools/list`, `tools/call`, protocol
versions 2024-11-05, 2025-03-26, and 2025-06-18). The server gets a private
home folder, the fixed environment of the command tool plus the `env` entries,
read access to `read_paths`, write access to `write_paths`, and the network
only if `network` is true. It does not see your folders or the Daemon's
environment.

The Daemon starts all servers at once and waits 10 seconds for `initialize` and
10 seconds for `tools/list`. A server that fails is off, and the Daemon logs
why. It does not restart a server that stops later. A call to a stopped server
returns `Error: the MCP server stopped`, and you restart Nook to bring it back.

A tool is offered only if all of this holds:

- The server name uses letters, digits, `_`, or `-`, up to 32 characters, and
  no other server uses it.
- The tool name has no `:` and fits the model name rules after cleaning.
  The model sees `mcp__<server>__<tool>`, with other characters changed to `_`.
- The input schema is a valid JSON Schema of type `object`.
- The cleaned name does not collide with another tool.
- The server lists at most 64 tools. The Daemon ignores the rest.

A tool without a description gets its name. Every description ends with the
server name, so the model and the approval can tell where the tool comes from.

The Daemon checks the arguments against the server's schema before it asks
Cedar. Cedar action `mcp_call` has the context values `server` and `tool`. The
shared `mcp:*` entry in `tools.yaml` offers `allow_once` and `allow_for_chat`.
A chat grant covers one tool of one server in one chat. The approval shows the
server, the tool, and the first 300 characters of the arguments.

An MCP result becomes text. Text parts pass through. An image, audio, or binary
resource becomes a short marker, and a resource link shows its address. A result
with only structured content becomes its JSON. `isError` returns an error
status. The server description and results are untrusted text, as web pages are.

## Policy files

- `policy/tools.yaml` contains tool metadata, limits, approval tiers, and
  the approval actions that the Panel shows.
- `policy/cedar/schema.cedarschema` defines Cedar entities and request context.
- `policy/cedar/policies.cedar` contains Cedar authorization rules.

YAML supplies metadata. Cedar makes authorization decisions. The Worker must
not use YAML or hardcoded values as a substitute for Daemon authorization.

A YAML action can never permit a request that Cedar denies. The Daemon records
a grant for the action, asks Cedar again, and removes the grant if Cedar still
denies the call. For example, an `always_allow` action on `nook:web_fetch` ends
in a `deny`, because the Cedar policy accepts only `once` and `chat_host`
grants for that tool.

## Tool entries in tools.yaml

Each tool has a `display_name`, a `description`, and an `approval` block. The
`approval` block has a `tier` and a list of `actions`. Each action is an object
with three required fields:

```yaml
approval:
  tier: ask_once_per_host
  actions:
    - id: allow_once
      label: "Allow once"
      description: "Allow this exact request one time."
    - id: deny
      label: "Deny"
      description: "Reject this request. The tool does not run."
```

- `id` must be one of `allow_once`, `allow_for_chat`, `allow_for_chat_host`,
  `always_allow`, or `deny`. An id may appear only once for a tool.
- `label` is the button text. It must not be empty.
- `description` is the scope in plain words. It must not be empty.
- `display_name` must not be empty.

The Daemon stops at startup if an action is unknown, repeated, or has an empty
label or description. A plain string in the list is also invalid. A change to
`label` or `description` reaches the Panel after the next Daemon start.

## How a tool call is authorized

1. The Daemon loads `policy/tools.yaml` and the Cedar files at startup. It
   stops if a file is missing or invalid. It checks that each tool has a Cedar
   action, a known tier, a display name, and only known, unique approval actions
   that have a label and a description.
2. The Worker validates the arguments and sends `authorize_tool`.
3. The Daemon normalizes the request. For `nook:web_fetch` it lowercases the
   host, removes a trailing dot, converts it with IDNA, and ignores the port.
   It computes the argument digest.
4. The Daemon asks Cedar. A tool in the `allow` tier passes. A call that
   matches a scoped grant passes. A tool in the `deny` tier never passes.
5. If Cedar denies and the tier is not `deny`, the Daemon asks the user. It
   sends the action objects listed for the tool in `policy/tools.yaml`. It
   accepts only the `id` of an action that it sent.
6. The Daemon records a grant for the chosen action, asks Cedar again, and
   sends the result to the Worker. The Worker runs the tool only after `allow`.

The Daemon keeps grants in memory. A Worker restart does not remove them. A
Daemon restart does. These grant scopes exist:

- `once` covers one call with the same argument digest. The Daemon removes it
  when it is used.
- `chat_host` covers one host in one chat.
- `chat` covers one chat.
- `persistent` covers every chat.

Deleting a chat removes its `once`, `chat_host`, and `chat` grants. It keeps
`persistent` grants. Editable policy and stored grants are TP12.

## Tool events

The Worker sends `tool_call_started` after the Daemon allows a call and before
the tool runs. It sends `tool_call_finished` after the tool returns. The result
text is cut to 2000 characters. A denied call, and a call with invalid
arguments, send no tool events. The Daemon drops tool events from a request that
the user cancelled. The Panel shows a line such as "Running web fetch" and then
"Finished web fetch" under the reply. A reply that ends before the tool
finishes shows "Stopped web fetch".

## Denial ends the reply

A denial ends the reply. This covers a Deny click, a policy denial, and a
timeout. The Worker returns the denial to the model for that one call, then
stops the run at the next chunk. It adds the text "Request denied. I stopped
here." and ends the reply as complete. The model does not continue.

## Approval timeout

The Worker waits 300 seconds for a result. The Daemon expires a pending approval
after the same time and sends the Worker a denial with the reason `Approval
timed out`. A click after that gets an error and records no grant. The Worker
ends the reply on the denial, and the Panel removes the card when the reply
ends. The timer holds only a weak reference to the Daemon state, so a decision
that arrives first leaves it with nothing to keep alive.

The `AppState` field `approval_timeout` holds the time. A `nookd` built with the
`e2e` feature reads `NOOK_E2E_APPROVAL_TIMEOUT_MS`. The deterministic native
run sets it to 15 seconds.

## Startup rules for `tools.yaml`

A tool whose tier asks the user must list at least one approval action. A tool
in the `allow` or `deny` tier needs none. `nook:web_search` is in the `allow`
tier and lists no actions. One table in `policy_assets.rs` maps each action id
to its grant scope. A test checks that the Panel schema lists the same ids.

Run `nookd --check-policy` to check the policy files without starting the
Daemon. It prints `policy ok` and the tool count, or an error, and sets the exit
code. The package workflow runs it on the staged Daemon for each platform.
`nookd` accepts no argument or `--check-policy` alone. Any other argument prints
a usage line and exits with code 2.

## After a cancel

After a cancel, the Daemon stops forwarding `text_delta`, tool events,
`message_started`, and error events to the Panel. It still saves the full text
and forwards the final `message_finished`.

## Work still open

The plan in `plans/tool-platform-closeout-and-next-phase.md` blocks new
privileged tools until TP6 passes.

## Deferred platform work

The next platform work includes explicit MCP registration, sandboxed terminal
and filesystem execution, browser automation, native-system tools,
user-editable policy, persistent grants, and more search types. Each capability
needs a structured argument schema and Daemon authorization.
