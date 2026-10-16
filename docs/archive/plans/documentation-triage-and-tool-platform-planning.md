# Documentation triage and tool-platform planning

## Purpose

This plan creates a verified closeout plan for incomplete tools and
authorization work, defines the deferred tool-platform work, and reduces the
current documentation set to documents that describe the shipped system.

## Scope

The work covers three areas:

1. Verify and plan the incomplete authorization path from Phase 2.
2. Define the next tool-platform work as sequenced, testable increments.
3. Replace or remove current documents that conflict with the Tauri Panel,
   bundled runtime packaging, and user-managed PostgreSQL.

Historical ADRs, completed plans, release records, and model-check results are
not current system references. They remain available as records unless the
owner explicitly authorizes deletion.

## Evidence

- `docs/archive/spec/06-tools-and-shortcuts.md` records deferred MCP support,
  sandboxed tools, user-editable policy, persistent fetch grants, and additional
  search types.
- `daemon/crates/nook-core/src/policy.rs` contains Cedar evaluation and an
  in-memory grant store.
- `daemon/crates/nookd/src/main.rs` does not load Cedar or YAML policy files.
- `policy/tools.yaml` exists, but the running Daemon does not load it.
- `README.md` describes the Tauri Panel and bundled runtime distribution.
- `docs/archive/spec/00-overview.md` records the retired macOS SwiftUI Panel
  and its earlier tool scope.

## Work sequence

1. Build an evidence table for current behavior, current requirements, and
   historical records.
2. Create a single plan that closes the policy integration gap before adding
   new privileged tools.
3. Update the overview, implementation plan, and checklist to distinguish
   completed work from incomplete work.
4. Create a current specification for the next tool platform. Keep the
   deferred scope separate from code that has not been designed or built.
5. Replace stale current specifications with a small, linked reference set.
6. Add an index for historical plans so current work does not rely on prior
   execution records.
7. Delete only files that the owner identifies after reviewing the triage
   result. Move retained historical records to an archive only if the owner
   chooses that structure.

## Validation

1. Check every changed cross-reference and Markdown link.
2. Confirm current behavior claims against source files and workflow files.
3. Confirm that `docs/plan.md` and `CHECKLIST.md` describe the same active
   tasks.
4. Confirm that no current document says the Panel is SwiftUI or that tools are
   excluded from the released product.
5. Run a final prose review for unsupported claims and stale delivery status.

## Open decision

The owner must choose whether superseded plans and specifications are deleted
or retained in an archive. Deleting a historical record removes evidence that
may be useful when tracing earlier decisions.
