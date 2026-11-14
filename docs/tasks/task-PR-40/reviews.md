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

- Head: `97edd68ec3180cc094475de56568e3c668480c07`
- Reviewer: the T3 reviewer, follow-up of round 8. Verdict: APPROVE (P0: 0, P1: 0, P2: 0, P3: 0). It resolved the thread of S25.
- SonarCloud: Quality Gate passed.
- Checks: `Code Quality and Tests`, `Windows named-pipe IPC`, `Native Tauri E2E Gate` and `SonarCloud Code Analysis` all pass. The CI gate check passes. The round exit check passes: the verdict is APPROVE, no finding is open, and 0 review threads are unresolved.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| none | The review has no finding. | none | - |

## Round 10 (spec)

- Head: `0517bf3230598c57b19692d8c9da5fe2cb6abd0d`
- Reviewer: the T3 reviewer, follow-up of round 9. The owner widened the design after the approval of round 9: six tools in a folder that the user chooses in Settings (gaps G1, G2 and G5). This round reviewed the whole new spec. Verdict: COMMENT (P0: 0, P1: 0, P2: 5, P3: 2). The reviewer found the claims about existing code true, and the Windows compile, the Cedar and `tools.yaml` design and the decisions G1 to G5 sound.
- SonarCloud: Quality Gate passed.
- Checks: pending when the reviewer read them. See "Round 11" for the state on the next head.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S26 (P2) | The Panel unit test cannot render `ToolsTab.tsx`: the project has no test renderer. | fix | `specs/design.md` "Tests": the field logic moves to `panel/src/lib/notesFolder.ts` with a `node --test` file, and the native spec covers the rendered field. |
| S27 (P2) | The Panel cannot repair a bad folder name, because `available` hides the field when the tools are off. | fix | `specs/design.md` "The folder setting": `notes_settings` carries `supported` and an optional `reason`, and the field stays visible when `supported` is true. |
| S28 (P2) | No way to drop chat grants by tool, `policy.rs` missing from the file list, and a waiting approval stays open after a folder change because `facts` succeeds. | fix | `specs/design.md` "The folder setting": `GrantStore::remove_tools`, and a function that denies every pending Notes approval at the change. File list and tests updated. |
| S29 (P2) | Append, edit and delete are not written out in full. | fix | `specs/design.md` "Run in the Daemon": six full scripts, with the argument index of each value. |
| S30 (P2) | Risk 9 says every later call needs its own approval, which is false for `allow_for_chat`. | fix | `specs/design.md` Risks 8 and 9: only delete always asks, the data-loss path is named, and the owner can remove `allow_for_chat` from edit and append in `tools.yaml` (gap G5). The owner chose the approval table. |
| S31 (P3) | A list over the cap is cut in the middle of a line with no marker. | fix | `specs/design.md` rule 7: the script cuts a title at 200 characters, the list cap is 128 KiB, and the Daemon adds a last line `[Cut]` when it drops bytes. A test covers it. |
| S32 (P3) | The contract test change is hedged. | fix | `specs/design.md` "Tests" and the file list: the negative cases are named. |

## Round 11 (spec)

- Head: `03dbb2947696d7bcfaadcee9fc35b8e3bfd6a2da`
- Reviewer: the T3 reviewer, follow-up of round 10. Verdict: REQUEST_CHANGES (P0: 0, P1: 1, P2: 2, P3: 1). It closed S26 to S29, S31 and S32, and it read the six scripts as text with no problem.
- SonarCloud: Quality Gate passed.
- Checks: pending when the reviewer read them. See "Round 12" for the state on the next head.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S33 (P1) | The list and read caps ignore `MAX_TOOL_OUTPUT_BYTES` (64 KiB), which cuts every tool result and adds `[output truncated]`. The `[Cut]` line and the 128 KiB and 96 KiB caps cannot happen. | fix | `specs/design.md` rule 7: one limit, `MAX_TOOL_OUTPUT_BYTES`. The provider keeps one byte more, so `truncate_output` cuts and marks. No `[Cut]`. Boundary tests: the cap, the cap plus one, and a four-byte character at the cut. |
| S34 (P2) | The folder change leaves a window: a claimed decision runs in the new folder, a card built from the old registry can be added after the pass, and a grant for the old folder can match. The text says "never". | fix | `specs/design.md` "The folder setting": the whole sequence runs inside the write lock of `state.config`, each pending approval stores its folder, and a decision compares it under the read lock before it records a grant. Race tests added. |
| S30 (P2, open part) | The owner's choice of `allow_for_chat` is in no gap. | fix | `open-gaps.md` G6 records the options and the owner's earlier word "works" to the proposal. `specs/design.md` Risk 8 and "Open gaps". |
| S35 (P3) | No row for a failed write of `config.toml`, no tests that another tool keeps its approval and grant, no `supported` false test and no fixed reason texts. | fix | `specs/design.md` "Failure behavior" and "Tests": the rows, the tests, and the two fixed `reason` texts. |
| Residual | A note title with a line break could add a false row to a list. | fix (small) | `specs/design.md` manual test: a title typed with Shift and Return. |

