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
  allowed approval actions.
- `policy/cedar/schema.cedarschema` defines Cedar entities and request context.
- `policy/cedar/policies.cedar` contains Cedar authorization rules.

YAML supplies metadata. Cedar makes authorization decisions. The Worker must
not use YAML or hardcoded values as a substitute for Daemon authorization.

## How a tool call is authorized

1. The Daemon loads `policy/tools.yaml` and the Cedar files at startup. It
   stops if a file is missing or invalid. It checks that each tool has a Cedar
   action, a known tier, and only known approval actions.
2. The Worker validates the arguments and sends `authorize_tool`.
3. The Daemon normalizes the request. For `nook:web_fetch` it lowercases the
   host, removes a trailing dot, converts it with IDNA, and ignores the port.
   It computes the argument digest.
4. The Daemon asks Cedar. A tool in the `allow` tier passes. A call that
   matches a scoped grant passes. A tool in the `deny` tier never passes.
5. If Cedar denies and the tier is not `deny`, the Daemon asks the user. It
   offers the actions listed for the tool in `policy/tools.yaml`.
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

## Work still open

The Panel still draws its own button labels for the actions. TP4 makes it use
the labels from the policy file. TP5 adds tool start and finish events for the
live agent and a rule for the end of a reply after a denial. The fetcher still
checks private addresses after approval, because it must check them again when
it connects. TP6 tests those checks.

The plan in `plans/tool-platform-closeout-and-next-phase.md` blocks new
privileged tools until TP6 passes.

## Deferred platform work

The next platform work includes explicit MCP registration, sandboxed terminal
and filesystem execution, browser automation, native-system tools,
user-editable policy, persistent grants, and more search types. Each capability
needs a structured argument schema and Daemon authorization.
