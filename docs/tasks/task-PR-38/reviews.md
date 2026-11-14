# Review rounds: PR #38

## Round 1 (spec)

The "Where" column refers to the rule before the review rule was split out in
round 2.

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

- Head: `baf8dc0`
- Reviewer: T3 reviewer (spec review, required by the review rule). Verdict
  COMMENT. P0: 0, P1: 0, P2: 6, P3: 7. Round 2 findings fixed, except 5 and 21
  (partly, now P3).
- SonarCloud: gate passed.
- Checks: all four passed.
- Owner input in the same round: every CI check is a gate, and a failing check
  is fixed before the review starts. Each task ends with a retro on the
  framework.
- Gaps closed by recommendation since round 2: none.
- Round limit: this was round 3 under the old limit of 3. The owner removed
  the limit (gap G12): rounds repeat until every finding is fixed and the
  reviewer approves.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| 27 | `gh pr checks` exits 0 with a missing check. | fix | Review rule "Read the inputs" 2: the CI gate check counts four `pass` lines. Tested: PR 38 passes, PR 34 fails. |
| 28 | Resume cannot find a merged pull request. | fix | SDLC "Resume": `--state all`, merged resumes at stage 9, branch without a pull request resumes at stage 1 step 3. |
| 29 | The Actions reviewer may post a plain comment with no verdict. | fix | Review rule "Ask the GitHub Actions reviewer" and "Read the inputs" 1. The script change is gap G11. |
| 30 | A spec round appends to a plan that does not exist. | fix | Review rule "Run a round" 5 splits spec and code rounds. |
| 31 | Reviews are counted without a login filter. | fix | Review rule "Read the inputs" 1: owner login for T3, `github-actions[bot]` for Actions. |
| 32 | `reviewers.yml` runs PR code with secrets in scope. | fix and defer | Review rule: never on a fork pull request. The workflow fix is gap G11. |
| 5 | Boundary check has no baseline, other bans unchecked. | fix | New `.agents/rules/delegation-standard.md` "Boundary check". |
| 21 | No command for the run id and the wait. | fix | SDLC stage 9: `run list` and `run watch --exit-status`. |
| 33 | `reviewers.yml` reliability. | defer | Gap G11. |
| 34 | `base_ref` interpolated in `run:`. | defer | Gap G11. |
| 35 | Attribution ban scope for the bot. | fix | SDLC "Roles": the ban covers text under the owner's account. |
| 36 | Stale "Where" column, no round 3 head. | fix | Note under round 1, head above. |
| 37 | Comment cannot be typed, small change has no design file, no pagination. | fix | Review rule: `--body-file`, spec paragraph for a small change, `--paginate`. |
| size | The SDLC file nears 12,000 characters. | fix | Implementer and T3 rules moved to `.agents/rules/delegation-standard.md`. |

## Round 4 (spec)

- Head: `80dcded`
- Reviewer: T3 reviewer. Verdict REQUEST_CHANGES. P0: 0, P1: 1, P2: 4, P3: 8.
  Round 3 findings fixed, except 27 (see 38) and 5. Deferral of 32 to 34 (G11)
  accepted.
