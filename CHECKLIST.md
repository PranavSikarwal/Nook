# Nook checklist

The active task definitions and acceptance conditions are in `docs/plan.md` and
`docs/plans/tool-platform-closeout-and-next-phase.md`.

## Current work

### Close the current web-tool authorization path

- [x] TP1. Load and validate YAML and Cedar policy assets at Daemon startup. The package workflow run 38966546745 started the staged Daemon with the policy on macOS, Ubuntu, and Windows.
- [x] TP2. Define one normalized authorization request.
- [x] TP3. Move grants and Cedar decisions to the Daemon.
- [x] TP4. Make approval actions policy-driven.
- [x] TP5. Complete tool lifecycle events and cancellation.
- [x] TP6. Verify the complete web-tool path. The package workflow run 39016561692 ran the staged Daemon, the staged Worker, and the staged policy through an allowed and a denied tool call on macOS and Ubuntu. Windows has the file and policy check only.

### Extend the tool platform

- [x] TP7. Define the provider and sandbox contract. See ADR 0007 and `docs/plans/tp7-tp13-tool-providers.md`.
- [x] TP8. Add explicit MCP server registration.
- [x] TP9. Add sandboxed filesystem tools.
- [x] TP10. Add sandboxed terminal tools.
- [ ] TP11. Add browser and native-system capability designs. The designs are written in `docs/design/`. Approval is open.
- [x] TP12. Add editable policy and persistent grants.
- [x] TP13. Add additional search providers.

## Completed work

Baseline chat, Tauri migration, cross-platform packaging, initial web tools,
and release validation are documented in `docs/archive/plans/` and
`CHANGELOG.md`.