## Round 12 (spec)

- Head: `8b6b23ca2186ac76aa32244f8901241634a17831`
- Reviewer: the T3 reviewer, follow-up of round 11. Verdict: REQUEST_CHANGES (P0: 0, P1: 1, P2: 2, P3: 1). It closed S33 as designed and found the folder-change mechanism incomplete.
- SonarCloud: Quality Gate passed.
- Checks: pending when the reviewer read them. See "Round 13" for the state on the next head.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S36 (P1) | The folder guarantee is not met: `Execution` has no registry and `start_execution` reads the current one again, a call allowed by a chat grant has no lock, and the new deny function does not say what it does with `decision_in_flight`. | fix | `specs/design.md` "The folder setting": four rules (read lock in both paths, `registry` in `Execution`, folder stored in the pending entry and compared at decision time, skip `decision_in_flight`), and the file list. |
| S37 (P2) | The race tests cannot be forced without sleeps. | fix | `specs/design.md` "Tests": the test holds `state.config.write()`, and the harness owns `AppState`. Both orders and the chat-grant path are tested. |
| S38 (P2) | The boundary test cannot pass for a list, because the result text holds a header before the cut. | fix | `specs/design.md` "Tests": the limit counts the final result text, and the list fake is sized minus the header. |
| S39 (P3) | A missing test for the `reason` text, a sentence with no use, and a failure row that still says "kept part". | fix | `specs/design.md` rule 7, "Failure behavior" and "Tests". |

## Round 13 (spec)

- Head: `7eb0283a2593681fb9b550f4d35f1cc40d4f9aa1`
- Reviewer: the T3 reviewer, follow-up of round 12. Verdict: COMMENT (P0: 0, P1: 0, P2: 1, P3: 1). It closed S36 to S39 and found the four rules sound against the real lock use.
- SonarCloud: Quality Gate passed.
- Checks: pending when the reviewer read them. See "Round 14" for the state on the next head.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S40 (P2) | Rule 1 holds the read lock across the delivery and the tool run, and starts it too late, after the `Execution` is built and the registry is read twice. | fix | `specs/design.md` "The folder setting" rule 1: the lock starts at the beginning of both paths, the registry is read once, and the lock ends when the `Execution` holds it and the grants are checked. Delivery and run are after the release. |
| S41 (P3) | `Arc<ProviderRegistry>` does not compile with `derive(Debug)`, and a claimed decision comes before the lock. | fix | `specs/design.md` rules 2 and 4: a manual `Debug` for `ProviderRegistry`, and a claimed decision that waits behind the writer is denied by the comparison. |

## Round 14 (spec)

- Head: `55c3ca143fd99b0586d5cd6d0be1073d27640b01`
- Reviewer: the T3 reviewer, follow-up of round 13. Verdict: APPROVE (P0: 0, P1: 0, P2: 0, P3: 0). It closed S40 and S41.
- SonarCloud: Quality Gate passed.
- Checks: `Code Quality and Tests`, `Windows named-pipe IPC`, `Native Tauri E2E Gate` and `SonarCloud Code Analysis` all pass. The CI gate check passes. The round exit check passes: the verdict is APPROVE, no finding is open, and 0 review threads are unresolved.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| none | The review has no finding. | none | - |

## Commands run

One row for each named check that the task adds to a rule or a document. A row lists the commands of its check.

This task adds no named check to a rule or a document, so the table has no row.

| Command | Run | Result |
| --- | --- | --- |
