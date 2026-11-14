# Reviews

## Round 1 (spec)

- Head: `3a7375dc1c791bd384bba50ae5f454c254835a25`
- Reviewer: the T3 reviewer ("Review Pull Requests"). The task changes MCP docs and the tests of an authorization path, so the review standard requires it.
- Verdict: `Verdict: COMMENT (P0: 0, P1: 0, P2: 4, P3: 4)` (review 5900811489)
- SonarCloud: Quality Gate passed. 0 new issues, 0 hotspots.
- Checks: `Code Quality and Tests` pass, `Windows named-pipe IPC` pass, `SonarCloud Code Analysis` pass, `Native Tauri E2E Gate` pending when recorded. Run the CI gate check again on the final head.
- Gaps closed by recommendation: none. G1 to G6 wait for the owner at gate 1.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S1 (P2) | The tests do not use `network = true` or the token folder in `write_paths`. | fix | `specs/design.md`, "The fake server and the tests": the fake gets `refreshToken`, `readTokenFile`, `exportDocument` and `connect`, and three new tests use them. |
| S2 (P2) | The fake keeps no record of calls, so "no call recorded" cannot be tested. | fix | `specs/design.md`: `insertText` appends to `inserted.log` in the token folder, and the deny test asserts that the file does not exist. |
| S4 (P2) | G2 option 1 is wrong: `createGoogleDoc` needs a Drive scope. | fix | `open-gaps.md` G2, and `specs/design.md` "What the model sees". |
| S5 (P2) | The card shows 300 characters, not each write. | fix | `specs/design.md` "What the model sees" and Risks 7. |
| S3 (P3) | Three of the five tests repeat existing tests. | fix | `specs/design.md`: the call and error tests are merged or dropped. The list now has seven tests, each with a new behavior. |
| S6 (P3) | The sample test is not specified enough. | fix | `specs/design.md`: the new file `tests/docs_sample_tests.rs`, the wrapper, the name check and `#[cfg(unix)]`. |
| S7 (P3) | Two existing passages of `docs/mcp-servers.md` become wrong. | fix | `specs/design.md` "Central docs that change". |
| S8 (P3) | Three facts miss from the risks: the client secret under `network`, the 10 second start limit, and the masked `env` values in the log. | fix | `specs/design.md` Risks 2, 8 and 9, and "When Google Docs stops working" 5 and 6. |
| G1, G4, G5 text | The reviewer adds a sentence to each. | fix | `open-gaps.md` G1 (accept `network = true`), G4 (`ps` reads `env`), G5 (decide again with a Drive scope). |

## Round 2 (spec)

- Head: `f9b0fad58b8e1d2c7f8d5b1bb1b26d6882e0b5db` (the reviewed head)
- Reviewer: the T3 reviewer ("Review Pull Requests"), follow-up of round 1.
- Verdict: `Verdict: APPROVE (P0: 0, P1: 0, P2: 0, P3: 0)` (review 5900857395). The reviewer resolved all 8 threads. 0 threads are unresolved.
- SonarCloud: Quality Gate passed.
- Checks: all four pass on the head (`Code Quality and Tests`, `Windows named-pipe IPC`, `SonarCloud Code Analysis`, `Native Tauri E2E Gate`).
- Gaps closed by recommendation: none. G1 to G6 wait for the owner at gate 1.

The review makes four notes for the plan, not findings:

| # | Note | Action | Where |
| --- | --- | --- | --- |
| N1 | The name rule of the sample test copies `valid_server_name`. Use a `pub` helper or a test in `mcp.rs` so that it cannot drift. | plan | Plan task for the sample test. |
| N2 | Test 6 asserts that the outside file does not exist, and uses a read-only bound folder as the outside target. | plan | Plan task for the tests. |
| N3 | The fake `connect` tool uses `TcpStream::connect_timeout`. | plan | Plan task for the fake server. |
| N4 | Tests 3 and 4 share the helper that builds the `inserted.log` path. | plan | Plan task for the tests. |

## Round 3 (spec)

