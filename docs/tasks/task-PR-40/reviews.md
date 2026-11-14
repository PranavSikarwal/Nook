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

- Head: `882682149422f9f6e368e5541c090610feec0af7`
- Reviewer: the T3 reviewer, follow-up of round 3. Verdict: APPROVE (P0: 0, P1: 0, P2: 0, P3: 0). It resolved the thread of S7.
- SonarCloud: Quality Gate passed.
- Checks: all four pass. The CI gate check passes. The round exit check passes: the verdict is APPROVE, no finding is open, and 0 review threads are unresolved.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| none | The review has no finding. | none | - |

## Round 5 (spec)

- Head: `130dea7a30e587eaff8f53f5c5c82c06997e6f5e`
- Reviewer: the T3 reviewer, follow-up of round 4. The spec changed after the owner decided G4 (the Daemon runs the tool), so this round reviewed the new design. Verdict: COMMENT (P0: 0, P1: 0, P2: 6, P3: 5). It closed S1 to S7 as moot or fixed, and it found no hole in the data path.
- SonarCloud: Quality Gate passed, 0 new issues, 0 hotspots.
- Checks: all four pass on the head (the reviewer read the native gate while it was still running). The CI gate check passes.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S8 (P2) | The step list sends `authorize_tool` for a Daemon tool, and the Daemon denies that message. | fix | `specs/design.md` "The call, step by step": `execute_tool`, then the Daemon asks Cedar and the user, then `tool_call_started`, the call, `tool_call_finished`, `tool_result`. |
| S9 (P2) | No `osascript` argument list and no script, so the closed operation cannot be checked. | fix | `specs/design.md` "Run in the Daemon": the argument list `-e SCRIPT HTML`, the script text, the working folder `/`, `required` and `additionalProperties: false`, and tests for exactly three arguments and an identical script. |
| S10 (P2) | No path from `nookd` to `assemble`, no `Carry` field, no `e2e` knowledge in `nook-core`, no `cfg(unix)` gate. | fix | `specs/design.md` "Offering the tool": `build_registry(tools, notes_program)`, `Carry` field, `nookd` decides, `#[cfg(unix)] mod notes`, tests. |
| S11 (P2) | Missing failure rows and tests: success rule, `-1743` match, output drain, NUL, non-ASCII, not offered, cannot start. | fix | `specs/design.md` rules 7 and 8 of "Run in the Daemon", "Failure behavior" and "Tests". |
| S12 (P2) | The title `pattern` behaves differently in the Worker (`re.search`) and the Daemon (Rust). | fix | `specs/design.md` "The tool": no `pattern`, one Rust check in `notes.rs` that trims one trailing line break. The reviewer recommended trim over refuse, because a refusal ends the reply. |
| S13 (P2) | The log claim is false on the schema-refusal path, which prints the refused value. | fix | `specs/design.md` "Data handling": the message names the keyword and the property and never the value, with a test. |
| S14 (P3) | The native test needs files that the spec does not list, and `e2e` is already on. | fix | `specs/design.md` "Tests" (native flow) and "Other files that change". The Notes source shows in the Tools tab, so the count in `tools.spec.mjs` changes. |
| S15 (P3) | Reuse the helpers of `Sandbox::run`, and put the group kill in a drop guard. | fix | `specs/design.md` rule 6: reuse `GroupKill`, `read_capped` and `join_reader`. |
| S16 (P3) | Three wrong sentences about existing code. | fix | `specs/design.md` "Where each tool runs", "The test build" and "Other files that change". |
| S17 (P3) | Records and Risk 7 still carry the Panel design and an old count. | fix | `open-gaps.md` G1 and G2, `plans/plan.md` title, `specs/design.md` header and Risk 7 (five places, check again at merge). The PR title still says "run by the Panel" (small fix after this round). |
| S18 (P3) | The file that holds the usage description is not named. | fix | `specs/design.md` "Signing check": `panel/src-tauri/Info.plist`, `panel/src-tauri/entitlements.plist`, the setting in `tauri.conf.json` and the text. |
| Residual 2 | Two parallel first calls could both create the `Nook` folder. | fix | `specs/design.md` rule 5 (one `osascript` at a time) and a test. |
| Residual 2 | `allow_for_chat` has no count limit. | no change | `specs/design.md` Risk 8 records it. The other tools have the same grant scope. |

