# Tools and shortcuts

Status: ready for implementation

## Goal

Deliver focused-Panel shortcut configuration and two built-in web tools behind a
registry, Cedar authorization, and an approval flow.

## Scope

- Configurable focused-Panel shortcuts.
- `nook:web_search` with DuckDuckGo-only `ddgs` search.
- `nook:web_fetch` with Nook-owned `httpx` fetching.
- YAML tool metadata and prompt-action mappings.
- Cedar policy files and Daemon authorization.
- Approval cards, scoped in-memory grants, rejection, and Escape cancellation.

## Out of scope

- MCP registration, discovery, and installation.
- Filesystem, terminal, browser, and native system tools.
- Process sandboxing.
- User policy settings.

## Design records

- `docs/spec/06-tools-and-shortcuts.md`
- `docs/adr/0004-cedar-authorizes-tool-requests.md`
- `docs/plans/phase-2-tools-and-shortcuts.md`
