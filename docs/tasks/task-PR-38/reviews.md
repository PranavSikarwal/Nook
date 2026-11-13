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

- Head: `77a4c00`
- Reviewer: T3 reviewer, review on `77a4c00`, verdict REQUEST_CHANGES. P0: 0,
  P1: 1, P2: 8, P3: 6. Round 1 findings 2, 4, 6, 7, 9, 10, 11, 13, 15, 16 and
  17 are fixed.
- SonarCloud: gate passed.
- Checks: all four passed.
- Owner input in the same round: the review process moves into its own rule,
  `.agents/rules/review-standard.md`, with two reviewers (gap G10, decided).
  The owner confirmed what a round is.
- Gaps closed by recommendation since round 1: none.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| 18 | The merged head is not the reviewed head, and the native gate does not restart. | fix | SDLC stage 8, steps 2 to 6. |
| 1 | Three binding texts still say the opposite. | fix | Spec "Rule changes": `AGENTS.md` "Where things are" and "Issue tracker", `docs/agents/issue-tracker.md`, `CONTRIBUTING.md` "Pull requests". The pull request merges only with every change in the table. |
| 3 | No verdict line format, no rule for whose review counts. | fix | Review rule "Verdict line" and "Read the inputs" 1. |
| 5 | The implementer ban has no check. | fix | SDLC stage 6 "Check the boundary", and "The brief passes no token". |
| 12 | The AI attribution ban covers commits only. | fix | SDLC "Roles" last paragraph, and the `CONTRIBUTING.md` row in the spec. |
| 14 | An agent cannot read the SonarCloud page. | fix | Review rule "Read the inputs" 3 and "When an input fails to arrive" 4 and 5. |
| 19 | The commands do not return what the rule says. | fix | Review rule "Read the inputs": the reviews REST call and `gh pr checks` exit codes. |
| 20 | Stage 3 loops but has no limit. | fix | Review rule: at most 3 rounds in a stage. |
| 21 | No commands for ready, merge, rerun, version, package, release. | fix | SDLC stages 5, 7, 8, 9 and review rule "When an input fails to arrive" 3. |
| 8 | `Head:` cannot hold its own SHA, stage 9.2, resume before the folder. | fix | State block without `Head:`, stage 9.2 removed, "Resume" starts with `gh pr list --head`. |
| 22 | The reviewer also uses `scripts/gh`. | fix | Review rule "What the reviewer may do". The SDLC role line names push, merge, tag, workflows and the task folder. |
| 23 | One 60-minute command cannot remind at 30 minutes. | fix | Review rule "Run a round" 3 and SDLC "T3 Code" 2: commands of at most 30 minutes. |
| 24 | Missing failure cases. | fix | SDLC "Push", stage 6 crash with uncommitted files, a design over several pull requests in "Task folder", review rule: a round after a conflict counts toward the limit. |
| 25 | The G8 precedent is wrong. | fix | G8 text, and SDLC stage 5 release task for round fixes after the release task. |
| 26 | The empty commit has no scope. | fix | `chore(tasks): open task`. |
| G9 | Rename, and add "touches only the named files". | fix | G9 title and option (a), SDLC stage 6. Each round lists closed gaps. |

## Round 3 (spec)

- Reviewer: T3 reviewer (spec review, required by the review rule).
- Inputs pending.
