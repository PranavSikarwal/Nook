# Reviews

## Round 1 (spec)

- Head: `5c6d811e84a89702b7dcb3a109b38d744b8cce8a`
- Reviewer: the T3 reviewer (required for a new feature with authorization and sandbox impact). Verdict: REQUEST_CHANGES (P0: 0, P1: 1, P2: 4, P3: 1).
- SonarCloud: Quality Gate passed, 0 new issues, 0 hotspots.
- Checks: `Code Quality and Tests`, `Windows named-pipe IPC`, `Native Tauri E2E Gate` and `SonarCloud Code Analysis` all pass. The CI gate check passes.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S1 (P1) | The link lifecycle relies on a back-off that does not exist. | fix | `specs/design.md` "Link lifecycle": plain connect, retry every 5 s, 60 s after an `error`, ends with the Panel. |
| S2 (P2) | The registry swap names no link state, no lock and no stale-approval rule. | fix | `specs/design.md` "Offering the tool": `DesktopLink` handle in `Carry`, one lock, `deny_stale_approvals` reason, catalog text, error cases. |
| S3 (P2) | Link replacement is not defined, and Risk 1 relied on it. | fix | `specs/design.md` "The desktop link" (first live link wins) and Risk 1. |
| S4 (P2) | The Panel does not check the arguments, and the title may be read as an `osascript` option. | fix | `specs/design.md` "Run in the Panel": Panel checks the arguments, one HTML argument that starts with `<`, tests with `-e` and `-l`. |
| S5 (P2) | The Panel Rust tests have no command and no CI step. | fix | `specs/design.md` "Commands" and "Central docs that change" (`docs/testing.md`, `.github/workflows/ci.yml`). |
| S6 (P3) | `git diff --check` fails on `plans/plan.md` (blank line at the end). | fix (small) | `plans/plan.md`. |
| Gate 1 item | The long-lived link is a design decision for the owner. | ask owner | `open-gaps.md` G4. |
| Gate 1 item | The move of the draft leaves four dangling paths. | no change | The owner told me to move the draft. Risk 6 and the gate 1 message name the four files. |
| G1 note | Under option 1 the Cedar `folder` condition never fails. | fix | `open-gaps.md` G1 recommendation says so. |
| G3 note | `scripts/bundle.sh` signs with no `--entitlements`. | fix | `specs/design.md` "Signing check": `bundle.sh` gets only the usage description. |

## Round 2 (spec)

- Head: `9d191c3d26bb4cb61c1aafbdef22685f9439a3c9`
- Reviewer: the T3 reviewer, follow-up of round 1. Verdict: COMMENT (P0: 0, P1: 0, P2: 1, P3: 0). It resolved the threads of S1, S3, S4 and S5.
- SonarCloud: Quality Gate passed.
- Checks: all four pass. The CI gate check passes.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S2 (P2) | A call after the swap is denied with `Tool runs in the Worker` (`tool_approval.rs:370-377`), not `Unknown tool`, and the denial ends the reply. | fix | `specs/design.md` "Offering the tool" and the failure table: the spec states the denial and a test pins it. |
| R1 | The tokio lock in `folders.rs` `apply` is not re-entrant. | fix | `specs/design.md` "Offering the tool" rule 2: the function takes the held guard. |
| R2 | The new Panel `cargo test` step may fail on the CI runner. | fix | `specs/design.md` "Commands": the gap rule covers tests, and the CI step comes after the base passes. |
| G4 | The reviewer recommends option 2 with an addition. | ask owner | `open-gaps.md` G4 lists both options and the reviewer's position. |

## Round 3 (spec)

- Head: `b9a08632a3f55ca85e4a0e68c2a2e86e0337e479`
- Reviewer: the T3 reviewer, follow-up of round 2. Verdict: COMMENT (P0: 0, P1: 0, P2: 0, P3: 1). It resolved the thread of S2.
- SonarCloud: Quality Gate passed.
- Checks: all four pass. The CI gate check passes.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S7 (P3) | The spec says a test pins the post-swap denial, but the Tests list has none. | fix | `specs/design.md` "Tests" (Daemon bullet) and "Central docs that change" item 3. |

## Round 4 (spec)

- Head: pending
- Reviewer: the T3 reviewer, follow-up of round 3
- SonarCloud:
- Checks:
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |

## Commands run

One row for each named check that the task adds to a rule or a document. A row lists the commands of its check.

This task adds no named check to a rule or a document, so the table has no row.

| Command | Run | Result |
| --- | --- | --- |
