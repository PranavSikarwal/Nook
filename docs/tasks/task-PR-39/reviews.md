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

- Head: pending (the commit that adds this heading)
- Reviewer: the T3 reviewer ("Review Pull Requests"), follow-up of round 1.
- SonarCloud:
- Checks:
- Gaps closed by recommendation: none

| # | Finding | Action | Where |
| --- | --- | --- | --- |

## Commands run

This task adds no named check to a rule or a document.

| Command | Run | Result |
| --- | --- | --- |