- Head: `a11df5d9df070d2dff2bec44288685083cd1e384`
- Reviewer: the T3 reviewer ("Review Pull Requests"). The owner decided G1 to G7 and the spec changed to add the secret store, so this round reviews the new design.
- Verdict: `Verdict: REQUEST_CHANGES (P0: 0, P1: 2, P2: 7, P3: 3)` (review 5901338192)
- SonarCloud: Quality Gate passed.
- Checks: `Code Quality and Tests` pass, `SonarCloud Code Analysis` pass, `Windows named-pipe IPC` pending and `Native Tauri E2E Gate` not listed when recorded. Run the CI gate check again on the final head.
- Gaps closed by recommendation: none. G8 is new and decided by the owner.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S9 (P1) | The mask list is built by the Panel, so the plan to add resolved secrets in `daemon_log.rs` fails. | fix | `specs/design.md` "The `env_secrets` setting" 4: a redacting writer in `nookd` over a shared list, and the server stderr line goes through it. Test 5 of the unit tests. |
| S10 (P1) | On Linux each env value is a `bwrap` argument that any user can read. | fix (owner decided, G8) | `specs/design.md` "The `env_secrets` setting" 6, Risk 11, new `sandbox_tests.rs` tests. |
| S11 (P2) | `keyring` 4.2.0 has no mock and links the real store. | fix | `specs/design.md` "The secret store" 3: `keyring-core` and three store crates behind the feature `os-store`, mock store `keyring_core::mock::Store`. |
| S12 (P2) | A store read can block the Daemon. | fix | `specs/design.md` "The secret store": `spawn_blocking` with a 10 second timeout. Test 8. Restart in "The `env_secrets` setting" 3 and test 9. |
| S13 (P2) | The new store shares the namespace of the model key. | fix | `specs/design.md` "The secret store" 1 and 3: service `Nook MCP`, the name `model-api-key` is refused. |
| S14 (P2) | `nookctl secret` needs the Daemon socket, echo-off is unnamed, `\r\n` is not trimmed, and `nookctl` has no download place. | fix | `specs/design.md` "The secret store" and "The steps for the owner" 11. |
| S15 (P2) | A refusal at load time fails the whole file. | fix | `specs/design.md` "The `env_secrets` setting" 2: the check runs in `start_reporting` and skips one server. Test 11. |
| S16 (P2) | The error text of a failing store can hold the secret. | fix | `specs/design.md` "The secret store" 2: `SecretError` with fixed texts. Test 7. |
| S17 (P2) | A secret can leave in a tool result. | fix | `specs/design.md` "The `env_secrets` setting" 5: scrub, and the fake `checkEnv` and `echoEnv`. Tests 5 and 10. Risk 12. |
| S18 (P3) | Two passages of `docs/mcp-servers.md` and a place for `nookctl` are missing from the docs list. | fix | `specs/design.md` "Central docs that change" 1.7, 1.8 and 1.10. |
| S19 (P3) | In external mode the Drive tools fail inside the server. | fix | `specs/design.md` "The server" 5 and "What the model sees". `open-gaps.md` G2 and G5. |
| S20 (P3) | The state block and `reviews.md` did not show the round. | fix | The state block is `Stage: 3 spec review`, and this heading exists. |

## Round 4 (spec)

- Head: `ac0b6c6bfec9ec035aaca19ca69f401a8b340045`
- Reviewer: the T3 reviewer ("Review Pull Requests"), follow-up of round 3.
- Verdict: `Verdict: COMMENT (P0: 0, P1: 0, P2: 1, P3: 5)` (review 5901830268)
- SonarCloud: Quality Gate passed.
- Checks: `Code Quality and Tests`, `Windows named-pipe IPC` and `SonarCloud Code Analysis` pass. `Native Tauri E2E Gate` runs at stage 6. Run the CI gate check again on the final head.
- Gaps closed by recommendation: none. G9 is new and decided by the owner.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S21 (P2) | `LD_*` names in a server `env` would change how `bwrap` loads libraries on the host. | fix (owner decided, G9) | `specs/design.md` "The `env_secrets` setting" 7, test 11. `open-gaps.md` G9. |
| S22 (P3) | The G8 text names a wrong test, and promises more than "other users cannot read it". | fix | `specs/design.md` "The `env_secrets` setting" 6 and "Tests in `sandbox_tests.rs`". |
| S23 (P3) | The writer starts with an empty list, and page cause 6 is true only when the Panel starts the Daemon. | fix | `specs/design.md` "The `env_secrets` setting" 4.1 and "When Google Docs stops working" 6. |
| S24 (P3) | The store crates need named features, and a store that cannot be built must not crash. | fix | `specs/design.md` "The secret store". |
| S25 (P3) | `spawn_blocking` cannot be cancelled. | fix | `specs/design.md` "The secret store". |
| S26 (P3) | "Why" 3 promises more than Risk 12, and test 7 needs the `keyring-core` mock. | fix | `specs/design.md` "Why" 3 and test 7. |

## Round 5 (spec)

- Head: `af647a5a0c8702c44a4fc36fcf189c4941981047`
- Reviewer: the T3 reviewer ("Review Pull Requests"), follow-up of round 4.
- Verdict: `Verdict: COMMENT (P0: 0, P1: 0, P2: 0, P3: 4)` (review 5901857793)
- SonarCloud: Quality Gate passed.
- Checks: run the CI gate check again on the final head.
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S27 (P3) | The G9 rule has no stated limit or text. | fix | `specs/design.md` "The `env_secrets` setting" 7. |
| S28 (P3) | The G9 test has one case and no counter case. | fix | `specs/design.md` test 11. |
| S29 (P3) | The docs list and the changelog do not name G9, and it breaks existing servers. | fix | `specs/design.md` "Scope", "Central docs that change" 1.11 and 5, Risk 11. |
| S30 (P3) | `open-gaps.md` ends with a blank line. | fix | `open-gaps.md` ends with one newline. |

