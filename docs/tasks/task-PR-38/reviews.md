# Review rounds: PR #38

## Round 1 (spec)

- Head: `9155703`
- Reviewer: review on `9155703`, verdict REQUEST_CHANGES. P0: 0, P1: 5, P2: 9,
  P3: 3.
- SonarCloud: gate passed.
- Checks: `Code Quality and Tests`, `Windows named-pipe IPC`, `SonarCloud Code
  Analysis` and `Native Tauri E2E Gate` passed.
- Owner input in the same round: the spec is the design (no separate design
  file). The deliverable is the rule itself, named SDLC Work Standard, in
  `.agents/rules/`.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| 1 | `docs/tasks/` conflicts with the planning rules in `AGENTS.md` and other files. | fix | "Rule changes" table in the spec. |
| 2 | An implementer closes gaps and continues, against "Done" 3. | fix | Rule stage 6 and "Implementer rules": the implementer stops and reports, the orchestrator decides. Gap G9. |
| 3 | No observable exit for a review round, no round limit. | fix | Rule "Review round" exit check, limit of 3 rounds in stage 7. |
| 4 | Nothing starts the native gate, the checks are not named. | fix | Rule "Review round" step 2 and the four check names. |
| 5 | No role table, delegate powers open. | fix | Rule "Roles" and the last paragraph of "Implementer rules". |
| 6 | Stage 1 cannot open a pull request before the folder exists. | fix | Rule stage 1 opens with an empty commit. Spec "This task is the first one". |
| 7 | Version bump and changelog come too late. | fix | Rule stage 5 release task, stage 9. Gap G8. |
| 8 | No state block, no resume. | fix | Rule state block, "Resume", round opened at its start. |
| 9 | Several writers on one worktree. | fix | Rule stage 6: one implementer at a time. Implementer rule 3: `git add <paths>`. |
| 10 | No rule for a dead, duplicate or stale reviewer. | fix | Rule "When an input fails to arrive" 1 and 2. `reviewers.yml` is a separate GitHub reviewer and is not part of the rule. |
| 11 | Round fixes have no plan task. | fix | Rule "Review round" step 5. |
| 12 | Codex and Antigravity miss the commit rules. | fix | Implementer rule 4 points to `CONTRIBUTING.md` and `AGENTS.md`. Gap G5 adds the PR-text ban to `CONTRIBUTING.md`. |
| 13 | G6 is out of scope and cannot enforce the rule. | fix | G6 recommendation changed to (b). |
| 14 | No rule for flaky CI, missing checks, SonarCloud issues, conflicts with `main`. | fix | Rule "When an input fails to arrive" 3 to 6. |
| 15 | Gap fields do not match the text, no template. | fix | `Decided by` and `Date` added. The template is implementation task 1. |
| 16 | The shell poll has no bound. | fix | Rule "T3 Code" 2: one command, at most 60 minutes. |
| 17 | Importing four rule files contradicts the load risk. | fix | One rule file, one import. |

## Round 2 (spec)

Opened. Inputs pending.
