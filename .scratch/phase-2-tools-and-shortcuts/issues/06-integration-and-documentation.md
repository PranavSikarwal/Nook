# Verify the tools and shortcuts flow

Type: task
Status: open
Blocked by: 02, 04, 05

## Goal

Test the complete feature and align the existing specifications with the new
behavior.

## Work

- Add unit and contract tests for all new messages and policies.
- Add integration tests for grants, denial, Escape cancellation, and a new
  Message after cancellation.
- Add Panel tests for focused shortcuts and approval cards.
- Update overview and component specifications with links to the new spec.
- Run Worker, Daemon, Panel, and packaging validation.

## Done when

The test suite proves the acceptance criteria in
`docs/spec/06-tools-and-shortcuts.md`.
