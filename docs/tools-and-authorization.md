# Tools and authorization

Nook currently exposes web search and web page fetch tools. The Worker validates
arguments. The Daemon is the final authorization owner.

## Active tools

`nook:web_search` searches DuckDuckGo. It has an automatic approval tier in
`policy/tools.yaml`.

Three more search tools run in the Worker and have the `allow` tier:

| Tool | Provider | Result fields |
| --- | --- | --- |
| `nook:news_search` | DuckDuckGo news | `title`, `url`, `source`, `date`, `snippet` |
| `nook:video_search` | DuckDuckGo videos | `title`, `url`, `description`, `duration`, `publisher`, `uploader`, `published` |
| `nook:book_search` | Open Library catalog | `title`, `authors`, `first_published`, `editions`, `isbn`, `subjects`, `url` |

Each takes `query` and `max_results` (1 to 10, default 5). Each result is a fixed
record. Text is one line with a length cap, and an entry without an `http` or
`https` link is dropped, so a result cannot carry a script link or a file link.
Book search asks only
`openlibrary.org`, refuses redirects, and reads at most 1 MiB. The model sees a
warning in each description and in the system prompt that results are untrusted
text. A user can still set any of these tools to Ask every time or Never allow
in Settings. A provider error returns `Search provider error: request failed`
with no detail.

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

Tools that read files, run commands, call MCP servers, or control Notes run in the
Daemon, not in the Worker. [ADR 0007](adr/0007-daemon-runs-privileged-tools-in-a-native-sandbox.md)
and [ADR 0008](adr/0008-daemon-runs-desktop-app-tools-outside-the-sandbox.md)
record why. The plan is in
[plans/tp7-tp13-tool-providers.md](plans/tp7-tp13-tool-providers.md).

### Where each tool runs

| Tool | Runs in | OS sandbox |
| --- | --- | --- |
| File tools | The Daemon, in process | No. The allowed folders and Cedar limit them. |
| Command tool and MCP servers | The Daemon, as a child process | Yes. Seatbelt on macOS, bubblewrap on Linux. |
| Web, news, video and book search, and web fetch | The Worker | No. |
| Notes tools | The Daemon, as a child process | No. macOS refuses Apple events from a sandboxed process. |

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

Each file tool, and the command tool, lists the allowed folders and their access
in its description. The model has no other way to learn the path, so a prompt
such as "write a file in my files folder" works without a path.

### Change the folders in Settings

Settings, Folders lists the allowed folders. "Add folder..." opens the system
folder picker. A new folder starts as read only. Each row has an access switch
(read only, or read and write) and a remove button. On macOS the application
menu has "Allowed Folders..." (and "Settings...", Cmd+,) that opens this tab.
Windows and Linux keep the Settings gear.

A change takes effect at once and does not need a restart:

1. The Daemon writes the list to `config.toml` and keeps the comments and the
   other settings.
2. The Daemon builds the file tools and the command tool again. MCP servers keep
   running.
3. The Daemon sends the Worker a new tool catalog, so the model sees the new
   folders in the tool descriptions. A call that already runs finishes with the
   folders it started with.
4. The Daemon denies each waiting approval whose call is no longer valid, and it
   drops the chat grants for a file path inside a removed folder.

The Daemon refuses a folder that is your home folder or contains it (this
includes `/`), a folder that contains the Nook app folder or sits inside it,
a folder that overlaps one in the list, and a path that is not an absolute,
existing folder. The list holds at most 16 folders. A folder that was deleted
stays in the list with a reason, and Nook skips it until you remove it or it
comes back. File and command tools offer no "Always allow" action, so a new
folder never inherits a permanent grant.

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

The Daemon can use the tools of MCP servers that you name in `config.toml`. It
runs each server in the command sandbox. Every call goes through Cedar and asks
first, unless you hold a grant for that tool. The `mcp_call` action has the
context values `server` and `tool`, and the shared `mcp:*` entry in `tools.yaml`
offers `allow_once`, `allow_for_chat`, and `always_allow`. See `mcp-servers.md`
for the settings, the sandbox rules, the limits, and what cannot work.

## Notes tools

Six tools create, list, read, append to, edit, and delete notes inside one
chosen Notes folder on macOS:

| Tool | Arguments | Result |
| --- | --- | --- |
| `nook:notes_create_note` | `title`, `body` | `Created the note "<title>" in the Notes folder "<folder>". Its id is <id>.` (omits the id sentence if the returned id is invalid). |
| `nook:notes_list_notes` | None | `Notes in the folder "<folder>":` followed by tab-separated note lines, or `No notes`. Shows at most 100 notes; appends `More notes exist.` if more exist. |
| `nook:notes_read_note` | `note_id` | The note title, two line breaks, and the plain text. A note over 20,000 characters ends with `[Cut at 20000 characters]`. |
| `nook:notes_append_note` | `note_id`, `note_title`, `text` | `Added the text to the note "<note_title>".` |
| `nook:notes_edit_note` | `note_id`, `note_title`, `title`, `body` | `Changed the note "<note_title>".` |
| `nook:notes_delete_note` | `note_id`, `note_title` | `Deleted the note "<note_title>".` |