- SonarCloud: gate passed.
- Checks: all four passed.
- Gaps closed by recommendation since round 3: none.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| 38 | The CI gate check fails with GNU grep (`\t` in `-E`). | fix | Review rule "Read the inputs" 2: `[[:space:]]+` and `sort -u`. Tested with `/usr/bin/grep`: PR 32 and PR 38 pass, PR 34 fails. |
| 39 | The unlimited loop has no stop-and-ask trigger. | fix | Review rule "Run a round": four stop-and-ask triggers, a status line every 5 rounds, and `ask owner` for a fix that changes the spec, `contracts/`, `policy/`, a public interface, authorization, the sandbox or a dependency. Rounds still have no limit. |
| 40 | The SDLC file is 19 characters under the limit. | fix | Task folder, open gaps and retro moved to `.agents/rules/task-records.md`. The SDLC file is now 9,850 characters. |
| 41 | The retro has no exit command and no home. | fix | Task records "Retro": exact headings, `Owner: accepted/rejected` lines, an exit check, accepted proposals as deferred gaps. |
| 42 | The spec is stale. | fix | Spec "The rules" lists four files, risk 1 updated, "This pull request applies every row". |
| 5 | The boundary check misses merges, comments, reviews, runs. | fix | Delegation rule "Boundary check": `pr view` and `run list` baselines. |
| 43 | The exit check has no reading command. | fix | Review rule "Round exit check" 1 (tested on PR 37: `true`) and 2 (unresolved threads). |
| 44 | The T3 request lacks the no-attribution rule. | fix | Review rule "Ask the T3 reviewer" 2. |
| 45 | Placeholder and derived verdict. | fix | Review rule: read after the run completes; a derived 0-finding verdict counts. |
| 46 | The fork ban has no check. | fix | Review rule "Ask the GitHub Actions reviewer" 2: `isCrossRepository`. |
| 47 | Task folder contradiction and boundary diff base. | fix | SDLC "Roles" allows the plan checkbox; boundary check uses `task-base`. |
| 48 | `git diff --check` is not in `docs/testing.md`. | fix | New row in the spec's "Rule changes" table. |
| 49 | G11 has no home or start condition. | fix | G11 decision: the first task after the merge, before the Actions reviewer runs again. |

The commit `6b91f7a` holds only the rule changes of this pass. A script error
left the spec and this record out. The next commit adds them.

## Round 5 (spec)

- Head: `d579503`
- Reviewer: T3 reviewer. Verdict COMMENT. P0: 0, P1: 0, P2: 1, P3: 3. Round 4
  findings fixed, except 45 (partly).
- SonarCloud: gate passed.
- Checks: `Code Quality and Tests` and `Windows named-pipe IPC` passed.
  `Native Tauri E2E Gate` failed: flake. Run 43477721481, job "Deterministic
  native tests": Postgres on the macOS runner was still "rejecting connections"
  when the step checked it. The tests did not run. The native gate on the next
  head is the one rerun.
- Gaps closed by recommendation since round 4: none.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| 45 | The run-list command finds a skipped run, so a placeholder can count as a review. | fix | Review rule "Ask the GitHub Actions reviewer": newest run that is not skipped, created after the comment, `completed`; a placeholder never counts. |
| 50 | Two rules claim the first task after the merge. | fix | Retro: "a later small-change task", in the order of `open-gaps.md`. G11 comes first. The review rule sends small changes to the T3 reviewer until G11 merges. |
| 51 | The exit-check commands fail open past 100 threads or 30 reviews. | fix | Reviews with `?per_page=100`. Threads with `last:100` and `hasPreviousPage`. |
| 52 | Boundary check 4 fails on the plan checkbox. | fix | Delegation rule: "and `plans/plan.md`". `isDraft` added to the baseline. |
| native gate | Flake on the macOS runner. | fix | Rerun on the next head. |

## Round 6 (spec)

- Head: `8f75795`
- Reviewer: T3 reviewer. Verdict COMMENT. P0: 0, P1: 0, P2: 0, P3: 1. Round 5
  findings 45, 50 and 52 fixed, 51 partly.
- SonarCloud: gate passed.
- Checks: all four passed. The native gate rerun after the round 5 flake passed
  (run 43479291034).
- Gaps closed by recommendation since round 5: none.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| 51 | The thread command hides `hasPreviousPage`. | fix | Review rule "Round exit check" 2 prints both values, `false` and `0`. |
| residual 1, 2 | No rule for an empty run list, no source for the comment time. | fix | Review rule: "An empty result means the run has not started", `created_at` from the issue comments. |
| residual 4 | The T3-until-G11 clause has no owner. | fix | G11: its task removes the clause. |

