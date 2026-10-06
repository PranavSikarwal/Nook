# Add DuckDuckGo search and guarded page fetch

Type: task
Status: open
Blocked by: 03, 04

## Goal

Implement the two authorized built-in web tools.

## Work

- Pin `ddgs` and lock the dependency graph.
- Call `ddgs` with `backend="duckduckgo"` only.
- Normalize search results.
- Add a Nook-owned `httpx` fetch adapter.
- Reject unsafe schemes, private addresses, and unsafe redirect targets.
- Enforce response limits and readable response types.

## Done when

Search uses only DuckDuckGo. Fetch rejects blocked targets before connecting and
returns bounded extracted text from a supported public page.
