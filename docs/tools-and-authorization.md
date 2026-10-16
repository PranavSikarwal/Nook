# Tools and authorization

Nook currently exposes web search and web page fetch tools. The Worker validates
arguments. The Daemon is the final authorization owner.

## Active tools

`nook:web_search` searches DuckDuckGo. It has an automatic approval tier in
`policy/tools.yaml`.

`nook:web_fetch` retrieves public HTTP and HTTPS pages. It has a host-scoped
approval tier in `policy/tools.yaml`. The fetcher checks URL schemes, addresses,
redirect targets, response types, timeouts, and response size.

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

## Work still open

A denied call returns `Error: request denied` to the model, and the model keeps
going. Whether a denial should end the reply is not decided. The fetcher still
checks private addresses after approval, because it must check them again when
it connects. TP6 tests those checks and adds a start check on a built package.

The plan in `plans/tool-platform-closeout-and-next-phase.md` blocks new
privileged tools until TP6 passes.

## Deferred platform work

The next platform work includes explicit MCP registration, sandboxed terminal
and filesystem execution, browser automation, native-system tools,
user-editable policy, persistent grants, and more search types. Each capability
needs a structured argument schema and Daemon authorization.