## Round 7 (spec)

- Head: `f1ec3a6`
- Reviewer: T3 reviewer. Verdict APPROVE. P0: 0, P1: 0, P2: 0, P3: 0.
- SonarCloud: gate passed.
- Checks: all four passed. The CI gate check passed.
- Threads: `false 0`.
- Gaps closed by recommendation since round 6: none.
- Result: the round exit check passed. Stage 3 is done. The owner approved the
  spec at gate 1.

## Live rule load check (plan task 4)

| Harness | Loaded at start | After the pointer | Answer |
| --- | --- | --- | --- |
| Codex `gpt-6-luna` via T3 | `AGENTS.md` with the "Work standard" pointer | Read `.agents/rules/sdlc-work-standard.md` | Stage 4 follows the spec review. Correct. |
| Antigravity `gemini-3.8-flash` via T3 | No workspace rule, no `AGENTS.md`, no `GEMINI.md` | Not applicable | "no rule in context". Gap G13. |

## Round 1 (code)

- Head: `dde3f6c`
- Reviewer: T3 reviewer (code review, required: this task changes the rules).
  Verdict COMMENT. P0: 0, P1: 0, P2: 2, P3: 2. All 13 rows of "Rule changes"
  applied with the approved text. The template matches the rule.
- SonarCloud: gate passed.
- Checks: all four passed. The CI gate at the end of stage 6 passed on
  `2391447`.
- Gaps closed by recommendation since gate 1: none.
- Owner input in the same round: the implementer model order (gap G14).

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| C1 | The stage 6 exit check counts a quoted `- [ ]` and uses a relative path. | fix (owner approved, G15) | Plan task 8. |
| C2 | `git diff --check` prints nothing after `git add` or the commit. | fix (owner approved, G15) | Plan task 9, the spec row and the plan header. |
| C3 | Blank line at the end of `task-records.md`. | fix | Plan task 10. |
| C4 | The round heading does not follow `## Round N (spec|code)`. | fix | Plan task 11, and this heading. |
| G14 | Implementer model order. | fix | Plan task 12. |

Fix pass for round 1 (code): plan tasks 8 to 12 by `gpt-6-luna` (`f5d00d3`,
`73e9f63`, `216b894`, `2fc288e`, `c3ded12`), task 13 by the orchestrator
(`3befbe1`: the child put a sentence inside list item 2). Boundary check passed.
`git diff --check origin/main HEAD` printed nothing.

## Round 2 (code)

- Head: `1cb3660`
- Reviewer: T3 reviewer. Verdict APPROVE. P0: 0, P1: 0, P2: 0, P3: 0. C1 to C4,
  G14 and G15 checked and fixed.
- SonarCloud: gate passed, 0 new issues, 0 hotspots.
- Checks: all four passed. The CI gate check passed.
- Threads: `false 0`.
- Gaps closed by recommendation since round 1: none.
- Result: the round exit check passed on `1cb3660`. Stage 7 is done. The reviewed
  head is `1cb3660`.

Fix pass for the retro proposals: plan tasks 15 to 17 by `gpt-6-luna` (`700234b`,
`b16004f`, `3e3cbc6`). Boundary check passed. `git diff --check origin/main HEAD`
printed nothing.

## Commands run

