# Cedar authorizes tool requests

## Status

Accepted

## Date

October 7, 2026

## Context

Nook needs a deterministic decision before an agent runs a tool. The decision
must account for the tool, validated arguments, target resource, configured
approval tier, and a temporary approval grant.

The tool platform must work on macOS, Ubuntu, and Windows. The Daemon already
runs in Rust. The Worker runs Python and invokes the agent.

Tool metadata and the Panel's approval actions must remain easy to change. The
metadata also needs a future path to user-specific settings.

## Decision

Nook uses Cedar embedded in the Rust Daemon as the final authorization engine.

Nook stores tool metadata, approval tiers, and prompt-action mappings in
`policy/tools.yaml`. It stores Cedar schema and policy files in `policy/cedar/`.
The Daemon validates both sets of files at startup.

The Daemon evaluates each normalized tool request twice when approval is
required:

1. Before approval, without a grant.
2. After approval, with a scoped in-memory grant.

Cedar returns `allow` or `deny`. Nook owns approval tiers and grant storage.
YAML does not bypass Cedar and does not become a second authorization language.

## Alternatives considered

### Glob and regular expression rules

A glob or regular expression table can match paths and command text. It cannot
reliably classify arbitrary shell syntax, interpreter input, redirects, or path
normalization. It also creates a separate security evaluator that is hard to
validate.

Nook may use structured middleware to normalize facts, but not to make the
final authorization decision.

### Open Policy Agent

Open Policy Agent provides a capable general policy system. Nook would need a
separate process or a WebAssembly integration. Cedar embeds directly in the
existing Rust Daemon and fits the local desktop authorization scope.

### YAML-only policy evaluation

YAML is appropriate for metadata and UI configuration. A custom YAML policy
evaluator would become security-sensitive application code. Cedar provides the
policy parser, validator, and authorization evaluator.

## Consequences

- The Daemon adds the `cedar-policy` crate and exposes a narrow authorization
  interface to the Worker supervisor.
- The Worker sends normalized tool requests and cannot execute a tool without a
  Daemon authorization result.
- Tests must cover both policy files and the request-to-Cedar mapping.
- Policy changes require a Daemon restart in the first implementation.
- A later settings feature can write validated per-user overrides without
  changing the Cedar authorization boundary.

## Sources

- [Cedar policy language](https://docs.cedarpolicy.com/)
- [Cedar policy formats](https://docs.cedarpolicy.com/policies/json-format.html)
- [Cedar Rust API](https://github.com/cedar-policy/cedar/blob/main/cedar-policy/src/api.rs)