## Round 6 (spec)

- Head: `f2206ef94a2cb202d1588b7103d67b81b115f466` (the reviewed head)
- Reviewer: the T3 reviewer ("Review Pull Requests"), follow-up of round 5.
- Verdict: `Verdict: APPROVE (P0: 0, P1: 0, P2: 0, P3: 0)` (review 5901867062). 0 threads are unresolved.
- SonarCloud: Quality Gate passed.
- Checks: all four pass on the head (`Code Quality and Tests`, `Windows named-pipe IPC`, `SonarCloud Code Analysis`, `Native Tauri E2E Gate`, run 43561832880). The CI gate check printed `GATE_OK`.
- Gaps closed by recommendation: none. G1 to G9 are decided by the owner.

No findings. The spec went through rounds 1 to 6 with the owner's decisions G1 to G9 in between.

## Round 1 (code)

- Head: `03db730db01a6eac6e747ca825fa2fe5c7575a59`
- Reviewer: the T3 reviewer ("Review Pull Requests"). The task changes the sandbox, authorization-adjacent MCP code and the Daemon core, so the review standard requires it.
- Verdict: `Verdict: COMMENT (P0: 0, P1: 0, P2: 2, P3: 8)` (review 5903614644). 0 threads are unresolved.
- SonarCloud: Quality Gate passed.
- Checks: `Code Quality and Tests`, `Windows named-pipe IPC`, `SonarCloud Code Analysis` and `Native Tauri E2E Gate` pass on the head.
- Gaps closed by recommendation: none
- Found in stage 6, before this round:
  1. SonarCloud failed the Quality Gate on head `efd389a` (`C Reliability Rating on New Code`): three blocking file calls in the async `main` of `nookctl`. Fixed by plan task 13 (`494b770`). SonarCloud passes on the later heads.
  2. The job `Code Quality and Tests` hung on heads `ebdf354` to `494b770` in `a_store_that_never_answers_keeps_the_server_off_after_the_timeout`: the test helper blocked forever in a blocking thread, and the test runtime waits for blocking threads when it stops. The MCP tests skip on the author machine, so only CI ran it. Fixed as a small fix (`67e0c5f`): the helper returns after 5 seconds. All four checks pass on `67e0c5f`.
  3. One run of `the_daemon_exits_on_sigterm_with_no_client` failed once on the author machine (exit by signal) and passed in 11 later runs. The Daemon binds its socket before it installs the signal handler, so a SIGTERM that comes right after the connect can win. This is older than this task. No change in this task.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| P-1 (stage 6) | SonarCloud reliability finding in `nookctl` | fix | plan task 13, `494b770` |
| P-2 (stage 6) | Test helper that never returns hangs CI | fix (small) | `67e0c5f` |
| C1 (P2) | Replacement order leaks the tail of a longer secret; two masking routines exist. | fix | plan R1-1 |
| C2 (P2) | The sandbox environment test fails when `USER` is unset on the host, and checks one variable only. | fix | plan R1-2 |
| C3 (P3) | An empty stored value starts the server; the startup error reaches the report unscrubbed. | fix | plan R1-3 |
| C4 (P3) | `SandboxSpec` derives `Debug` and holds secret values. | fix | plan R1-2 |
| C5 (P3) | Lock poisoning is handled two ways, and `scrub` fails open. | fix | plan R1-1 |
| C6 (P3) | The newline is trimmed twice, and `check_name` gives a misleading text. | fix | plan R1-4 |
| C7 (P3) | The restart test never reads the log, and `docs/testing.md` says more than the test checks. | fix | plan R1-3 and R1-5 |
| C8 (P3) | The Panel `Cargo.lock` has 14 unrelated changed lines. | fix | plan R1-5 |
| C9 (P3) | Four public items have no doc comment. | fix | plan R1-4 |
| C10 (P3) | `check_server_settings` writes the `LD_` message twice. | fix | plan R1-3 |

## Round 2 (code)

- Head: pending (the commit that adds this heading)
- Reviewer: the T3 reviewer ("Review Pull Requests"), follow-up of round 1.
- Verdict:
- SonarCloud:
- Checks:
- Gaps closed by recommendation since round 1: G10 (extra variant `SecretError::Empty`), G11 (overlapping secrets in the mask are accepted as best effort).
- Fix commits for round 1: R1-1 `0f5ed7a`, R1-2 `9ffa939`, R1-3 `cc20f1b`, R1-4 `4f570a2`, R1-5 `7cb811f`. CI passed on `9ffa939`.

| # | Finding | Action | Where |
| --- | --- | --- | --- |

## Commands run

This task adds no named check to a rule or a document.

| Command | Run | Result |
| --- | --- | --- |