| Command (rule file) | Run | Result |
| --- | --- | --- |
| CI gate check (`review-standard.md` "Read the inputs" 2) | `/usr/bin/grep`, PR 32, 34, 38 | PR 32 and 38 pass, PR 34 fails (missing checks). |
| Verdict check (`review-standard.md` "Round exit check" 1) | PR 37 and PR 38 | `true` on PR 37's approved head and on each APPROVE head of PR 38. |
| Thread count (`review-standard.md` "Round exit check" 2) | PR 38 | Prints `false` and the count of unresolved threads. |
| Reviewer run list (`review-standard.md`) | PR 38 | Prints nothing when every run is skipped. |
| Plan check (`sdlc-work-standard.md` stage 6 exit) | repo root, this plan | Prints `0`. |
| Retro exit check (`task-records.md`) | this retro | Passes after the `Owner:` line fix. |
| Markdown check (`docs/testing.md`) | `git diff --check origin/main HEAD` | Prints nothing. |
| Resume query (`sdlc-work-standard.md`) | PR 36 and 37 branches | Lists the merged pull request with `--state all`. |
| Reviews call (`review-standard.md` "Read the inputs" 1) | `reviews?per_page=100 ... @tsv`, PR 38 | Prints login, short commit and first line for each review. |
| SonarCloud read (`review-standard.md` "Read the inputs" 3) | `check-runs` of the head, PR 38 | Prints `Quality Gate passed`. |
| Fork check (`review-standard.md`) | `isCrossRepository`, PR 38 | Prints `false`. |
| Merge exit (`sdlc-work-standard.md` stage 8) | `pr view --json mergedAt,mergeCommit`, PR 37 | Prints the merge time and `3f843da`. |
| `gh pr checks` exit code (`review-standard.md`) | PR 34 and PR 38 | Prints exit 0 for both, although PR 34 reports one check. This is why the CI gate check counts four `pass` lines. |
| Task folder only (`sdlc-work-standard.md` stage 8 step 6) | `git diff --name-only 1cb3660 HEAD` outside the task folder | Lists rule files, as it must when other paths changed. |
| Package run list (`sdlc-work-standard.md` stage 9) | `run list --workflow package.yml --branch main` | Prints run 43430991494 and `3f843da`. |
| Boundary check (`delegation-standard.md`): `git rev-parse origin/<branch>`, `git ls-remote --tags origin`, `pr view --json state,isDraft,comments,reviews`, `run list --branch`, `git diff --name-only <task-base> HEAD` | after each child task of this PR | Passed after every task. It does not see a missing edit, which the diff read found once (retro event 8). |
| Stage 6 exit (`sdlc-work-standard.md`): plan check, `git status --porcelain`, CI gate check | repo root at the head | Plan check prints `0`, `git status --porcelain` prints nothing (the untracked schema file is outside the task), CI gate check as the table row. |
| Stage 9 exit (`sdlc-work-standard.md`): `run watch --exit-status`, `release view v<version>` | `run watch --help`, `release view v0.1.6 --json assets` | `--exit-status` exists. The release lists 11 assets. |
| State-changing commands (`pr create`, `pr ready`, `pr merge`, `pr comment`, `workflow run`, `run rerun`, `release create`, `git push`) | not run: changes state | Run for real during this task as steps, not as tests. |

## Round 3 (code)

- Head: `f900743`
- Reviewer: T3 reviewer. Verdict COMMENT. P0: 0, P1: 0, P2: 3, P3: 2.
- SonarCloud: gate passed.
- Checks: all four passed. The "Live baseline check" job of the native gate
  failed on `3befbe1` and `1cb3660` and passed on `1803d12` and `f900743`. That
  job reports the live baseline on `main`. The gate status comes from the
  deterministic tests, which passed. Recorded as a flake, no fix.
- Gaps closed by recommendation since round 2: none.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| C5 | The "run every command" rule is not workable. | fix | Plan task 18. |
| C6 | "Even when the plan does not list it" conflicts with `AGENTS.md` Done 3 and the review rule, and has no record. | fix | Plan task 19. The record is a row in `reviews.md`, no plan mark (owner choice, G17). |
| C7 | The bound of a small fix is open. | fix | Plan task 20. |
| C8 | P1 contradicts the rest of stage 2 and the template. | fix | Plan task 21. |
| C9 | The spec says the retro changes are not in scope. | fix (orchestrator) | Spec "Not in scope". |
| retro | The retro names stage 5 for P2, the rule says stage 6. | fix (orchestrator) | `retro.md` P2 file line. |

