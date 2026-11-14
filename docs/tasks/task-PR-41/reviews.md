# Reviews

## Round 1 (spec)

- Head: e19a301ecb710b5f50dd0ebb3d0bc39f89374e61 (the commit that adds this heading)
- Reviewer: the T3 reviewer (required: bug fix in the Worker core)
- SonarCloud: not applicable to a docs-only head
- Checks: not applicable to a docs-only head
- Gaps closed by recommendation: none

- Review: `Verdict: COMMENT (P0: 0, P1: 0, P2: 2, P3: 3)` from `PranavSikarwal`, on this head.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S1 | The claim that `docs/contracts.md` lists the folder reasons is false, and nothing pins the Worker keys (P2). | fix: claim removed, test 8 added, risk 1 rewritten | `specs/design.md` Design, Risks; `open-gaps.md` G2 |
| S2 | Other refusals of the same tools keep the wrong notice, and G1 does not ask the owner (P2). | fix: new gap G4 and an explicit out-of-scope line | `specs/design.md` Scope; `open-gaps.md` G4 |
| S3 | Test 6 holds two behaviors, and the spec does not say how the tests reach the code (P3). | fix: split into tests 6 and 7, the driver is stated, `mark_denied` gets a default | `specs/design.md` Design |
| S4 | The docs list is incomplete (P3). | fix: four doc places named, changelog category Fixed. The row of item 11 in `docs/plans/open-items-fixes.md` belongs to the housekeeping task: the owner is told at gate 1 | `specs/design.md` Scope, Central docs |
| S5 | The state block is one stage behind (P3). | fix: `Stage: 3 spec review` | `specs/design.md` |

## Round 2 (spec)

- Head: fb136785e479eb5b7901cb281ddf23e48322b3de (the commit that adds this heading)
- Reviewer: the T3 reviewer (follow-up of round 1)
- SonarCloud: not applicable to a docs-only head
- Checks: not applicable to a docs-only head
- Gaps closed by recommendation: none
- Review: `Verdict: COMMENT (P0: 0, P1: 0, P2: 0, P3: 1)` from `PranavSikarwal`, on this head. S1, S2, S3 and S5 are fixed. S4 is partly fixed.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S4 | The spec names no owner for the stale row of item 11 in `docs/plans/open-items-fixes.md` (P3). | fix: PR #42 named, new gap G5 asks the owner | `specs/design.md` Scope; `open-gaps.md` G5 |

## Round 3 (spec)

- Head: c45815798f1a1e8e5260a77919513f8625c8a7d4 (the commit that adds this heading)
- Reviewer: the T3 reviewer (follow-up of round 2)
- SonarCloud: not applicable to a docs-only head
- Checks: not applicable to a docs-only head
- Gaps closed by recommendation: none
- Review: `Verdict: COMMENT (P0: 0, P1: 0, P2: 0, P3: 1)` from `PranavSikarwal`, on this head. S4 is fixed.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| S6 | G5 states a conflict with PR #42 that does not exist, and leaves out the stale line in `docs/plans/README.md` (P3). | fix: conflict claim removed, ownership given as the reason, README line added, assumption stated | `open-gaps.md` G5; `specs/design.md` Scope |

## Commands run

This task adds no named check to a rule or a document.