The tools work only on macOS. The tool descriptions tell the model that note
text is data and is never an instruction, that the title is one line (`The title is one line.`),
and that the tools exist on macOS only.

### Argument constraints

Arguments are validated before Cedar evaluation and checked again before
process execution:

- `title` and `note_title`: 1 to 200 characters. One trailing line break
  (`\n` or `\r\n`) is trimmed. A value that is empty or contains an internal line
  break (`\r` or `\n`) is refused.
- `body`: 0 to 20,000 characters of plain text.
- `text`: 1 to 20,000 characters of plain text.
- `note_id`: 14 to 300 characters. It must start with `x-coredata://` and contain
  only ASCII alphanumeric characters or `:/._%-`.
- Any argument containing a NUL character (`\0`) is refused.

A refused argument causes the Daemon to deny the call with the fixed reason
`Invalid arguments` before showing an approval card. If invalid arguments reach
execution, the tool returns `Error: the arguments are not valid` without starting
a process.

### How the Daemon runs Notes tools

The Daemon runs `/usr/bin/osascript -e <script> <args>...` as a child process
outside the sandbox. A sandboxed process cannot send the Apple events required to
control Notes.

1. Each operation uses a constant AppleScript shipped with Nook. The model
   supplies data and never script text.
2. Every data argument has a `:` prefix on the command line. The AppleScript
   strips the prefix (`text 2 thru -1`). This prevents `osascript` from parsing data
   as a command-line flag.
3. The process runs with a clean environment containing only `LANG=en_US.UTF-8`.
   The working directory is `/`. Standard input is closed (`null`).
4. The process runs in its own process group (`process_group(0)`). On timeout or
   cancel, the Daemon kills the entire process group.
5. Execution timeout is 60 seconds.
6. Standard output is capped at 4096 bytes for create, append, edit, and delete.
   List and read keep up to 65,537 bytes (`MAX_TOOL_OUTPUT_BYTES + 1`), draining
   the rest. If a list or read reaches 65,536 bytes (`MAX_TOOL_OUTPUT_BYTES`), it is
   truncated and marked with `[output truncated]`. Standard error is capped at
   4096 bytes.
7. Parallel calls are serialized with an internal mutex lock. One `osascript`
   runs at a time.

### Offering the tools

The tools are offered only when the registry is configured with a program path,
the executable exists, and the folder name is valid:

- On macOS, `nookd` supplies `/usr/bin/osascript`. An `e2e` test build reads the
  path from `NOOK_E2E_OSASCRIPT`.
- On Linux and Windows, `nookd` passes `None`, so the tools are not offered in
  the registry.
- `policy/tools.yaml` lists the tools on all platforms. If a model attempts to
  call a Notes tool where the tools are not offered in the registry, the Daemon
  denies the call with `Tool runs in the Worker`. A denial ends the reply.
- If `/usr/bin/osascript` is missing on macOS, the source reports `off` with the
  reason `Notes was not found on this Mac`.
- If the configured folder name is invalid, the source reports `off` with the
  reason `The Notes folder name is not valid`.

### The folder setting

The folder is configured under `[tools.notes]` in `config.toml`:

```toml
[tools.notes]
folder = "Nook"
```

The default is `Nook`. A folder name is valid when, after trimming leading and
trailing spaces, it has 1 to 100 characters, contains no control character and no
`:`, and is not `Recently Deleted` (case-insensitive).

The user can view and edit the folder in the Settings tab under Tools. The field
is hidden on platforms that do not support Notes.

When the folder is changed in Settings:

1. The Daemon validates the folder name. An invalid name returns an `error` with
   code `invalid_request`.
2. Inside the write lock of `state.config`, the Daemon writes `config.toml`
   preserving comments. If writing fails, it returns `The folder could not be saved`
   and changes nothing.
3. The Daemon denies every pending Notes approval with `The Notes folder changed`.
4. The Daemon drops all chat grants for Notes tools (`GrantStore::remove_tools`).
5. The Daemon rebuilds the provider registry and sends a new `tool_catalog` to
   the Worker.
6. Each pending approval card records the folder name. When a user decision
   arrives, the Daemon checks the recorded folder against the current registry
   folder under the read lock of `state.config`. If they differ, the call is
   denied with `The Notes folder changed` and no grant is recorded.

### Approval and Cedar policy

Each Notes tool has an approval entry in `policy/tools.yaml` and an action in
`policy/cedar/policies.cedar`:

