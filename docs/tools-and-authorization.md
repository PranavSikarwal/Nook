# Tools and authorization

Nook currently exposes web search and web page fetch tools. The Worker validates
arguments, and the tool policy is intended to make the Daemon the final
authorization owner.

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

## Current closeout work

`CedarAuthorizer` parses and validates Cedar policy files and evaluates scoped
grants in `daemon/crates/nook-core/src/policy.rs`. The running Daemon does not
yet load `policy/tools.yaml` or the Cedar files during startup. It also does not
yet authorize every active tool request through that component.

The plan in `plans/tool-platform-closeout-and-next-phase.md` makes those gaps
explicit. The plan blocks new privileged tools until the Daemon owns policy
loading, decision evaluation, and grants.

## Deferred platform work

The next platform work includes explicit MCP registration, sandboxed terminal
and filesystem execution, browser automation, native-system tools,
user-editable policy, persistent grants, and more search types. Each capability
needs a structured argument schema and Daemon authorization.