## Round 6 (spec)

- Head: `8684185c75a250552c6b11458c9e80d8930df78a`
- Reviewer: the T3 reviewer, follow-up of round 5. Verdict: COMMENT (P0: 0, P1: 0, P2: 0, P3: 3). It resolved the 11 threads of S8 to S18 and found no new P2.
- SonarCloud: Quality Gate passed.
- Checks: pending when the reviewer read them. See "Round 7" for the state on the next head.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S19 (P3) | The one-at-a-time lock is new after a folder change, so two calls can run together. | fix | `specs/design.md` rule 5: one `Arc<Mutex<()>>` in `Carry`, and a test across `rebuilt`. |
| S20 (P3) | Five test details cannot pass or cannot be seen: the exact environment, the trim cases, the log test, two rows without a test, the fake Worker test. | fix | `specs/design.md` "Tests" and "Commands": `/proc/$$/environ`, the five title cases, a registry test with a second provider, a direct `call` test, a `None` registry test, and a new test in `test_fake_worker.py`. |
| S21 (P3) | The model is not told that the title is one line. | fix | `specs/design.md` "The tool": the description says it, and a test checks it. |
| Residual 4 | The worst-case HTML size is not tested. | fix | `specs/design.md` "Tests": a body of 20,000 line breaks. |
| Residual 6 | `GroupKill` has a private field, and the `assemble` code needs the `cfg(unix)` gate. | fix (small) | `specs/design.md` rule 6. |

## Round 7 (spec)

- Head: `a1a9d20665b28395b6c537e14e542ee163c33a5e`
- Reviewer: the T3 reviewer, follow-up of round 6. Verdict: COMMENT (P0: 0, P1: 0, P2: 1, P3: 2). It resolved the threads of S19, S20 and S21.
- SonarCloud: Quality Gate passed.
- Checks: pending when the reviewer read them. See "Round 8" for the state on the next head.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S22 (P2) | The test with 20,000 line breaks cannot pass on Linux: one argument may not exceed 131,072 bytes. | fix | `specs/design.md` "Tests": the Linux test uses 6,000 line breaks (under 100,000 bytes), and the Mac test sends 20,000. |
| S23 (P3) | The environment test needs `/proc`, which macOS does not have, and the folder is not read in a stated way. | fix | `specs/design.md` "Tests": that test has `cfg(target_os = "linux")`, and the fake writes `pwd -P`. |
| S24 (P3) | The description test is not in the Tests list, and the sentence is not pinned. | fix | `specs/design.md` "The tool" (the constant `The title is one line.`) and a bullet in "Tests". |

## Round 8 (spec)

- Head: `79d1c8c45355c5b89f8cb5682b48b9a2b4834bfe`
- Reviewer: the T3 reviewer, follow-up of round 7. Verdict: COMMENT (P0: 0, P1: 0, P2: 0, P3: 1). It resolved the threads of S22, S23 and S24.
- SonarCloud: Quality Gate passed.
- Checks: pending when the reviewer read them. See "Round 9" for the state on the next head.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S25 (P3) | The new Daemon test file has no name or gate, the environment assertion shares a bullet with Mac-safe checks, and the native gate does not run `cargo test`. | fix | `specs/design.md` "Tests" and "Other files that change": the file `tests/notes_tool_tests.rs` with `#![cfg(unix)]`, the environment check in its own bullet with `cfg(target_os = "linux")`, and CI named as the place that runs it. |

## Round 9 (spec)

- Head: pending (the commit that adds this heading)
- Reviewer: the T3 reviewer, follow-up of round 8.
- SonarCloud: pending
- Checks: pending
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |

## Commands run

One row for each named check that the task adds to a rule or a document. A row lists the commands of its check.

This task adds no named check to a rule or a document, so the table has no row.

| Command | Run | Result |
| --- | --- | --- |