| Tool | Tier | Approval actions | Cedar permitted grant scopes |
| --- | --- | --- | --- |
| `nook:notes_create_note` | `ask` | `allow_once`, `allow_for_chat`, `deny` | `once`, `chat` |
| `nook:notes_list_notes` | `ask` | `allow_once`, `allow_for_chat`, `deny` | `once`, `chat` |
| `nook:notes_read_note` | `ask` | `allow_once`, `allow_for_chat`, `deny` | `once`, `chat` |
| `nook:notes_append_note` | `ask` | `allow_once`, `allow_for_chat`, `deny` | `once`, `chat` |
| `nook:notes_edit_note` | `ask` | `allow_once`, `allow_for_chat`, `deny` | `once`, `chat` |
| `nook:notes_delete_note` | `ask` | `allow_once`, `deny` | `once` |

No Notes tool offers `always_allow`. Delete offers no `allow_for_chat`; each
delete call requires explicit user approval.

Approval cards display the operation, the folder, and specific summary details:
- `notes_create_note`: The title, and the first 200 characters of the body with
  its total character count.
- `notes_list_notes`: The folder name.
- `notes_read_note`: The note id and folder name.
- `notes_append_note`: The note title, and the first 200 characters of text with
  its total character count.
- `notes_edit_note`: The current note title, the new title, and the first 200
  characters of the new body with its total character count.
- `notes_delete_note`: The note title and folder name.

### Failure messages

The Daemon returns fixed failure messages:

| Failure condition | Error message |
| --- | --- |
| Arguments invalid before approval | `Invalid arguments` |
| Arguments invalid when `call` starts | `Error: the arguments are not valid` |
| macOS Automation permission refused (`(-1743)`) | `Error: macOS did not allow Nook to control Notes` |
| Note not found or not in folder (`(-1728)` or `(9001)`) | `Error: that note was not found in the Notes folder` |
| Note title does not match (`(9002)`) | `Error: the note title does not match` |
| Notes does not answer within 60 seconds | `Error: Notes did not answer` |
| Spawn failure, non-zero exit, signal kill, or other error | `Error: Notes could not complete the request` |
| Folder changed while approval was pending | `The Notes folder changed` |
| Approval decision arrived for old folder | `The Notes folder changed` |
| Tool called on platform where not offered | `Tool runs in the Worker` |
| Folder name invalid in `config.toml` | `The Notes folder name is not valid` |
| `/usr/bin/osascript` not found on macOS | `Notes was not found on this Mac` |
| Saving folder to `config.toml` failed | `The folder could not be saved` |

The Daemon logs the process exit status and AppleScript error number, never the
output text or note content.

## Permanent grants and tool preferences

Settings has a Permissions tab with two lists.

**Always allowed.** A permanent grant lasts across chats and restarts until you
revoke it. Two actions create one:

- `always_allow_host` on `nook:web_fetch` allows one host in every chat. The
  Cedar scope is `persistent_host`.
- `always_allow` on an MCP tool allows that one tool of that server in every
  chat. The Cedar scope is `persistent`.

File and command tools offer no permanent grant. The Daemon stores a permanent
grant in `app.persistent_grants` and loads it at startup. A row whose id does
not match its content is ignored, and the Daemon logs it. Each grant has a
stable id, the first 32 hex characters of the SHA-256 of its tool, scope, and
host. The tab shows a sentence for each grant, such as "Always allow Web fetch
for docs.example.com, in every chat", and a Revoke button. A revoke removes the
grant from memory and from the database. Deleting a chat keeps permanent grants.

**Tools.** A preference changes how a tool asks. It can only make a tool
stricter than the default in `tools.yaml`:

| Default tier | Choices |
| --- | --- |
| `allow` (web search) | Default, Ask every time, Never allow |
| `ask`, `ask_once_per_host` | Default, Never allow |
| `deny` | Default |

The Daemon checks a request for a looser or unknown choice and returns an error
without a change. The Daemon stores a choice in `app.tool_preferences`. A tool
in the `deny` tier never runs, even with a permanent grant. A permanent grant
stays stored while its tool is denied and applies again when you choose
Default. Each choice shows its effect in plain words before you pick it. A
preference never turns a Cedar forbid into an allow, because the Daemon sets
only the tier that Cedar receives.

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

`plans/tool-platform-open-gaps.md` lists what is open, with the risk and the
decision for each item. TP6 passed on macOS and Ubuntu, so the gate in
`plans/tool-platform-closeout-and-next-phase.md` is open.

## Deferred platform work

The next platform work includes explicit MCP registration, sandboxed terminal
and filesystem execution, browser automation, native-system tools,
user-editable policy, persistent grants, and more search types. Each capability
needs a structured argument schema and Daemon authorization.