Fix pass for round 3 (code): plan tasks 18 to 21 by `gpt-6-luna` (`58615a5`,
`4cb6a32`, `7a725a9`, `4b9e81b`). The child's report said all four were done.
The orchestrator's diff read found that task 20 changed only the checkbox and
task 19 missed `delegation-standard.md`. The child redid both (`3af8fc8`,
`60edd5b`). Boundary check passed. `git diff --check origin/main HEAD` printed
nothing.

## Round 4 (code)

- Head: `1a6fd36`
- Reviewer: T3 reviewer. Verdict COMMENT. P0: 0, P1: 0, P2: 2, P3: 2. C6, C8 and
  C9 fixed. C5 and C7 partly fixed.
- SonarCloud: gate passed.
- Checks: all four passed.
- Gaps closed by recommendation since round 3: none.

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| C10 | The commands table has 8 rows, the rule asks for one row per command (42). | fix (owner chose option 2, G18) | Plan task 22 changes the rule to check commands only. The table has rows for the check commands now. |
| C11 | The small-fix bound leaves out `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, `docs/testing.md`, and has no unit or `Where` content. | fix | Plan task 23. |
| C12 | Task 18's stage 6 edit is missing. | fix | Plan task 22. Retro note updated. |
| C13 | The spec row for `AGENTS.md` lags, G17 does not name it, the state block says stage 6. | fix (orchestrator) | Spec row, G18, state block `Stage: 7 review rounds`. |

Fix pass for round 4 (code): plan tasks 22 and 23 by `gpt-6-luna` (`d220827`,
`b8f8431`). The brief required `git diff --stat` to list every named file, and
it did. The orchestrator's diff read confirmed it. Boundary check passed.
`git diff --check origin/main HEAD` printed nothing.

## Round 5 (code)

- Head: `36e6cda`
- Reviewer: T3 reviewer. Verdict COMMENT. P0: 0, P1: 0, P2: 1, P3: 1. C11 to C13
  fixed, C12 fixed, C10 partly fixed.
- SonarCloud: gate passed.
- Checks: all four passed.
- Gaps closed by recommendation since round 4: none.
- Stop and ask: the same finding (the commands table) stayed open after two fix
  passes, so the orchestrator asked the owner (G19).

| # | Finding | Action | Where |
| --- | --- | --- | --- |
| C10 | The table lacks 7 check commands, and the definition leaves out the per-task boundary check. | fix (owner chose option 1, G19) | Plan task 24 (`70dc3d0`) changes the rule to one row for each named check. The table has rows for the boundary check, the stage 6 exit and the stage 9 exit. |
| C14 | The SHA cannot be in its own commit, the central docs and `docs/adr/` are not excluded, `git diff --shortstat` misses staged changes. | fix | Plan task 25 (`dee6390`). |

## Round 6 (code)

- Head: `9e368f3`
- Reviewer: T3 reviewer (code review), follow-up of round 5. Verdict APPROVE.
  P0: 0, P1: 0, P2: 0, P3: 0. C10 and C14 fixed.
- SonarCloud: gate passed, 0 new issues, 0 hotspots.
- Checks: all four passed. The CI gate check printed 4.
- Threads: `false 0`.
- Gaps closed by recommendation since round 5: none.
- Flake: `Code Quality and Tests` failed once on `a4411ec` in the `nook-core` test
  `test_server_ping_pong_and_permissions` (run 43511244397). The same test passed
  on `36e6cda` and `9e368f3`. `a4411ec` was a docs-only commit and not a round
  head. The next head passed, so the failure counts as a flake. The reviewer
  suggests a follow-up on that test.
- Observation for the next task: SDLC stage 8 step 5 says "the review exit
  check", and the heading in the review rule is "Round exit check". It blocks
  nothing and changes no rule in this pull request.
- Result: the round exit check passed.

Reviewed head: `9e368f3f14b6066ddf3cf2e2f34af86e696cf672`
